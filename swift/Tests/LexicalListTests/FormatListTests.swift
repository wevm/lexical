import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/formatList.test.ts. The
// test inserting a list in a table cell waits for @lexical/table.

final class ExtendedTestListNode: ListNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(ExtendedTestListNode.self, type: "extended-test-list", extends: ListNode.self)
  }
}

final class ExtendedTestListItemNode: ListItemNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(
      ExtendedTestListItemNode.self, type: "extended-test-list-item", extends: ListItemNode.self)
  }
}

private var formatListNodes: [LexicalNodeConfig] {
  listTestNodes + [.klass(ExtendedTestListNode.self), .klass(ExtendedTestListItemNode.self)]
}

/// `$createNestedListItem(listType, ...texts)`: an item holding a nested
/// list of `listType`, one item per text.
func createNestedListItem(_ listType: ListType, _ texts: String...) throws -> ListItemNode {
  try createListItemNode().append(
    createListNode(listType).append(
      contentsOf: texts.map { try createListItemNode().append(createTextNode($0)) }))
}

/// `$getNestedList(node)`: the nested list a list item holds.
func getNestedList(_ node: LexicalNode) throws -> ListNode {
  let item = try #require(node as? ListItemNode)
  return try #require(try item.getFirstChild() as? ListNode)
}

private func texts(_ list: ListNode) throws -> [String] {
  try list.getChildren().map { try $0.getTextContent() }
}

@Suite struct InsertListTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::insertList > inserting with empty root selection"
    ))
  func emptyRootSelection() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) { try getRoot().select() }
    try update(editor) { try insertList("number") }
    try editor.read { () throws -> Void in
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try isListNode(getRoot().getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::insertList > inserting in root selection with existing child"
    ))
  func rootSelectionWithChild() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) {
      try getRoot().select()
      try getRoot().append(createParagraphNode().append(createTextNode("hello")))
    }
    try update(editor) { try insertList("number") }
    try editor.read { () throws -> Void in
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try isListNode(getRoot().getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::insertList > formatting empty list items"
    ))
  func formattingEmptyItems() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) {
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("Level 1")),
          createListItemNode().append(createListNode("bullet").append(createListItemNode()))))
    }
    try update(editor) {
      try selectAll()
      try insertList("number")
    }
    let numbered = try editor.read {
      try nodesOfType(ListNode.self).filter { try $0.getListType() == "number" }.count
    }
    #expect(numbered == 2)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::insertList > preserves element-anchored selection when converting paragraph with linebreak to list"
    ))
  func preservesElementSelection() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) {
      let paragraph = try createParagraphNode()
      try paragraph.append(createTextNode("Item 1"), createLineBreakNode())
      try getRoot().append(paragraph)
      try paragraph.select()
    }
    try update(editor) { try insertList("bullet") }
    try editor.read { () throws -> Void in
      let list = try #require(try getRoot().getFirstChildOrThrow() as? ListNode)
      let listItem = try #require(try list.getFirstChildOrThrow() as? ListItemNode)
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == listItem.getKey())
      #expect(selection.focus.key == listItem.getKey())
      #expect(selection.anchor.offset == 2)
      #expect(selection.focus.offset == 2)
      #expect(selection.anchor.type == .element)
      #expect(selection.focus.type == .element)
    }
  }
}

