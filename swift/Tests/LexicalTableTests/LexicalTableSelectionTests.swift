import Lexical
import LexicalSelection
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx.

/// The `beforeEach` fixture: a 2 × 2 table of `col,row` texts, with a
/// `TableSelection` of all of it.
private struct Fixture {
  let editor: Editor
  var tableNode: TableNode!
  var tableMap: TableMapType = []
  var tableSelection: TableSelection!

  init() throws {
    editor = try createMountedTestEditor(nodes: tableTestNodes)
    var tableNode: TableNode!
    var tableMap: TableMapType = []
    var tableSelection: TableSelection!
    try update(editor) {
      tableNode = try createTableNode()
      for row in 0..<2 {
        let rowNode = try createTableRowNode()
        for col in 0..<2 {
          try rowNode.append(
            createTableCellNode().append(
              createParagraphNode().append(createTextNode("\(col),\(row)"))))
        }
        try tableNode.append(rowNode)
      }
      try getRoot().clear().append(tableNode)
      tableMap = try computeTableMapSkipCellCheck(tableNode, nil, nil).tableMap
      tableSelection = try createTableSelectionFrom(
        tableNode, tableMap[0][0]!.cell, tableMap[1][1]!.cell)
      try setSelection(tableSelection)
    }
    self.tableNode = tableNode
    self.tableMap = tableMap
    self.tableSelection = tableSelection
  }

  func cellTexts() throws -> [[String]] {
    try tableNode.getChildren().compactMap { $0 as? TableRowNode }.map { row in
      try row.getChildren().map { try $0.getTextContent() }
    }
  }
}

