import Lexical
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts.
// Upstream builds the editor from TableExtension only to register the table
// nodes; these tests register them directly.

private func createTestTable(_ rows: Int, _ columns: Int) throws -> TableNode {
  let tableNode = try createTableNode()
  for r in 0..<rows {
    let row = try createTableRowNode()
    for c in 0..<columns {
      let cell = try createTableCellNode()
      try cell.append(createParagraphNode().append(createTextNode("r\(r)c\(c)")))
      try row.append(cell)
    }
    try tableNode.append(row)
  }
  return tableNode
}

private func tableCellTexts(_ tableNode: TableNode) throws -> [[String]] {
  try tableNode.getChildren().map { row in
    try #require(row as? TableRowNode).getChildren().map { cell in
      try #require(cell as? TableCellNode).getTextContent()
    }
  }
}

private func headerStates(_ table: TableNode, _ flag: TableCellHeaderState) throws -> [[Bool]] {
  try table.getChildren().map { row in
    try #require(row as? TableRowNode).getChildren().map { cell in
      try #require(cell as? TableCellNode).hasHeaderState(flag)
    }
  }
}

private func firstTable() throws -> TableNode {
  try #require(try getRoot().getFirstChild() as? TableNode)
}

/// The `beforeEach` editor, with an empty root.
private func makeEditor() throws -> Editor {
  let editor = try createTestEditor(nodes: tableTestNodes)
  try editor.update(discrete: true) { try getRoot().clear() }
  return editor
}

/// Builds the document, runs `action` on the table in a second update, and
/// reads the table back from the latest state.
private func runOnTable<T>(
  _ build: @escaping () throws -> Void, _ action: @escaping (TableNode) throws -> Void,
  _ result: (TableNode) throws -> T
) throws -> T {
  let editor = try makeEditor()
  try editor.update(discrete: true, build)
  try editor.update(discrete: true) { try action(firstTable()) }
  return try editor.read(.latest) { () throws -> T in try result(firstTable()) }
}

/// A row of 2 merged columns and a normal cell, over a row of 3 normal cells.
private func appendMergedTable() throws {
  let tableNode = try createTableNode()
  // Row 0: cell spanning 2 columns, then a normal cell
  let row0 = try createTableRowNode()
  let mergedCell = try createTableCellNode()
  try mergedCell.setColSpan(2)
  try mergedCell.append(createParagraphNode().append(createTextNode("merged")))
  let normalCell = try createTableCellNode()
  try normalCell.append(createParagraphNode().append(createTextNode("r0c2")))
  try row0.append(mergedCell, normalCell)
  // Row 1: 3 normal cells
  let row1 = try createTableRowNode()
  for c in 0..<3 {
    let cell = try createTableCellNode()
    try cell.append(createParagraphNode().append(createTextNode("r1c\(c)")))
    try row1.append(cell)
  }
  try tableNode.append(row0, row1)
  try getRoot().append(tableNode)
}

/// Should be unchanged because table has merged cells.
private func expectMergedTableUnchanged(_ table: TableNode) throws {
  let firstRow = try #require(try table.getChildren()[0] as? TableRowNode)
  let firstRowCells = try firstRow.getChildren()
  #expect(firstRowCells.count == 2)  // merged cell + normal cell
  let mergedCell = try #require(firstRowCells[0] as? TableCellNode)
  #expect(try mergedCell.getColSpan() == 2)
  #expect(try mergedCell.getTextContent() == "merged")
}

/// A move of a column or row in a `rows` × `columns` test table.
struct TableMove: CustomTestStringConvertible, Sendable {
  let rows: Int
  let columns: Int
  let origin: Int
  let target: Int
  let expected: [[String]]
  var testDescription: String { "\(origin) → \(target) in \(rows)×\(columns)" }
}

private let unchanged2x3 = [["r0c0", "r0c1", "r0c2"], ["r1c0", "r1c1", "r1c2"]]
private let unchanged3x2 = [["r0c0", "r0c1"], ["r1c0", "r1c1"], ["r2c0", "r2c1"]]

