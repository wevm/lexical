import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts.
// Upstream's repaint and sticky scrollbar cases read the reconciled DOM;
// here they read the table's `createDOM`, which is what the reconciler
// renders. `drag selection` dispatches a pointer event to the table's DOM,
// which the views handle.

private func makeEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "table-test", dependencies: [TableExtension],
      editorConfig: InitialEditorConfig(
        theme: ["tableScrollableWrapper": "table-scrollable-wrapper"],
        onError: { error, _ in throw error })))
}

private func output(_ editor: Editor) throws -> NamedSignals<TableConfig> {
  try getExtensionDependencyFromEditor(editor, TableExtension).output
}

private func firstTable() throws -> TableNode {
  try #require(try getRoot().getFirstChild() as? TableNode)
}

/// A table with one cell holding an empty paragraph, which is selected.
private func appendOneCellTable(selectParagraph: Bool) throws {
  let root = try getRoot().clear()
  let table = try createTableNode()
  let row = try createTableRowNode()
  let cell = try createTableCellNode()
  if selectParagraph {
    let paragraph = try createParagraphNode()
    try cell.append(paragraph)
    try row.append(cell)
    try table.append(row)
    try root.append(table)
    try paragraph.select()
  } else {
    try row.append(cell)
    try table.append(row)
    try root.append(table)
    try cell.select()
  }
}

/// Whether the cell of the one-cell table holds a table.
private func firstCellHoldsTable(_ editor: Editor) throws -> Bool {
  try editor.read(.latest) { () throws -> Bool in
    let row = try #require(try firstTable().getFirstChild() as? TableRowNode)
    let cell = try #require(try row.getFirstChild() as? TableCellNode)
    return try cell.getChildren().contains { $0 is TableNode }
  }
}

/// The rendered element of the root's table.
private func renderTable(_ editor: Editor) throws -> DOMElement {
  try editor.read {
    let table = try #require(try getRoot().getChildren().first { $0 is TableNode })
    return try table.createDOM(editor._config, editor)
  }
}

