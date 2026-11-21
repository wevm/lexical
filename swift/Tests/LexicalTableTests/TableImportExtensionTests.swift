import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableImportExtension.test.ts.

/// TableExtension registers its own import rules (and the shared
/// CoreImportExtension baseline); no dedicated import extension is needed.
private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "table-host", dependencies: [TableExtension],
      editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": ""])))
}

private func generate(_ html: String) throws -> [LexicalNode] {
  let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom)
}

private func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) { try getRoot().clear().append(contentsOf: generate(html)) }
}

private func rootTable() throws -> TableNode {
  try #require(try getRoot().getFirstChild() as? TableNode, "expected TableNode at root")
}

private func rows(_ table: TableNode) throws -> [TableRowNode] {
  try table.getChildren().compactMap { $0 as? TableRowNode }
}

private func cells(_ row: TableRowNode) throws -> [TableCellNode] {
  try row.getChildren().compactMap { $0 as? TableCellNode }
}

@Suite struct TableImportExtensionTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > basic 2x2 table imports with correct structure"
    ))
  func basicTable() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor, "<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>")
    try editor.read {
      let rows = try rows(rootTable())
      #expect(rows.count == 2)
      let firstRowCells = try cells(rows[0])
      #expect(firstRowCells.count == 2)
      #expect(try firstRowCells[0].getTextContent() == "a")
      #expect(try firstRowCells[1].getTextContent() == "b")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > <th scope=\"col\"> → header cell"
    ))
  func headerCell() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<table><tr><th scope="col">H</th></tr></table>"#)
    try editor.read {
      let cell = try cells(rows(rootTable())[0])[0]
      #expect(try cell.hasHeader())
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > table picks up <thead>/<tbody> rows via $descendantsMatching"
    ))
  func theadTbodyRows() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor,
      "<table><thead><tr><th>H</th></tr></thead><tbody><tr><td>B</td></tr></tbody></table>")
    try editor.read {
      let rows = try rows(rootTable())
      #expect(rows.count == 2)
      #expect(try rows[0].getFirstChild()?.getTextContent() == "H")
      #expect(try rows[1].getFirstChild()?.getTextContent() == "B")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableImportExtension.test.ts::TableImportExtension > row picks up cells via $descendantsMatching"
    ))
  func rowCells() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<table><tr><td>a</td></tr></table>")
    try editor.read {
      let cell = try cells(rows(rootTable())[0])[0]
      #expect(try cell.getTextContent() == "a")
    }
  }
}