@Suite struct MoveTableColumnTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column forward"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column backward"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column to the first position"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column to the last position"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when origin equals target"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when origin is out of bounds"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when target is out of bounds"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when origin is negative"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > swaps adjacent columns"
    ),
    arguments: [
      TableMove(
        rows: 2, columns: 4, origin: 0, target: 2,
        expected: [["r0c1", "r0c2", "r0c0", "r0c3"], ["r1c1", "r1c2", "r1c0", "r1c3"]]),
      TableMove(
        rows: 2, columns: 4, origin: 3, target: 1,
        expected: [["r0c0", "r0c3", "r0c1", "r0c2"], ["r1c0", "r1c3", "r1c1", "r1c2"]]),
      TableMove(
        rows: 2, columns: 3, origin: 2, target: 0,
        expected: [["r0c2", "r0c0", "r0c1"], ["r1c2", "r1c0", "r1c1"]]),
      TableMove(
        rows: 2, columns: 3, origin: 0, target: 2,
        expected: [["r0c1", "r0c2", "r0c0"], ["r1c1", "r1c2", "r1c0"]]),
      TableMove(rows: 2, columns: 3, origin: 1, target: 1, expected: unchanged2x3),
      TableMove(rows: 2, columns: 3, origin: 5, target: 0, expected: unchanged2x3),
      TableMove(rows: 2, columns: 3, origin: 0, target: 10, expected: unchanged2x3),
      TableMove(rows: 2, columns: 3, origin: -1, target: 0, expected: unchanged2x3),
      TableMove(
        rows: 3, columns: 2, origin: 0, target: 1,
        expected: [["r0c1", "r0c0"], ["r1c1", "r1c0"], ["r2c1", "r2c0"]]),
    ])
  func moves(_ m: TableMove) throws {
    let texts = try runOnTable(
      { try getRoot().append(createTestTable(m.rows, m.columns)) },
      { try moveTableColumn($0, m.origin, m.target) },
      tableCellTexts)
    #expect(texts == m.expected)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > reorders colWidths when present"
    ))
  func reordersColWidths() throws {
    let colWidths = try runOnTable(
      {
        let table = try createTestTable(2, 4)
        try table.setColWidths([100, 200, 300, 400])
        try getRoot().append(table)
      },
      { try moveTableColumn($0, 0, 2) },
      { try $0.getColWidths() })
    #expect(colWidths == [200, 300, 100, 400])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > does not modify table with merged cells"
    ))
  func mergedCells() throws {
    try runOnTable(appendMergedTable, { try moveTableColumn($0, 0, 1) }, expectMergedTableUnchanged)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > preserves table structure after move"
    ))
  func preservesStructure() throws {
    let sizes = try runOnTable(
      { try getRoot().append(createTestTable(3, 4)) },
      { try moveTableColumn($0, 1, 3) },
      { try $0.getChildren().map { try #require($0 as? TableRowNode).getChildrenSize() } })
    // Verify row and column count is preserved
    #expect(sizes == [4, 4, 4])
  }
}

@Suite struct MoveTableRowTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row forward"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row backward"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row to the first position"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row to the last position"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when origin equals target"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when origin is out of bounds"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when target is out of bounds"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when origin is negative"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > swaps adjacent rows"
    ),
    arguments: [
      TableMove(
        rows: 4, columns: 2, origin: 0, target: 2,
        expected: [["r1c0", "r1c1"], ["r2c0", "r2c1"], ["r0c0", "r0c1"], ["r3c0", "r3c1"]]),
      TableMove(
        rows: 4, columns: 2, origin: 3, target: 1,
        expected: [["r0c0", "r0c1"], ["r3c0", "r3c1"], ["r1c0", "r1c1"], ["r2c0", "r2c1"]]),
      TableMove(
        rows: 4, columns: 2, origin: 2, target: 0,
        expected: [["r2c0", "r2c1"], ["r0c0", "r0c1"], ["r1c0", "r1c1"], ["r3c0", "r3c1"]]),
      TableMove(
        rows: 4, columns: 2, origin: 0, target: 3,
        expected: [["r1c0", "r1c1"], ["r2c0", "r2c1"], ["r3c0", "r3c1"], ["r0c0", "r0c1"]]),
      TableMove(rows: 3, columns: 2, origin: 1, target: 1, expected: unchanged3x2),
      TableMove(rows: 3, columns: 2, origin: 3, target: 0, expected: unchanged3x2),
      TableMove(rows: 3, columns: 2, origin: 0, target: 3, expected: unchanged3x2),
      TableMove(rows: 3, columns: 2, origin: -1, target: 1, expected: unchanged3x2),
      TableMove(
        rows: 2, columns: 3, origin: 0, target: 1,
        expected: [["r1c0", "r1c1", "r1c2"], ["r0c0", "r0c1", "r0c2"]]),
    ])
  func moves(_ m: TableMove) throws {
    let texts = try runOnTable(
      { try getRoot().append(createTestTable(m.rows, m.columns)) },
      { try moveTableRow($0, m.origin, m.target) },
      tableCellTexts)
    #expect(texts == m.expected)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > does not modify table with merged cells"
    ))
  func mergedCells() throws {
    try runOnTable(appendMergedTable, { try moveTableRow($0, 0, 1) }, expectMergedTableUnchanged)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves header cells along with the row"
    ))
  func movesHeaderCells() throws {
    try runOnTable(
      {
        let tableNode = try createTableNode()
        for r in 0..<3 {
          let row = try createTableRowNode()
          for c in 0..<2 {
            let cell = try createTableCellNode(
              r == 0 ? TableCellHeaderStates.ROW : TableCellHeaderStates.NO_STATUS)
            try cell.append(createParagraphNode().append(createTextNode("r\(r)c\(c)")))
            try row.append(cell)
          }
          try tableNode.append(row)
        }
        try getRoot().append(tableNode)
      },
      { try moveTableRow($0, 0, 2) },
      { table in
        #expect(
          try tableCellTexts(table) == [["r1c0", "r1c1"], ["r2c0", "r2c1"], ["r0c0", "r0c1"]])
        let movedRow = try #require(try table.getChildren()[2] as? TableRowNode)
        for cell in try movedRow.getChildren() {
          #expect(
            try #require(cell as? TableCellNode).getHeaderStyles() == TableCellHeaderStates.ROW)
        }
      })
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > preserves table structure after move"
    ))
  func preservesStructure() throws {
    let sizes = try runOnTable(
      { try getRoot().append(createTestTable(4, 3)) },
      { try moveTableRow($0, 1, 3) },
      { try $0.getChildren().map { try #require($0 as? TableRowNode).getChildrenSize() } })
    // Verify row and column count is preserved
    #expect(sizes == [3, 3, 3, 3])
  }
}

/// One row of cells with the given header states, each with an empty
/// paragraph.
private func appendHeaderRow(_ states: [TableCellHeaderState]) throws {
  let row = try createTableRowNode()
  for state in states {
    try row.append(createTableCellNode(state).append(createParagraphNode()))
  }
  try getRoot().append(createTableNode().append(row))
}

/// `[spanCell(rowSpan=2), cell01]` over `[cell11]`.
private func appendRowSpanTable() throws {
  let row0 = try createTableRowNode()
  let spanCell = try createTableCellNode()
  try spanCell.setRowSpan(2)
  try spanCell.append(createParagraphNode())
  try row0.append(spanCell, createTableCellNode().append(createParagraphNode()))
  let row1 = try createTableRowNode().append(createTableCellNode().append(createParagraphNode()))
  try getRoot().append(createTableNode().append(row0, row1))
}

@Suite struct SetTableRowIsHeaderTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > sets a row as header"
    ))
  func setsRow() throws {
    let states = try runOnTable(
      { try getRoot().append(createTestTable(3, 3)) },
      { try setTableRowIsHeader($0, 0, true) },
      { try headerStates($0, TableCellHeaderStates.ROW) })
    #expect(states == [[true, true, true], [false, false, false], [false, false, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > clears a header row"
    ))
  func clearsRow() throws {
    let states = try runOnTable(
      {
        let table = try createTableNode()
        let headerRow = try createTableRowNode()
        for c in 0..<3 {
          let cell = try createTableCellNode(TableCellHeaderStates.ROW)
          try cell.append(createParagraphNode().append(createTextNode("h\(c)")))
          try headerRow.append(cell)
        }
        try table.append(headerRow)
        let bodyRow = try createTableRowNode()
        for c in 0..<3 {
          let cell = try createTableCellNode()
          try cell.append(createParagraphNode().append(createTextNode("b\(c)")))
          try bodyRow.append(cell)
        }
        try table.append(bodyRow)
        try getRoot().append(table)
      },
      { try setTableRowIsHeader($0, 0, false) },
      { try headerStates($0, TableCellHeaderStates.ROW) })
    #expect(states == [[false, false, false], [false, false, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > preserves COLUMN bits when setting ROW header"
    ))
  func preservesColumnBits() throws {
    let (row, column) = try runOnTable(
      { try appendHeaderRow([TableCellHeaderStates.COLUMN, TableCellHeaderStates.NO_STATUS]) },
      { try setTableRowIsHeader($0, 0, true) },
      {
        (
          try headerStates($0, TableCellHeaderStates.ROW),
          try headerStates($0, TableCellHeaderStates.COLUMN)
        )
      })
    #expect(row == [[true, true]])
    #expect(column == [[true, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > handles colSpan cells"
    ))
  func colSpanCells() throws {
    let states = try runOnTable(
      {
        let row = try createTableRowNode()
        let spanCell = try createTableCellNode()
        try spanCell.setColSpan(2)
        try spanCell.append(createParagraphNode())
        try row.append(spanCell, createTableCellNode().append(createParagraphNode()))
        try getRoot().append(createTableNode().append(row))
      },
      { try setTableRowIsHeader($0, 0, true) },
      { try headerStates($0, TableCellHeaderStates.ROW) })
    #expect(states == [[true, true]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > handles rowSpan cells"
    ))
  func rowSpanCells() throws {
    let states = try runOnTable(
      appendRowSpanTable, { try setTableRowIsHeader($0, 0, true) },
      { try headerStates($0, TableCellHeaderStates.ROW) })
    #expect(states == [[true, true], [false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > sets a middle row as header"
    ))
  func setsMiddleRow() throws {
    let states = try runOnTable(
      { try getRoot().append(createTestTable(3, 3)) },
      { try setTableRowIsHeader($0, 1, true) },
      { try headerStates($0, TableCellHeaderStates.ROW) })
    #expect(states == [[false, false, false], [true, true, true], [false, false, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > throws on out-of-range row index"
    ))
  func throwsOutOfRange() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { try getRoot().append(createTestTable(3, 3)) }
    #expect(throws: (any Error).self) {
      try editor.update(discrete: true) { try setTableRowIsHeader(firstTable(), 5, true) }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > clears ROW from BOTH header state, preserving COLUMN"
    ))
  func clearsRowFromBoth() throws {
    let (row, column) = try runOnTable(
      { try appendHeaderRow([TableCellHeaderStates.BOTH]) },
      { try setTableRowIsHeader($0, 0, false) },
      {
        (
          try headerStates($0, TableCellHeaderStates.ROW),
          try headerStates($0, TableCellHeaderStates.COLUMN)
        )
      })
    #expect(row == [[false]])
    #expect(column == [[true]])
  }
}

@Suite struct SetTableColumnIsHeaderTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > sets a column as header"
    ))
  func setsColumn() throws {
    let states = try runOnTable(
      { try getRoot().append(createTestTable(3, 3)) },
      { try setTableColumnIsHeader($0, 0, true) },
      { try headerStates($0, TableCellHeaderStates.COLUMN) })
    #expect(states == [[true, false, false], [true, false, false], [true, false, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > clears a header column"
    ))
  func clearsColumn() throws {
    let states = try runOnTable(
      {
        let table = try createTableNode()
        for _ in 0..<3 {
          let row = try createTableRowNode()
          try row.append(
            createTableCellNode(TableCellHeaderStates.COLUMN).append(createParagraphNode()),
            createTableCellNode().append(createParagraphNode()))
          try table.append(row)
        }
        try getRoot().append(table)
      },
      { try setTableColumnIsHeader($0, 0, false) },
      { try headerStates($0, TableCellHeaderStates.COLUMN) })
    #expect(states == [[false, false], [false, false], [false, false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > preserves ROW bits when setting COLUMN header"
    ))
  func preservesRowBits() throws {
    let (column, row) = try runOnTable(
      { try appendHeaderRow([TableCellHeaderStates.ROW, TableCellHeaderStates.ROW]) },
      { try setTableColumnIsHeader($0, 0, true) },
      {
        (
          try headerStates($0, TableCellHeaderStates.COLUMN),
          try headerStates($0, TableCellHeaderStates.ROW)
        )
      })
    #expect(column == [[true, false]])
    #expect(row == [[true, true]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > handles rowSpan cells"
    ))
  func rowSpanCells() throws {
    let states = try runOnTable(
      appendRowSpanTable, { try setTableColumnIsHeader($0, 0, true) },
      { try headerStates($0, TableCellHeaderStates.COLUMN) })
    #expect(states == [[true, false], [false]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > throws on out-of-range column index"
    ))
  func throwsOutOfRange() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { try getRoot().append(createTestTable(3, 3)) }
    #expect(throws: (any Error).self) {
      try editor.update(discrete: true) { try setTableColumnIsHeader(firstTable(), 5, true) }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > clears COLUMN from BOTH header state, preserving ROW"
    ))
  func clearsColumnFromBoth() throws {
    let (column, row) = try runOnTable(
      { try appendHeaderRow([TableCellHeaderStates.BOTH]) },
      { try setTableColumnIsHeader($0, 0, false) },
      {
        (
          try headerStates($0, TableCellHeaderStates.COLUMN),
          try headerStates($0, TableCellHeaderStates.ROW)
        )
      })
    #expect(column == [[false]])
    #expect(row == [[true]])
  }
}

