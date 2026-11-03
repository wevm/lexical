import Lexical

// Port of HorizontalRuleNode from
// packages/lexical-extension/src/HorizontalRuleExtension.ts. The extension
// itself (INSERT_HORIZONTAL_RULE_COMMAND, click selection and the selected
// class) needs EditorStateExtension and NodeSelectionExtension, and ports
// with them.

/// `HorizontalRuleNode`: an `<hr>`, a block decorator with no fields.
open class HorizontalRuleNode: DecoratorNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(HorizontalRuleNode.self, type: "horizontalrule", extends: DecoratorNode.self)
  }

  open override class func importDOM() -> DOMConversionMap? {
    [
      "hr": { _ in DOMConversion { _ in DOMConversionOutput(node: try createHorizontalRuleNode()) }
      }
    ]
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    DOMExportOutput(element: getDocument().createElement("hr"))
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("hr")
    addClassNamesToElement(element, config.theme["hr"]?.stringValue)
    return element
  }

  open override func getTextContent() throws -> String { "\n" }

  open override func isInline() throws -> Bool { false }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    false
  }
}

/// `$createHorizontalRuleNode()`.
public func createHorizontalRuleNode() throws -> HorizontalRuleNode {
  try create(HorizontalRuleNode.self)
}

/// `$isHorizontalRuleNode(node)`.
public func isHorizontalRuleNode(_ node: LexicalNode?) -> Bool {
  node is HorizontalRuleNode
}
