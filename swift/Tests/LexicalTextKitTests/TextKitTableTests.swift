import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalTable
import LexicalTesting
import LexicalTextKit
import Testing

// Tables in the TextKit views (DESIGN.md, The table view): the root's
// document shows a table as one attachment character, and each cell's text
// view has a controller of its own over the same editor, rendering the
// cell's subtree.

private struct Fixture {
  let editor: Editor
  let root: TextKitInputController
  /// The cell keys, by row.
  let cells: [[NodeKey]]
  let table: NodeKey
}

/// "before", a 2 × 2 table whose cells read "a1", "b1", "a2", "b2" (the
/// letter is the column), then "after".
private func makeFixture() throws -> Fixture {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        RichTextExtension, configExtension(HistoryExtension) { $0.now = { 0 } },
        configExtension(TableExtension) { $0.hasHorizontalScroll = false },
      ]))
  let root = TextKitInputController(editor: editor)
  // The table observers run while the editor has a root element, which a
  // view is.
  try editor.setRootElement(TestRootElement())
  var cells: [[NodeKey]] = []
  var tableKey: NodeKey = ""
  try editor.update(discrete: true) {
    let table = try createTableNodeWithDimensions(2, 2, false)
    for (r, row) in try table.getChildren().enumerated() {
      var keys: [NodeKey] = []
      for (c, cell) in try (row as! ElementNode).getChildren().enumerated() {
        let paragraph = try (cell as! ElementNode).getFirstChild() as! ElementNode
        try paragraph.append(createTextNode(["a", "b"][c] + String(r + 1)))
        keys.append(cell.getKey())
      }
      cells.append(keys)
    }
    tableKey = table.getKey()
    try getRoot().clear()
    try getRoot().append(
      createParagraphNode().append(createTextNode("before")), table,
      createParagraphNode().append(createTextNode("after")))
  }
  return Fixture(editor: editor, root: root, cells: cells, table: tableKey)
}

@Suite struct TextKitTableTests {
  @Test func rootDocumentShowsATableAsOneCharacter() throws {
    let f = try makeFixture()
    #expect(f.root.document.string == "before\n\u{FFFC}\nafter")
    #expect(f.root.document.span(f.table).map { [$0.location, $0.length] } == [7, 1])
    #expect(f.root.document.span(f.cells[0][0]) == nil)
  }

  @Test func aTableComesWithItsAttachmentSpan() throws {
    let f = try makeFixture()
    var spans: [TextKitDecoratorSpan] = []
    f.root.onRender = { spans += $0.decorators }
    try f.editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(1, 1, false))
    }
    #expect(spans.count == 1)
    #expect(spans.first?.isTable == true)
    #expect(spans.first?.location == f.root.document.length - 1)
  }

  @Test func aCellControllerRendersAndEditsItsCell() throws {
    let f = try makeFixture()
    let cell = TextKitInputController(editor: f.editor, cell: f.cells[0][1], root: f.root)
    let other = TextKitInputController(editor: f.editor, cell: f.cells[1][0], root: f.root)
    #expect(cell.document.string == "b1")
    #expect(cell.root === f.root)
    var otherEdits = 0
    other.onRender = { otherEdits += $0.edits.count }
    var rootEdits = 0
    f.root.onRender = { rootEdits += $0.edits.count }

    try cell.setSelectedRange(location: 2, length: 0)
    try cell.insertText("!")
    #expect(cell.document.string == "b1!")
    #expect(try cell.selectedRange() == TextKitRange(location: 3, length: 0))
    // The keystroke touches only its cell's storage.
    #expect(otherEdits == 0)
    #expect(rootEdits == 0)
    #expect(f.root.document.string == "before\n\u{FFFC}\nafter")
    #expect(try f.root.selectedRange() == nil)
    #expect(try other.selectedRange() == nil)
  }

  @Test func deletingMovesWithinTheCellsDocument() throws {
    let f = try makeFixture()
    let cell = TextKitInputController(editor: f.editor, cell: f.cells[1][1], root: f.root)
    try cell.setSelectedRange(location: 2, length: 0)
    try cell.beforeInput("deleteContentBackward")
    #expect(cell.document.string == "b")
    try cell.beforeInput("deleteContentBackward")
    try cell.beforeInput("deleteContentBackward")
    // At the start of the cell, deleting goes no further (the extension
    // keeps the cell's paragraph).
    #expect(cell.document.string == "")
    #expect(f.root.document.string == "before\n\u{FFFC}\nafter")
  }

  @Test func aRangeIntoACellTakesInTheTable() throws {
    let f = try makeFixture()
    let cell = TextKitInputController(editor: f.editor, cell: f.cells[0][0], root: f.root)
    let cellText = try f.editor.read {
      try (getNodeByKey(f.cells[0][0]) as! ElementNode).getFirstDescendant()!
    }
    let textKey = cellText.getKey()
    try f.editor.update(discrete: true) {
      let first = try getRoot().getFirstDescendant() as! TextNode
      let selection = createRangeSelection()
      try selection.anchor.set(first.getKey(), 2, .text)
      try selection.focus.set(textKey, 1, .text)
      try setSelection(selection)
    }
    #expect(try f.root.selectedRange() == TextKitRange(location: 2, length: 6))
    #expect(try cell.selectedRange() == nil)
  }

  @Test func aTableSelectionNamesItsCells() throws {
    let f = try makeFixture()
    var selected: Set<NodeKey> = []
    f.root.onRender = { selected = $0.selectedTableCells }
    try f.editor.update(discrete: true) {
      let table = try getNodeByKey(f.table) as! TableNode
      let anchor = try getNodeByKey(f.cells[0][0]) as! TableCellNode
      let focus = try getNodeByKey(f.cells[1][0]) as! TableCellNode
      try setSelection(createTableSelectionFrom(table, anchor, focus))
    }
    #expect(selected == [f.cells[0][0], f.cells[1][0]])
    #expect(try f.root.selectedRange() == nil)
  }

  @Test func extendingIntoAnotherCellMakesATableSelection() throws {
    let f = try makeFixture()
    let a1 = TextKitInputController(editor: f.editor, cell: f.cells[0][0], root: f.root)
    let b2 = TextKitInputController(editor: f.editor, cell: f.cells[1][1], root: f.root)
    var a1Update: TextKitUpdate?
    a1.onRender = { a1Update = $0 }
    try a1.setSelectedRange(location: 1, length: 0)
    try b2.extendSelection(to: 1)
    let shape = try f.editor.read { () throws -> [NodeKey] in
      let selection = try #require(getSelection() as? TableSelection)
      return [selection.anchor.key, selection.focus.key]
    }
    #expect(shape == [f.cells[0][0], f.cells[1][1]])
    #expect(a1Update?.selectionIsElsewhere == true)
    #expect(a1Update?.selectedTableCells.count == 4)
    // Back into the anchor's cell, it is a range in that cell again (from
    // the cell's start, the table selection's anchor).
    try a1.extendSelection(to: 2)
    #expect(try a1.selectedRange() == TextKitRange(location: 0, length: 2))
    #expect(a1Update?.selectionIsElsewhere == false)
  }

  @Test func aRemovedCellShowsNothing() throws {
    let f = try makeFixture()
    let cell = TextKitInputController(editor: f.editor, cell: f.cells[1][1], root: f.root)
    try f.editor.update(discrete: true) {
      try getNodeByKey(f.cells[1][1])!.remove()
    }
    #expect(cell.document.string == "")
  }
}
