import Lexical

// Port of packages/lexical-overflow/src/index.ts.

/// `OverflowNode`: an inline `<span>` that holds the text past the character
/// limit. It removes itself once it is empty, and is left out of copies.
open class OverflowNode: ElementNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(
      OverflowNode.self, type: "overflow", extends: ElementNode.self,
      transform: { node in
        if let node = node as? OverflowNode, try node.isEmpty() {
          try node.remove()
        }
      })
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let div = getDocument().createElement("span")
    if let className = config.theme["characterLimit"]?.stringValue {
      div.className = className
    }
    return div
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    false
  }

  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    let parent = try getParentOrThrow()
    return try parent.insertNewAfter(selection, restoreSelection: restoreSelection)
  }

  open override func excludeFromCopy(_ destination: String? = nil) throws -> Bool { true }
}

/// `$createOverflowNode()`.
public func createOverflowNode() throws -> OverflowNode {
  try applyNodeReplacement(OverflowNode())
}

/// `$isOverflowNode(node)`.
public func isOverflowNode(_ node: LexicalNode?) -> Bool {
  node is OverflowNode
}

/// `OverflowExtension`: registers `OverflowNode`.
public let OverflowExtension = defineExtension(
  name: "@lexical/overflow", nodes: { [.klass(OverflowNode.self)] })
