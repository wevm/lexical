import Lexical
import LexicalHTML
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/LexicalListNode.test.ts.
// The reconciled HTML upstream compares is asserted through each node's
// createDOM, which builds the same elements.

var listTestNodes: [LexicalNodeConfig] { [.klass(ListNode.self), .klass(ListItemNode.self)] }

private var listTheme: EditorThemeClasses {
  [
    "list": [
      "ol": "my-ol-list-class",
      "olDepth": .array((1...7).map { .string("my-ol-list-class-\($0)") }),
      "ul": "my-ul-list-class",
      "ulDepth": .array((1...7).map { .string("my-ul-list-class-\($0)") }),
    ]
  ]
}

private var editorConfig: EditorConfig { EditorConfig(namespace: "", theme: listTheme) }

@Suite struct LexicalListNodeTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try createListNode("bullet", start: 1)
      #expect(listNode.getType() == "list")
      #expect(try listNode.getTag() == "ul")
      #expect(try listNode.getTextContent() == "")
      #expect(throws: Never.self) { try createListNode() }
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.updateFromJSON normalizes the legacy tag-form listType"
    ))
  func updateFromJSONLegacyListType() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let bullet = try createListNode().updateFromJSON(JSONObject([("listType", "ul")]))
      #expect(try bullet.getListType() == "bullet")
      #expect(try bullet.getTag() == "ul")
      let number = try createListNode("bullet").updateFromJSON(JSONObject([("listType", "ol")]))
      #expect(try number.getListType() == "number")
      #expect(try number.getTag() == "ol")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.getTag()"
    ))
  func getTag() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let tags = try ["bullet", "number", "check"].map { try createListNode($0).getTag() }
      #expect(tags == ["ul", "ol", "ul"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try createListNode("bullet", start: 1)
      #expect(
        try listNode.createDOM(editorConfig).outerHTML
          == #"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#)
      #expect(
        try listNode.createDOM(EditorConfig(namespace: "", theme: ["list": [:]])).outerHTML
          == "<ul></ul>")
      #expect(try listNode.createDOM(EditorConfig(namespace: "")).outerHTML == "<ul></ul>")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.createDOM() correctly applies classes to a nested ListNode"
    ))
  func createDOMNested() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let lists = try (0..<7).map { _ in try createListNode("bullet") }
      let items = try (0..<4).map { _ in try createListItemNode() }
      try lists[0].append(items[0])
      try items[0].append(lists[1])
      try lists[1].append(items[1])
      try items[1].append(lists[2])
      try lists[2].append(items[2])
      try items[2].append(lists[3])
      try lists[3].append(items[3])
      try lists[3].append(lists[4])
      try lists[4].append(lists[5])
      try lists[5].append(lists[6])
      #expect(
        try lists[0].createDOM(editorConfig).outerHTML
          == #"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#)
      #expect(
        try lists[0].createDOM(EditorConfig(namespace: "", theme: ["list": [:]])).outerHTML
          == "<ul></ul>")
      #expect(try lists[0].createDOM(EditorConfig(namespace: "")).outerHTML == "<ul></ul>")
      for depth in 2...7 {
        #expect(
          try lists[depth - 1].createDOM(editorConfig).outerHTML
            == #"<ul class="my-ul-list-class my-ul-list-class-\#(depth)"></ul>"#)
      }
      var theme = listTheme
      var list = theme["list"]!.objectValue!
      list["ulDepth"] = .array((1...3).map { .string("my-ul-list-class-\($0)") })
      theme["list"] = .object(list)
      #expect(
        try lists[4].createDOM(EditorConfig(namespace: "", theme: theme)).outerHTML
          == #"<ul class="my-ul-list-class my-ul-list-class-2"></ul>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.updateDOM()"
    ))
  func updateDOM() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try createListNode("bullet", start: 1)
      let domElement = try listNode.createDOM(editorConfig)
      #expect(domElement.outerHTML == #"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#)
      let newListNode = try createListNode("number", start: 1)
      #expect(try newListNode.updateDOM(listNode, domElement, editorConfig) == true)
      #expect(domElement.outerHTML == #"<ul class="my-ul-list-class my-ul-list-class-1"></ul>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.exportDOM() round-trips the dir attribute"
    ))
  func exportDOMDir() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    let document = DOMParser().parseFromString(#"<ul dir="rtl"><li dir="rtl">שלום</li></ul>"#)
    try editor.update(discrete: true) {
      try getRoot().clear().append(contentsOf: generateNodesFromDOM(editor, document))
    }
    try editor.read {
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      #expect(try listNode.getDirection() == "rtl")
      #expect(try generateHtmlFromNodes(editor).contains(#"<ul dir="rtl">"#))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ListItemNode"
    ))
  func appendListItem() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try ListNode("bullet", start: 1)
      let listItemNode = try ListItemNode()
      try listItemNode.append(TextNode("Hello"))
      #expect(try listNode.append(listItemNode) === listNode)
      #expect(try listNode.getFirstChild() === listItemNode)
      #expect(try listNode.getFirstChild()?.getTextContent() == "Hello")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ListNode"
    ))
  func appendList() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try ListNode("bullet", start: 1)
      let nestedListNode = try ListNode("bullet", start: 1)
      let listItemNode = try ListItemNode()
      try listItemNode.append(TextNode("Hello"))
      try nestedListNode.append(listItemNode)
      #expect(try listNode.append(nestedListNode) === listNode)
      let first = try #require(try listNode.getFirstChild() as? ListItemNode)
      #expect(try first.getFirstChild() === nestedListNode)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.append() should properly transform a ParagraphNode"
    ))
  func appendParagraph() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try ListNode("bullet", start: 1)
      let paragraph = try ParagraphNode()
      try paragraph.append(TextNode("Hello"))
      #expect(try listNode.append(paragraph) === listNode)
      #expect(try isListItemNode(listNode.getFirstChild()))
      #expect(try listNode.getFirstChild()?.getTextContent() == "Hello")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > ListNode.splice() should wrap multiple non-ListItem nodes in individual ListItem nodes"
    ))
  func spliceWrapsEach() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let list = try createListNode("bullet").append(
        createListItemNode().append(createTextNode("A")),
        createListItemNode().append(createTextNode("D")))
      try getRoot().append(list)
      try list.splice(1, 0, [createTextNode("B"), createTextNode("C")])
    }
    try editor.read {
      let list = try #require(try getRoot().getFirstChild() as? ListNode)
      let children = try list.getChildren()
      #expect(children.count == 4)
      #expect(isListItemNode(children[1]))
      #expect(isListItemNode(children[2]))
      #expect(children[1] !== children[2])
      #expect(try children[1].getTextContent() == "B")
      #expect(try children[2].getTextContent() == "C")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > Should update list children when switching from checklist to bullet"
    ))
  func checklistToBullet() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      try getRoot().clear().append(createListNode("check").append(createListItemNode()))
    }
    #expect(
      try itemsHTML(editor)
        == [#"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#])
    try update(editor) {
      try #require(try getRoot().getFirstChild() as? ListNode).setListType("bullet")
    }
    #expect(try itemsHTML(editor) == [#"<li value="1"></li>"#])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > Should clear checklist attributes when nesting lists"
    ))
  func nestingClearsChecklistAttributes() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      try getRoot().clear().append(
        createListNode("check").append(createListItemNode(), createListItemNode()))
    }
    #expect(
      try itemsHTML(editor) == [
        #"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#,
        #"<li role="checkbox" tabindex="-1" aria-checked="false" value="2"></li>"#,
      ])
    try update(editor) {
      let listNode = try #require(try getRoot().getFirstChild() as? ListNode)
      let listItemNode = try #require(try listNode.getChildAtIndex(1) as? ListItemNode)
      try listItemNode.append(createListNode("bullet").append(createListItemNode()))
    }
    #expect(
      try itemsHTML(editor) == [
        #"<li role="checkbox" tabindex="-1" aria-checked="false" value="1"></li>"#,
        #"<li value="2"></li>"#, #"<li value="1"></li>"#,
      ])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $createListNode()"
    ))
  func createListNodeFunction() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try createListNode("bullet", start: 1)
      let createdListNode = try createListNode("bullet")
      #expect(listNode.getType() == createdListNode.getType())
      #expect(try listNode.getParent() === createdListNode.getParent())
      #expect(listNode.__tag == createdListNode.__tag)
      #expect(listNode.getKey() != createdListNode.getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $isListNode()"
    ))
  func isListNodeFunction() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let listNode = try createListNode("bullet", start: 1)
      #expect(isListNode(listNode))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode tests > $createListNode() with tag name (backward compatibility)"
    ))
  func createListNodeListType() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let numberList = try createListNode("number", start: 1)
      let bulletList = try createListNode("bullet", start: 1)
      #expect(numberList.__listType == "number")
      #expect(bulletList.__listType == "bullet")
    }
  }
}

