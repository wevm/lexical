import Lexical
import LexicalList
import LexicalRichText

// Port of packages/lexical-markdown/src/MarkdownImport.ts.

/// `$importMarkdownNodes(markdownString, container, transformers,
/// shouldPreserveNewLines)`: parses `markdownString` and appends the nodes
/// to `container`. Does not clear the container or touch the selection.
package func importMarkdownNodes(
  _ markdownString: String, _ container: ElementNode, _ transformers: [any Transformer],
  _ shouldPreserveNewLines: Bool = false
) throws {
  let byType = transformersByType(transformers)
  let textFormatTransformersIndex = createTextFormatTransformersIndex(byType.textFormat)
  let lines = markdownString.jsSplit("\n")

  // A list line is measured against the column its parent item's content
  // starts at, which only the line that opened that level knows. Blank
  // lines between list lines make the list loose rather than ending it,
  // except when they are being preserved, where they are content like any
  // block.
  try withListIndentColumns(!shouldPreserveNewLines) {
    var i = 0
    while i < lines.count {
      let (imported, shiftedIndex) = try importMultiline(
        lines, i, byType.multilineElement, container)
      if imported {
        i = shiftedIndex + 1
        continue
      }
      try importBlocks(
        lines[i], container, byType.element, textFormatTransformersIndex, byType.textMatch,
        shouldPreserveNewLines)
      i += 1
    }
  }

  for child in try container.getChildren() {
    if try !shouldPreserveNewLines && isEmptyParagraph(child) && container.getChildrenSize() > 1 {
      try child.remove()
      continue
    }
    if let child = child as? ElementNode {
      for textNode in try child.getAllTextNodes() {
        try normalizeMarkdownTextNode(textNode)
      }
    }
  }
}

/// Imports the multiline element that starts at `startLineIndex`, if one
/// does: whether one was imported, and the index of the last line it took.
private func importMultiline(
  _ lines: [String], _ startLineIndex: Int,
  _ multilineElementTransformers: [MultilineElementTransformer], _ rootNode: ElementNode
) throws -> (Bool, Int) {
  for transformer in multilineElementTransformers {
    guard let startMatch = lines[startLineIndex].jsMatch(transformer.regExpStart) else {
      continue
    }
    if let handleImportAfterStartMatch = transformer.handleImportAfterStartMatch {
      switch try handleImportAfterStartMatch(
        MultilineImportArgs(
          lines: lines, rootNode: rootNode, startLineIndex: startLineIndex,
          startMatch: startMatch, transformer: transformer))
      {
      case .skip: continue
      case .handled(let imported, let index): return (imported, index)
      case .useDefault: break
      }
    }

    let regexpEndRegex = transformer.regExpEnd?.regExp
    let isEndOptional = transformer.regExpEnd?.optional ?? true
    var endLineIndex = startLineIndex
    let linesLength = lines.count

    // Check every line for the closing match. It could also be on the same
    // line as the opening match.
    while endLineIndex < linesLength {
      let endMatch = regexpEndRegex.flatMap { lines[endLineIndex].jsMatch($0) }
      if endMatch == nil {
        if !isEndOptional || endLineIndex < linesLength - 1 {
          // Search the next line for the closing match.
          endLineIndex += 1
          continue
        }
      }
      // A closing match that is the opening match is not one.
      if let endMatch, startLineIndex == endLineIndex, endMatch.index == startMatch.index {
        endLineIndex += 1
        continue
      }
      // The lines between the opening and closing matches, without them.
      var linesInBetween: [String] = []
      if let endMatch, startLineIndex == endLineIndex {
        linesInBetween.append(
          lines[startLineIndex].jsSlice(startMatch.length, -endMatch.length))
      } else {
        for i in startLineIndex...endLineIndex {
          if i == startLineIndex {
            linesInBetween.append(lines[i].jsSlice(startMatch.length))
          } else if i == endLineIndex, let endMatch {
            linesInBetween.append(lines[i].jsSlice(0, -endMatch.length))
          } else {
            linesInBetween.append(lines[i])
          }
        }
      }
      if try transformer.replace(rootNode, nil, startMatch, endMatch, linesInBetween, true)
        != false
      {
        // Only one multiline element is processed at a time.
        return (true, endLineIndex)
      }
      // The transformer declined despite the matching open and close tags:
      // the remaining transformers get the lines from the beginning.
      break
    }
  }
  return (false, startLineIndex)
}