@Suite struct InsertTableColumnAtNodeTests {
  /// Renders the resolved grid (accounting for row/col spans) as a matrix of
  /// the text at each grid coordinate, so column alignment across rows is
  /// asserted directly rather than via raw DOM child order.
  private static func gridTexts(_ table: TableNode) throws -> [[String]] {
    let (tableMap, _, _) = try computeTableMapSkipCellCheck(table, nil, nil)
    return try tableMap.map { row in try row.map { try #require($0).cell.getTextContent() } }
  }

  private static func cell(
    _ text: String, _ rowSpan: Int = 1, _ colSpan: Int = 1
  ) throws
    -> TableCellNode
  {
    let cell = try createTableCellNode()
    try cell.setRowSpan(rowSpan)
    try cell.setColSpan(colSpan)
    return try cell.append(createParagraphNode().append(createTextNode(text)))
  }

  private static func appendTable(_ rows: [[TableCellNode]]) throws {
    let table = try createTableNode()
    for cells in rows {
      let row = try createTableRowNode()
      for cell in cells { try row.append(cell) }
      try table.append(row)
    }
    try getRoot().append(table)
  }

  /// Inserts a column after the cell that occupies the given grid coordinate.
  private static func insertColumnAfterGridCell(
    _ table: TableNode, _ row: Int, _ column: Int
  )
    throws
  {
    let (tableMap, _, _) = try computeTableMapSkipCellCheck(table, nil, nil)
    try insertTableColumnAtNode(try #require(tableMap[row][column]).cell, true, false)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > walks left by each visited cell colSpan when a row is spanned"
    ))
  func walksLeftByColSpan() throws {
    // Grid:
    //   row0: [A0][X(rowSpan=2)][C(colSpan=2,rowSpan=2)][D0]
    //   row1: [A1]                                      [D1]
    let texts = try runOnTable(
      {
        try Self.appendTable([
          [Self.cell("A0"), Self.cell("X", 2), Self.cell("C", 2, 2), Self.cell("D0")],
          [Self.cell("A1"), Self.cell("D1")],
        ])
      },
      // C occupies grid columns 2-3 of row 0; insert a column after it.
      { try Self.insertColumnAfterGridCell($0, 0, 2) },
      Self.gridTexts)
    #expect(texts == [["A0", "X", "C", "C", "", "D0"], ["A1", "X", "C", "C", "", "D1"]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > inserts the new cell in the correct column for rows spanned by a rowSpan cell"
    ))
  func rowSpannedRows() throws {
    // Grid:
    //   row0: [A(rowSpan=2), B]
    //   row1: [C]              (grid col 0 is covered by A's rowSpan)
    //   row2: [D, E]
    let texts = try runOnTable(
      {
        try Self.appendTable([
          [Self.cell("A", 2), Self.cell("B")], [Self.cell("C")], [Self.cell("D"), Self.cell("E")],
        ])
      },
      { try Self.insertColumnAfterGridCell($0, 0, 0) },
      Self.gridTexts)
    // The inserted (empty) column must line up at grid column 1 in every
    // row. Row 1 is entirely covered at column 0 by A's rowSpan, so the new
    // cell has to be prepended before C rather than appended after it.
    #expect(texts == [["A", "", "B"], ["A", "", "C"], ["D", "", "E"]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > does not prepend when a spanned row still owns a cell left of a colSpan > 1 anchor"
    ))
  func ownedCellLeftOfAnchor() throws {
    // Grid:
    //   row0: [P, A(rowSpan=2), C(rowSpan=2, colSpan=2)]  cols P=0 A=1 C=2-3
    //   row1: [B]                                         cols 1-3 are covered
    let texts = try runOnTable(
      {
        try Self.appendTable([
          [Self.cell("P"), Self.cell("A", 2), Self.cell("C", 2, 2)], [Self.cell("B")],
        ])
      },
      // Insert after C, i.e. after grid column 3.
      { try Self.insertColumnAfterGridCell($0, 0, 3) },
      Self.gridTexts)
    // Row 1 owns B at column 0, so the new cell belongs after B, not before.
    #expect(texts == [["P", "A", "C", "C", ""], ["B", "A", "C", "C", ""]])
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > inserts after the last owned cell of a spanned row, not an earlier one"
    ))
  func lastOwnedCell() throws {
    // Grid:
    //   row0: [A, B, V(rowSpan=2), X(rowSpan=2, colSpan=2)]  cols V=2 X=3-4
    //   row1: [C0, C1]                                       cols 2-4 covered
    let texts = try runOnTable(
      {
        try Self.appendTable([
          [Self.cell("A"), Self.cell("B"), Self.cell("V", 2), Self.cell("X", 2, 2)],
          [Self.cell("C0"), Self.cell("C1")],
        ])
      },
      // Insert after X, i.e. after grid column 4.
      { try Self.insertColumnAfterGridCell($0, 0, 4) },
      Self.gridTexts)
    // C1 is the last cell row 1 owns before the insertion column, so the new
    // cell goes after C1 rather than after C0.
    #expect(texts == [["A", "B", "V", "X", "X", ""], ["C0", "C1", "V", "X", "X", ""]])
  }
}