@Suite struct HandleListInsertParagraphTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > exits list when list item is completely empty"
    ))
  func exitsWhenEmpty() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    _ = try registerList(editor)
    var emptyListItemKey = ""
    try update(editor) {
      let listItemWithContent = try createListItemNode()
      let listItemEmpty = try createListItemNode()
      emptyListItemKey = listItemEmpty.getKey()
      let listNode = try createListNode("bullet")
      try listItemWithContent.append(createTextNode("item"))
      try listNode.append(listItemWithContent, listItemEmpty)
      try getRoot().append(listNode)
      try listItemEmpty.select()
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      #expect(isListNode(children[0]))
      #expect(isParagraphNode(children[1]))
      #expect(try (children[0] as! ListNode).getChildrenSize() == 1)
      #expect(try getNodeByKey(emptyListItemKey) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > exits list when list item contains only whitespace"
    ))
  func exitsWhenWhitespace() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    _ = try registerList(editor)
    var whitespaceListItemKey = ""
    try update(editor) {
      let listItemWithContent = try createListItemNode()
      let listItemWhitespace = try createListItemNode()
      whitespaceListItemKey = listItemWhitespace.getKey()
      let whitespaceTextNode = try createTextNode(" ")
      let listNode = try createListNode("bullet")
      try listItemWithContent.append(createTextNode("item"))
      try listItemWhitespace.append(whitespaceTextNode)
      try listNode.append(listItemWithContent, listItemWhitespace)
      try getRoot().append(listNode)
      try whitespaceTextNode.select()
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      #expect(isListNode(children[0]))
      #expect(isParagraphNode(children[1]))
      #expect(try (children[0] as! ListNode).getChildrenSize() == 1)
      #expect(try getNodeByKey(whitespaceListItemKey) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > extends list when list item contains non-whitespace content"
    ))
  func extendsWithContent() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    _ = registerRichText(editor)
    _ = try registerList(editor)
    try update(editor) {
      let listItem1 = try createListItemNode().append(createTextNode("item 1"))
      let textNode = try createTextNode("item 2")
      let listItem2 = try createListItemNode().append(textNode)
      try getRoot().append(createListNode("bullet").append(listItem1, listItem2))
      try textNode.selectEnd()
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(try (children[0] as? ListNode)?.getChildrenSize() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > extends list when list item contains a decorator node"
    ))
  func extendsWithDecorator() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    _ = registerRichText(editor)
    _ = try registerList(editor)
    try update(editor) {
      let listItem1 = try createListItemNode().append(createTextNode("item 1"))
      let listItem2 = try createListItemNode().append(createTestDecoratorNode())
      try getRoot().append(createListNode("bullet").append(listItem1, listItem2))
      try listItem2.select()
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(try (children[0] as? ListNode)?.getChildrenSize() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleListInsertParagraph > splits list when the empty element is not the last one"
    ))
  func splitsWhenNotLast() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    _ = try registerList(editor)
    var emptyListItemKey = ""
    try update(editor) {
      let first = try ExtendedTestListItemNode().append(createTextNode("item1"))
      let second = try ExtendedTestListItemNode().append(createTextNode("item2"))
      let listItemEmpty = try ExtendedTestListItemNode()
      emptyListItemKey = listItemEmpty.getKey()
      try getRoot().append(
        ExtendedTestListNode("bullet").append(first, listItemEmpty, second))
      try listItemEmpty.select()
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      let firstList = try #require(children[0] as? ExtendedTestListNode)
      #expect(isParagraphNode(children[1]))
      // The new list is of the same class and type.
      let secondList = try #require(children[2] as? ExtendedTestListNode)
      #expect(try firstList.getChildrenSize() == 1)
      #expect(try secondList.getChildrenSize() == 1)
      #expect(try getNodeByKey(emptyListItemKey) == nil)
      #expect(try firstList.getListType() == secondList.getListType())
    }
  }
}

@Suite struct HandleIndentTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > creates a new nested sublist"
    ))
  func createsSublist() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) {
      let listNode = try ExtendedTestListNode("bullet")
      let listItem1 = try ExtendedTestListItemNode()
      let listItem2 = try ExtendedTestListItemNode()
      try listNode.append(listItem1, listItem2)
      try getRoot().append(listNode)
      try handleIndent(listItem2)
      // The new item keeps the same class.
      let newListItem2 = try #require(try listNode.getChildren()[1] as? ExtendedTestListItemNode)
      #expect(try newListItem2.getChildren().count == 1)
      // The nested list holds the original item.
      let nestedList = try #require(try newListItem2.getChildren()[0] as? ExtendedTestListNode)
      #expect(try nestedList.getChildren().count == 1)
      #expect(try nestedList.getChildren()[0].is(listItem2))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > merges the surrounding sublists when they are the same type"
    ))
  func mergesSameType() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    var middleItem: ListItemNode!
    try update(editor) {
      middleItem = try createListItemNode().append(createTextNode("middle"))
      try getRoot().append(
        createListNode("bullet").append(
          createNestedListItem("bullet", "first child"), middleItem,
          createNestedListItem("bullet", "second child")))
    }
    try update(editor) { try handleIndent(middleItem) }
    try editor.read { () throws -> Void in
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      let children = try listNode.getChildren()
      #expect(children.count == 1)
      let sublist = try getNestedList(children[0])
      #expect(try sublist.getListType() == "bullet")
      #expect(try texts(sublist) == ["first child", "middle", "second child"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > does not merge the surrounding sublists of a different listType"
    ))
  func keepsDifferentTypeAfter() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    var middleItem: ListItemNode!
    try update(editor) {
      middleItem = try createListItemNode().append(createTextNode("middle"))
      try getRoot().append(
        createListNode("bullet").append(
          createNestedListItem("bullet", "bullet child"), middleItem,
          createNestedListItem("number", "number one", "number two")))
    }
    try update(editor) { try handleIndent(middleItem) }
    try editor.read { () throws -> Void in
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      let children = try listNode.getChildren()
      let numberSublist = try getNestedList(children[children.count - 1])
      #expect(try numberSublist.getListType() == "number")
      #expect(try texts(numberSublist) == ["number one", "number two"])
      #expect(children.count == 2)
      let bulletSublist = try getNestedList(children[0])
      #expect(try bulletSublist.getListType() == "bullet")
      #expect(try texts(bulletSublist) == ["bullet child", "middle"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > does not merge the surrounding sublists of a different listType, the other way round"
    ))
  func keepsDifferentTypeBefore() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    var middleItem: ListItemNode!
    try update(editor) {
      middleItem = try createListItemNode().append(createTextNode("middle"))
      try getRoot().append(
        createListNode("number").append(
          createNestedListItem("number", "number one"), middleItem,
          createNestedListItem("bullet", "bullet one", "bullet two")))
    }
    try update(editor) { try handleIndent(middleItem) }
    try editor.read { () throws -> Void in
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      let children = try listNode.getChildren()
      let bulletSublist = try getNestedList(children[children.count - 1])
      #expect(try bulletSublist.getListType() == "bullet")
      #expect(try texts(bulletSublist) == ["bullet one", "bullet two"])
      #expect(children.count == 2)
      let numberSublist = try getNestedList(children[0])
      #expect(try numberSublist.getListType() == "number")
      #expect(try texts(numberSublist) == ["number one", "middle"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleIndent > keeps the checked state of a following check sublist"
    ))
  func keepsChecked() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    var middleItem: ListItemNode!
    try update(editor) {
      middleItem = try createListItemNode().append(createTextNode("middle"))
      let checkItem = try createListItemNode()
      try checkItem.setChecked(true)
      try checkItem.append(createTextNode("done"))
      try getRoot().append(
        createListNode("bullet").append(
          createNestedListItem("bullet", "bullet child"), middleItem,
          createListItemNode().append(createListNode("check").append(checkItem))))
    }
    try update(editor) { try handleIndent(middleItem) }
    try editor.read { () throws -> Void in
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      let children = try listNode.getChildren()
      let checkSublist = try getNestedList(children[children.count - 1])
      #expect(try checkSublist.getListType() == "check")
      let checkItem = try #require(try checkSublist.getFirstChild() as? ListItemNode)
      #expect(try checkItem.getChecked() == true)
      #expect(children.count == 2)
    }
  }
}

