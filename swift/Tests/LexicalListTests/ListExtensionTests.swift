import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListExtension.test.ts.
// Upstream's configured extension spreads ListExtension with a new config;
// configExtension is the Swift form.

@Suite struct ListExtensionTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListExtension.test.ts::ListExtension > Creates the list"
    ))
  func createsTheList() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]", dependencies: [ListExtension, RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createListNode("number").append(
                createListItemNode().append(createTextNode("item 1")),
                createListItemNode().append(createTextNode("item 2"))))
          })))
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      #expect(isListNode(try getRoot().getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListExtension.test.ts::CheckListExtension > Preserves numbering when configured via extension"
    ))
  func preservesNumberingWhenConfigured() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root-configured]",
        dependencies: [
          configExtension(ListExtension) {
            $0.hasStrictIndent = false
            $0.shouldPreserveNumbering = true
          },
          RichTextExtension,
        ],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createListNode("number").append(
                createListItemNode().append(createTextNode("A")),
                createListItemNode(),
                createListItemNode().append(createTextNode("B"))))
          })))
    defer { editor.dispose() }
    try editor.update {
      let list = try getRoot().getFirstChildOrThrow()
      if let list = list as? ListNode {
        let emptyItem = try #require(
          try list.getChildAtIndex(1) as? ListItemNode, "Expected second child to be a list item")
        _ = try emptyItem.select()
      }
    }
    _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND, ())
    try editor.update {
      let children = try getRoot().getChildren()
      // We expect 3 children now: List, Paragraph, List
      #expect(children.count == 3)
      let secondList = try #require(children[2] as? ListNode, "Expected third child to be a list")
      // Item A stays in the first list; B moves to the second, numbered 2.
      #expect(try secondList.getStart() == 2)
    }
  }
}

@Suite struct CheckListExtensionTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListExtension.test.ts::CheckListExtension > Creates the list"
    ))
  func createsTheList() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]", dependencies: [CheckListExtension, RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createListNode("check").append(
                createListItemNode(true).append(createTextNode("checked")),
                createListItemNode(false).append(createTextNode("unchecked"))))
          })))
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      #expect(isListNode(try getRoot().getFirstChildOrThrow()))
    }
  }

  @Test func insertCheckListCommand() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "[root]", dependencies: [CheckListExtension, RichTextExtension]))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try createTextNode("todo")
      try getRoot().clear().append(createParagraphNode().append(text))
      _ = try text.select(0, 0)
    }
    #expect(try editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND, ()))
    try editor.read { () throws -> Void in
      let list = try #require(try getRoot().getFirstChild() as? ListNode)
      #expect(try list.getListType() == "check")
      #expect(try list.getTextContent() == "todo")
    }
  }
}
