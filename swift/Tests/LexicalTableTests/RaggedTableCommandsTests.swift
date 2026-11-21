import Lexical
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts.

private struct CellSpec {
  var text: String
  var colSpan = 1
  var rowSpan = 1
  var headerState = TableCellHeaderStates.NO_STATUS
}

/// Builds an editor whose table has exactly the given rows. The table
/// transform is deliberately not registered: this is the state a merge of
/// concurrent collaborative edits can leave behind (e.g. one peer inserts a
/// row while another inserts a column), because remote updates are applied
/// with transforms skipped.
private func buildTableEditor(_ rows: [[CellSpec]]) throws -> Editor {
  let editor = try createTestEditor(nodes: tableTestNodes)
  try editor.update(discrete: true) {
    let table = try createTableNode()
    for cells in rows {
      let row = try createTableRowNode()
      for spec in cells {
        try row.append(
          createTableCellNode(spec.headerState, colSpan: spec.colSpan)
            .setRowSpan(spec.rowSpan)
            .append(createParagraphNode().append(createTextNode(spec.text))))
      }
      try table.append(row)
    }
    try getRoot().clear().append(table, createParagraphNode())
  }
  return editor
}

private func specs(_ rows: [[String]]) -> [[CellSpec]] {
  rows.map { $0.map { CellSpec(text: $0) } }
}

// Rows of 3, 3 and 2 cells.
private let RAGGED = specs([
  ["00", "01", "02"],
  ["10", "11", "12"],
  ["20", "21"],
])

private func table() throws -> TableNode {
  try #require(try getRoot().getChildren().first { $0 is TableNode } as? TableNode)
}

private func cell(_ r: Int, _ c: Int) throws -> TableCellNode {
  let row = try #require(try table().getChildAtIndex(r) as? ElementNode)
  return try #require(try row.getChildAtIndex(c) as? TableCellNode)
}

private func selectCell(_ r: Int, _ c: Int) throws {
  try #require(try cell(r, c).getFirstChild() as? ElementNode).selectEnd()
}

/// Grid columns covered by each row, spans included.
private func widths() throws -> [Int] {
  try computeTableMapSkipCellCheck(table(), nil, nil).tableMap.map { row in
    row.filter { $0 != nil }.count
  }
}

/// Text of each row's cells, in document order.
private func texts() throws -> [[String]] {
  try table().getChildren().map { row in
    try #require(row as? TableRowNode).getChildren().map { try $0.getTextContent() }
  }
}

/// A command on the ragged table, and the table it leaves.
struct RaggedCommand: CustomTestStringConvertible, Sendable {
  let name: String
  let row: Int
  let column: Int
  let command: @Sendable () throws -> Void
  let widths: [Int]
  let texts: [[String]]?
  var testDescription: String { name }
}

