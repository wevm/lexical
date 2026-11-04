import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts.

private final class IsolatedTestDecoratorNode: TestDecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      IsolatedTestDecoratorNode.self, type: "isolated_test_decorator",
      extends: TestDecoratorNode.self)
  }
  override func isIsolated() -> Bool { true }
}

private func createIsolatedTestDecoratorNode() throws -> IsolatedTestDecoratorNode {
  try IsolatedTestDecoratorNode().setIsInline(false)
}

private func backspaceEvent() -> KeyboardEvent {
  KeyboardEvent("keydown", key: "Backspace", cancelable: true)
}

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[root]", dependencies: [ListExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(IsolatedTestDecoratorNode.self)] }))
}

/// Builds the root's children with `setUp`, which returns where to put a
/// collapsed caret, then dispatches Backspace with `event`.
private func backspace(
  _ editor: Editor, _ event: KeyboardEvent = backspaceEvent(),
  _ setUp: @escaping () throws -> (LexicalNode, Int?)
) throws -> Bool {
  var handled = false
  try editor.update(discrete: true) {
    try getRoot().clear()
    let (node, offset) = try setUp()
    if let text = node as? TextNode {
      _ = try text.select(offset, offset)
    } else if let element = node as? ElementNode {
      _ = try element.select(offset, offset)
    }
    handled = try editor.dispatchCommand(KEY_BACKSPACE_COMMAND, event)
  }
  return handled
}

/// The demotion cases: a decorator, then a two-item list of `listType`.
private func demotesFirstItem(_ listType: ListType, inline: Bool) throws {
  let editor = try buildEditor()
  defer { editor.dispose() }
  let event = backspaceEvent()
  let handled = try backspace(editor, event) {
    let decorator = try createTestDecoratorNode().setIsInline(inline)
    let firstItem = try createListItemNode().append(createTextNode("first"))
    let list = try createListNode(listType).append(
      firstItem, createListItemNode().append(createTextNode("second")))
    try getRoot().append(decorator, list)
    return (firstItem, 0)
  }
  #expect(handled)
  #expect(event.defaultPrevented)
  try editor.read { () throws -> Void in
    let children = try getRoot().getChildren()
    try #require(children.count == 3)
    if inline {
      let paragraph = try #require(
        children[0] as? ParagraphNode,
        "Expected leading ParagraphNode wrapping inline TestDecoratorNode")
      #expect(try paragraph.getChildren().map { $0 is TestDecoratorNode } == [true])
    } else {
      #expect(children[0] is TestDecoratorNode, "Expected leading TestDecoratorNode")
    }
    let paragraph = try #require(children[1] as? ParagraphNode, "Expected demoted ParagraphNode")
    #expect(try paragraph.getTextContent() == "first")
    let tail = try #require(children[2] as? ListNode, "Expected trailing ListNode")
    #expect(try tail.getListType() == listType)
    #expect(try tail.getChildrenSize() == 1)
    #expect(try tail.getTextContent() == "second")
    let selection = try #require(try getSelection() as? RangeSelection, "Expected RangeSelection")
    #expect(selection.isCollapsed())
    #expect(try selection.anchor.key == paragraph.getFirstChildOrThrow().getKey())
    #expect(selection.anchor.offset == 0)
  }
}

