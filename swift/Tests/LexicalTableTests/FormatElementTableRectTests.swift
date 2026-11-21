import Lexical
import LexicalExtension
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/FormatElementTableRect.test.ts.

@Suite struct FormatElementTableRectTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/FormatElementTableRect.test.ts::FORMAT_ELEMENT_COMMAND over a table selection > formats every cell the selection contains, including one pulled in by a merge"
    ))
  func formatsEveryCellInTheRect() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "format-element-rect-host",
        dependencies: [TableExtension],
        editorConfig: InitialEditorConfig(
          theme: ["tableScrollableWrapper": "table-scrollable-wrapper"],
          initialEditorState: .update { _ in
            let table = try createTableNodeWithDimensions(3, 3, false)
            try getRoot().append(table)
            let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
            // Merge grid columns 0 and 1 of row 1. The merged cell straddles the
            // left edge of the rect the next selection describes, so the rect has
            // to grow to column 0 to contain it.
            let merged = try mergeCells([map[1][0]!.cell, map[1][1]!.cell])
            #expect(merged != nil, "expected the cells to merge")
          })))
    defer { editor.dispose() }
    // The FORMAT_ELEMENT_COMMAND handler is registered by the table selection
    // observer, which needs the editor to have a root element.
    try editor.setRootElement(TestRootElement())

    try editor.update(discrete: true) {
      let table = try #require(try getRoot().getFirstChild() as? TableNode)
      let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
      // Anchor at (row 0, column 1), focus at (row 1, column 2).
      try setSelection(createTableSelectionFrom(table, map[0][1]!.cell, map[1][2]!.cell))
    }

    _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "center")

    try editor.read(.forceCommit) {
      let selection = try #require(try getSelection(), "expected a selection")
      let selectedCells = try selection.getNodes().compactMap { $0 as? TableCellNode }
      #expect(selectedCells.count > 0)
      #expect(
        try selectedCells.map { try $0.getFormatType() } == selectedCells.map { _ in "center" })
    }
  }
}
