import Lexical

// Port of packages/lexical-markdown/src/importTextFormatTransformer.ts.
// Indices are UTF-16 code units, as in JS.

/// `Delimiter`: a run of an emphasis delimiter.
private struct Delimiter {
  var index: Int
  var char: String
  var length: Int
  var canOpen: Bool
  var canClose: Bool
  var active: Bool
}

/// `FormatMatch`: a code span or emphasis, with its tag and content.
package struct FormatMatch: Equatable {
  public var startIndex: Int
  public var endIndex: Int
  public var tag: String
  public var content: String

  public init(startIndex: Int, endIndex: Int, tag: String, content: String) {
    self.startIndex = startIndex
    self.endIndex = endIndex
    self.tag = tag
    self.content = content
  }
}

/// `FormatScan`: the code spans and outermost emphasis of a text, found once
/// and read again for what is left of the text after each match, at
/// `offset`. A match there starts with the code span at `nextCodeSpan` or
/// the emphasis at `nextEmphasis`. A class, since JS moves the scan on in
/// place.
package final class FormatScan {
  public var text: String
  /// The characters that start an emphasis tag.
  public var delimiters: Set<String>
  public var codeSpans: [FormatMatch]
  public var emphasis: [FormatMatch]
  public var offset: Int
  public var nextCodeSpan: Int
  public var nextEmphasis: Int
  /// An emphasis that starts at `offset`, found after the scan.
  public var pending: FormatMatch?

  public init(
    text: String, delimiters: Set<String>, codeSpans: [FormatMatch], emphasis: [FormatMatch],
    offset: Int = 0, nextCodeSpan: Int = 0, nextEmphasis: Int = 0, pending: FormatMatch? = nil
  ) {
    self.text = text
    self.delimiters = delimiters
    self.codeSpans = codeSpans
    self.emphasis = emphasis
    self.offset = offset
    self.nextCodeSpan = nextCodeSpan
    self.nextEmphasis = nextEmphasis
    self.pending = pending
  }
}

/// `scanTextFormats(text, textFormatTransformersIndex, before, after)`.
package func scanTextFormats(
  _ text: String,
  _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ before: String?,
  _ after: String?
) -> FormatScan {
  // Find code spans first. Emphasis delimiters inside inline elements (e.g., code spans)
  // should not be processed. Currently only code spans are handled; other inline elements
  // (e.g., links, raw HTML) may need similar treatment in the future.
  let codeSpans =
    textFormatTransformersIndex.transformersByTag["`"] != nil
    ? scanCodeSpans(text).map {
      FormatMatch(startIndex: $0.startIndex, endIndex: $0.endIndex, tag: "`", content: $0.content)
    }
    : []
  let delimiterChars = Set(
    textFormatTransformersIndex.transformersByTag.keys
      .filter { $0.jsCharAt(0) != "`" }
      .compactMap { $0.jsCharAt(0) })
  var delimiters = scanDelimiters(text, delimiterChars, codeSpans, before, after)
  return FormatScan(
    text: text,
    delimiters: delimiterChars,
    codeSpans: codeSpans,
    emphasis: delimiters.count > 0
      ? processEmphasis(text, &delimiters, textFormatTransformersIndex) : [],
    offset: 0,
    nextCodeSpan: 0,
    nextEmphasis: 0,
    pending: nil)
}

/// `advanceFormatScan(scan, offset, isFormatEnd)`: moves `scan` on to the
/// text after `offset`, where a match ends. What it found there holds for
/// that text on its own as long as no code span or emphasis runs on from
/// before `offset` (a pair cannot reach past the end of the outermost one),
/// no backslash before it escapes it, and no run of a delimiter or backtick
/// goes on across it; returns false otherwise. A closing run that the end
/// of a format match cuts is the exception: the scan measured it by what
/// was left of it, as a scan of the rest does.
package func advanceFormatScan(_ scan: FormatScan, _ offset: Int, _ isFormatEnd: Bool) -> Bool {
  let codeSpans = scan.codeSpans
  let emphasis = scan.emphasis
  let pending = scan.pending
  let text = scan.text
  scan.offset = offset
  scan.pending = nil
  if let pending, pending.endIndex > offset {
    return false
  }
  while scan.nextCodeSpan < codeSpans.count && codeSpans[scan.nextCodeSpan].startIndex < offset {
    let span = codeSpans[scan.nextCodeSpan]
    scan.nextCodeSpan += 1
    if span.endIndex > offset {
      return false
    }
  }
  while scan.nextEmphasis < emphasis.count && emphasis[scan.nextEmphasis].startIndex < offset {
    let match = emphasis[scan.nextEmphasis]
    scan.nextEmphasis += 1
    if match.endIndex > offset {
      return false
    }
  }
  let cut = text.jsCharAt(offset - 1)
  return cut != "\\"
    // A pair found after the scan cut a run that the scan measured whole.
    && ((isFormatEnd && pending == nil) || cut != text.jsCharAt(offset)
      || (cut != "`" && !(cut.map { scan.delimiters.contains($0) } ?? false)))
}

