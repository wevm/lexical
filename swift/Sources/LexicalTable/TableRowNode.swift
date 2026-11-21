import Lexical
import LexicalUtils

// Port of packages/lexical-table/src/LexicalTableRowNode.ts. The generated
// JSON implementations (LexicalTableGeneratedJSON.ts) are not ported; the
// schema below drives both.

/// `TableRowNode`: a `<tr>`, holding table cells.
open class TableRowNode: ElementNode {
  package var __height: Double?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TableRowNode.self, type: "tablerow", extends: ElementNode.self,
      json: [
        SchemaField("height", optional(numberValue()), field: \TableRowNode.__height)
      ])
  }

  open override class func importDOM() -> DOMConversionMap? {
    ["tr": { _ in DOMConversion(priority: 0, convertTableRowElement) }]
  }

  /// `new TableRowNode(height, key)`.
  public convenience init(_ height: Double? = nil, key: NodeKey? = nil) throws {
    try self.init(key: key)
    __height = height
  }

  /// Copies every class field, as the list nodes do.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __height = (prevNode as! TableRowNode).__height
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("tr")

    if let height = __height, height != 0, !height.isNaN {
      element.style.setProperty("height", "\(JSONValue.number(height).stringify())px")
    }

    addClassNamesToElement(element, config.theme["tableRow"]?.stringValue)

    return element
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    destination == "html"
  }

  open override func isShadowRoot() -> Bool { true }

  @discardableResult
  open func setHeight(_ height: Double? = nil) throws -> Self {
    let writable = try getWritable()
    writable.__height = height
    return writable
  }

  open func getHeight() throws -> Double? {
    try getLatest().__height
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    (prevNode as! TableRowNode).__height != __height
  }

  open override func canBeEmpty() throws -> Bool { false }

  open override func canIndent() -> Bool { false }
}

/// `$convertTableRowElement(domNode)`.
package func convertTableRowElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let domNode_ = domNode as? DOMElement else { return nil }
  var height: Double? = nil

  if let match = try? PIXEL_VALUE_REG_EXP.wholeMatch(in: domNode_.style["height"]) {
    height = Double(match.1)
  }

  return DOMConversionOutput(
    node: try setDirectionFromDOM(createTableRowNode(height), domNode_),
    after: { children in try descendantsMatching(children, TableCellNode.self) })
}

/// `$createTableRowNode(height)`.
public func createTableRowNode(_ height: Double? = nil) throws -> TableRowNode {
  try applyNodeReplacement(TableRowNode(height))
}

/// `$isTableRowNode(node)`.
public func isTableRowNode(_ node: LexicalNode?) -> Bool { node is TableRowNode }
