import Lexical
import LexicalHTML
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts.
// The reconciled HTML upstream compares is rebuilt from each node's
// createDOM (see `listHTML`), without the reconciler's `dir` and with a text
// node as a bare `<span>`. The shadow-root table cell case waits for
// @lexical/table. Strings are compared exactly, so attributes and JSON keys
// are in the order JS writes them (`value` before `class`), which upstream's
// order-insensitive comparisons do not pin.

private var itemTheme: EditorThemeClasses {
  [
    "list": [
      "listitem": "my-listItem-item-class",
      "nested": ["listitem": "my-nested-list-listItem-class"],
    ]
  ]
}

private var editorConfig: EditorConfig { EditorConfig(namespace: "", theme: itemTheme) }

/// The committed document as the reconciled HTML upstream compares.
private func listHTML(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    try getRoot().getChildren().map { try html($0, editor._config) }.joined()
  }
}

private func html(_ node: LexicalNode, _ config: EditorConfig) throws -> String {
  if let text = node as? TextNode {
    return "<span>\(try text.getTextContent())</span>"
  }
  let element = try #require(node as? ElementNode)
  let dom = try element.createDOM(config)
  let close = "</\(dom.localName)>"
  let open = String(dom.outerHTML.dropLast(close.count)).replacing(#" dir="auto""#, with: "")
  let children = try element.getChildren().map { try html($0, config) }.joined()
  return open + children + close
}

private func li(_ children: LexicalNode...) throws -> ListItemNode {
  try createListItemNode().append(contentsOf: children)
}

private func li(_ text: String) throws -> ListItemNode { try li(createTextNode(text)) }

private func list(_ type: ListType = "bullet", _ items: ListItemNode...) throws -> ListNode {
  try createListNode(type, start: 1).append(contentsOf: items)
}

/// The `ListItemNode.replace()` and `insertNewAfter()` fixture: a bullet list
/// of "one", "two" and "three".
private func threeItems() throws -> (Editor, [ListItemNode]) {
  let editor = try createMountedTestEditor(nodes: listTestNodes)
  var items: [ListItemNode] = []
  try update(editor) {
    items = try ["one", "two", "three"].map(li)
    _ = try getRoot().append(list("bullet", items[0], items[1], items[2]))
  }
  #expect(
    try listHTML(editor)
      == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li><li value="3"><span>three</span></li></ul>"#
  )
  return (editor, items)
}