/// `findPairAtEdge(scan, rest, textFormatTransformersIndex)`: lets `scan`,
/// moved on to `rest` by `advanceFormatScan`, read the delimiter run that
/// starts `rest` beside the edge of a line rather than beside what came
/// before, where it opened or closed otherwise. Nothing in `rest` comes
/// before the run, so the rest reads the same either way but for the pair
/// the run opens, which nothing in `rest` can hold, and whatever pairs
/// across its end, which `advanceFormatScan` turns down there. The pair
/// ends before a code span that the scan found next, which is imported
/// first as it is with the match before it, and it is found in ever longer
/// prefixes of `rest`, so that finding it costs about as much as the text
/// it holds. Returns false when the rest has to be scanned again.
package func findPairAtEdge(
  _ scan: FormatScan,
  _ rest: String,
  _ textFormatTransformersIndex: TextFormatTransformersIndex
) -> Bool {
  let codeSpans = scan.codeSpans
  let offset = scan.offset
  let restLength = rest.jsLength
  var span = scan.nextCodeSpan
  let next = scan.nextEmphasis < scan.emphasis.count ? scan.emphasis[scan.nextEmphasis] : nil
  let end =
    span < codeSpans.count && (next == nil || codeSpans[span].startIndex < next!.startIndex)
    ? codeSpans[span].startIndex - offset
    : restLength
  var length = 64
  while true {
    // Cut neither a code span nor a run of a delimiter or backtick.
    while span < codeSpans.count && codeSpans[span].endIndex <= offset + length {
      span += 1
    }
    if span < codeSpans.count && codeSpans[span].startIndex < offset + length {
      length = codeSpans[span].endIndex - offset
    }
    if length < end, let last = rest.jsCharAt(length - 1),
      last == "`" || scan.delimiters.contains(last)
    {
      length = getDelimiterRunEdge(rest, length, 1)
    }
    if length >= end {
      if end == restLength {
        return false
      }
      length = end
    }
    let pair = scanTextFormats(
      rest.jsSlice(0, length),
      textFormatTransformersIndex,
      nil,
      characterAt(rest, length)
    ).emphasis.first
    if let pair, pair.startIndex == 0 {
      var pending = pair
      pending.endIndex = pair.endIndex + offset
      pending.startIndex = offset
      scan.pending = pending
      return true
    }
    if length == end {
      return true
    }
    length *= 2
  }
}

/// `partlyHoldsCodeSpan(scan, startIndex, endIndex)`: whether the text from
/// `startIndex` to `endIndex` after `scan`'s offset holds part of a code
/// span but not all of it.
package func partlyHoldsCodeSpan(_ scan: FormatScan, _ startIndex: Int, _ endIndex: Int) -> Bool {
  let codeSpans = scan.codeSpans
  let offset = scan.offset
  var i = scan.nextCodeSpan
  while i < codeSpans.count {
    let spanStart = codeSpans[i].startIndex - offset
    let spanEnd = codeSpans[i].endIndex - offset
    if spanStart >= endIndex {
      break
    }
    if spanEnd > startIndex && (spanStart < startIndex || spanEnd > endIndex) {
      return true
    }
    i += 1
  }
  return false
}

