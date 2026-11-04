import Lexical

// Port of packages/lexical-markdown/src/importTextTransformers.ts.

/// `canContainTransformableMarkdown(node)`: whether `node` is a TextNode
/// that can hold markdown. Code cannot: `code **bold**` stays as it is.
package func canContainTransformableMarkdown(_ node: LexicalNode?) -> Bool {
  guard let node = node as? TextNode else { return false }
  return (try? node.hasFormat("code")) == false
}

/// Whether the delimiter run that starts `text` can open or close
/// otherwise beside the edge of a line than beside `before`. Whether it can
/// close matters even with nothing before it, through the rule of 3.
private func readsOtherwiseBesideEdge(
  _ text: String, _ before: String?, _ outside: String?
)
  -> Bool
{
  let edge = getDelimiterRunEdge(text, 1, 1)
  let after = edge < text.jsLength ? characterAt(text, edge) : outside
  let marker = text.jsCharAt(0) ?? ""
  let atEdge = getEmphasisSides(marker, nil, after)
  let besideBefore = getEmphasisSides(marker, before, after)
  return atEdge != besideBefore
}

/// The markers whose runs CommonMark and GFM define.
private let SPEC_MARKERS: Set<String> = ["*", "_", "~"]

/// What a delimiter run cut at `index` of `text` sees beyond its far end
/// once the text is split there: before the run that starts at `index` when
/// `step` is -1, after the run that ends there when it is 1. `outside` is
/// what lies beyond the text. Beside a text match, which is written in a
/// syntax of its own, a run that the cut does not go through sees the edge
/// of a line.
private func characterBeyondRun(
  _ text: String, _ index: Int, _ step: Int, _ outside: String?, _ delimiters: Set<String>,
  _ isBesideTextMatch: Bool = false
) -> String? {
  // Only a delimiter's run is walked: beside anything else, what lies
  // beyond is never read, and a long run of it would be walked at every cut.
  let beside = text.jsCharAt(step == 1 ? index - 1 : index)
  let edge =
    beside == "`" || beside.map(delimiters.contains) == true
    ? getDelimiterRunEdge(text, index, step) : index
  if isBesideTextMatch && edge == index { return nil }
  if step == 1 {
    return edge < text.jsLength ? characterAt(text, edge) : outside
  }
  return edge > 0 ? characterBefore(text, edge) : outside
}

/// `importTextTransformers(textNode, index, textMatchTransformers, before,
/// after)`: applies the text format and text match transformers. It finds
/// the outermost text format or text match and applies it, goes on with the
/// text after it the same way, and then, from the last match back to the
/// first, imports the text before each match and then the text inside it.
/// `before` and `after` are the characters around this text in the text it
/// was split from, so that a delimiter run at its edge reads as it did
/// there.
package func importTextTransformers(
  _ textNode: TextNode, _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ textMatchTransformers: [TextMatchTransformer], _ before: String? = nil,
  _ after: String? = nil
) throws {
  // The text after each match is imported in this loop rather than by
  // recursion, so that a long paragraph does not nest a call per match, and
  // with what was found in the whole text, which still holds for the rest
  // of it as long as no match there reached into what came before.
  var deferred: [Deferred] = []
  var rest: TextRest? = TextRest(node: textNode, before: before, after: after, scan: nil)
  while let current = rest {
    rest = try importOutermost(
      current, textFormatTransformersIndex, textMatchTransformers, &deferred)
  }
  for entry in deferred.reversed() {
    try importTextPart(entry.outside, textFormatTransformersIndex, textMatchTransformers)
    try importTextPart(entry.inside, textFormatTransformersIndex, textMatchTransformers)
    if !canContainTransformableMarkdown(entry.inside.node),
      canContainTransformableMarkdown(entry.matched), let matched = entry.matched as? TextNode,
      try matched.getParent() != nil
    {
      // A text match transformer that returned no node may leave the
      // matched node in place or move it into a new parent. Read its
      // escapes once without trying the same match again.
      try matched.setTextContent(unescapeText(matched.getTextContent()))
    }
  }
}

