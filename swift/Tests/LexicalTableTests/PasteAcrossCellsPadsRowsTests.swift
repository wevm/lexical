import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalList
import LexicalRichText
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts.

private func buildEditor(
  name: String, _ extra: [any AnyLexicalExtensionArgument] = []
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name,
      dependencies: extra + [
        RichTextExtension,
        ListExtension,
        configExtension(TableExtension) { $0.hasHorizontalScroll = false },
      ],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
}

/// A 3x3 table whose cells each hold a bullet list with one item "rNcM text".
private func createListTable() throws {
  let table = try createTableNode()
  for r in 0..<3 {
    let row = try createTableRowNode()
    for c in 0..<3 {
      try row.append(
        createTableCellNode(TableCellHeaderStates.NO_STATUS).append(
          createListNode("bullet").append(
            createListItemNode().append(createTextNode("r\(r)c\(c) text")))))
    }
    try table.append(row)
  }
  try getRoot().clear().append(table, createParagraphNode())
}

/// Select from offset 2 of the text starting with `from` to offset 2 of the
/// text starting with `to`, and paste rich HTML holding a list over it.
private func pasteAcross(_ from: String, _ to: String) throws {
  let texts = try getRoot().getAllTextNodes()
  func find(_ prefix: String) throws -> TextNode {
    try #require(
      try texts.first { try $0.getTextContent().hasPrefix(prefix) },
      "no text starting with \(prefix)")
  }
  let selection = createRangeSelection()
  try selection.anchor.set(find(from).getKey(), 2, .text)
  try selection.focus.set(find(to).getKey(), 2, .text)
  try setSelection(selection)
  let html = "<p><b>bold</b> <i>italic</i></p><ul><li>item</li></ul>"
  let editor = try getEditor()
  try insertGeneratedNodes(
    editor,
    generateNodesFromDOM(editor, DOMParser().parseFromString(html, "text/html")),
    selection)
}

private func cellsPerRow() throws -> String {
  let table = try #require(
    try getRoot().getFirstChild() as? TableNode, "expected a TableNode at the root")
  return try table.getChildren().compactMap { $0 as? TableRowNode }.map {
    String(try $0.getChildrenSize())
  }.joined(separator: ",")
}

private func row(_ width: Int) throws -> TableRowNode {
  let row = try createTableRowNode()
  for _ in 0..<width {
    try row.append(
      createTableCellNode(TableCellHeaderStates.NO_STATUS).append(createParagraphNode()))
  }
  return row
}

@Suite struct PasteAcrossCellsPadsRowsTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells > from r0c1 to r1c1 keeps every row as wide as the table"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells > from r0c2 to r1c1 keeps every row as wide as the table"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells with NodeStructureExtension > from r0c1 to r1c1 keeps every row as wide as the table"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells with NodeStructureExtension > from r0c2 to r1c1 keeps every row as wide as the table"
    ),
    arguments: [false, true], [("r0c1", "r1c1"), ("r0c2", "r1c1")])
  func pasteKeepsRowsWide(withNodeStructure: Bool, range: (String, String)) throws {
    let editor = try buildEditor(
      name: "paste-across-cells", withNodeStructure ? [NodeStructureExtension] : [])
    defer { editor.dispose() }
    try editor.update(discrete: true) { try createListTable() }
    try editor.update(discrete: true) { try pasteAcross(range.0, range.1) }
    #expect(try editor.read { try cellsPerRow() } == "3,3,3")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a change to one row pads that row only, not the other short rows"
    ))
  func padsChangedRowOnly() throws {
    // Rows ragged before the update (a concurrent row and column insert from
    // collaborators, merged without transforms) belong to whoever made them:
    // padding them here would race with their authors' own padding.
    let editor = try buildEditor(name: "pad-changed-row")
    defer { editor.dispose() }
    try editor.update(discrete: true, skipTransforms: true) {
      let table = try createTableNode()
      for width in [3, 2, 3] {
        try table.append(row(width))
      }
      try getRoot().clear().append(table, createParagraphNode())
    }
    #expect(try editor.read { try cellsPerRow() } == "3,2,3")
    try editor.update(discrete: true) {
      let table = try #require(
        try getRoot().getFirstChild() as? TableNode, "expected a TableNode at the root")
      let last = try #require(try table.getLastChild() as? TableRowNode, "expected a row")
      try last.getLastChildOrThrow().remove()
    }
    #expect(try editor.read { try cellsPerRow() } == "3,2,3")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::an update that adds a row pads every short row, also when a row transform adds it after rows changed (in a transform: false)"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::an update that adds a row pads every short row, also when a row transform adds it after rows changed (in a transform: true)"
    ),
    arguments: [false, true])
  func addedRowPadsEveryShortRow(inTransform: Bool) throws {
    let editor = try buildEditor(name: "pad-after-row-added")
    defer { editor.dispose() }
    var first = ""
    var armed = false
    // Registered before the table exists, so that registering it does not
    // run the transforms on the ragged table.
    let remove = try editor.registerNodeTransform(TableRowNode.self) { tableRow in
      if armed && tableRow.getKey() == first {
        armed = false
        try tableRow.insertBefore(row(3))
      }
    }
    try editor.update(discrete: true, skipTransforms: true) {
      let table = try createTableNode()
      for width in [3, 3, 2] {
        try table.append(row(width))
      }
      try getRoot().clear().append(table, createParagraphNode())
      first = try table.getFirstChildOrThrow().getKey()
    }
    #expect(try editor.read { try cellsPerRow() } == "3,3,2")
    armed = inTransform
    try editor.update(discrete: true) {
      let table = try #require(
        try getRoot().getFirstChild() as? TableNode, "expected a TableNode at the root")
      let tableRow = try #require(try table.getFirstChild() as? TableRowNode, "expected a row")
      try tableRow.getLastChildOrThrow().remove()
      if !inTransform {
        try tableRow.insertBefore(row(3))
      }
    }
    remove()
    #expect(try editor.read { try cellsPerRow() } == "3,3,3,3")
  }
}