@Suite struct HandleOutdentTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::$handleOutdent > removes the nested list and replaces list item"
    ))
  func removesNestedList() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try update(editor) {
      let listNode = try ExtendedTestListNode("bullet")
      let listItem1 = try ExtendedTestListItemNode()
      let listItem2 = try ExtendedTestListItemNode()
      let indentedListItem = try ExtendedTestListItemNode()
      try listNode.append(
        listItem1, listItem2.append(ExtendedTestListNode("bullet").append(indentedListItem)))
      try getRoot().append(listNode)
      try handleOutdent(indentedListItem)
      let children = try listNode.getChildren()
      // The item is outdented and holds no nested list.
      #expect(children.count == 2)
      #expect(try children[1].is(indentedListItem))
      #expect(try indentedListItem.getChildren().count == 0)
    }
  }
}

private func collapseEditor() throws -> Editor {
  try buildEditorFromExtensions(defineExtension(name: "[root]", dependencies: [ListExtension]))
}

/// Builds `list` at the root, then calls `collapseAtStart` on `item` with a
/// caret at its start.
private func collapse(
  _ editor: Editor, _ build: @escaping () throws -> (ListNode, ListItemNode)
) throws {
  try editor.update(discrete: true) {
    let (list, item) = try build()
    try getRoot().clear().append(list)
    let selection = try item.select(0, 0)
    _ = try item.collapseAtStart(selection)
  }
}

private func item(_ text: String) throws -> ListItemNode {
  try createListItemNode().append(createTextNode(text))
}

@Suite struct ListItemCollapseAtStartTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > top-level single item converts to paragraph"
    ))
  func singleItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let listItem = try item("hello")
      return (try createListNode("bullet").append(listItem), listItem)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(isParagraphNode(children[0]))
      #expect(try children[0].getTextContent() == "hello")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > top-level first item extracts as paragraph, keeps remaining list"
    ))
  func firstItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let firstItem = try item("first")
      return (try createListNode("bullet").append(firstItem, item("second")), firstItem)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      #expect(isParagraphNode(children[0]))
      #expect(try children[0].getTextContent() == "first")
      #expect(try (children[1] as? ListNode)?.getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > indented item outdents instead of converting to paragraph"
    ))
  func indentedItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    var indentedItemKey = ""
    try collapse(editor) {
      let indentedItem = try item("child")
      indentedItemKey = indentedItem.getKey()
      let nestedWrapper = try createListItemNode().append(
        createListNode("bullet").append(indentedItem))
      return (try createListNode("bullet").append(item("parent"), nestedWrapper), indentedItem)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(try (children[0] as? ListNode)?.getChildrenSize() == 2)
      let outdentedItem = try #require(try getNodeByKey(indentedItemKey) as? ListItemNode)
      #expect(try outdentedItem.getTextContent() == "child")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > indented item with siblings outdents without breaking list"
    ))
  func indentedItemWithSiblings() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    var firstIndentedKey = ""
    try collapse(editor) {
      let indentedItem1 = try item("child1")
      firstIndentedKey = indentedItem1.getKey()
      let nestedWrapper = try createListItemNode().append(
        createListNode("bullet").append(indentedItem1, item("child2")))
      return (try createListNode("bullet").append(item("parent"), nestedWrapper), indentedItem1)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      let list = try #require(children[0] as? ListNode)
      let outdentedItem = try #require(try getNodeByKey(firstIndentedKey) as? ListItemNode)
      #expect(try outdentedItem.getTextContent() == "child1")
      #expect(
        try list.getChildren().contains {
          try isListItemNode($0) && $0.getTextContent().contains("child2")
        })
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > middle item converts to paragraph and splits list"
    ))
  func middleItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let item2 = try item("second")
      return (try createListNode("bullet").append(item("first"), item2, item("third")), item2)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      let first = try #require(children[0] as? ListNode)
      #expect(try first.getChildrenSize() == 1)
      #expect(try first.getTextContent() == "first")
      #expect(isParagraphNode(children[1]))
      #expect(try children[1].getTextContent() == "second")
      let third = try #require(children[2] as? ListNode)
      #expect(try third.getChildrenSize() == 1)
      #expect(try third.getTextContent() == "third")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > split preserves list type"
    ))
  func splitKeepsType() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let item2 = try item("second")
      return (try createListNode("number").append(item("first"), item2, item("third")), item2)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      #expect(try (children[0] as? ListNode)?.getListType() == "number")
      #expect(try (children[2] as? ListNode)?.getListType() == "number")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > last item converts to paragraph without splitting"
    ))
  func lastItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let item2 = try item("second")
      return (try createListNode("bullet").append(item("first"), item2), item2)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      let first = try #require(children[0] as? ListNode)
      #expect(try first.getChildrenSize() == 1)
      #expect(try first.getTextContent() == "first")
      #expect(isParagraphNode(children[1]))
      #expect(try children[1].getTextContent() == "second")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > empty middle item converts to empty paragraph and splits list"
    ))
  func emptyMiddleItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    try collapse(editor) {
      let emptyItem = try createListItemNode()
      return (
        try createListNode("bullet").append(item("first"), emptyItem, item("third")), emptyItem
      )
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      let first = try #require(children[0] as? ListNode)
      #expect(try first.getChildrenSize() == 1)
      #expect(try first.getFirstChildOrThrow().getTextContent() == "first")
      #expect(isParagraphNode(children[1]))
      #expect(try children[1].getTextContent() == "")
      let third = try #require(children[2] as? ListNode)
      #expect(try third.getChildrenSize() == 1)
      #expect(try third.getFirstChildOrThrow().getTextContent() == "third")
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.isCollapsed())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::ListItemNode.collapseAtStart > empty indented item outdents"
    ))
  func emptyIndentedItem() throws {
    let editor = try collapseEditor()
    defer { editor.dispose() }
    var emptyItemKey = ""
    try collapse(editor) {
      let emptyItem = try createListItemNode()
      emptyItemKey = emptyItem.getKey()
      let nestedWrapper = try createListItemNode().append(
        createListNode("bullet").append(emptyItem))
      return (try createListNode("bullet").append(item("parent"), nestedWrapper), emptyItem)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(isListNode(children[0]))
      let outdentedItem = try #require(try getNodeByKey(emptyItemKey) as? ListItemNode)
      #expect(try outdentedItem.getIndent() == 0)
    }
  }
}

@Suite struct MergeNextSiblingListTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/formatList.test.ts::mergeNextSiblingListIfSameType > does not merge nested sublists of a different listType"
    ))
  func keepsSublistTypes() throws {
    let editor = try createMountedTestEditor(nodes: formatListNodes)
    try editor.update(discrete: true) {
      let list1 = try createListNode("bullet").append(
        item("a"), createNestedListItem("number", "one"))
      let list2 = try createListNode("bullet").append(
        createNestedListItem("bullet", "two"), item("b"))
      try getRoot().clear().append(list1, list2)
    }
    try editor.read { () throws -> Void in
      let list = try #require(try getRoot().getChildren().first as? ListNode)
      // The two bullet lists merge, but their sublists stay separate.
      let sublistTypes = try list.getChildren()
        .compactMap { try ($0 as? ListItemNode)?.getFirstChild() as? ListNode }
        .map { try $0.getListType() }
      #expect(sublistTypes == ["number", "bullet"])
    }
  }
}