/// The result of `findOutermostTextFormatTransformer`.
package struct OutermostTextFormatMatch {
  public var startIndex: Int
  public var endIndex: Int
  public var transformer: TextFormatTransformer
  /// `[text, tag, content]`, at `startIndex` of the text.
  public var match: JSRegExpMatch
  /// True when the outermost format is a code span. Code spans are atomic:
  /// their content is never reprocessed and they bind tighter than
  /// text-match transformers, so this is tagged here rather than re-derived
  /// by callers.
  public var isCodeSpan: Bool
}

/// `findOutermostTextFormatTransformer(text, textFormatTransformersIndex,
/// scan)`: the outermost format match in `text`, what is left after
/// `scan`'s offset of the text it scanned.
package func findOutermostTextFormatTransformer(
  _ text: String,
  _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ scan: FormatScan
) -> OutermostTextFormatMatch? {
  let codeMatch: FormatMatch? =
    scan.nextCodeSpan < scan.codeSpans.count ? scan.codeSpans[scan.nextCodeSpan] : nil
  let emphasisMatch: FormatMatch? =
    scan.pending
    ?? (scan.nextEmphasis < scan.emphasis.count ? scan.emphasis[scan.nextEmphasis] : nil)
  // Delimiters inside a code span were excluded, so an emphasis match that
  // starts first either holds the whole span or ends before it, and either
  // way it is applied first.
  let isCodeSpan: Bool
  let resultMatch: FormatMatch
  if let emphasisMatch,
    codeMatch == nil || emphasisMatch.startIndex <= codeMatch!.startIndex
  {
    resultMatch = emphasisMatch
    isCodeSpan = false
  } else if let codeMatch {
    resultMatch = codeMatch
    isCodeSpan = true
  } else {
    return nil
  }
  guard let transformer = textFormatTransformersIndex.transformersByTag[resultMatch.tag] else {
    return nil
  }
  let offset = scan.offset
  let startIndex = resultMatch.startIndex - offset
  let endIndex = resultMatch.endIndex - offset
  let regexMatch = JSRegExpMatch(
    groups: [text.jsSlice(startIndex, endIndex), resultMatch.tag, resultMatch.content],
    index: startIndex, input: text)

  return OutermostTextFormatMatch(
    startIndex: startIndex,
    endIndex: endIndex,
    transformer: transformer,
    match: regexMatch,
    isCodeSpan: isCodeSpan)
}

/// A code span found by `scanCodeSpans`.
package struct CodeSpan: Equatable {
  public var startIndex: Int
  public var endIndex: Int
  public var content: String

  public init(startIndex: Int, endIndex: Int, content: String) {
    self.startIndex = startIndex
    self.endIndex = endIndex
    self.content = content
  }
}

