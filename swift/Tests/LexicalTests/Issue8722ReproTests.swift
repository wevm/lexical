import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8722Repro.test.ts.

private let ext = defineExtension(
  name: "[8722]", dependencies: [RichTextExtension, ListExtension],
  nodes: { [.klass(TestDecoratorNode.self)] })

private func emptyItemList() throws -> (ListNode, ListItemNode) {
  let item = try createListItemNode()
  return (try createListNode("bullet").append(item), item)
}

/// Runs `setup`, then `edit` on the selection it leaves, in two updates.
private func run(
  _ setup: @escaping () throws -> Void, _ edit: @escaping (RangeSelection) throws -> Void
) throws -> Editor {
  let editor = try buildEditorFromExtensions(ext)
  try editor.update(discrete: true, setup)
  try editor.update(discrete: true) {
    try edit(try #require(try getSelection() as? RangeSelection, "Expected RangeSelection"))
  }
  return editor
}

private func hasDecorator(_ editor: Editor) throws -> Bool {
  try editor.read { try getRoot().getChildren().contains { $0 is DecoratorNode } }
}

@Suite struct Issue8722ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteCharacter backward from empty ListItem should not remove the preceding decorator"
    ))
  func deleteCharacterKeepsDecorator() throws {
    let editor = try run(
      {
        let (list, item) = try emptyItemList()
        try getRoot().clear().append(createTestDecoratorNode().setIsInline(false), list)
        _ = try item.select(0, 0)
      }, { try $0.deleteCharacter(true) })
    defer { editor.dispose() }
    #expect(try hasDecorator(editor))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem with prece… should not remove the decorator"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty paragraph with prec… should not remove the decorator"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine forward from empty ListItem with follow… should not remove the decorator"
    ))
  func deleteLineKeepsDecorator() throws {
    let cases: [(String, Bool, () throws -> Void)] = [
      (
        "backward from empty ListItem with preceding decorator", true,
        {
          let (list, item) = try emptyItemList()
          try getRoot().clear().append(createTestDecoratorNode().setIsInline(false), list)
          _ = try item.select(0, 0)
        }
      ),
      (
        "backward from empty paragraph with preceding decorator", true,
        {
          let paragraph = try createParagraphNode()
          try getRoot().clear().append(createTestDecoratorNode().setIsInline(false), paragraph)
          _ = try paragraph.select(0, 0)
        }
      ),
      (
        "forward from empty ListItem with following decorator", false,
        {
          let (list, item) = try emptyItemList()
          try getRoot().clear().append(list, createTestDecoratorNode().setIsInline(false))
          _ = try item.select(0, 0)
        }
      ),
    ]
    for (name, isBackward, setup) in cases {
      let editor = try run(setup) { try $0.deleteLine(isBackward) }
      defer { editor.dispose() }
      #expect(try hasDecorator(editor), "deleteLine \(name)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem preceded by empty paragraph merges them"
    ))
  func deleteLineMergesWithEmptyParagraph() throws {
    let editor = try run(
      {
        let (list, item) = try emptyItemList()
        try getRoot().clear().append(createParagraphNode(), list)
        _ = try item.select(0, 0)
      }, { try $0.deleteLine(true) })
    defer { editor.dispose() }
    // In real browsers, modify('extend', ..., 'lineboundary') stays collapsed
    // in an empty list item, so deleteLine falls through to deleteCharacter,
    // which merges the empty item with the preceding paragraph.
    #expect(try editor.read { try getRoot().getChildrenSize() } == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8722Repro.test.ts::DELETE_LINE_COMMAND on empty ListItem with preceding decorator (#8722) > deleteLine backward from empty ListItem preceded by non-empty paragraph should work normally"
    ))
  func deleteLineAfterNonEmptyParagraph() throws {
    let editor = try run(
      {
        let (list, item) = try emptyItemList()
        try getRoot().clear().append(createParagraphNode().append(createTextNode("hello")), list)
        _ = try item.select(0, 0)
      }, { try $0.deleteLine(true) })
    defer { editor.dispose() }
    #expect(try editor.read { try getRoot().getTextContent() } == "hello")
  }
}