@Suite struct UnmergeCellNodeTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$unmergeCellNode > restores cells to the correct rows below the first row (#4876)"
    ))
  func restoresRowsBelowTheFirst() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { try getRoot().append(createTestTable(4, 4)) }

    try editor.update(discrete: true) {
      let cells = try firstTable().getChildren().dropFirst().flatMap { row in
        try #require(row as? TableRowNode).getChildren().prefix(2).map { cell in
          try #require(cell as? TableCellNode)
        }
      }
      try mergeCells(cells)
    }

    try editor.update(discrete: true) {
      let row = try #require(try firstTable().getChildAtIndex(1) as? TableRowNode)
      let merged = try #require(try row.getFirstChild() as? TableCellNode)
      #expect(try merged.getColSpan() == 2)
      #expect(try merged.getRowSpan() == 3)
      try unmergeCellNode(merged)
    }

    try editor.read {
      let table = try firstTable()
      #expect(
        try tableCellTexts(table) == [
          ["r0c0", "r0c1", "r0c2", "r0c3"],
          ["r1c0\n\nr1c1\n\nr2c0\n\nr2c1\n\nr3c0\n\nr3c1", "", "r1c2", "r1c3"],
          ["", "", "r2c2", "r2c3"],
          ["", "", "r3c2", "r3c3"],
        ])
      for row in try table.getChildren() {
        for cell in try #require(row as? TableRowNode).getChildren() {
          let tableCell = try #require(cell as? TableCellNode)
          #expect(try tableCell.getColSpan() == 1)
          #expect(try tableCell.getRowSpan() == 1)
        }
      }
    }
  }
}