nonisolated(unsafe) private let NOT_SPACE = JSRegExp(#"[^ ]"#)

private let BACKTICK = UInt16(UInt8(ascii: "`"))

/// `scanCodeSpans(text)`: finds all inline code spans, left to right and
/// non-overlapping, per CommonMark rules:
/// https://spec.commonmark.org/#code-spans. A run opens a span and the next
/// run of equal length closes it. An escaped backtick (`` \` ``) is
/// literal, and the rest of its run may open a span, but backslashes are
/// otherwise literal and don't prevent closing.
package func scanCodeSpans(_ text: String) -> [CodeSpan] {
  let units = Array(text.utf16)
  // Collect maximal backtick runs.
  var runs: [(index: Int, length: Int)] = []
  var i = 0
  while i < units.count {
    if units[i] == BACKTICK {
      var length = 1
      while i + length < units.count && units[i + length] == BACKTICK {
        length += 1
      }
      runs.append((i, length))
      i += length
    } else {
      i += 1
    }
  }

  // The runs of each length, in order, to find a closer in, and how many of
  // them the openers, which only move forward, have passed.
  var runsByLength: [Int: [Int]] = [:]
  for (index, run) in runs.enumerated() {
    runsByLength[run.length, default: []].append(index)
  }
  var passed: [Int: Int] = [:]
  // The first run after `index` that is `length` long, or -1.
  func findCloser(_ index: Int, _ length: Int) -> Int {
    guard let indexes = runsByLength[length] else {
      return -1
    }
    var next = passed[length] ?? 0
    while next < indexes.count && indexes[next] <= index {
      next += 1
    }
    passed[length] = next
    return next < indexes.count ? indexes[next] : -1
  }

  var spans: [CodeSpan] = []
  var openIdx = 0
  while openIdx < runs.count {
    var opener = runs[openIdx]

    // An escaped backtick is a literal backtick, and the rest of its run is
    // a run of its own that can open a span. Inside a span a backslash is
    // literal, so the whole run still closes one.
    if isEscaped(text, opener.index) {
      if opener.length == 1 {
        openIdx += 1
        continue
      }
      opener = (opener.index + 1, opener.length - 1)
    }

    let closeIdx = findCloser(openIdx, opener.length)

    if closeIdx == -1 {
      // No matching closer; treat this run as literal and try the next one.
      openIdx += 1
      continue
    }

    let closer = runs[closeIdx]
    var content = text.jsSlice(opener.index + opener.length, closer.index)
    if content.jsLength >= 2 && content.jsStartsWith(" ") && content.jsEndsWith(" ")
      && NOT_SPACE.test(content)
    {
      content = content.jsSlice(1, -1)
    }

    spans.append(
      CodeSpan(
        startIndex: opener.index, endIndex: closer.index + closer.length, content: content))
    openIdx = closeIdx + 1
  }

  return spans
}

/// `scanDelimiters`. `before` and `after` are what a run at the start (or
/// end) of the text sees beyond it; the edge of the line when `nil`.
private func scanDelimiters(
  _ text: String,
  _ delimiterChars: Set<String>,
  _ codeSpans: [FormatMatch],
  _ before: String?,
  _ after: String?
) -> [Delimiter] {
  var delimiters: [Delimiter] = []
  let units = Array(text.utf16)

  // The spans are in order and the scan only moves forward.
  var span = 0
  func isInCodeSpan(_ index: Int) -> Bool {
    while span < codeSpans.count && codeSpans[span].endIndex <= index {
      span += 1
    }
    return span < codeSpans.count && index >= codeSpans[span].startIndex
  }

  var i = 0
  while i < units.count {
    let char = String(decoding: [units[i]], as: UTF16.self)

    if !delimiterChars.contains(char) || isEscaped(text, i) || isInCodeSpan(i) {
      i += 1
      continue
    }

    let len = getDelimiterRunEdge(text, i + 1, 1) - i

    let sides = getEmphasisSides(
      char,
      i == 0 ? before : characterBefore(text, i),
      i + len == units.count ? after : characterAt(text, i + len))

    if sides.canOpen || sides.canClose {
      delimiters.append(
        Delimiter(
          index: i, char: char, length: len, canOpen: sides.canOpen, canClose: sides.canClose,
          active: true))
    }

    i += len
  }

  return delimiters
}

/// `processEmphasis`: the outermost emphasis matches, in order.
private func processEmphasis(
  _ text: String,
  _ delimiters: inout [Delimiter],
  _ transformersIndex: TextFormatTransformersIndex
) -> [FormatMatch] {
  var openersBottom: [String: Int] = [:]
  var currentPos = 0
  var matches: [FormatMatch] = []
  // In the object's insertion order, as JS reads them.
  let tags = transformersIndex.tags

  while currentPos < delimiters.count {
    let closer = delimiters[currentPos]

    if !closer.active || !closer.canClose || closer.length == 0 {
      currentPos += 1
      continue
    }

    // The "no opener below this point" shortcut must be keyed by everything
    // the opener search outcome depends on: the marker, whether the closer
    // can also open (toggles the rule of 3), and the closer's length mod 3
    // (the rule of 3 blocks different openers per class — CommonMark's
    // openers_bottom is likewise per length-mod-3). The length key uses the
    // *current* length: a partially consumed closer switches class and must
    // rescan openers a shorter closer already gave up on (#4895).
    let bottomKey = "\(closer.char)\(closer.canOpen)\(closer.length % 3)"
    let bottom = openersBottom[bottomKey] ?? -1
    var foundOpener = false

    for openIdx in stride(from: currentPos - 1, to: bottom, by: -1) {
      let opener = delimiters[openIdx]

      if !opener.active || !opener.canOpen || opener.length == 0 || opener.char != closer.char {
        continue
      }

      // Rule of 3, on the *remaining* lengths: a delimiter run partially
      // consumed by an earlier pairing is re-measured, matching micromark
      // (the CommonMark reference this models). Using the original lengths
      // instead would refuse pairings like the `**` opener with the two
      // markers left of a `****` run in `**llo*wor****`, so the exporter's
      // own output for overlapping formats would not re-import (#4895).
      if opener.canClose || closer.canOpen {
        let sum = opener.length + closer.length
        if sum % 3 == 0 && opener.length % 3 != 0 && closer.length % 3 != 0 {
          continue
        }
      }

      let maxLen = min(opener.length, closer.length)
      let candidates = tags.filter { $0.jsCharAt(0) == opener.char && $0.jsLength <= maxLen }
      // The longest tag, the first of equally long ones in key order (JS
      // sorts stably; `min` keeps the first of equal elements).
      let matchedTag = candidates.min { $0.jsLength > $1.jsLength }

      guard let matchedTag else {
        continue
      }

      foundOpener = true
      let matchLen = matchedTag.jsLength
      let match = FormatMatch(
        startIndex: opener.index + (opener.length - matchLen),
        endIndex: closer.index + matchLen,
        tag: matchedTag,
        content: text.jsSlice(opener.index + opener.length, closer.index))

      matches.append(match)

      for j in (openIdx + 1)..<currentPos {
        delimiters[j].active = false
      }

      delimiters[openIdx].length -= matchLen
      delimiters[currentPos].length -= matchLen
      delimiters[openIdx].active = delimiters[openIdx].length > 0

      if delimiters[currentPos].length > 0 {
        delimiters[currentPos].index += matchLen
      } else {
        delimiters[currentPos].active = false
        currentPos += 1
      }

      break
    }

    if !foundOpener {
      openersBottom[bottomKey] = currentPos - 1
      if !closer.canOpen {
        delimiters[currentPos].active = false
      }
      currentPos += 1
    }
  }

  // Pairs nest, so the outermost ones are those that no earlier one holds.
  matches.sort { a, b in
    a.startIndex != b.startIndex ? a.startIndex < b.startIndex : a.endIndex > b.endIndex
  }
  var outermost: [FormatMatch] = []
  for match in matches {
    if outermost.isEmpty || match.startIndex >= outermost[outermost.count - 1].endIndex {
      outermost.append(match)
    }
  }
  return outermost
}

/// `importTextFormatTransformer(textNode, startIndex, endIndex, transformer,
/// match)`. `nodeBefore` and `nodeAfter` are set when the node was split.
package func importTextFormatTransformer(
  _ textNode: TextNode,
  _ startIndex: Int,
  _ endIndex: Int,
  _ transformer: TextFormatTransformer,
  _ match: JSRegExpMatch
) throws -> (transformedNode: TextNode, nodeBefore: TextNode?, nodeAfter: TextNode?) {
  let textContent = try textNode.getTextContent()

  // No text matches - we can safely process the text format match
  let transformedNode: TextNode?
  var nodeAfter: TextNode?
  var nodeBefore: TextNode?

  // If matching full content there's no need to run splitText and can reuse existing textNode
  // to update its content and apply format. E.g. for **_Hello_** string after applying bold
  // format (**) it will reuse the same text node to apply italic (_)
  if match[0] == textContent {
    transformedNode = textNode
  } else {
    if startIndex == 0 {
      let parts = try textNode.splitText(endIndex)
      transformedNode = parts.first
      nodeAfter = parts.count > 1 ? parts[1] : nil
    } else {
      let parts = try textNode.splitText(startIndex, endIndex)
      nodeBefore = parts.first
      transformedNode = parts.count > 1 ? parts[1] : nil
      nodeAfter = parts.count > 2 ? parts[2] : nil
    }
  }
  guard let transformedNode else {
    // JS reads a property of `undefined` here.
    throw LexicalError("importTextFormatTransformer: the match is not in the text node")
  }

  try transformedNode.setTextContent(match[2] ?? "")

  for format in transformer.format {
    if try !transformedNode.hasFormat(format) {
      try transformedNode.toggleFormat(format)
    }
  }

  return (transformedNode: transformedNode, nodeBefore: nodeBefore, nodeAfter: nodeAfter)
}