/// Text to import, with the characters around it in the text it was cut
/// from.
private struct TextPart {
  var node: LexicalNode?
  var before: String?
  var after: String?
}

private func importTextPart(
  _ part: TextPart, _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ textMatchTransformers: [TextMatchTransformer]
) throws {
  if canContainTransformableMarkdown(part.node), let node = part.node as? TextNode {
    try importTextTransformers(
      node, textFormatTransformersIndex, textMatchTransformers, part.before, part.after)
  }
}

/// What a match leaves to import once the text after it is done: the text
/// before it, then the text inside it, or the escapes of the matched node
/// of a text match that returned no node.
private struct Deferred {
  var outside: TextPart
  var inside: TextPart
  var matched: LexicalNode?
}

/// The formats and text matches found in a text, for what is left of it,
/// which is `rest`.
private final class TextScan {
  let formats: FormatScan
  let matches: TextMatches
  var rest: String

  init(formats: FormatScan, matches: TextMatches, rest: String) {
    self.formats = formats
    self.matches = matches
    self.rest = rest
  }
}

/// The text after a match, and the scan of the text it was cut from.
private struct TextRest {
  var node: TextNode
  var before: String?
  var after: String?
  var scan: TextScan?
}

/// Applies the outermost transformer in the node of `textRest`, adds the
/// import of the text before and inside its match to `deferred`, and
/// returns the text after it, for the caller to import next.
private func importOutermost(
  _ textRest: TextRest, _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ textMatchTransformers: [TextMatchTransformer], _ deferred: inout [Deferred]
) throws -> TextRest? {
  let textNode = textRest.node
  let before = textRest.before
  let after = textRest.after
  let text = try textNode.getTextContent()
  let scan: TextScan
  if let previousScan = textRest.scan, previousScan.rest.utf16.elementsEqual(text.utf16) {
    scan = previousScan
  } else {
    scan = TextScan(
      formats: scanTextFormats(text, textFormatTransformersIndex, before, after),
      matches: TextMatches(), rest: text)
  }
  let formats = scan.formats
  let delimiters = formats.delimiters
  var foundTextFormat = findOutermostTextFormatTransformer(
    text, textFormatTransformersIndex, formats)
  var foundTextMatch = try findOutermostTextMatchTransformer(
    textNode, textMatchTransformers, scan.matches, formats.offset,
    // The scan finds code spans only for a code transformer.
    textFormatTransformersIndex.transformersByTag["`"] != nil
      ? formats.codeSpans.map { ($0.startIndex, $0.endIndex) } : nil)

  if let format = foundTextFormat, let textMatch = foundTextMatch {
    // Code spans bind tighter than text-match transformers (e.g. equations
    // or links). A code span must never be partially consumed by a text
    // match, so the match goes only when it fully wraps the span.
    // https://github.com/meta/lexical/issues/8687
    let startIndex = textMatch.startIndex
    let endIndex = textMatch.endIndex
    if partlyHoldsCodeSpan(formats, startIndex, endIndex) {
      // Apply the code span, or the emphasis before it, and not the text
      // match, which would take part of the span.
      foundTextMatch = nil
    } else if format.isCodeSpan {
      // The text match goes first when it wraps the code span or ends
      // before it; one after it waits for the span.
      if (startIndex <= format.startIndex && endIndex >= format.endIndex)
        || endIndex <= format.startIndex
      {
        foundTextFormat = nil
      } else {
        foundTextMatch = nil
      }
    } else if (format.startIndex <= textMatch.startIndex && format.endIndex >= textMatch.endIndex)
      || textMatch.startIndex > format.endIndex
    {
      // The text format wraps the text match, or the match is not inside it.
      foundTextMatch = nil
    } else {
      // The text match wraps the text format.
      foundTextFormat = nil
    }
  }

  // Defers the import of the text before a match from `startIndex` to
  // `endIndex` and of the text `inside` it (or the escapes of `matched`),
  // and returns the text after it, each with what lies beyond the run cut
  // at its edge. A text match's transformer has run, and when it changed the
  // text after it, that text is imported on its own; `textAfter` is what
  // that text was before (`isTextMatch`), and absent for a format match.
  func importAround(
    nodeBefore: LexicalNode?, nodeAfter: LexicalNode?, isTextMatch: Bool, textAfter: String?,
    startIndex: Int, endIndex: Int, inside: TextPart, matched: LexicalNode?
  ) throws -> TextRest? {
    deferred.append(
      Deferred(
        outside: TextPart(
          node: nodeBefore, before: before,
          after: characterBeyondRun(text, startIndex, 1, after, delimiters, isTextMatch)),
        inside: inside, matched: matched))
    guard canContainTransformableMarkdown(nodeAfter), let nodeAfter = nodeAfter as? TextNode
    else { return nil }
    let rest = try nodeAfter.getTextContent()
    if isTextMatch && !(textAfter ?? "").utf16.elementsEqual(rest.utf16) {
      return TextRest(node: nodeAfter, before: nil, after: nil, scan: nil)
    }
    let restBefore = characterBeyondRun(text, endIndex, -1, before, delimiters, isTextMatch)
    // The scan read a run at the start of the rest beside the end of the
    // match. Read beside the edge of a line instead, it can only change the
    // pair it opens, since it has nothing before it in the rest to close.
    var isReusable = advanceFormatScan(formats, formats.offset + endIndex, !isTextMatch)
    if isReusable && isTextMatch && restBefore == nil,
      let first = rest.jsCharAt(0), delimiters.contains(first),
      readsOtherwiseBesideEdge(rest, characterBefore(text, endIndex), after)
    {
      isReusable = findPairAtEdge(formats, rest, textFormatTransformersIndex)
    }
    scan.rest = rest
    return TextRest(
      node: nodeAfter, before: restBefore, after: after, scan: isReusable ? scan : nil)
  }

  if let format = foundTextFormat {
    let result = try importTextFormatTransformer(
      textNode, format.startIndex, format.endIndex, format.transformer, format.match)
    let tag = format.match[1] ?? ""
    let tagFirst = tag.jsCharAt(0)
    let tagLength = tag.jsLength
    // The content reads a run at its edge beside the tag, so that it flanks
    // as it does in the line, when it is of a marker that CommonMark or GFM
    // define and the tag does not cut it; otherwise beside the edge of a
    // line. A code span's content is not read for emphasis.
    func beside(_ char: String?) -> String? {
      guard !format.isCodeSpan, let char, SPEC_MARKERS.contains(char), char != tagFirst else {
        return nil
      }
      return tagFirst
    }
    return try importAround(
      nodeBefore: result.nodeBefore, nodeAfter: result.nodeAfter, isTextMatch: false,
      textAfter: nil, startIndex: format.startIndex, endIndex: format.endIndex,
      inside: TextPart(
        node: result.transformedNode,
        before: beside(text.jsCharAt(format.startIndex + tagLength)),
        after: beside(text.jsCharAt(format.endIndex - tagLength - 1))),
      matched: nil)
  } else if let textMatch = foundTextMatch {
    guard
      let result = try importFoundTextMatchTransformer(
        textNode, textMatch.startIndex, textMatch.endIndex, textMatch.transformer,
        textMatch.match)
    else { return nil }
    return try importAround(
      nodeBefore: result.nodeBefore, nodeAfter: result.nodeAfter, isTextMatch: true,
      textAfter: result.textAfter, startIndex: textMatch.startIndex,
      endIndex: textMatch.endIndex,
      inside: TextPart(node: result.transformedNode, before: nil, after: nil),
      matched: result.transformedNode == nil ? result.matchedNode : nil)
  }

  // Handle escape characters. Only a node that no transformer matched gets
  // here: retained non-code results above have already had their escapes
  // read, and code spans must keep their backslashes.
  try textNode.setTextContent(unescapeText(text))
  return nil
}
