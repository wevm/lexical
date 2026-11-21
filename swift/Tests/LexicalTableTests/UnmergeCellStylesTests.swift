import Lexical
import LexicalExtension
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "unmerge-styles-host",
      dependencies: [TableExtension],
      editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": ""])))
}

private func cells() throws -> [[TableCellNode]] {
  let table = try #require(
    try getRoot().getFirstChild() as? TableNode, "expected a TableNode at the root")
  return try table.getChildren().compactMap { $0 as? TableRowNode }.map {
    try $0.getChildren().compactMap { $0 as? TableCellNode }
  }
}

/// Fills the root with a `rows` × `columns` table and merges all of its
/// cells, returning the merged cell.
private func appendMerged(_ rows: Int, _ columns: Int) throws -> TableCellNode {
  try getRoot().clear().append(createTableNodeWithDimensions(rows, columns, false))
  let merged = try mergeCells(cells().flatMap { $0 })
  return try #require(merged, "expected the cells to merge")
}

@Suite struct UnmergeCellStylesTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the cells a merged cell splits into keep its backgroundColor and verticalAlign"
    ))
  func keepsBackgroundColorAndVerticalAlign() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      _ = try appendMerged(2, 2).setBackgroundColor("rgb(255, 0, 0)").setVerticalAlign("middle")
    }

    try editor.update(discrete: true) {
      let merged = try cells()[0][0]
      #expect(try merged.getColSpan() == 2)
      #expect(try merged.getRowSpan() == 2)
      try unmergeCellNode(merged)
    }

    try editor.read {
      let rows = try cells()
      #expect(rows.map(\.count) == [2, 2])
      for row in rows {
        for cell in row {
          #expect(try cell.getColSpan() == 1)
          #expect(try cell.getRowSpan() == 1)
          #expect(try cell.getBackgroundColor() == "rgb(255, 0, 0)")
          #expect(try cell.getVerticalAlign() == "middle")
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the split cells keep the element format, style and direction"
    ))
  func keepsFormatStyleAndDirection() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let merged = try appendMerged(2, 2)
      _ = try merged.setFormat("center")
      _ = try merged.setStyle("color: red;")
      _ = try merged.setDirection("rtl")
    }

    try editor.update(discrete: true) { try unmergeCellNode(cells()[0][0]) }

    try editor.read {
      // Every cell, not only the one that was already there: a split that
      // formats the original differently from the cells beside it is the bug.
      for row in try cells() {
        for cell in row {
          #expect(try cell.getFormatType() == "center")
          #expect(try cell.getStyle() == "color: red;")
          #expect(try cell.getDirection() == "rtl")
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the split cells take no width, which the merged cell measured wide"
    ))
  func splitCellsTakeNoWidth() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let merged = try appendMerged(1, 2)
      #expect(try merged.getColSpan() == 2)
      _ = try merged.setWidth(240)
    }

    try editor.update(discrete: true) { try unmergeCellNode(cells()[0][0]) }

    // The cell that was measured at 240px still is; the cell split off from
    // it must not claim that width for a single column as well, and like
    // every other cell created in LexicalTableUtils it leaves its width to
    // the table's colWidths.
    let widths = try editor.read { try cells()[0].map { try $0.getWidth() } }
    #expect(widths == [240, nil])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > an unstyled merged cell still splits into unstyled cells"
    ))
  func unstyledStaysUnstyled() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) { _ = try appendMerged(1, 2) }

    try editor.update(discrete: true) { try unmergeCellNode(cells()[0][0]) }

    try editor.read {
      for cell in try cells()[0] {
        #expect(try cell.getBackgroundColor() == nil)
        #expect(try cell.getVerticalAlign() == nil)
      }
    }
  }
}
