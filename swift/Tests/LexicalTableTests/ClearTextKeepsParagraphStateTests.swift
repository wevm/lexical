import Lexical
import LexicalExtension
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/ClearTextKeepsParagraphState.test.ts.

private struct ParagraphState: Equatable {
  var direction: ElementDirection?
  var format: ElementFormatType
  var style: String
  var text: String
}

private func firstRowParagraphState() throws -> [ParagraphState] {
  let table = try #require(try getRoot().getFirstChild() as? TableNode)
  let firstRow = try table.getChildren().compactMap { $0 as? TableRowNode }[0]
  return try firstRow.getChildren().compactMap { $0 as? TableCellNode }.map { cell in
    let paragraph = try #require(try cell.getFirstChild() as? ParagraphNode)
    return ParagraphState(
      direction: try paragraph.getDirection(), format: try paragraph.getFormatType(),
      style: try paragraph.getStyle(), text: try cell.getTextContent())
  }
}

@Suite struct ClearTextKeepsParagraphStateTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/ClearTextKeepsParagraphState.test.ts::clearing a table selection > keeps the paragraph format, style and direction of each cell"
    ))
  func keepsParagraphState() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "clear-text-host", dependencies: [TableExtension],
        editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": ""])))
    defer { editor.dispose() }
    // The table selection observer only registers its key handlers once the
    // editor has a root element in the document.
    try editor.setRootElement(TestRootElement())

    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(2, 2, false)
      try getRoot().clear().append(table)
      let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
      for case let value? in map[0] {
        let paragraph = try #require(try value.cell.getFirstChild() as? ParagraphNode)
        try paragraph.setFormat("center").setDirection("rtl").setStyle("line-height: 2;")
        try paragraph.append(createTextNode("text"))
      }
      try setSelection(createTableSelectionFrom(table, map[0][0]!.cell, map[0][1]!.cell))
    }

    // Backspace over a multi-cell selection routes to TableObserver.clearText.
    // The selection covers only the first row, so the table itself survives.
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(KEY_BACKSPACE_COMMAND, KeyboardEvent(key: "Backspace"))
    }

    let state = try editor.read(.latest) { try firstRowParagraphState() }
    let expected = ParagraphState(
      direction: "rtl", format: "center", style: "line-height: 2;", text: "")
    #expect(state == [expected, expected])
  }
}
