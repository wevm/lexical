import Lexical

// Port of packages/lexical-text/src/index.ts, rootTextContent.ts and
// isRootTextContentEmpty.ts.

/// `TextNodeWithOffset`.
public struct TextNodeWithOffset {
  public var node: TextNode
  public var offset: Int

  public init(node: TextNode, offset: Int) {
    self.node = node
    self.offset = offset
  }
}

/// `$rootTextContent()`: the text content of the root.
public func rootTextContent() throws -> String {
  let root = try getRoot()
  return try root.getTextContent()
}

/// `$isRootTextContentEmpty(isEditorComposing, trim)`: whether the root has
/// no text (only whitespace, when `trim`). Always `false` while composing.
public func isRootTextContentEmpty(_ isEditorComposing: Bool, trim: Bool = true) throws -> Bool {
  if isEditorComposing {
    return false
  }
  var text = try rootTextContent()
  if trim {
    text = jsTrim(text)
  }
  return text == ""
}

/// `$isRootTextContentEmptyCurry(isEditorComposing, trim)`.
public func isRootTextContentEmptyCurry(
  _ isEditorComposing: Bool, trim: Bool = true
) -> () throws -> Bool {
  { try isRootTextContentEmpty(isEditorComposing, trim: trim) }
}
