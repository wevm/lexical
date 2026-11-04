import Lexical

// Port of packages/lexical-markdown/src/importTextMatchTransformer.ts.

/// `CodeSpans`: code spans, in order, from where they start to where they
/// end.
package typealias CodeSpans = [(startIndex: Int, endIndex: Int)]

nonisolated(unsafe) private let STATEFUL_FLAGS = JSRegExp("[gy]", "g")

/// The first link in `text`, starting at the `[` that really opens its
/// text, with none of its syntax inside a code span. `codeSpans` are those
/// of the text that `text` is what is left of after `offset`, when known.
private func matchLink(
  _ text: String, _ regExp: JSRegExp, _ codeSpans: CodeSpans?, _ offset: Int
) -> JSRegExpMatch? {
  let search = JSRegExp(regExp.source, regExp.flags.jsReplace(STATEFUL_FLAGS, template: "") + "g")
  // Code spans bind tighter than link brackets
  // (https://spec.commonmark.org/0.31.2/#code-spans), so a link whose
  // syntax falls inside one is not a link.
  var spans: CodeSpans
  var spanOffset = offset
  if let codeSpans {
    spans = codeSpans
  } else {
    spans =
      text.utf16.contains(UInt16(UInt8(ascii: "`")))
      ? scanCodeSpans(text).map { ($0.startIndex, $0.endIndex) } : []
    spanOffset = 0
  }
  // The spans come in order, so a binary search finds the one that could
  // hold `index`, or -1 outside every span.
  func codeSpanAt(_ index: Int) -> Int {
    var low = 0
    var high = spans.count - 1
    while low <= high {
      let middle = (low + high) >> 1
      let (startIndex, endIndex) = spans[middle]
      if index + spanOffset < startIndex {
        high = middle - 1
      } else if index + spanOffset >= endIndex {
        low = middle + 1
      } else {
        return middle
      }
    }
    return -1
  }
  // No link starts after the last `](`, and searching from each `[` there
  // would walk on to the end of the text.
  let lastCloser = text.jsLastIndexOf("](")
  while search.lastIndex < lastCloser, let match = search.exec(text) {
    let openerSpan = codeSpanAt(match.index)
    if openerSpan != -1 {
      // No link can open inside this code span.
      search.lastIndex = spans[openerSpan].endIndex - spanOffset
      continue
    }
    let label = match[1] ?? ""
    let labelEnd = match.index + 1 + label.jsLength
    if codeSpanAt(labelEnd) != -1 || codeSpanAt(match.index + match.length - 1) != -1 {
      // A search from any `[` before the last character of the label ends
      // at the same `](` and `)`, so it is not a link either.
      search.lastIndex = labelEnd - 1
      continue
    }
    // A `[` that the label holds outside a code span is the link's real
    // opener, and what comes before it is ordinary text, so the search goes
    // on from there, where it finds the same link. A link with an empty
    // label, as in `x[[](u)`, stays text, as `[](u)` does.
    if let opener = getLinkLabelOpener(label), opener >= 0 {
      search.lastIndex = match.index + 1 + opener
      continue
    }
    if !isEscaped(text, match.index) {
      return match
    }
    // The label of an escaped `[` holds no opener, so each escaped `[` up to
    // the next one written as is finds what the last of them finds: that
    // opener or nothing.
    var next = match.index
    repeat {
      next = text.jsIndexOf("[", next + 1)
    } while next != -1 && isEscaped(text, next)
    if next == -1 { break }
    let last = text.jsLastIndexOf("[", next - 1)
    search.lastIndex = last > match.index ? last : next
  }
  return nil
}

