import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableFieldLeavesRaggedRows.test.ts.
//
// A table whose rows are ragged (collaborators' concurrent row and column
// inserts, which each author pads) and an update that changes only the
// table itself: the short row is left to its author.

private func widths() throws -> [Int] {
  let table = try #require(try getRoot().getFirstChild() as? TableNode)
  return try table.getChildren().compactMap { $0 as? TableRowNode }.map {
    try $0.getChildrenSize()
  }
}

@Suite struct TableFieldLeavesRaggedRowsTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableFieldLeavesRaggedRows.test.ts::a table-only field change leaves pre-existing short rows alone"
    ))
  func tableFieldChangeLeavesShortRows() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "review-9539-table-field",
        dependencies: [
          RichTextExtension,
          configExtension(TableExtension) { $0.hasHorizontalScroll = false },
        ],
        editorConfig: InitialEditorConfig(
          onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none))
    )
    defer { editor.dispose() }
    try editor.update(discrete: true, skipTransforms: true) {
      let table = try createTableNode()
      for width in [3, 2, 3] {
        let row = try createTableRowNode()
        for _ in 0..<width {
          try row.append(createTableCellNode().append(createParagraphNode()))
        }
        try table.append(row)
      }
      try getRoot().append(table)
    }
    #expect(try editor.read { try widths() } == [3, 2, 3])
    try editor.update(discrete: true) {
      try #require(try getRoot().getFirstChild() as? TableNode).setRowStriping(true)
    }
    #expect(try editor.read { try widths() } == [3, 2, 3])
  }
}