@Suite struct MergeCellsTests {
  /// An empty string gives the cell a single empty paragraph, which is what
  /// `mergeCells` treats as having no content.
  static func cell(_ text: String) throws -> TableCellNode {
    let paragraph = try createParagraphNode()
    if text != "" {
      try paragraph.append(createTextNode(text))
    }
    return try createTableCellNode().append(paragraph)
  }

  static func appendTable(_ rows: [[TableCellNode]]) throws {
    let table = try createTableNode()
    for cells in rows {
      try table.append(createTableRowNode().append(contentsOf: cells))
    }
    try getRoot().append(table)
  }

  static func allCells() throws -> [TableCellNode] {
    try firstTable().getChildren().flatMap { row in
      try #require(row as? TableRowNode).getChildren().map { try #require($0 as? TableCellNode) }
    }
  }

  static func mergedCell() throws -> TableCellNode {
    let row = try #require(try firstTable().getFirstChild() as? TableRowNode)
    return try #require(try row.getFirstChild() as? TableCellNode)
  }

  static func blockTexts(_ cell: TableCellNode) throws -> [String] {
    try cell.getChildren().map { try $0.getTextContent() }
  }

  /// Builds the table, merges all of its cells, and reads the merged cell's
  /// block texts and spans from the latest state.
  static func merge(
    _ rows: @escaping () throws -> [[TableCellNode]]
  ) throws -> (
    [String], Int, Int
  ) {
    let editor = try makeEditor()
    try editor.update(discrete: true) { try appendTable(rows()) }
    try editor.update(discrete: true) { try mergeCells(allCells()) }
    return try editor.read(.latest) {
      let merged = try mergedCell()
      return (try blockTexts(merged), try merged.getColSpan(), try merged.getRowSpan())
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > drops the empty paragraph of the target cell"
    ))
  func dropsEmptyTargetParagraph() throws {
    let (texts, colSpan, rowSpan) = try Self.merge {
      [[try Self.cell(""), try Self.cell("hello")]]
    }
    #expect(texts == ["hello"])
    #expect(colSpan == 2)
    #expect(rowSpan == 1)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps every cell's content in order, and the spans"
    ))
  func keepsContentInOrder() throws {
    let (texts, colSpan, rowSpan) = try Self.merge {
      [[try Self.cell(""), try Self.cell("b")], [try Self.cell("c"), try Self.cell("d")]]
    }
    #expect(texts == ["b", "c", "d"])
    #expect(colSpan == 2)
    #expect(rowSpan == 2)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps the same empty paragraph when every cell is empty"
    ))
  func keepsTheEmptyParagraph() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) {
      try Self.appendTable([[try Self.cell(""), try Self.cell(""), try Self.cell("")]])
    }
    let paragraphKey = try editor.read(.latest) {
      try Self.mergedCell().getFirstChildOrThrow().getKey()
    }
    try editor.update(discrete: true) { try mergeCells(Self.allCells()) }
    try editor.read(.latest) {
      let merged = try Self.mergedCell()
      #expect(try merged.getChildrenSize() == 1)
      let firstChild = try merged.getFirstChildOrThrow()
      #expect(try (firstChild as? ParagraphNode)?.isEmpty() == true)
      // Nothing clears the target unless content is about to land, so this is
      // the paragraph the cell started with rather than a replacement.
      #expect(firstChild.getKey() == paragraphKey)
      #expect(try merged.getColSpan() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > leaves the target alone when only it has content"
    ))
  func leavesTargetAlone() throws {
    let (texts, colSpan, _) = try Self.merge {
      [[try Self.cell("keep"), try Self.cell(""), try Self.cell("")]]
    }
    #expect(texts == ["keep"])
    #expect(colSpan == 3)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps the content of a non empty target ahead of the rest"
    ))
  func keepsNonEmptyTargetFirst() throws {
    let (texts, colSpan, _) = try Self.merge {
      [[try Self.cell("one"), try Self.cell("two"), try Self.cell("three")]]
    }
    #expect(texts == ["one", "two", "three"])
    #expect(colSpan == 3)
  }
}