@Suite struct LexicalTableExtensionTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Creates a table with INSERT_TABLE_COMMAND"
    ))
  func insertTableCommand() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().selectEnd()
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND,
        InsertTableCommandPayload(columns: "3", rows: "2", includeHeaders: true))
    }
    try editor.read {
      let children = try getRoot().getChildren()
      #expect(children.map { $0.getType() } == ["paragraph", "table", "paragraph"])
      let table = try #require(children[1] as? TableNode)
      let rows = try table.getChildren()
      #expect(rows.count == 2)
      for row in rows {
        let row = try #require(row as? TableRowNode)
        let cells = try row.getChildren()
        #expect(cells.count == 3)
        #expect(cells.allSatisfy { $0 is TableCellNode })
      }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > repaints existing tables when hasHorizontalScroll toggles"
    ))
  func repaintsOnHorizontalScroll() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      try getRoot().selectEnd()
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "2", rows: "2"))
    }
    let hasHorizontalScroll = try output(editor).hasHorizontalScroll

    // Default config enables horizontal scroll: the table is wrapped in the
    // scrollable <div>.
    var dom = try renderTable(editor)
    #expect(dom.classList.contains("table-scrollable-wrapper"))
    #expect(dom.children.first?.nodeName == "TABLE")

    // Toggling the signal re-renders existing tables, removing the wrapper.
    hasHorizontalScroll.value = false
    drainMicrotasks()
    #expect(try isScrollableTablesActive(editor) == false)
    dom = try renderTable(editor)
    #expect(dom.nodeName == "TABLE")

    // And restored when re-enabled.
    hasHorizontalScroll.value = true
    drainMicrotasks()
    dom = try renderTable(editor)
    #expect(dom.classList.contains("table-scrollable-wrapper"))
    #expect(dom.children.first?.nodeName == "TABLE")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > renders and removes the sticky scrollbar DOM when hasStickyScrollbar toggles"
    ))
  func stickyScrollbar() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    let hasStickyScrollbar = try output(editor).hasStickyScrollbar
    hasStickyScrollbar.value = true
    drainMicrotasks()

    try editor.update(discrete: true) {
      try getRoot().clear().selectEnd()
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "2", rows: "2"))
    }

    // outer wrapper (marked with the data attribute) > scrollable wrapper > table
    var dom = try renderTable(editor)
    #expect(dom.hasAttribute("data-lexical-sticky-scrollbar"))
    let scrollable = try #require(dom.children.first)
    #expect(scrollable.classList.contains("table-scrollable-wrapper"))
    #expect(scrollable.children.first?.nodeName == "TABLE")
    // The scrollbar is the wrapper's next sibling, hidden from the
    // accessibility tree, with its spacer child.
    let scrollbar = try #require(dom.children.dropFirst().first)
    #expect(scrollbar.nodeName == "DIV")
    #expect(scrollbar.getAttribute("aria-hidden") == "true")
    #expect(scrollbar.children.first != nil)

    // Toggling off restores the plain scrollable wrapper DOM.
    hasStickyScrollbar.value = false
    drainMicrotasks()
    dom = try renderTable(editor)
    #expect(!dom.hasAttribute("data-lexical-sticky-scrollbar"))
    #expect(dom.classList.contains("table-scrollable-wrapper"))
    #expect(dom.children.first?.nodeName == "TABLE")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Prevents nested tables by default"
    ))
  func preventsNestedTables() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try appendOneCellTable(selectParagraph: false) }
    // Try to insert a table inside the cell
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "2", rows: "2"))
    }
    #expect(try firstCellHoldsTable(editor) == false)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Allows nested tables when hasNestedTables is true"
    ))
  func allowsNestedTables() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try output(editor).hasNestedTables.value = true
    try editor.update(discrete: true) { try appendOneCellTable(selectParagraph: false) }
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: "2", rows: "2"))
    }
    #expect(try firstCellHoldsTable(editor) == true)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler prevents pasting whole table into cells by default"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler allows pasting whole table into a single cell when hasNestedTables is true"
    ),
    arguments: [false, true])
  func pasteWholeTable(hasNestedTables: Bool) throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    if hasNestedTables {
      try output(editor).hasNestedTables.value = true
    }
    try editor.update(discrete: true) { try appendOneCellTable(selectParagraph: true) }
    // Whole table paste (as opposed to a table merge) is done by having
    // multiple nodes on the clipboard.
    try editor.update(discrete: true) {
      let tableNode = try createTableNode().append(
        createTableRowNode().append(createTableCellNode().append(createParagraphNode())))
      let selection = try #require(try getSelection() as? RangeSelection)
      try insertGeneratedNodes(editor, [tableNode, createParagraphNode()], selection)
    }
    #expect(try firstCellHoldsTable(editor) == hasNestedTables)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler allows extending table when hasNestedTables is true"
    ))
  func pasteExtendsTable() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try output(editor).hasNestedTables.value = true
    try editor.update(discrete: true) { try appendOneCellTable(selectParagraph: true) }
    // Try to paste a wider table inside the cell
    try editor.update(discrete: true) {
      let row = try createTableRowNode().append(
        createTableCellNode().append(createParagraphNode().append(createTextNode("a"))),
        createTableCellNode().append(createParagraphNode().append(createTextNode("b"))))
      let tableNode = try createTableNode().append(row)
      let selection = try #require(try getSelection())
      try insertGeneratedNodes(editor, [tableNode], selection)
    }
    try editor.read(.latest) {
      let row = try #require(try firstTable().getFirstChild() as? ElementNode)
      let cells = try row.getChildren()
      #expect(cells.count == 2)
      let cell = try #require(cells[0] as? TableCellNode)
      let cell2 = try #require(cells[1] as? TableCellNode)
      #expect(try #require(try cell.getFirstChild() as? ParagraphNode).getTextContent() == "a")
      #expect(try #require(try cell2.getFirstChild() as? ParagraphNode).getTextContent() == "b")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND clips to selection boundary with TableSelection"
    ))
  func pasteClipsToTableSelection() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try createTableNode()
      for r in 0..<2 {
        let row = try createTableRowNode()
        for c in 0..<2 {
          try row.append(
            createTableCellNode().append(createParagraphNode().append(createTextNode("\(c),\(r)"))))
        }
        try table.append(row)
      }
      try getRoot().clear().append(table)
      let (tableMap, _, _) = try computeTableMapSkipCellCheck(table, nil, nil)
      try setSelection(createTableSelectionFrom(table, tableMap[0][0]!.cell, tableMap[1][1]!.cell))
    }
    try editor.update(discrete: true) {
      let template = try createTableNode()
      let letters = Array("ABCDEFGHI")
      for r in 0..<3 {
        let row = try createTableRowNode()
        for c in 0..<3 {
          try row.append(
            createTableCellNode().append(
              createParagraphNode().append(createTextNode(String(letters[r * 3 + c])))))
        }
        try template.append(row)
      }
      let selection = try #require(try getSelection() as? TableSelection)
      try insertGeneratedNodes(editor, [template], selection)
    }
    try editor.read(.latest) {
      let rows = try firstTable().getChildren().compactMap { $0 as? TableRowNode }
      #expect(rows.count == 2)
      #expect(try rows[0].getChildren().count == 2)
      let texts = try rows.map { row in try row.getChildren().map { try $0.getTextContent() } }
      #expect(texts == [["A", "B"], ["D", "E"]])
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > pasting a table carries the cell backgroundColor and verticalAlign"
    ))
  func pasteCarriesCellStyle() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let cell = try createTableCellNode().append(
        createParagraphNode().append(createTextNode("old")))
      try getRoot().clear().append(createTableNode().append(createTableRowNode().append(cell)))
      try cell.selectStart()
    }
    try editor.update(discrete: true) {
      let cell = try createTableCellNode().setBackgroundColor("rgb(255, 0, 0)")
        .setVerticalAlign("middle")
      try cell.append(createParagraphNode().append(createTextNode("new")))
      let template = try createTableNode().append(createTableRowNode().append(cell))
      let selection = try #require(try getSelection(), "Expected a selection")
      try insertGeneratedNodes(editor, [template], selection)
    }
    try editor.read(.latest) {
      let row = try #require(try firstTable().getFirstChild() as? TableRowNode)
      let cell = try #require(try row.getFirstChild() as? TableCellNode)
      #expect(try cell.getTextContent() == "new")
      #expect(try cell.getBackgroundColor() == "rgb(255, 0, 0)")
      #expect(try cell.getVerticalAlign() == "middle")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $tableTransform padding > a short header row is padded with header cells"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $tableTransform padding > a short body row is padded with body cells"
    ),
    arguments: [true, false])
  func paddingKeepsRowKind(shortHeader: Bool) throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try createTableNode()
      let headerRow = try createTableRowNode()
      for text in shortHeader ? ["h"] : ["h1", "h2"] {
        try headerRow.append(
          createTableCellNode(TableCellHeaderStates.ROW).append(
            createParagraphNode().append(createTextNode(text))))
      }
      try table.append(headerRow)
      let bodyRow = try createTableRowNode()
      for text in shortHeader ? ["a", "b"] : ["a"] {
        try bodyRow.append(
          createTableCellNode().append(createParagraphNode().append(createTextNode(text))))
      }
      try table.append(bodyRow)
      try getRoot().clear().append(table)
    }
    let tags = try editor.read(.latest) { () throws -> [[String]] in
      try firstTable().getChildren().compactMap { $0 as? TableRowNode }.map { row in
        try row.getChildren().compactMap { $0 as? TableCellNode }.map { try $0.getTag() }
      }
    }
    #expect(tags == [["th", "th"], ["td", "td"]])
  }

  struct ColWidthsCase: CustomTestStringConvertible, Sendable {
    let columns: String
    let colWidths: [Double]
    let expected: [Double]?
    var testDescription: String { "\(columns) columns, \(colWidths)" }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > removes colWidths if it is an empty array"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > uses the last column width if the column count is greater than the number of column widths"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > shortens the colWidths if the column count is less than the number of column widths"
    ),
    arguments: [
      ColWidthsCase(columns: "2", colWidths: [], expected: nil),
      ColWidthsCase(columns: "3", colWidths: [10, 20], expected: [10, 20, 20]),
      ColWidthsCase(columns: "2", colWidths: [10, 20, 30], expected: [10, 20]),
    ])
  func colWidths(_ c: ColWidthsCase) throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().selectStart()
      _ = try editor.dispatchCommand(
        INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: c.columns, rows: "2"))
      try firstTable().setColWidths(c.colWidths)
    }
    #expect(try editor.read(.latest) { try firstTable().getColWidths() } == c.expected)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Selects all cells in table without merged cells when table is only content"
    ))
  func selectAllInOnlyTable() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(3, 3, false)
      try getRoot().clear().append(table)
      let firstRow = try #require(try table.getFirstChild() as? TableRowNode)
      let firstCell = try #require(try firstRow.getFirstChild() as? TableCellNode)
      try #require(try firstCell.getFirstChild() as? ParagraphNode).selectStart()
    }
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(SELECT_ALL_COMMAND, KeyboardEvent())
    }
    try editor.read(.latest) {
      let selection = try #require(
        try getSelection() as? TableSelection, "Expected TableSelection after Ctrl+A")
      let (tableMap, _, _) = try computeTableMapSkipCellCheck(firstTable(), nil, nil)
      let firstCellMap = try #require(tableMap[0][0])
      let lastCellMap = try #require(tableMap[tableMap.count - 1].last!)
      let anchorCell = try #require(try selection.anchor.getNode() as? TableCellNode)
      let focusCell = try #require(try selection.focus.getNode() as? TableCellNode)
      // Verify selection spans from first to last cell
      #expect(anchorCell.getKey() == firstCellMap.cell.getKey())
      #expect(focusCell.getKey() == lastCellMap.cell.getKey())
      // Verify all cells are in the selection
      let selectedNodes = try selection.getNodes().compactMap { $0 as? TableCellNode }
      let totalCells = tableMap.reduce(0) { acc, row in
        acc + Set(row.compactMap { $0?.cell.getKey() }).count
      }
      #expect(selectedNodes.count == totalCells)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Selects all cells in table with merged cells when table is only content"
    ))
  func selectAllInOnlyTableWithMergedCells() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // Create a 3x3 table
      let table = try createTableNodeWithDimensions(3, 3, false)
      try getRoot().clear().append(table)
      // Merge two cells in the last column (merge cells in row 0 and row 1, column 2)
      let row0 = try #require(try table.getChildAtIndex(0) as? TableRowNode, "Expected row 0")
      let row1 = try #require(try table.getChildAtIndex(1) as? TableRowNode, "Expected row 1")
      let cell0_2 = try #require(try row0.getChildAtIndex(2) as? TableCellNode, "Expected cell 0,2")
      let cell1_2 = try #require(try row1.getChildAtIndex(2) as? TableCellNode, "Expected cell 1,2")
      try mergeCells([cell0_2, cell1_2])
      // Place cursor in any cell (not the merged one)
      let cell0_0 = try #require(try row0.getChildAtIndex(0) as? TableCellNode, "Expected cell 0,0")
      try #require(try cell0_0.getFirstChild() as? ElementNode, "Expected paragraph in cell")
        .selectStart()
    }
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(SELECT_ALL_COMMAND, KeyboardEvent())
    }
    try editor.read(.latest) {
      let selection = try #require(
        try getSelection() as? TableSelection,
        "Expected TableSelection after Ctrl+A with merged cells")
      let (tableMap, _, _) = try computeTableMapSkipCellCheck(firstTable(), nil, nil)
      let firstCellMap = try #require(tableMap[0][0], "Expected first cell")
      let lastCellMap = try #require(tableMap[tableMap.count - 1].last!, "Expected last cell")
      let anchorCell = try #require(
        try selection.anchor.getNode() as? TableCellNode, "Expected anchor to be in a table cell")
      let focusCell = try #require(
        try selection.focus.getNode() as? TableCellNode, "Expected focus to be in a table cell")
      // Verify selection spans from first to last cell
      #expect(anchorCell.getKey() == firstCellMap.cell.getKey())
      #expect(focusCell.getKey() == lastCellMap.cell.getKey())
      // Verify all unique cells are in the selection (merged cells should appear once)
      let uniqueCellKeys = Set(
        try selection.getNodes().compactMap { $0 as? TableCellNode }.map { $0.getKey() })
      let totalUniqueCells = Set(tableMap.flatMap { row in row.compactMap { $0?.cell.getKey() } })
      #expect(uniqueCellKeys.count == totalUniqueCells.count)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Does not intercept SELECT_ALL_COMMAND when cursor is outside table"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Does not intercept SELECT_ALL_COMMAND when there is paragraph after table"
    ),
    arguments: [true, false])
  func selectAllNotIntercepted(cursorOutside: Bool) throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let table = try createTableNodeWithDimensions(2, 2, false)
      if cursorOutside {
        let text = try createTextNode("Some text")
        try root.append(createParagraphNode().append(text), table)
        try text.selectStart()
      } else {
        // A paragraph after the table, as insertTable leaves one
        try root.append(table, createParagraphNode())
        let firstRow = try #require(try table.getFirstChild() as? TableRowNode)
        let firstCell = try #require(try firstRow.getFirstChild() as? TableCellNode)
        try #require(try firstCell.getFirstChild() as? ElementNode).selectStart()
      }
    }
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(SELECT_ALL_COMMAND, KeyboardEvent())
    }
    try editor.read(.latest) {
      let selection = try getSelection()
      #expect(selection is RangeSelection)
      #expect(!isTableSelection(selection))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > triple click in a cell > selects the paragraph with text points, as a native selection of it resolves"
    ))
  func tripleClickInCell() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    var firstKey = ""
    var lastKey = ""
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(1, 2, false)
      try getRoot().clear().append(table)
      let row = try #require(try table.getFirstChild() as? TableRowNode)
      let cell = try #require(try row.getLastChild() as? TableCellNode)
      let paragraph = try #require(try cell.getFirstChild() as? ParagraphNode)
      let first = try createTextNode("Foo")
      let last = try createTextNode("Bar").toggleFormat("bold")
      try paragraph.append(first, last)
      firstKey = first.getKey()
      lastKey = last.getKey()
    }
    // The third click's click event, with detail 3, on the clicked text.
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(CLICK_COMMAND, MouseEvent(detail: 3, targetKey: lastKey))
    }
    try editor.read(.forceCommit) {
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == firstKey)
      #expect(selection.anchor.offset == 0)
      #expect(selection.anchor.type == .text)
      #expect(selection.focus.key == lastKey)
      #expect(selection.focus.offset == 3)
      #expect(selection.focus.type == .text)
    }
  }

  struct FormatDirection: CustomTestStringConvertible, Sendable {
    let anchor: (Int, Int)
    let focus: (Int, Int)
    let name: String
    var testDescription: String { name }
  }

  private func cell(_ table: TableNode, _ row: Int, _ column: Int) throws -> TableCellNode {
    let rowNode = try #require(try table.getChildAtIndex(row) as? TableRowNode)
    return try #require(try rowNode.getChildAtIndex(column) as? TableCellNode)
  }

  /// The `FORMAT_ELEMENT_COMMAND` editor: mounted, so the tables have
  /// observers, with a 3 × 3 table.
  private func formatEditor() throws -> Editor {
    let editor = try makeEditor()
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      try getRoot().clear().append(createTableNodeWithDimensions(3, 3, false))
    }
    return editor
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected top-left -> bottom-right"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected bottom-right -> top-left"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected top-right -> bottom-left"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected bottom-left -> top-right"
    ),
    arguments: [
      FormatDirection(anchor: (0, 0), focus: (2, 2), name: "top-left -> bottom-right"),
      FormatDirection(anchor: (2, 2), focus: (0, 0), name: "bottom-right -> top-left"),
      FormatDirection(anchor: (0, 2), focus: (2, 0), name: "top-right -> bottom-left"),
      FormatDirection(anchor: (2, 0), focus: (0, 2), name: "bottom-left -> top-right"),
    ])
  func alignsFullTable(_ d: FormatDirection) throws {
    let editor = try formatEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try firstTable()
      try setSelection(
        createTableSelectionFrom(
          table, cell(table, d.anchor.0, d.anchor.1), cell(table, d.focus.0, d.focus.1)))
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "center")
    }
    try editor.read {
      let table = try firstTable()
      #expect(try table.getFormatType() == "center")
      for row in 0..<3 {
        for column in 0..<3 {
          let paragraph = try #require(
            try cell(table, row, column).getFirstChild() as? ParagraphNode)
          #expect(try paragraph.getFormatType() == "")
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when the full selection includes a merged cell, selected in reverse"
    ))
  func alignsFullTableWithMergedCell() throws {
    let editor = try formatEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try firstTable()
      try mergeCells([cell(table, 0, 2), cell(table, 1, 2)])
    }
    try editor.update(discrete: true) {
      let table = try firstTable()
      try setSelection(createTableSelectionFrom(table, cell(table, 2, 2), cell(table, 0, 0)))
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "justify")
    }
    #expect(try editor.read { try firstTable().getFormatType() } == "justify")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > applies per-cell alignment (not table alignment) when only part of the table is selected, even in reverse direction"
    ))
  func alignsPartOfTable() throws {
    let editor = try formatEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let table = try firstTable()
      try setSelection(createTableSelectionFrom(table, cell(table, 1, 1), cell(table, 0, 0)))
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "right")
    }
    try editor.read {
      let table = try firstTable()
      #expect(try table.getFormatType() == "")
      for row in 0..<3 {
        for column in 0..<3 {
          let paragraph = try #require(
            try cell(table, row, column).getFirstChild() as? ParagraphNode)
          #expect(try paragraph.getFormatType() == (row <= 1 && column <= 1 ? "right" : ""))
        }
      }
    }
  }
}