@Suite struct LexicalTableSelectionTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7076 > $patchStyleText works on a TableSelection"
    ))
  func patchStyleText() throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      #expect(
        try getRoot().getAllTextNodes().map { try $0.getStyle() } == Array(repeating: "", count: 4))
      #expect(isTableSelection(try getSelection()))
      try LexicalSelection.patchStyleText(try #require(try getSelection()), ["color": "red"])
      #expect(isTableSelection(try getSelection()))
      #expect(
        try getRoot().getAllTextNodes().map { try $0.getStyle() }
          == Array(repeating: "color: red;", count: 4))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7076 > $patchStyleText applies styles to empty table cells"
    ))
  func patchStyleTextEmptyCells() throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      let selection = try #require(try getSelection())
      #expect(isTableSelection(selection))
      let emptyCell = f.tableMap[0][0]!.cell
      let emptyParagraph = try #require(emptyCell.getFirstChild() as? ParagraphNode)
      try emptyParagraph.clear()
      #expect(try emptyParagraph.getChildrenSize() == 0)

      try LexicalSelection.patchStyleText(selection, ["color": "blue"])

      let filledCell = f.tableMap[0][1]!.cell
      let filledParagraph = try #require(filledCell.getFirstChild() as? ParagraphNode)
      let textNode = try #require(filledParagraph.getFirstChild() as? TextNode)
      #expect(try textNode.getStyle() == "color: blue;")
      #expect(try emptyParagraph.getTextStyle() == "color: blue;")
    }
  }

  /// The `insertRawText` cases on the fixture's whole-table selection.
  struct RawText: CustomTestStringConvertible {
    let text: String
    let expected: [[String]]
    var testDescription: String { text.debugDescription }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > fills 2x2 table with matching TSV"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > single value fills anchor cell only"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > single row TSV fills one row with two columns"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands rows when TSV has more rows"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands columns when TSV has more columns"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands both rows and columns"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > strips trailing newline from clipboard"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > empty string is no-op"
    ),
    arguments: [
      RawText(text: "A\tB\nC\tD", expected: [["A", "B"], ["C", "D"]]),
      RawText(text: "hello", expected: [["hello", "1,0"], ["0,1", "1,1"]]),
      RawText(text: "a\tb", expected: [["a", "b"], ["0,1", "1,1"]]),
      RawText(text: "A\tB\nC\tD\nE\tF", expected: [["A", "B"], ["C", "D"], ["E", "F"]]),
      RawText(text: "A\tB\tC\nD\tE\tF", expected: [["A", "B", "C"], ["D", "E", "F"]]),
      RawText(
        text: "A\tB\tC\nD\tE\tF\nG\tH\tI",
        expected: [["A", "B", "C"], ["D", "E", "F"], ["G", "H", "I"]]),
      RawText(text: "X\tY\nZ\tW\n", expected: [["X", "Y"], ["Z", "W"]]),
      RawText(text: "", expected: [["0,0", "1,0"], ["0,1", "1,1"]]),
    ])
  func insertRawText(_ c: RawText) throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      try f.tableSelection.insertRawText(c.text)
      #expect(try f.cellTexts() == c.expected)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > unmerges and fills merged cells"
    ))
  func insertRawTextUnmerges() throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      // Build a 3x3 table where cell (0,0) spans 2 cols
      let merged = try createTableNode()
      let topLeft = try createTableCellNode().setColSpan(2)
      try topLeft.append(createParagraphNode().append(createTextNode("M")))
      try merged.append(
        createTableRowNode().append(
          topLeft, createTableCellNode().append(createParagraphNode().append(createTextNode("c")))),
        createTableRowNode().append(
          contentsOf: (0..<3).map { _ in
            try createTableCellNode().append(createParagraphNode().append(createTextNode("x")))
          }))
      try getRoot().clear().append(merged)
      let mergedMap = try computeTableMapSkipCellCheck(merged, nil, nil).tableMap
      let sel = try createTableSelectionFrom(
        merged, mergedMap.first!.first!!.cell, mergedMap.last!.last!!.cell)
      try sel.insertRawText("A\tB\tC\nD\tE\tF")
      let texts = try merged.getChildren().compactMap { $0 as? TableRowNode }.map {
        try $0.getChildren().map { try $0.getTextContent() }
      }
      // Merged cell is unmerged, each cell gets its TSV value
      #expect(texts[0] == ["A", "B", "C"])
      #expect(texts[1] == ["D", "E", "F"])
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > paste from non-origin anchor fills offset cells"
    ))
  func insertRawTextFromOffset() throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      let offsetSelection = try createTableSelectionFrom(
        f.tableNode, f.tableMap[1][1]!.cell, f.tableMap[1][1]!.cell)
      try offsetSelection.insertRawText("X\tY\nZ\tW")
      let texts = try f.cellTexts()
      // Original cells preserved, offset filled + expanded
      #expect(texts[0][0] == "0,0")
      #expect(texts[0][1] == "1,0")
      #expect(texts[1][1] == "X")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > getShape > returns correct shape for non-merged table"
    ))
  func getShape() throws {
    let f = try Fixture()
    try f.editor.update(discrete: true) {
      let shape = try f.tableSelection.getShape()
      #expect(shape.fromX == 0)
      #expect(shape.fromY == 0)
      #expect(shape.toX == 1)
      #expect(shape.toY == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7140 > selection points to missing nodes after deleting table rows"
    ))
  func selectionAfterDeletingRows() throws {
    let f = try Fixture()
    try update(f.editor) {
      let root = try getRoot()
      try root.clear()
      try root.append(createParagraphNode().append(createTextNode("Before table")))
      try root.append(f.tableNode)
      let newTableMap = try computeTableMapSkipCellCheck(f.tableNode, nil, nil).tableMap
      try setSelection(
        createTableSelectionFrom(f.tableNode, newTableMap[0][0]!.cell, newTableMap[1][1]!.cell))
    }
    // Delete table rows
    try update(f.editor) {
      #expect(isTableSelection(try getSelection()))
      try deleteTableRowAtSelection()
    }
    // Try to access the selection after deletion
    try update(f.editor) {
      if let selection = try getSelection() as? TableSelection {
        #expect(try selection.isValid() == false)
        #expect(try selection.getNodes().isEmpty)
      }
    }
  }

  private func deleteForward() throws {
    let selection = try #require(try getSelection() as? RangeSelection)
    try selection.deleteCharacter(false)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete removes an empty paragraph before a table"
    ))
  func forwardDeleteRemovesEmptyParagraph() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      try getRoot().clear().append(paragraph, createTableNodeWithDimensions(2, 2, false))
      try paragraph.selectStart()
    }
    try editor.update(discrete: true) { try deleteForward() }
    try editor.read {
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(isTableNode(children.first))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete does not merge a table into a non-empty paragraph"
    ))
  func forwardDeleteKeepsTable() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("before"))
      try getRoot().clear().append(paragraph, createTableNodeWithDimensions(2, 2, false))
      try paragraph.selectEnd()
    }
    try editor.update(discrete: true) { try deleteForward() }
    try editor.read {
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      #expect(try children[0].getTextContent() == "before")
      #expect(isTableNode(children[1]))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete from an empty paragraph in a table cell does nothing"
    ))
  func forwardDeleteInEmptyCell() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(1, 2, false)
      try getRoot().clear().append(table)
      let row = try #require(table.getFirstChild() as? TableRowNode)
      let cell = try #require(row.getFirstChild() as? TableCellNode)
      let paragraph = try createParagraphNode()
      try cell.clear().append(paragraph)
      try paragraph.selectStart()
    }
    try editor.update(discrete: true) { try deleteForward() }
    try editor.read {
      let table = try #require(getRoot().getFirstChild() as? TableNode)
      let row = try #require(table.getFirstChild() as? TableRowNode)
      // The next cell was not pulled into the empty one.
      #expect(try row.getChildrenSize() == 2)
      #expect(try row.getFirstChildOrThrow().getTextContent() == "")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::repeated select-all inside a table cell > stays scoped to the cell and leaves the table intact"
    ))
  func repeatedSelectAll() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(2, 2, false)
      try getRoot().clear().append(createParagraphNode().append(createTextNode("before")), table)
      let row = try #require(table.getFirstChild() as? TableRowNode)
      let cell = try #require(row.getFirstChild() as? TableCellNode)
      let paragraph = try #require(cell.getFirstChild() as? ParagraphNode)
      try paragraph.append(createTextNode("a1"))
      try paragraph.selectEnd()
    }
    // All in one update: no DOM round-trip renormalizes the points in
    // between.
    try editor.update(discrete: true) {
      var selection = try #require(try getSelection() as? RangeSelection)
      for _ in 0..<3 {
        selection = try selectAll(selection)
        #expect(selection.anchor.type == .text)
        #expect(selection.focus.type == .text)
      }
      try selection.removeText()
    }
    try editor.read {
      #expect(try getRoot().getChildrenSize() == 2)
      let table = try #require(getRoot().getLastChild() as? TableNode)
      // Every row and cell survives, and no ParagraphNode was appended
      // directly into the table or a row.
      let texts = try table.getChildren().map { row in
        try #require(row as? TableRowNode).getChildren().map { cell in
          try #require(cell as? TableCellNode).getTextContent()
        }
      }
      #expect(texts == [["", ""], ["", ""]])
    }
  }
}
