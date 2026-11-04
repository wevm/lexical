import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/Issue7032Repro.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "issue-7032-host", dependencies: [ListExtension]))
}

private func listAt(_ index: Int) throws -> ListNode {
  try #require(
    try getRoot().getChildAtIndex(index) as? ListNode, "expected a ListNode at root index \(index)")
}

private func itemValues(_ list: ListNode) throws -> [Int] {
  try list.getChildren().compactMap { $0 as? ListItemNode }.map { try $0.getValue() }
}

/// A list of `texts` at the root, then a "break" paragraph inserted after
/// the item at `index`.
private func splitList(
  _ list: @escaping () throws -> ListNode, _ texts: [String], after index: Int
)
  throws -> Editor
{
  let editor = try buildEditor()
  try editor.update(discrete: true) {
    let list = try list()
    for text in texts {
      try list.append(createListItemNode().append(createTextNode(text)))
    }
    try getRoot().clear().append(list)
  }
  try editor.update(discrete: true) {
    let item = try #require(try listAt(0).getChildren()[index] as? ListItemNode)
    // Inserting a non-list node between two items splits the list around
    // it — the page-break case from issue #7032.
    _ = try item.insertAfter(createParagraphNode().append(createTextNode("break")))
  }
  return editor
}

@Suite struct Issue7032ReproTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > inserting a block between list items continues the numbering"
    ))
  func continuesNumbering() throws {
    let editor = try splitList({ try createListNode("number") }, ["a", "b", "c"], after: 0)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      #expect(try getRoot().getChildren().map { $0.getType() } == ["list", "paragraph", "list"])
      #expect(try itemValues(listAt(0)) == [1])
      let tail = try listAt(2)
      #expect(try tail.getStart() == 2)
      #expect(try itemValues(tail) == [2, 3])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > a list that does not start at 1 continues from the split point"
    ))
  func continuesFromStart() throws {
    let editor = try splitList(
      { try createListNode("number", start: 5) }, ["a", "b", "c"], after: 1)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      #expect(try itemValues(listAt(0)) == [5, 6])
      #expect(try listAt(2).getStart() == 7)
      #expect(try itemValues(listAt(2)) == [7])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/Issue7032Repro.test.ts::Issue #7032: splitting a numbered list restarts its numbering > a bullet list keeps its start when it is split"
    ))
  func bulletKeepsStart() throws {
    let editor = try splitList({ try createListNode("bullet") }, ["a", "b"], after: 0)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in #expect(try listAt(2).getStart() == 1) }
  }
}
