import Lexical
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx.

private func makeEditor() throws -> Editor {
  let editor = try createTestEditor(
    nodes: [.klass(TableNode.self), .klass(TableCellNode.self), .klass(TableRowNode.self)])
  _ = try registerTablePlugin(editor)
  return editor
}

@Suite struct LexicalTablePluginTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx::LexicalTablePlugin > INSERT_TABLE_COMMAND inserts a table"
    ))
  func insertsTable() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) {
      try getRoot().select()
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "3", rows: "2"))
    }

    try editor.read(.latest) {
      let table = try #require(try getRoot().getFirstChild() as? TableNode, "Expected table node")
      let rows = try table.getChildren()
      #expect(rows.count == 2)
      let firstRow = try #require(rows[0] as? ElementNode, "Expected row node")
      #expect(try firstRow.getChildren().count == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTablePlugin.test.tsx::LexicalTablePlugin > INSERT_TABLE_COMMAND inserts a table when the editor is blurred"
    ))
  func insertsTableWhenBlurred() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { try getRoot().select() }

    try editor.update(discrete: true) {
      // Simulate a blur event
      try setSelection(nil)
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "3", rows: "2"))
    }

    try editor.read(.latest) { () throws -> Void in
      #expect(try getRoot().getFirstChild() is TableNode)
    }
  }
}
