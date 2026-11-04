import Lexical
import LexicalExtension
import LexicalList
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.nestedCopy.test.ts.
// A copy of a block lands where it is not an identified block (a number list
// pasted into a nested item of a bullet list nests there): it keeps no id,
// so the block it was copied from stays the only holder of its id.

private func all(_ node: LexicalNode) throws -> [LexicalNode] {
  [node] + ((try (node as? ElementNode)?.getChildren().flatMap(all)) ?? [])
}

@Suite struct BlockIdNestedCopyTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.nestedCopy.test.ts::a list copied into a nested list item (where it nests) keeps no block id"
    ))
  func listCopiedIntoNestedItemKeepsNoId() throws {
    var n = 0
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          ListExtension,
          configExtension(BlockIdExtension) {
            $0.generateId = {
              n += 1
              return "g\(n)"
            }
          },
        ],
        editorConfig: InitialEditorConfig(
          onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(
        createListNode("number").append(
          createListItemNode().append(createTextNode("one")),
          createListItemNode().append(createTextNode("two"))),
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("top")),
          createListItemNode().append(
            createListNode("bullet").append(createListItemNode().append(createTextNode("nested"))))
        ))
    }
    let listId = try editor.read { try getBlockId(#require(try getRoot().getFirstChild())) }
    #expect(listId != nil)
    // A paste of the whole number list into the nested item: its JSON, with
    // the list's NodeState, parsed into a new node.
    try editor.update(discrete: true) {
      let source = try #require(try getRoot().getFirstChildOrThrow() as? ListNode)
      let copy = try #require(try parseSerializedNode(source.exportJSON()) as? ElementNode)
      for item in try source.getChildren() {
        try copy.append(parseSerializedNode(item.exportJSON()))
      }
      let bullet = try #require(try getRoot().getLastChildOrThrow() as? ElementNode)
      let wrapper = try #require(try bullet.getLastChildOrThrow() as? ElementNode)
      let nestedList = try #require(try wrapper.getFirstChildOrThrow() as? ElementNode)
      try #require(try nestedList.getFirstChildOrThrow() as? ElementNode).append(copy)
    }
    try editor.read { () throws -> Void in
      let holders = try all(getRoot()).filter {
        try $0 is ListNode && getBlockId($0) == listId
      }
      #expect(holders.count == 1)
      #expect(holders.first?.is(try getRoot().getFirstChild()) == true)
    }
  }
}
