import Lexical
import LexicalClipboard
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts.
// Upstream builds the editor from TableExtension only to register the table
// nodes; this registers them directly.

/// A shadow-root block used as a slot value, mirroring the shape a slot host
/// holds in production.
private final class PlainShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainShadowRootNode.self, type: "plain_shadow_root_table", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  { false }
  override func isShadowRoot() -> Bool { true }
}

private let CELL_TEXT = "SlotTableTarget"

private func buildEditorWithSlottedTable() throws -> Editor {
  let editor = try createTestEditor(nodes: tableTestNodes + [.klass(PlainShadowRootNode.self)])
  try editor.update(discrete: true) {
    let table = try createTableNode()
    for r in 0..<2 {
      let row = try createTableRowNode()
      for c in 0..<2 {
        try row.append(
          createTableCellNode().append(
            createParagraphNode().append(createTextNode("\(CELL_TEXT)-\(r)\(c)"))))
      }
      try table.append(row)
    }
    let host = try createParagraphNode()
    try setSlot(host, "media", create(PlainShadowRootNode.self).append(table))
    try getRoot().append(host)

    let (tableMap, _, _) = try computeTableMapSkipCellCheck(table, nil, nil)
    try setSelection(createTableSelectionFrom(table, tableMap[0][0]!.cell, tableMap[1][1]!.cell))
  }
  return editor
}

// A TableSelection is neither a RangeSelection nor a NodeSelection, so the
// slot frame redirect has to key off the nodes it reports rather than off a
// selection-type check. Without that, copying cells from a table nested in a
// slot walks the root's children, never reaches the slot subtree, and both
// clipboard channels come out empty.
@Suite struct SlotTableSelectionExportTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts::slot frame redirect for a TableSelection > text/html export contains the selected cells"
    ))
  func htmlExport() throws {
    let editor = try buildEditorWithSlottedTable()
    try editor.read { () throws -> Void in
      #expect(try generateHtmlFromNodes(editor, getSelection()).contains(CELL_TEXT))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/SlotTableSelectionExport.test.ts::slot frame redirect for a TableSelection > application/x-lexical-editor export contains the selected cells"
    ))
  func jsonExport() throws {
    let editor = try buildEditorWithSlottedTable()
    try editor.read { () throws -> Void in
      let nodes = try generateJSONFromSelectedNodes(editor, getSelection()).nodes
      #expect(JSONValue.array(nodes.map { .object($0) }).stringify().contains(CELL_TEXT))
    }
  }
}
