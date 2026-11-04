import Lexical

// Port of packages/lexical-code-core/src/FlatStructureUtils.ts. Text is
// indexed by UTF-16 code unit, as in JS.

/// The last node of the run of CodeHighlightNodes and TabNodes next to
/// `anchor` in `direction`, or `anchor` itself.
private func getLastMatchingCodeNode(
  _ anchor: LexicalNode, _ direction: CaretDirection
) throws
  -> LexicalNode
{
  var matchingNode = anchor
  let first: SiblingCaret = getSiblingCaret(anchor, direction)
  var caret: SiblingCaret? = first
  while let current = caret, current.origin is CodeHighlightNode || current.origin is TabNode {
    matchingNode = current.origin
    caret = try current.getAdjacentCaret()
  }
  return matchingNode
}

/// `$getFirstCodeNodeOfLine(anchor)`.
public func getFirstCodeNodeOfLine(_ anchor: LexicalNode) throws -> LexicalNode {
  try getLastMatchingCodeNode(anchor, .previous)
}

/// `$getLastCodeNodeOfLine(anchor)`.
public func getLastCodeNodeOfLine(_ anchor: LexicalNode) throws -> LexicalNode {
  try getLastMatchingCodeNode(anchor, .next)
}

/// `$getCodeLineDirection(anchor)`: the direction of the first strong
/// character on the line, else the code block's own direction.
public func getCodeLineDirection(_ anchor: LexicalNode) throws -> ElementDirection? {
  let start = try getFirstCodeNodeOfLine(anchor)
  let end = try getLastCodeNodeOfLine(anchor)
  var node: LexicalNode? = start
  while let current = node {
    if current is CodeHighlightNode,
      let direction = getTextDirection(try current.getTextContent())
    {
      return direction
    }
    if current === end { break }
    node = try current.getNextSibling()
  }
  if let parent = try start.getParent() {
    let parentDirection = try parent.getDirection()
    if parentDirection == "ltr" || parentDirection == "rtl" { return parentDirection }
  }
  return nil
}

/// A node and an offset in it, from `$getStartOfCodeInLine`.
public struct CodeLinePoint {
  public var node: LexicalNode
  public var offset: Int
}

private let SPACE: UInt16 = 0x20

/// `$getStartOfCodeInLine(anchor, offset)`.
public func getStartOfCodeInLine(_ anchor: LexicalNode, _ offset: Int) throws -> CodeLinePoint? {
  var last: CodeLinePoint?
  var lastNonBlank: CodeLinePoint?
  var node: LexicalNode = anchor
  var nodeOffset = offset
  var nodeTextContent = Array(try anchor.getTextContent().utf16)

  while true {
    if nodeOffset == 0 {
      guard let prevSibling = try node.getPreviousSibling() else { break }
      try invariant(
        prevSibling is CodeHighlightNode || prevSibling is TabNode
          || prevSibling is LineBreakNode,
        "Expected a valid Code Node: CodeHighlightNode, TabNode, LineBreakNode")
      node = prevSibling
      if node is LineBreakNode {
        last = CodeLinePoint(node: node, offset: 1)
        break
      }
      nodeOffset = max(0, try node.getTextContentSize() - 1)
      nodeTextContent = Array(try node.getTextContent().utf16)
    } else {
      nodeOffset -= 1
    }
    let character = nodeOffset < nodeTextContent.count ? nodeTextContent[nodeOffset] : nil
    if node is CodeHighlightNode && character != SPACE {
      lastNonBlank = CodeLinePoint(node: node, offset: nodeOffset)
    }
  }
  // Anchor in the middle of code: move to the line beginning.
  if let lastNonBlank { return lastNonBlank }
  // Spaces, tabs or nothing ahead of the anchor.
  var codeCharacterAtAnchorOffset: UInt16?
  if try offset < anchor.getTextContentSize() {
    if anchor is CodeHighlightNode {
      codeCharacterAtAnchorOffset = Array(try anchor.getTextContent().utf16)[offset]
    }
  } else if let nextSibling = try anchor.getNextSibling() as? CodeHighlightNode {
    codeCharacterAtAnchorOffset = try nextSibling.getTextContent().utf16.first
  }
  if let codeCharacterAtAnchorOffset, codeCharacterAtAnchorOffset != SPACE {
    // Borderline whitespace and code: move to the line beginning.
    return last
  }
  return try findNextNonBlankInLine(anchor, offset) ?? last
}

private func findNextNonBlankInLine(
  _ anchor: LexicalNode, _ offset: Int
) throws
  -> CodeLinePoint?
{
  var node: LexicalNode = anchor
  var nodeOffset = offset
  var nodeTextContent = Array(try anchor.getTextContent().utf16)
  while true {
    if !(node is CodeHighlightNode) || nodeOffset == nodeTextContent.count {
      guard let next = try node.getNextSibling(), !(next is LineBreakNode) else { return nil }
      node = next
      if node is CodeHighlightNode {
        nodeOffset = 0
        nodeTextContent = Array(try node.getTextContent().utf16)
      }
    }
    if node is CodeHighlightNode {
      let character = nodeOffset < nodeTextContent.count ? nodeTextContent[nodeOffset] : nil
      if character != SPACE {
        return CodeLinePoint(node: node, offset: nodeOffset)
      }
      nodeOffset += 1
    }
  }
}

/// `$getEndOfCodeInLine(anchor)`.
public func getEndOfCodeInLine(_ anchor: LexicalNode) throws -> LexicalNode {
  let lastNode = try getLastCodeNodeOfLine(anchor)
  try invariant(!(lastNode is LineBreakNode), "Unexpected lineBreakNode in getEndOfCodeInLine")
  return lastNode
}

/// `$plainifyCodeContent(text)`: `text` as CodeHighlightNodes, TabNodes
/// and LineBreakNodes.
public func plainifyCodeContent(_ text: String) throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  try tokenizeRawText(
    text,
    RawTextVisitor(
      linebreak: { out.append(try createLineBreakNode()) },
      tab: { out.append(try createTabNode()) },
      text: { out.append(try createCodeHighlightNode($0)) }))
  return out
}

/// `$outdentLeadingSpaces(node, tabSize, selection)`: removes up to
/// `tabSize` leading spaces from `node`, keeping the selection on the same
/// characters.
public func outdentLeadingSpaces(
  _ node: CodeHighlightNode, _ tabSize: Int, _ selection: RangeSelection
) throws -> Bool {
  if tabSize <= 0 { return false }
  let text = try node.getTextContent().utf16
  let leading = text.prefix { $0 == SPACE }.count
  if leading == 0 { return false }
  let stripCount = min(tabSize, leading)
  let lineKey = node.getKey()
  let oldAnchorOffset =
    selection.anchor.key == lineKey && selection.anchor.type == .text
    ? selection.anchor.offset : nil
  let oldFocusOffset =
    selection.focus.key == lineKey && selection.focus.type == .text
    ? selection.focus.offset : nil
  try node.spliceText(0, stripCount, "")
  if let oldAnchorOffset {
    try selection.anchor.set(lineKey, max(0, oldAnchorOffset - stripCount), .text)
  }
  if let oldFocusOffset {
    try selection.focus.set(lineKey, max(0, oldFocusOffset - stripCount), .text)
  }
  return true
}
