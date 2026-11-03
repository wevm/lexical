import Lexical

// Port of packages/lexical-html/src/__tests__/utils/setBlockText.ts, the
// `$setBlockText` of the Block IDs documentation page
// (packages/lexical-website/docs/extensions/block-ids.md). Offsets are in
// UTF-16 code units, as in JS.

/// `$setBlockText(block, text)`: changes a block's text in place, only the
/// characters that differ, in the block's own text nodes. Returns false,
/// changing nothing, unless the block holds only simple text, directly or in
/// inline elements such as links: no line breaks, nested blocks, named slots
/// or decorators. Text added at the edge of an inline element that takes no
/// text there (a link) goes beside it, as typing does.
@discardableResult
func setBlockText(_ block: LexicalNode, _ text: String) throws -> Bool {
  guard let block = block as? ElementNode, try holdsOnlyText(block) else {
    return false
  }
  let nodes = try block.getAllTextNodes()
  let old = Array(try nodes.map { try $0.getTextContent() }.joined().utf16)
  let new = Array(text.utf16)
  var start = 0
  while start < old.count && start < new.count && old[start] == new[start] {
    start += 1
  }
  var end = 0
  while end < old.count - start && end < new.count - start
    && old[old.count - 1 - end] == new[new.count - 1 - end]
  {
    end += 1
  }
  let inserted = String(decoding: new[start..<(new.count - end)], as: UTF16.self)
  let deleteEnd = old.count - end
  var offset = 0
  var done = false
  for node in nodes {
    let size = try node.getTextContentSize()
    let from = max(start - offset, 0)
    let to = min(deleteEnd - offset, size)
    if !done && (start - offset < size || (start - offset == size && start == deleteEnd)) {
      // The node where the change starts (for an insert at a node boundary,
      // the node before it): delete and insert there.
      if start == deleteEnd {
        try insertText(node, from, inserted)
      } else {
        _ = try node.spliceText(from, to - from, inserted)
      }
      done = true
    } else if to > from {
      _ = try node.spliceText(from, to - from, "")
    }
    offset += size
  }
  if !done && !inserted.isEmpty {
    try block.append(createTextNode(inserted))
  }
  return true
}

private func holdsOnlyText(_ element: ElementNode) throws -> Bool {
  // Named slots hold blocks of their own (and count in getTextContent).
  try getSlotNames(element).isEmpty
    && element.getChildren().allSatisfy { child in
      if let text = child as? TextNode {
        return try text.isSimpleText()
      }
      if let inline = child as? ElementNode {
        return try inline.isInline() && holdsOnlyText(inline)
      }
      return false
    }
}

/// Inserts text at `at` in `node`, or, at an edge of inline elements that
/// take no text there, beside the outermost of them.
private func insertText(_ node: TextNode, _ at: Int, _ text: String) throws {
  if text.isEmpty {
    return
  }
  let atEnd = try at == node.getTextContentSize()
  var outermost: ElementNode?
  var inner: LexicalNode = node
  var parent = try node.getParent()
  while let element = parent, try element.isInline() {
    let stop =
      try atEnd
      ? inner.getNextSibling() != nil : at != 0 || inner.getPreviousSibling() != nil
    if stop {
      break
    }
    if try atEnd ? !element.canInsertTextAfter() : !element.canInsertTextBefore() {
      outermost = element
    }
    inner = element
    parent = try element.getParent()
  }
  guard let outermost else {
    _ = try node.spliceText(at, 0, text)
    return
  }
  let beside = try atEnd ? outermost.getNextSibling() : outermost.getPreviousSibling()
  if let beside = beside as? TextNode, try beside.isSimpleText() {
    _ = try beside.spliceText(atEnd ? 0 : beside.getTextContentSize(), 0, text)
  } else if atEnd {
    try outermost.insertAfter(createTextNode(text))
  } else {
    try outermost.insertBefore(createTextNode(text))
  }
}
