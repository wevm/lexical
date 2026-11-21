import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableExportAttributes.test.ts.
// Upstream configures `TableExtension` with `hasHorizontalScroll`; until it
// is ported, the editor sets what the extension sets
// (`setScrollableTablesActive`).

private func buildEditor(_ hasHorizontalScroll: Bool) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "table-export-attributes-host",
      nodes: {
        [.klass(TableNode.self), .klass(TableRowNode.self), .klass(TableCellNode.self)]
      },
      editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": "scroll-wrapper"])))
  setScrollableTablesActive(editor, hasHorizontalScroll)
  return editor
}

/// Export a one-cell table set up by `configureTable`. With
/// `hasHorizontalScroll` on, `createDOM` returns the scroll wrapper, so
/// everything `ElementNode.exportDOM` writes lands on an element the export
/// throws away. Both settings have to produce the same `<table>`.
private func exportTable(
  _ hasHorizontalScroll: Bool, _ configureTable: @escaping (TableNode) throws -> Void
) throws -> String {
  let editor = try buildEditor(hasHorizontalScroll)
  defer { editor.dispose() }
  try editor.update(discrete: true) {
    let table = try createTableNodeWithDimensions(1, 1, false)
    try configureTable(table)
    try getRoot().clear().append(table)
  }
  return try editor.read {
    try #require(getRoot().getFirstChild() as? TableNode)
    return try generateHtmlFromNodes(editor)
  }
}

@Suite struct TableExportAttributesTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM direction > exports dir with scrollable tables active"
    ))
  func dirWithScroll() throws {
    #expect(try exportTable(true) { try $0.setDirection("rtl") }.contains(#"<table dir="rtl">"#))
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM direction > exports dir without scrollable tables"
    ))
  func dirWithoutScroll() throws {
    #expect(try exportTable(false) { try $0.setDirection("rtl") }.contains(#"<table dir="rtl">"#))
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > exports the indent with scrollable tables active"
    ))
  func indentWithScroll() throws {
    let html = try exportTable(true) { try $0.setIndent(2) }
    #expect(html.contains(#"data-lexical-indent="2""#))
    #expect(html.contains("padding-inline-start: 80px"))
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > exports the indent without scrollable tables"
    ))
  func indentWithoutScroll() throws {
    let html = try exportTable(false) { try $0.setIndent(2) }
    #expect(html.contains(#"data-lexical-indent="2""#))
    #expect(html.contains("padding-inline-start: 80px"))
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableExportAttributes.test.ts::TableNode.exportDOM indent > writes nothing for an unindented table"
    ))
  func noIndent() throws {
    #expect(try !exportTable(true) { _ in }.contains("data-lexical-indent"))
  }
}
