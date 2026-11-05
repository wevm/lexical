import Lexical

// Port of packages/lexical-hashtag/src/LexicalHashtagNode.ts.

/// `HashtagNode`: a `#hashtag`, a text entity that takes the theme's
/// `hashtag` class.
open class HashtagNode: TextNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(HashtagNode.self, type: "hashtag", extends: TextNode.self)
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = try super.createDOM(config, editor)
    addClassNamesToElement(element, config.theme["hashtag"]?.stringValue)
    return element
  }

  open override func canInsertTextBefore() throws -> Bool { false }

  open override func isTextEntity() -> Bool { true }
}

/// `$createHashtagNode(text)`.
public func createHashtagNode(_ text: String = "") throws -> HashtagNode {
  try applyNodeReplacement(HashtagNode(text))
}

/// `$isHashtagNode(node)`.
public func isHashtagNode(_ node: LexicalNode?) -> Bool {
  node is HashtagNode
}