@Suite struct RegisterListBackspaceDecoratorTests {
  // MARK: preserves the decorator and demotes the first list item

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + bullet list"
    ))
  func blockDecoratorBulletList() throws { try demotesFirstItem("bullet", inline: false) }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + numbered list"
    ))
  func blockDecoratorNumberedList() throws { try demotesFirstItem("number", inline: false) }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + check list"
    ))
  func blockDecoratorCheckList() throws { try demotesFirstItem("check", inline: false) }

  // Upstream's name is "inline keyboard-selectable decorator + bullet list";
  // the ledger records vitest's truncated form.
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > inline keyboard-selectable decorator + …"
    ))
  func inlineDecoratorBulletList() throws { try demotesFirstItem("bullet", inline: true) }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > block decorator + single-item list — list is removed"
    ))
  func singleItemListRemoved() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let event = backspaceEvent()
    let handled = try backspace(editor, event) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      let onlyItem = try createListItemNode().append(createTextNode("only"))
      try getRoot().append(decorator, createListNode("bullet").append(onlyItem))
      return (onlyItem, 0)
    }
    #expect(handled)
    #expect(event.defaultPrevented)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(children[0] is TestDecoratorNode, "Expected leading TestDecoratorNode")
      let paragraph = try #require(children[1] as? ParagraphNode, "Expected demoted ParagraphNode")
      #expect(try paragraph.getTextContent() == "only")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > preserves the decorator and demotes the first list item > two adjacent block decorators + list — both preserved"
    ))
  func twoBlockDecoratorsPreserved() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let event = backspaceEvent()
    let handled = try backspace(editor, event) {
      let first = try createTestDecoratorNode().setIsInline(false)
      let second = try createTestDecoratorNode().setIsInline(false)
      let helloItem = try createListItemNode().append(createTextNode("hello"))
      try getRoot().append(first, second, createListNode("bullet").append(helloItem))
      return (helloItem, 0)
    }
    #expect(handled)
    #expect(event.defaultPrevented)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(children[0] is TestDecoratorNode, "Expected first TestDecoratorNode")
      #expect(children[1] is TestDecoratorNode, "Expected second TestDecoratorNode")
      let paragraph = try #require(children[2] as? ParagraphNode, "Expected demoted ParagraphNode")
      #expect(try paragraph.getTextContent() == "hello")
    }
  }

  // MARK: first item converts to paragraph via collapseAtStart

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > no decorator before the list"
    ))
  func noDecoratorBeforeList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let paragraph = try createParagraphNode().append(createTextNode("intro"))
      let firstItem = try createListItemNode().append(createTextNode("first"))
      try getRoot().append(paragraph, createListNode("bullet").append(firstItem))
      return (firstItem, 0)
    }
    #expect(handled)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let intro = try #require(children[0] as? ParagraphNode, "Expected intro ParagraphNode")
      #expect(try intro.getTextContent() == "intro")
      let converted = try #require(
        children[1] as? ParagraphNode, "Expected converted ParagraphNode")
      #expect(try converted.getTextContent() == "first")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > caret on the second list item — converts to paragraph via collapseAtStart"
    ))
  func caretOnSecondItem() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      let secondItem = try createListItemNode().append(createTextNode("second"))
      try getRoot().append(
        decorator,
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("first")), secondItem))
      return (secondItem, 0)
    }
    #expect(handled)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      let list = try #require(children[1] as? ListNode, "Expected ListNode at index 1")
      #expect(try list.getChildrenSize() == 1)
      let converted = try #require(
        children[2] as? ParagraphNode, "Expected converted ParagraphNode")
      #expect(try converted.getTextContent() == "second")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > caret in the middle of the first list item"
    ))
  func caretInMiddleOfFirstItem() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      let text = try createTextNode("first")
      try getRoot().append(
        decorator, createListNode("bullet").append(createListItemNode().append(text)))
      return (text, 2)
    }
    #expect(handled == false)
    try editor.read { () throws -> Void in
      #expect(isListNode(try getRoot().getChildAtIndex(1)), "Expected ListNode at index 1")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > nested list — outdents inner item"
    ))
  func nestedListOutdents() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      let innerLi = try createListItemNode().append(createTextNode("nested"))
      let outerLi = try createListItemNode().append(createListNode("bullet").append(innerLi))
      try getRoot().append(decorator, createListNode("bullet").append(outerLi))
      return (innerLi, 0)
    }
    #expect(handled)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > isolated decorator before the list"
    ))
  func isolatedDecoratorBeforeList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let decorator = try createIsolatedTestDecoratorNode()
      let firstItem = try createListItemNode().append(createTextNode("first"))
      try getRoot().append(decorator, createListNode("bullet").append(firstItem))
      return (firstItem, 0)
    }
    #expect(handled)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(children[0] is IsolatedTestDecoratorNode)
      let converted = try #require(
        children[1] as? ParagraphNode, "Expected converted ParagraphNode")
      #expect(try converted.getTextContent() == "first")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListBackspaceDecorator.test.ts::registerList — Backspace adjacent to DecoratorNode (#5072) > first item converts to paragraph via collapseAtStart > empty first list item — converts to empty paragraph"
    ))
  func emptyFirstItem() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let handled = try backspace(editor) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      let emptyItem = try createListItemNode()
      try getRoot().append(
        decorator,
        createListNode("bullet").append(
          emptyItem, createListItemNode().append(createTextNode("second"))))
      return (emptyItem, nil)
    }
    #expect(handled)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(children[0] is TestDecoratorNode)
      let paragraph = try #require(children[1] as? ParagraphNode, "Expected empty ParagraphNode")
      #expect(try paragraph.getTextContent() == "")
      let list = try #require(children[2] as? ListNode, "Expected remaining ListNode")
      #expect(try list.getFirstChildOrThrow().getTextContent() == "second")
    }
  }
}