nonisolated(unsafe) private let STATE_FLAGS = JSRegExp("[gyv]")
nonisolated(unsafe) private let BOUNDARY_ESCAPE = JSRegExp("[bB]")
nonisolated(unsafe) private let NAMED_GROUP = JSRegExp(#"^\(\?<[^=!]"#)

/// Whether a match of `regExp` depends only on the text from where it
/// starts on: it has no `^`, word boundary or lookbehind outside a
/// character class, and no flag that makes it keep state. Its first match
/// in what is left of a text after a cut is then the first one found at or
/// after the cut. JS caches the answer per regex in a WeakMap; it is
/// computed each time here, which avoids shared mutable state.
private func looksOnlyAhead(_ regExp: JSRegExp) -> Bool {
  let source = regExp.source
  var result = !STATE_FLAGS.test(regExp.flags)
  var inClass = false
  var i = 0
  let length = source.jsLength
  while result && i < length {
    let character = source.jsCharAt(i)
    if character == "\\" {
      i += 1
      result = inClass || !BOUNDARY_ESCAPE.test(source.jsCharAt(i) ?? "undefined")
    } else if inClass {
      inClass = character != "]"
    } else if character == "[" {
      inClass = true
    } else {
      result =
        (character != "^" && !source.jsSlice(i).jsStartsWith("(?<"))
        || NAMED_GROUP.test(source.jsSlice(i, i + 4))
    }
    i += 1
  }
  return result
}

/// `TextMatches`: the match each text-match transformer found in a text and
/// the offset of the text it was found in, or `nil` when there is none.
/// Kept for the built-in link syntax and for patterns that only look ahead,
/// for which it is still the first match in what is left of the text after
/// any offset up to where it was found.
package final class TextMatches {
  var found: [ObjectIdentifier: (match: JSRegExpMatch, offset: Int)?] = [:]
  public init() {}
}

/// A text match transformer's match, from `findOutermostTextMatchTransformer`.
package struct FoundTextMatch {
  public var startIndex: Int
  public var endIndex: Int
  public var transformer: TextMatchTransformer
  public var match: JSRegExpMatch
}

/// `findOutermostTextMatchTransformer(textNode, transformers, matches,
/// offset, codeSpans)`: the match that starts first (or wraps the one that
/// does). `matches` holds those found earlier in the text that `textNode`
/// is what is left of from `offset`; `codeSpans` are that text's code
/// spans, when known.
package func findOutermostTextMatchTransformer(
  _ textNode: TextNode, _ textMatchTransformers: [TextMatchTransformer],
  _ matches: TextMatches = TextMatches(), _ offset: Int = 0, _ codeSpans: CodeSpans? = nil
) throws -> FoundTextMatch? {
  var found: FoundTextMatch?
  for transformer in textMatchTransformers {
    guard transformer.replace != nil, let importRegExp = transformer.importRegExp else {
      continue
    }
    let text = try textNode.getTextContent()
    // Transformers sharing the built-in link syntax need the same candidate
    // rules.
    let isLink = LINK.importRegExp.map { $0.source == importRegExp.source } ?? false
    var match: JSRegExpMatch?
    if isLink || looksOnlyAhead(importRegExp) {
      let key = ObjectIdentifier(transformer)
      let earlier = matches.found[key]
      if earlier == nil
        || (earlier!.map { $0.offset + $0.match.index < offset } ?? false)
      {
        let result =
          isLink ? matchLink(text, importRegExp, codeSpans, offset) : importRegExp.exec(text)
        matches.found[key] = .some(result.map { ($0, offset) })
        match = result
      } else {
        match = earlier!.map { $0.match.moved(text, offset - $0.offset) }
      }
    } else {
      match = text.jsMatch(importRegExp)
    }
    guard let match else { continue }

    let startIndex = match.index
    let endIndex: Int
    if let getEndIndex = transformer.getEndIndex {
      guard let end = try getEndIndex(textNode, match) else { continue }
      endIndex = end
    } else {
      endIndex = startIndex + match.length
    }
    if let current = found {
      // Wraps the previous match or is strictly before it.
      guard
        startIndex < current.startIndex
          && (endIndex > current.endIndex || endIndex <= current.startIndex)
      else { continue }
    }
    found = FoundTextMatch(
      startIndex: startIndex, endIndex: endIndex, transformer: transformer, match: match)
  }
  return found
}

/// The nodes `importFoundTextMatchTransformer` leaves.
package struct ImportedTextMatch {
  public var matchedNode: TextNode
  public var transformedNode: TextNode?
  /// The text before the match, if it was split off.
  public var nodeBefore: TextNode?
  /// The text after the match, if it was split off.
  public var nodeAfter: TextNode?
  /// The text of `nodeAfter` before the transformer ran, which may change
  /// it.
  public var textAfter: String?
}

/// `importFoundTextMatchTransformer(textNode, startIndex, endIndex,
/// transformer, match)`: splits out the matched text and replaces it.
package func importFoundTextMatchTransformer(
  _ textNode: TextNode, _ startIndex: Int, _ endIndex: Int,
  _ transformer: TextMatchTransformer, _ match: JSRegExpMatch
) throws -> ImportedTextMatch? {
  let transformedNode: TextNode
  var nodeBefore: TextNode?
  var nodeAfter: TextNode?
  if startIndex == 0 {
    let parts = try textNode.splitText(endIndex)
    transformedNode = parts[0]
    nodeAfter = parts.count > 1 ? parts[1] : nil
  } else {
    let parts = try textNode.splitText(startIndex, endIndex)
    nodeBefore = parts[0]
    transformedNode = parts[1]
    nodeAfter = parts.count > 2 ? parts[2] : nil
  }
  guard let replace = transformer.replace else { return nil }
  let textAfter = try nodeAfter?.getTextContent()
  let potentialTransformedNode = try replace(transformedNode, match)
  return ImportedTextMatch(
    matchedNode: transformedNode, transformedNode: potentialTransformedNode,
    nodeBefore: nodeBefore, nodeAfter: nodeAfter, textAfter: textAfter)
}
