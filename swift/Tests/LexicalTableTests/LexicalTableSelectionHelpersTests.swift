import Lexical
import LexicalExtension
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts.
// The root elements are `TestRootElement`s. The four regression #8832
// cases dispatch a DOM `copy` event to the document, whose listener the
// views own.

private let TableTestThemeExtension = defineExtension(
  name: "TableTestThemeExtension", dependencies: [TableExtension],
  editorConfig: InitialEditorConfig(theme: ["tableScrollableWrapper": ""]))

/// Selects from the first cell to the second of a new 1x2 table with a
/// range selection, then dispatches `SELECTION_CHANGE_COMMAND`.
private func selectAcrossCellsOfNewTable(_ editor: Editor) throws {
  let table = try createTableNodeWithDimensions(1, 2)
  try getRoot().clear().append(table)
  let map = try computeTableMapSkipCellCheck(table, nil, nil).tableMap
  let selection = createRangeSelection()
  try selection.anchor.set(map[0][0]!.cell.getKey(), 0, .element)
  try selection.focus.set(map[0][1]!.cell.getKey(), 0, .element)
  try setSelection(selection)
  _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
}

private func createTableBelowParagraph() throws {
  let paragraph = try createParagraphNode().append(createTextNode("above"))
  try getRoot().clear().append(paragraph, createTableNodeWithDimensions(2, 2, false))
  try paragraph.selectEnd()
}

@Suite struct LexicalTableSelectionHelpersTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > table normalization keeps its priority order across root attachment"
    ))
  func normalizationKeepsPriorityOrder() throws {
    let editor = try buildEditorFromExtensions(TableTestThemeExtension)
    defer { editor.dispose() }
    let root = TestRootElement()
    var normalized = false
    editor.registerCommand(
      SELECTION_CHANGE_COMMAND,
      { _, _ in
        normalized = try getSelection() is TableSelection
        return true
      }, COMMAND_PRIORITY_HIGH)
    for _ in 0..<2 {
      try editor.setRootElement(root)
      try editor.update(discrete: true) {
        try selectAcrossCellsOfNewTable(editor)
        #expect(normalized)
      }
      try editor.setRootElement(nil)
      normalized = false
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > table selection handling follows the root lifecycle"
    ))
  func followsRootLifecycle() throws {
    let editor = try buildEditorFromExtensions(TableTestThemeExtension)
    defer { editor.dispose() }
    let firstRoot = TestRootElement()
    let secondRoot = TestRootElement()

    // Explicit dispatch also exercises the rootless path, where core does not
    // automatically notify range selection changes.
    func selectAcrossCells(_ mounted: Bool) throws {
      try editor.update(discrete: true) {
        try selectAcrossCellsOfNewTable(editor)
        #expect((try getSelection() is TableSelection) == mounted)
        #expect((try getSelection() is RangeSelection) == !mounted)
      }
    }

    try selectAcrossCells(false)
    try editor.setRootElement(firstRoot)
    try selectAcrossCells(true)
    try editor.setRootElement(nil)
    try selectAcrossCells(false)
    try editor.setRootElement(firstRoot)
    try selectAcrossCells(true)
    try editor.setRootElement(secondRoot)
    try selectAcrossCells(true)
    try editor.setRootElement(nil)
    try selectAcrossCells(false)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > regression #8670 > selection change ignores the table recorded on ArrowDown when it was removed"
    ))
  func ignoresRemovedArrowDownTable() throws {
    let editor = try buildEditorFromExtensions(TableTestThemeExtension)
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) { try createTableBelowParagraph() }

    // Pressing ArrowDown above the table records the table key to check
    // on the next selection change (the Firefox workaround for scrollable
    // tables, which TableExtension enables by default via
    // hasHorizontalScroll)
    _ = try editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, KeyboardEvent(key: "ArrowDown"))

    // The table is removed before the next selection change occurs
    try editor.update(discrete: true) {
      let root = try getRoot()
      try root.getLastChildOrThrow().remove()
      try root.selectEnd()
    }

    #expect(throws: Never.self) { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > regression #8670 > selection change self-heals observers for tables removed while the root element was detached"
    ))
  func selfHealsObservers() throws {
    let editor = try buildEditorFromExtensions(TableTestThemeExtension)
    defer { editor.dispose() }
    let container = TestRootElement()
    try editor.setRootElement(container)
    try editor.update(discrete: true) { try createTableBelowParagraph() }

    // Removing the table while the root element is detached skips
    // reconciliation, so the TableNode destroyed mutation never fires
    // and the observers registry keeps a stale entry
    try editor.setRootElement(nil)
    try editor.update(discrete: true) { try getRoot().getLastChildOrThrow().remove() }
    try editor.setRootElement(container)

    // Without self-healing every subsequent selection change throws
    #expect(throws: Never.self) {
      _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
      _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
    }

    // New tables still work after the registry healed itself
    try editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(2, 2, false))
    }
    #expect(throws: Never.self) { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > DELETE_LINE_COMMAND in table cells > DELETE_LINE_COMMAND propagates past the table handler in the first cell paragraph"
    ))
  func deleteLinePropagates() throws {
    let editor = try buildEditorFromExtensions(TableTestThemeExtension)
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(2, 2, false)
      try getRoot().clear().append(table)
      let firstRow = try #require(try table.getFirstChildOrThrow() as? TableRowNode)
      let firstCell = try #require(try firstRow.getFirstChildOrThrow() as? TableCellNode)
      let paragraph = try #require(try firstCell.getFirstChildOrThrow() as? ParagraphNode)
      try paragraph.clear().append(createTextNode("hello world"))
      try paragraph.selectEnd()
    }

    var propagated = false
    let unregister = editor.registerCommand(
      DELETE_LINE_COMMAND,
      { _, _ in
        propagated = true
        return true
      }, COMMAND_PRIORITY_EDITOR)

    _ = try editor.dispatchCommand(DELETE_LINE_COMMAND, true)
    #expect(propagated)

    propagated = false
    _ = try editor.dispatchCommand(DELETE_LINE_COMMAND, false)
    #expect(propagated)
    unregister()
  }
}