private func importBlocks(
  _ lineText: String, _ rootNode: ElementNode, _ elementTransformers: [ElementTransformer],
  _ textFormatTransformersIndex: TextFormatTransformersIndex,
  _ textMatchTransformers: [TextMatchTransformer], _ shouldPreserveNewLines: Bool
) throws {
  let textNode = try createTextNode(lineText)
  let elementNode = try createParagraphNode()
  try elementNode.append(textNode)
  try rootNode.append(elementNode)

  for transformer in elementTransformers {
    if let match = lineText.jsMatch(transformer.regExp) {
      try textNode.setTextContent(lineText.jsSlice(match.length))
      if try transformer.replace(elementNode, [textNode], match, true) != false {
        break
      }
    }
  }

  try importTextTransformers(textNode, textFormatTransformersIndex, textMatchTransformers)

  // If no transformer matched and the paragraph is left, its content can
  // be appended to the previous node if that is a paragraph, quote or list.
  guard try elementNode.getParent() != nil, !lineText.isEmpty, !shouldPreserveNewLines,
    let previousNode = try elementNode.getPreviousSibling() as? ElementNode,
    previousNode is ParagraphNode || previousNode is QuoteNode || previousNode is ListNode
  else { return }
  var targetNode: ElementNode? = previousNode
  if let list = previousNode as? ListNode {
    targetNode = try list.getLastDescendant().flatMap {
      try findMatchingParent($0) { $0 is ListItemNode } as? ElementNode
    }
  }
  if let targetNode, try targetNode.getTextContentSize() > 0 {
    try targetNode.splice(
      targetNode.getChildrenSize(), 0,
      [try createMarkdownLineBreakNode(targetNode)] + elementNode.getChildren())
    try elementNode.remove()
  }
}

private let TAB = UInt16(UInt8(ascii: "\t"))

/// Splits each `\t` of `textNode` into a TabNode. The nodes are built
/// directly rather than through `splitText`, which would take one offset
/// per tab boundary.
private func normalizeMarkdownTextNode(_ textNode: TextNode) throws {
  // A TabNode is a TextNode whose content is a tab; rebuilding it would
  // replace it with an equivalent one.
  if textNode is TabNode { return }
  let text = try textNode.getTextContent()
  let units = Array(text.utf16)
  guard units.contains(TAB) else { return }
  let format = try textNode.getFormat()
  let style = try textNode.getStyle()
  var nodes: [LexicalNode] = []
  var start = 0
  for (index, unit) in units.enumerated() where unit == TAB {
    if index > start {
      nodes.append(
        try createTextNode(text.jsSlice(start, index)).setFormat(format).setStyle(style))
    }
    nodes.append(try createTabNode())
    start = index + 1
  }
  if start < units.count {
    nodes.append(try createTextNode(text.jsSlice(start)).setFormat(format).setStyle(style))
  }
  try textNode.getParentOrThrow().splice(textNode.getIndexWithinParent(), 1, nodes)
}

nonisolated(unsafe) private let TAG_SPECIALS = JSRegExp(#"(\*|\^|\+)"#, "g")

private func createTextFormatTransformersIndex(
  _ textTransformers: [TextFormatTransformer]
)
  -> TextFormatTransformersIndex
{
  var transformersByTag: [String: TextFormatTransformer] = [:]
  var fullMatchRegExpByTag: [String: JSRegExp] = [:]
  var openTagsRegExp: [String] = []
  var tags: [String] = []

  for transformer in textTransformers {
    let tag = transformer.tag
    if transformersByTag[tag] == nil { tags.append(tag) }
    transformersByTag[tag] = transformer
    let tagRegExp = tag.jsReplace(TAG_SPECIALS, template: #"\$1"#)
    openTagsRegExp.append(tagRegExp)

    if tag.jsLength == 1 {
      if tag == "`" {
        // The preceding character is captured in group 1 (empty at the
        // start) rather than with a lookbehind; consumers add its length to
        // the match index to find where the span starts.
        fullMatchRegExpByTag[tag] = JSRegExp(#"(^|[^\\`])(`)((?:\\`|[^`])+?)(`)(?!`)"#)
      } else {
        fullMatchRegExpByTag[tag] = JSRegExp(
          #"(^|[^\\"# + tagRegExp + "])(" + tagRegExp + #")((\\"# + tagRegExp + #")?.*?[^"#
            + tagRegExp + #"\s](\\"# + tagRegExp + ")?)(" + tagRegExp + #")(?![\\"#
            + tagRegExp + "])")
      }
    } else {
      // Multi-character tags (e.g. "**").
      fullMatchRegExpByTag[tag] = JSRegExp(
        #"(^|[^\\])("# + tagRegExp + #")((\\"# + tagRegExp + #")?.*?[^\s](\\"# + tagRegExp
          + ")?)(" + tagRegExp + #")(?!\\)"#)
    }
  }

  return TextFormatTransformersIndex(
    // The regex for an open tag, content and close tag.
    fullMatchRegExpByTag: fullMatchRegExpByTag,
    // The regex for any opening tag (longest first). Escapes are filtered
    // by the delimiter scanner's isEscaped() check at match time.
    openTagsRegExp: JSRegExp("(" + openTagsRegExp.joined(separator: "|") + ")", "g"),
    transformersByTag: transformersByTag, tags: tags)
}