/// Each list item in document order, as its createDOM builds it.
private func itemsHTML(_ editor: Editor) throws -> [String] {
  try editor.read {
    try allItems(getRoot()).map { try $0.createDOM(editor._config).outerHTML }
  }
}

private func allItems(_ node: ElementNode) throws -> [ListItemNode] {
  var items: [ListItemNode] = []
  for child in try node.getChildren() {
    if let item = child as? ListItemNode { items.append(item) }
    if let element = child as? ElementNode { items += try allItems(element) }
  }
  return items
}

// MARK: Subclassing

private final class ListNodeConfig: ListNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(ListNodeConfig.self, type: "list-$config", extends: ListNode.self)
  }
}

private final class ListNodeSubclass: ListNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(ListNodeSubclass.self, type: "list-subclass", extends: ListNode.self)
  }
}

private func initialState(_ makeList: () throws -> ListNode) throws {
  try getRoot().clear().append(
    makeList().append(
      createListItemNode().append(createTextNode("first")),
      createListItemNode().append(createTextNode("second"))))
}

private func listItemValues() throws -> [Int?] {
  try getRoot().getAllTextNodes().map { try ($0.getParent() as? ListItemNode)?.getValue() }
}

@Suite struct LexicalListNodeSubclassingTests {
  func check(_ nodes: [LexicalNodeConfig], _ makeList: @escaping () throws -> ListNode) throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes + nodes)
    try editor.update(discrete: true) {
      try initialState(makeList)
      #expect(try listItemValues() == [1, 1])
    }
    #expect(try editor.read { try listItemValues() } == [1, 2])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode subclassing tests ($config) > ListNode as-is > applies transform"
    ))
  func asIs() throws {
    try check([]) { try createListNode() }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode subclassing tests ($config) > ListNodeConfig (no replacement) > applies transform"
    ))
  func configSubclass() throws {
    try check([.klass(ListNodeConfig.self)]) { try ListNodeConfig() }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalListNode.test.ts::LexicalListNode subclassing tests ($config) > ListNodeSubclass (no replacement) > applies transform"
    ))
  func subclass() throws {
    try check([.klass(ListNodeSubclass.self)]) { try ListNodeSubclass() }
  }
}
