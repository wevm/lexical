import Lexical
import LexicalExtension
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts.

private let BOTH = TableCellHeaderStates.BOTH
private let COLUMN = TableCellHeaderStates.COLUMN
private let NO_STATUS = TableCellHeaderStates.NO_STATUS
private let ROW = TableCellHeaderStates.ROW

private func toggle(
  _ from: TableCellHeaderState, _ toToggle: TableCellHeaderState
) throws -> TableCellHeaderState {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "toggle-host", dependencies: [TableExtension],
      editorConfig: InitialEditorConfig(
        theme: ["tableScrollableWrapper": "table-scrollable-wrapper"])))
  defer { editor.dispose() }
  var result = NO_STATUS
  try editor.update(discrete: true) {
    let cell = try createTableCellNode(from)
    result = try cell.toggleHeaderStyle(toToggle).getHeaderStyles()
  }
  return result
}

@Suite struct ToggleHeaderStyleTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts::TableCellNode.toggleHeaderStyle > stays inside the enum when toggling BOTH"
    ))
  func togglingBoth() throws {
    // Toggling a state the cell does not already have in full adds it; a cell
    // that has all of it loses it.
    #expect(try toggle(ROW, BOTH) == BOTH)
    #expect(try toggle(COLUMN, BOTH) == BOTH)
    #expect(try toggle(NO_STATUS, BOTH) == BOTH)
    #expect(try toggle(BOTH, BOTH) == NO_STATUS)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/ToggleHeaderStyle.test.ts::TableCellNode.toggleHeaderStyle > single-bit toggles are unchanged"
    ))
  func singleBitToggles() throws {
    #expect(try toggle(NO_STATUS, ROW) == ROW)
    #expect(try toggle(ROW, ROW) == NO_STATUS)
    #expect(try toggle(COLUMN, ROW) == BOTH)
    #expect(try toggle(BOTH, ROW) == COLUMN)
    #expect(try toggle(NO_STATUS, COLUMN) == COLUMN)
    #expect(try toggle(COLUMN, COLUMN) == NO_STATUS)
    #expect(try toggle(ROW, COLUMN) == BOTH)
    #expect(try toggle(BOTH, COLUMN) == ROW)
  }
}