/// The `ListItemNode.remove()` cases: `x` between `before` and `after`, each
/// a list item, removed after checking `html` (when given) is the start.
private func removeX(
  _ before: @escaping () throws -> ListItemNode, _ after: @escaping () throws -> ListItemNode,
  initially: String?, then expected: String
) throws {
  let editor = try createMountedTestEditor(nodes: listTestNodes)
  var x: ListItemNode!
  try update(editor) {
    x = try li("x")
    _ = try getRoot().append(list("bullet", before(), x, after()))
  }
  if let initially { #expect(try listHTML(editor) == initially) }
  try update(editor) { try x.remove() }
  #expect(try listHTML(editor) == expected)
}

/// A list item holding only a nested list of `items`.
private func nested(_ type: ListType = "bullet", _ items: ListItemNode...) throws -> ListItemNode {
  try li(createListNode(type, start: 1).append(contentsOf: items))
}

@Suite struct LexicalListItemNodeTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItemNode = try ListItemNode()
      #expect(listItemNode.getType() == "listitem")
      #expect(try listItemNode.getTextContent() == "")
    }
    #expect(throws: (any Error).self) { try ListItemNode() }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItemNode = try ListItemNode()
      #expect(
        try listItemNode.createDOM(editorConfig).outerHTML
          == #"<li value="1" class="my-listItem-item-class"></li>"#)
      #expect(
        try listItemNode.createDOM(EditorConfig(namespace: "", theme: [:])).outerHTML
          == #"<li value="1"></li>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.updateDOM() > base"
    ))
  func updateDOMBase() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItemNode = try ListItemNode()
      let domElement = try listItemNode.createDOM(editorConfig)
      #expect(domElement.outerHTML == #"<li value="1" class="my-listItem-item-class"></li>"#)
      let newListItemNode = try ListItemNode()
      #expect(try newListItemNode.updateDOM(listItemNode, domElement, editorConfig) == false)
      #expect(domElement.outerHTML == #"<li value="1" class="my-listItem-item-class"></li>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.updateDOM() > nested list"
    ))
  func updateDOMNestedList() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let parentListNode = try createListNode("bullet", start: 1)
      let parentListItemNode = try ListItemNode()
      _ = try parentListNode.append(parentListItemNode)
      let domElement = try parentListItemNode.createDOM(editorConfig)
      #expect(domElement.outerHTML == #"<li value="1" class="my-listItem-item-class"></li>"#)
      let nestedListNode = try createListNode("bullet", start: 1)
      _ = try nestedListNode.append(ListItemNode())
      _ = try parentListItemNode.append(nestedListNode)
      #expect(
        try parentListItemNode.updateDOM(parentListItemNode, domElement, editorConfig) == false)
      #expect(
        domElement.outerHTML
          == #"<li value="1" class="my-listItem-item-class my-nested-list-listItem-class"></li>"#)
    }
  }

  // MARK: ListItemNode.replace()

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > another list item node"
    ))
  func replaceWithListItem() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[0].replace(li("bar")) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>bar</span></li><li value="2"><span>two</span></li><li value="3"><span>three</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > first list item with a non list item node"
    ))
  func replaceFirstWithParagraph() throws {
    let (editor, items) = try threeItems()
    try update(editor) {}
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li><li value="3"><span>three</span></li></ul>"#
    )
    try update(editor) { _ = try items[0].replace(createParagraphNode()) }
    #expect(
      try listHTML(editor)
        == #"<p></p><ul><li value="1"><span>two</span></li><li value="2"><span>three</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > last list item with a non list item node"
    ))
  func replaceLastWithParagraph() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[2].replace(createParagraphNode()) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li></ul><p></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > middle list item with a non list item node"
    ))
  func replaceMiddleWithParagraph() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[1].replace(createParagraphNode()) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li></ul><p></p><ul><li value="1"><span>three</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.replace() > the only list item with a non list item node"
    ))
  func replaceOnlyWithParagraph() throws {
    let (editor, items) = try threeItems()
    try update(editor) {
      try items[1].remove()
      try items[2].remove()
    }
    #expect(try listHTML(editor) == #"<ul><li value="1"><span>one</span></li></ul>"#)
    try update(editor) { _ = try items[0].replace(createParagraphNode()) }
    #expect(try listHTML(editor) == "<p></p>")
  }

  // MARK: ListItemNode.remove()

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > siblings are not nested"
    ))
  func removeSiblingsNotNested() throws {
    try removeX(
      { try li("A") }, { try li("B") },
      initially:
        #"<ul><li value="1"><span>A</span></li><li value="2"><span>x</span></li><li value="3"><span>B</span></li></ul>"#,
      then: #"<ul><li value="1"><span>A</span></li><li value="2"><span>B</span></li></ul>"#)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the previous sibling is nested"
    ))
  func removePreviousNested() throws {
    try removeX(
      { try nested("bullet", li("A")) }, { try li("B") },
      initially:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li></ul></li><li value="1"><span>x</span></li><li value="2"><span>B</span></li></ul>"#,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li></ul></li><li value="1"><span>B</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the next sibling is nested"
    ))
  func removeNextNested() throws {
    try removeX(
      { try li("A") }, { try nested("bullet", li("B")) },
      initially:
        #"<ul><li value="1"><span>A</span></li><li value="2"><span>x</span></li><li value="3"><ul><li value="1"><span>B</span></li></ul></li></ul>"#,
      then:
        #"<ul><li value="1"><span>A</span></li><li value="2"><ul><li value="1"><span>B</span></li></ul></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are nested"
    ))
  func removeBothNested() throws {
    try removeX(
      { try nested("bullet", li("A")) }, { try nested("bullet", li("B")) },
      initially:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li></ul></li><li value="1"><span>x</span></li><li value="2"><ul><li value="1"><span>B</span></li></ul></li></ul>"#,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li><li value="2"><span>B</span></li></ul></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are nested with a different listType"
    ))
  func removeBothNestedDifferentType() throws {
    try removeX(
      { try nested("bullet", li("A")) }, { try nested("number", li("B")) },
      initially: nil,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li></ul></li><li value="1"><ol><li value="1"><span>B</span></li></ol></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the previous sibling is nested deeper than the next sibling"
    ))
  func removePreviousDeeper() throws {
    try removeX(
      { try nested("bullet", li("A1"), nested("bullet", li("A2"))) },
      { try nested("bullet", li("B")) },
      initially:
        #"<ul><li value="1"><ul><li value="1"><span>A1</span></li><li value="2"><ul><li value="1"><span>A2</span></li></ul></li></ul></li><li value="1"><span>x</span></li><li value="2"><ul><li value="1"><span>B</span></li></ul></li></ul>"#,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A1</span></li><li value="2"><ul><li value="1"><span>A2</span></li></ul></li><li value="2"><span>B</span></li></ul></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > the next sibling is nested deeper than the previous sibling"
    ))
  func removeNextDeeper() throws {
    try removeX(
      { try nested("bullet", li("A")) },
      { try nested("bullet", nested("bullet", li("B1")), li("B2")) },
      initially:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li></ul></li><li value="1"><span>x</span></li><li value="2"><ul><li value="1"><ul><li value="1"><span>B1</span></li></ul></li><li value="1"><span>B2</span></li></ul></li></ul>"#,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A</span></li><li value="2"><ul><li value="1"><span>B1</span></li></ul></li><li value="2"><span>B2</span></li></ul></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.remove() > both siblings are deeply nested"
    ))
  func removeBothDeeplyNested() throws {
    try removeX(
      { try nested("bullet", li("A1"), nested("bullet", li("A2"))) },
      { try nested("bullet", nested("bullet", li("B1")), li("B2")) },
      initially:
        #"<ul><li value="1"><ul><li value="1"><span>A1</span></li><li value="2"><ul><li value="1"><span>A2</span></li></ul></li></ul></li><li value="1"><span>x</span></li><li value="2"><ul><li value="1"><ul><li value="1"><span>B1</span></li></ul></li><li value="1"><span>B2</span></li></ul></li></ul>"#,
      then:
        #"<ul><li value="1"><ul><li value="1"><span>A1</span></li><li value="2"><ul><li value="1"><span>A2</span></li><li value="2"><span>B1</span></li></ul></li><li value="2"><span>B2</span></li></ul></li></ul>"#
    )
  }

  // MARK: ListItemNode.insertNewAfter(): non-empty list items

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > first list item"
    ))
  func insertNewAfterFirst() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[0].insertNewAfter(createRangeSelection()) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"></li><li value="3"><span>two</span></li><li value="4"><span>three</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > last list item"
    ))
  func insertNewAfterLast() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[2].insertNewAfter(createRangeSelection()) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li><li value="3"><span>three</span></li><li value="4"></li></ul>"#
    )
  }

  // Upstream inserts after the third item here too.
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > middle list item"
    ))
  func insertNewAfterMiddle() throws {
    let (editor, items) = try threeItems()
    try update(editor) { _ = try items[2].insertNewAfter(createRangeSelection()) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li><li value="3"><span>three</span></li><li value="4"></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.insertNewAfter(): non-empty list items > the only list item"
    ))
  func insertNewAfterOnly() throws {
    let (editor, items) = try threeItems()
    try update(editor) {
      try items[1].remove()
      try items[2].remove()
    }
    #expect(try listHTML(editor) == #"<ul><li value="1"><span>one</span></li></ul>"#)
    try update(editor) { _ = try items[0].insertNewAfter(createRangeSelection()) }
    #expect(
      try listHTML(editor) == #"<ul><li value="1"><span>one</span></li><li value="2"></li></ul>"#)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > $createListItemNode()"
    ))
  func createListItemNodeFunction() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItemNode = try ListItemNode()
      let createdListItemNode = try createListItemNode()
      #expect(listItemNode.__type == createdListItemNode.__type)
      #expect(listItemNode.__parent == createdListItemNode.__parent)
      #expect(listItemNode.__key != createdListItemNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > $isListItemNode()"
    ))
  func isListItemNodeFunction() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) { () throws in #expect(isListItemNode(try ListItemNode())) }
  }

  // MARK: ListItemNode.setIndent()

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.setIndent() > indents and outdents list item"
    ))
  func setIndent() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    var one: ListItemNode!
    try update(editor) {
      one = try li("one")
      _ = try getRoot().append(list("bullet", one, li("two")))
    }
    try update(editor) { _ = try one.setIndent(3) }
    try update(editor) { () throws in #expect(try one.getIndent() == 3) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><ul><li value="1"><ul><li value="1"><ul><li value="1"><span>one</span></li></ul></li></ul></li></ul></li><li value="1"><span>two</span></li></ul>"#
    )
    try update(editor) { _ = try one.setIndent(0) }
    try update(editor) { () throws in #expect(try one.getIndent() == 0) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>one</span></li><li value="2"><span>two</span></li></ul>"#)
  }

  // `setIndent(0.5)` truncates upstream; Swift's indent is an Int, so the
  // fraction cannot be passed, and 0 is what upstream ends with.
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode.setIndent() > handles fractional indent values"
    ))
  func setIndentFractional() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    var one: ListItemNode!
    try update(editor) {
      one = try li("one")
      _ = try getRoot().append(list("bullet", one, li("two")))
    }
    try update(editor) { _ = try one.setIndent(Int(0.5)) }
    try update(editor) { () throws in #expect(try one.getIndent() == 0) }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Can serialize a node that is not attached"
    ))
  func serializeUnattached() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItemNode = try createListItemNode()
      _ = try createListNode().append(listItemNode)
      #expect(
        try listItemNode.exportJSON().stringify()
          == #"{"children":[],"indent":0,"value":1,"direction":null,"format":"","type":"listitem","version":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode marker style inheritance on indent"
    ))
  func markerStyleInheritedOnIndent() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listItem1 = try createListItemNode()
      let listItem2 = try createListItemNode()
      _ = try listItem2.setTextStyle("font-size: 19px;")
      _ = try getRoot().append(createListNode("bullet").append(listItem1, listItem2))
      try handleIndent(listItem2)
      let parentListItem = try listItem2.getParentOrThrow().getParentOrThrow()
      #expect(isListItemNode(parentListItem))
      #expect(try (parentListItem as! ListItemNode).getTextStyle() == "font-size: 19px;")
    }
  }

  // MARK: Splitting a numbered list

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Default: Splitting a list resets numbering to 1 (Backward Compatibility)"
    ))
  func splitResetsNumbering() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let emptyItem = try createListItemNode()
      _ = try getRoot().append(list("number", li("A"), li("B"), emptyItem, li("C")))
      _ = try emptyItem.select()
    }
    try update(editor) { _ = try handleListInsertParagraph() }
    #expect(
      try listHTML(editor)
        == #"<ol><li value="1"><span>A</span></li><li value="2"><span>B</span></li></ol><p></p><ol><li value="1"><span>C</span></li></ol>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Option Enabled: Splitting a list preserves numbering (Smart Behavior)"
    ))
  func splitPreservesNumbering() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let emptyItem = try createListItemNode()
      _ = try getRoot().append(list("number", li("A"), li("B"), emptyItem, li("C")))
      _ = try emptyItem.select()
    }
    try update(editor) { _ = try handleListInsertParagraph(true) }
    #expect(
      try listHTML(editor)
        == #"<ol><li value="1"><span>A</span></li><li value="2"><span>B</span></li></ol><p></p><ol start="3"><li value="3"><span>C</span></li></ol>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > Option Enabled: Preserves numbering when a nested sublist precedes the split"
    ))
  func splitPreservesNumberingAfterNestedSublist() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let emptyItem = try createListItemNode()
      _ = try getRoot().append(
        list("number", li("A"), nested("bullet", li("A.1")), li("B"), emptyItem, li("C")))
      _ = try emptyItem.select()
    }
    try update(editor) { _ = try handleListInsertParagraph(true) }
    try editor.read(.latest) { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      #expect(isListNode(children[0]))
      #expect(isParagraphNode(children[1]))
      let secondList = try #require(children[2] as? ListNode)
      #expect(try secondList.getStart() == 3)
      let firstItem = try #require(try secondList.getFirstChild() as? ListItemNode)
      #expect(try firstItem.getValue() == 3)
    }
  }

  // MARK: ListItemNode $transform wraps orphan ListItemNodes

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps a single orphan ListItemNode under root in a ListNode"
    ))
  func wrapsSingleOrphan() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try editor.update(discrete: true) { _ = try getRoot().append(li("orphan")) }
    try editor.read { () throws -> Void in
      let wrapper = try #require(try getRoot().getFirstChild() as? ListNode)
      #expect(try wrapper.getListType() == "bullet")
    }
    #expect(try listHTML(editor) == #"<ul><li value="1"><span>orphan</span></li></ul>"#)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps adjacent orphan ListItemNodes in a single ListNode"
    ))
  func wrapsAdjacentOrphans() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try editor.update(discrete: true) { _ = try getRoot().append(li("a"), li("b"), li("c")) }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>a</span></li><li value="2"><span>b</span></li><li value="3"><span>c</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > does not merge orphan ListItemNodes separated by a ParagraphNode"
    ))
  func orphansSeparatedByParagraph() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try editor.update(discrete: true) {
      _ = try getRoot().append(
        li("a"), createParagraphNode().append(createTextNode("middle")), li("b"))
    }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>a</span></li></ul><p><span>middle</span></p><ul><li value="1"><span>b</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > parses HTML with orphan <li> outside of <ul>"
    ))
  func parsesOrphanLIFromHTML() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    let input = """
      <div>
        <ul>
          <li>test</li>
          <li>test</li>
          <li>test</li>
        </ul>
        hello world
        <li>test</li>
      </div>
      """
    try editor.update(discrete: true) {
      _ = try getRoot().clear().select()
      let dom = DOMParser().parseFromString(input, "text/html")
      try insertNodes(generateNodesFromDOM(editor, dom))
    }
    #expect(
      try listHTML(editor)
        == #"<ul><li value="1"><span>test</span></li><li value="2"><span>test</span></li><li value="3"><span>test</span></li></ul><p><span>hello world</span></p><ul><li value="1"><span>test</span></li></ul>"#
    )
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > preserves the selection on the orphan when it is wrapped"
    ))
  func preservesSelectionOnOrphan() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try editor.update(discrete: true) {
      let text = try createTextNode("orphan")
      _ = try getRoot().clear().append(
        createParagraphNode().append(createTextNode("hello world")), li(text))
      _ = try text.select(2, 4)
    }
    try expectSelection(editor, "orphan", 2, 4)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > preserves a selection anchored outside the orphan"
    ))
  func preservesSelectionOutsideOrphan() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try editor.update(discrete: true) {
      let helloText = try createTextNode("hello world")
      _ = try getRoot().clear().append(createParagraphNode().append(helloText), li("orphan"))
      _ = try helloText.select(3, 5)
    }
    try expectSelection(editor, "hello world", 3, 5)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with no siblings"
    ))
  func wrapsOrphanInParagraphAlone() throws {
    #expect(try orphanInParagraph { [try li("item")] } == ["list:item"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with prev siblings"
    ))
  func wrapsOrphanInParagraphAfterText() throws {
    #expect(
      try orphanInParagraph { [try createTextNode("before"), try li("item")] }
        == ["paragraph:before", "list:item"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with next siblings"
    ))
  func wrapsOrphanInParagraphBeforeText() throws {
    #expect(
      try orphanInParagraph { [try li("item"), try createTextNode("after")] }
        == ["list:item", "paragraph:after"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListItemNode.test.ts::LexicalListItemNode tests > ListItemNode $transform wraps orphan ListItemNodes > wraps orphan ListItemNode inside a paragraph with both siblings"
    ))
  func wrapsOrphanInParagraphBetweenText() throws {
    #expect(
      try orphanInParagraph {
        [try createTextNode("before"), try li("item"), try createTextNode("after")]
      }
        == ["paragraph:before", "list:item", "paragraph:after"])
  }
}

private func expectSelection(_ editor: Editor, _ text: String, _ anchor: Int, _ focus: Int) throws {
  try editor.read { () throws -> Void in
    let selection = try #require(try getSelection() as? RangeSelection)
    #expect(try selection.anchor.getNode().getTextContent() == text)
    #expect(selection.anchor.offset == anchor)
    #expect(try selection.focus.getNode().getTextContent() == text)
    #expect(selection.focus.offset == focus)
  }
}

/// The root's children after a paragraph of `children` is committed, each as
/// `type:text`; a list must hold exactly one item.
private func orphanInParagraph(
  _ children: @escaping () throws -> [LexicalNode]
) throws
  -> [String]
{
  let editor = try createMountedTestEditor(nodes: listTestNodes)
  try editor.update(discrete: true) {
    _ = try getRoot().clear().append(createParagraphNode().append(contentsOf: children()))
  }
  return try editor.read { () throws -> [String] in
    try getRoot().getChildren().map { child throws -> String in
      if let list = child as? ListNode {
        #expect(try list.getChildrenSize() == 1)
        #expect(isListItemNode(try list.getFirstChild()))
      }
      return "\(child.getType()):\(try child.getTextContent())"
    }
  }
}