@Suite struct RaggedTableCommandsTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert column after the last column of a full row"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert column after the last cell of the short row"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert row after the short row"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert row before the first row"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > delete the last column"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > delete the short row"
    ),
    arguments: [
      RaggedCommand(
        name: "insert column after the last column of a full row", row: 0, column: 2,
        command: { try insertTableColumnAtSelection(true) }, widths: [4, 4, 4],
        texts: [["00", "01", "02", ""], ["10", "11", "12", ""], ["20", "21", "", ""]]),
      RaggedCommand(
        name: "insert column after the last cell of the short row", row: 2, column: 1,
        command: { try insertTableColumnAtSelection(true) }, widths: [4, 4, 4],
        texts: [["00", "01", "", "02"], ["10", "11", "", "12"], ["20", "21", "", ""]]),
      RaggedCommand(
        name: "insert row after the short row", row: 2, column: 0,
        command: { try insertTableRowAtSelection(true) }, widths: [3, 3, 3, 3], texts: nil),
      RaggedCommand(
        name: "insert row before the first row", row: 0, column: 0,
        command: { try insertTableRowAtSelection(false) }, widths: [3, 3, 3, 3], texts: nil),
      RaggedCommand(
        name: "delete the last column", row: 0, column: 2,
        command: { try deleteTableColumnAtSelection() }, widths: [2, 2, 2],
        texts: [["00", "01"], ["10", "11"], ["20", "21"]]),
      RaggedCommand(
        name: "delete the short row", row: 2, column: 0,
        command: { try deleteTableRowAtSelection() }, widths: [3, 3],
        texts: [["00", "01", "02"], ["10", "11", "12"]]),
    ])
  func command(_ c: RaggedCommand) throws {
    let editor = try buildTableEditor(RAGGED)
    try editor.update(discrete: true) {
      try selectCell(c.row, c.column)
      try c.command()
    }
    #expect(try editor.read(widths) == c.widths)
    if let expected = c.texts {
      #expect(try editor.read(texts) == expected)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > merge a block that reaches past the end of the short row"
    ))
  func mergePastShortRow() throws {
    let editor = try buildTableEditor(RAGGED)
    try editor.update(discrete: true) { try mergeCells([cell(1, 1), cell(1, 2), cell(2, 1)]) }
    #expect(try editor.read(widths) == [3, 3, 3])
    let texts = try editor.read(texts)
    #expect(texts[0] == ["00", "01", "02"])
    #expect(texts[1][0] == "10")
    #expect(texts[1][1].contains(/11[\s\S]*12[\s\S]*21/))
    #expect(texts[2] == ["20"])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > merge cells in the last column"
    ))
  func mergeLastColumn() throws {
    let editor = try buildTableEditor(RAGGED)
    try editor.update(discrete: true) { try mergeCells([cell(1, 1), cell(1, 2)]) }
    #expect(try editor.read(widths) == [3, 3, 3])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > unmerge a header-column cell when another row is short"
    ))
  func unmergeHeaderColumnCell() throws {
    let editor = try buildTableEditor([
      [
        CellSpec(text: "00"), CellSpec(text: "01"),
        CellSpec(text: "M", rowSpan: 2, headerState: TableCellHeaderStates.COLUMN),
      ],
      [CellSpec(text: "10"), CellSpec(text: "11")],
      [CellSpec(text: "20")],
    ])
    try editor.update(discrete: true) {
      try selectCell(0, 2)
      try unmergeCell()
    }
    #expect(try editor.read(widths) == [3, 3, 3])
    #expect(try editor.read(texts) == [["00", "01", "M"], ["10", "11", ""], ["20", "", ""]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::pasting a table into a ragged table > paste a 2x2 table at the last cell of the short row"
    ))
  func pasteIntoShortRow() throws {
    let editor = try buildTableEditor(RAGGED)
    try editor.update(discrete: true) {
      let template = try createTableNode()
      for texts in [["a", "b"], ["c", "d"]] {
        let row = try createTableRowNode()
        for text in texts {
          try row.append(
            createTableCellNode().append(createParagraphNode().append(createTextNode(text))))
        }
        try template.append(row)
      }
      try selectCell(2, 1)
      guard let selection = try getSelection() as? RangeSelection else {
        throw LexicalError("Expected a RangeSelection")
      }
      _ = try insertTableIntoGrid(template, selection)
    }
    #expect(try editor.read(widths) == [3, 3, 3, 3])
    #expect(
      try editor.read(texts) == [
        ["00", "01", "02"],
        ["10", "11", "12"],
        ["20", "a", "b"],
        ["", "c", "d"],
      ])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::$padTableRows > pads short rows and leaves a rectangular table alone"
    ))
  func padsShortRows() throws {
    let editor = try buildTableEditor(RAGGED)
    var changed: Bool?
    try editor.update(discrete: true) { changed = try padTableRows(table()) }
    #expect(changed == true)
    #expect(try editor.read(widths) == [3, 3, 3])
    try editor.update(discrete: true) { changed = try padTableRows(table()) }
    #expect(changed == false)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::$padTableRows > fills a hole left of a rowSpan from an earlier row"
    ))
  func fillsHoleLeftOfRowSpan() throws {
    let editor = try buildTableEditor([
      [
        CellSpec(text: "00"), CellSpec(text: "01"), CellSpec(text: "02", rowSpan: 2),
        CellSpec(text: "03"),
      ],
      [CellSpec(text: "10")],
    ])
    try editor.update(discrete: true) { try padTableRows(table()) }
    #expect(try editor.read(widths) == [4, 4])
    #expect(try editor.read(texts) == [["00", "01", "02", "03"], ["10", "", ""]])
  }
}
