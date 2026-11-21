import Lexical
import LexicalExtension
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableRowPadLater.test.ts.
//
// Three rows of three cells; an update removes a cell from the first two,
// and a transform that runs later in the same update widens the third row
// (it appends two cells, or raises a cell's span). The rows that lost
// cells are padded to the width the table has once the update is done:
// every row covers five grid positions.

private func cell() throws -> TableCellNode {
  try createTableCellNode().append(createParagraphNode().append(createTextNode("x")))
}

private func widths() throws -> [Int] {
  let table = try #require(try getRoot().getFirstChildOrThrow() as? TableNode)
  return try computeTableMapSkipCellCheck(table, nil, nil).tableMap.map(\.count)
}

@Suite struct TableRowPadLaterTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableRowPadLater.test.ts::rows that lost cells are padded to a width another row reaches later in the update (append)"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/TableRowPadLater.test.ts::rows that lost cells are padded to a width another row reaches later in the update (span)"
    ),
    arguments: ["append", "span"])
  func padsToLaterWidth(change: String) throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "table-row-pad-later", dependencies: [TableExtension],
        editorConfig: InitialEditorConfig(
          onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none))
    )
    defer { editor.dispose() }
    var first = ""
    var active = false
    var changed = false
    try editor.registerNodeTransform(TableRowNode.self) { row in
      if active && !changed && row.getKey() == first {
        changed = true
        let table = try #require(try row.getParent() as? TableNode)
        let last = try #require(try table.getLastChildOrThrow() as? TableRowNode)
        if change == "append" {
          try last.append(cell(), cell())
        } else {
          try #require(try last.getFirstChildOrThrow() as? TableCellNode).setColSpan(3)
        }
      }
    }
    try editor.update(discrete: true) {
      let table = try createTableNode()
      for i in 0..<3 {
        let row = try createTableRowNode().append(cell(), cell(), cell())
        if i == 0 {
          first = row.getKey()
        }
        try table.append(row)
      }
      try getRoot().append(table)
    }
    #expect(try editor.read { try widths() } == [3, 3, 3])
    active = true
    try editor.update(discrete: true) {
      let rows = try #require(try getRoot().getFirstChildOrThrow() as? TableNode).getChildren()
      try #require(rows[0] as? TableRowNode).getLastChildOrThrow().remove()
      try #require(rows[1] as? TableRowNode).getLastChildOrThrow().remove()
    }
    #expect(try editor.read { try widths() } == [5, 5, 5])
  }
}
