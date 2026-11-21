import Lexical
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts.
// Upstream builds the editor from TableExtension only to register the table
// nodes; this registers them directly.

private struct DeleteResult: Equatable {
  var rootChildren: Int
  var selectionType: String
  var staleTableSelection: Bool
}

private enum DeleteKind { case row, column }

private func deleteWholeTable(_ kind: DeleteKind) throws -> DeleteResult {
  let editor = try createTestEditor(nodes: tableTestNodes)
  try editor.update(discrete: true) {
    let root = try getRoot().clear()
    try root.append(createParagraphNode().append(createTextNode("before")))
    try root.append(createTableNodeWithDimensions(2, 2, false))
    let table = try #require(try root.getChildAtIndex(1) as? TableNode, "expected a TableNode")
    let (map, _, _) = try computeTableMapSkipCellCheck(table, nil, nil)
    // Every cell selected, so deleting rows/columns empties the table.
    try setSelection(createTableSelectionFrom(table, map[0][0]!.cell, map[1][1]!.cell))
  }

  try editor.update(discrete: true) {
    switch kind {
    case .row: try deleteTableRowAtSelection()
    case .column: try deleteTableColumnAtSelection()
    }
  }

  return try editor.read {
    let selection = try getSelection()
    return DeleteResult(
      rootChildren: try getRoot().getChildrenSize(),
      selectionType: selection is RangeSelection
        ? "range" : selection is TableSelection ? "table" : String(describing: selection),
      // A TableSelection left behind by the removed table reports isValid()
      // false, because the cells it points at are gone.
      staleTableSelection: try (selection as? TableSelection)?.isValid() == false)
  }
}

@Suite struct DeleteAllRowsSelectionTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts::deleting every row of a table > leaves the selection outside the removed table"
    ))
  func deleteRows() throws {
    #expect(
      try deleteWholeTable(.row)
        == DeleteResult(rootChildren: 1, selectionType: "range", staleTableSelection: false))
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/DeleteAllRowsSelection.test.ts::deleting every row of a table > matches what deleting every column already does"
    ))
  func deleteColumns() throws {
    #expect(
      try deleteWholeTable(.column)
        == DeleteResult(rootChildren: 1, selectionType: "range", staleTableSelection: false))
  }
}
