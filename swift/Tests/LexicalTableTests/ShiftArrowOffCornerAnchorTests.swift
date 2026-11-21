import Lexical
import LexicalExtension
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/ShiftArrowOffCornerAnchor.test.ts.

private func table() throws -> TableNode {
  try #require(try getRoot().getFirstChild() as? TableNode, "expected a TableNode at the root")
}

private struct SelectionRect: Equatable {
  let anchor: String
  let cellCount: Int
  let focus: String
  let isValid: Bool
}

private func selectionRect() throws -> SelectionRect {
  let selection = try #require(try getSelection() as? TableSelection, "expected a TableSelection")
  let map = try computeTableMapSkipCellCheck(table(), nil, nil).tableMap
  func startOf(_ key: NodeKey) -> String {
    for row in map {
      for case let cellMap? in row where cellMap.cell.getKey() == key {
        return "\(cellMap.startRow),\(cellMap.startColumn)"
      }
    }
    return "\(key) is not in the table"
  }
  return SelectionRect(
    anchor: startOf(selection.anchor.key),
    cellCount: try selection.getNodes().filter { $0 is TableCellNode }.count,
    focus: startOf(selection.focus.key),
    isValid: try selection.isValid())
}

@Suite struct ShiftArrowOffCornerAnchorTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/ShiftArrowOffCornerAnchor.test.ts::Shift+Arrow with an anchor that is not on a rect corner > extends the selection instead of throwing"
    ))
  func extendsInsteadOfThrowing() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "shift-arrow-corner-host",
        dependencies: [TableExtension],
        editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": ""])))
    defer { editor.dispose() }
    // The arrow key handlers come from the table selection observer, which needs
    // the editor to have a root element.
    try editor.setRootElement(TestRootElement())

    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(3, 3, false)
      try getRoot().clear().append(table)
      let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
      // Merge grid columns 0 and 1 of row 1. This cell straddles the left
      // edge of the rect selected below, so $computeTableCellRectBoundary
      // grows the rect out to column 0 — past the anchor, which then sits on
      // no corner of it.
      let merged = try mergeCells([map[1][0]!.cell, map[1][1]!.cell])
      #expect(merged != nil, "expected the cells to merge")
    }

    try editor.update(discrete: true) {
      let table = try table()
      let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
      // Anchor at (row 0, column 1), focus at (row 1, column 2).
      try setSelection(createTableSelectionFrom(table, map[0][1]!.cell, map[1][2]!.cell))
    }

    // Not throwing: a throw fails the test.
    try editor.update(discrete: true) {
      _ = try editor.dispatchCommand(
        KEY_ARROW_DOWN_COMMAND, KeyboardEvent("keydown", key: "ArrowDown", shiftKey: true))
    }

    let rect = try editor.read(.latest) { try selectionRect() }
    // The anchor was not on a corner, so getAnchorCorner falls back to the
    // corner opposite the focus — top-left — and ArrowDown grows the rect
    // down to the last row. The merged cell keeps the rect out at column 0,
    // so the result is the whole table: 9 grid positions, 8 distinct cells.
    #expect(rect == SelectionRect(anchor: "0,0", cellCount: 8, focus: "2,2", isValid: true))
  }
}
