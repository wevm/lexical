import Lexical

// Port of packages/lexical-text/src/canShowPlaceholder.ts.

/// `$canShowPlaceholder(isComposing)`: whether the editor shows its
/// placeholder: not composing, no text, and at most one top-level block,
/// an unindented paragraph that holds only text nodes.
public func canShowPlaceholder(_ isComposing: Bool) throws -> Bool {
  if try !isRootTextContentEmpty(isComposing, trim: false) {
    return false
  }
  let root = try getRoot()
  let children = try root.getChildren()
  if children.count > 1 {
    return false
  }
  for topBlock in children {
    if topBlock is DecoratorNode {
      return false
    }
    if let topBlock = topBlock as? ElementNode {
      if !(topBlock is ParagraphNode) {
        return false
      }
      if topBlock.__indent != 0 {
        return false
      }
      for child in try topBlock.getChildren() where !(child is TextNode) {
        return false
      }
    }
  }
  return true
}

/// `$canShowPlaceholderCurry(isEditorComposing)`.
public func canShowPlaceholderCurry(_ isEditorComposing: Bool) -> () throws -> Bool {
  { try canShowPlaceholder(isEditorComposing) }
}
