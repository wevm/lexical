import Lexical
import LexicalTesting
import Testing

// Port of nodes/__tests__/unit/LexicalElementNode.test.tsx, except the
// getDOMSlot and indexPath tests, which are dom-only.

/// A text node whose `getTextContentSize()` is overridden to return 1, which
/// upstream does by assigning the method on one instance.
final class SizeOneTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SizeOneTextNode.self, type: "size-one-text", extends: TextNode.self)
  }
  override func getTextContentSize() throws -> Int { 1 }
}

/// `init()`: a test block holding "Foo", a segmented "Bar" and "Baz", with
/// the caret at the start of "Foo".
private func elementNodeEditor() throws -> Editor {
  let editor = try createTestEditor(nodes: [.klass(SizeOneTextNode.self)])
  try update(editor) { () throws in
    let block = try createTestElementNode()
    let text = try createTextNode("Foo")
    let text2 = try createTextNode("Bar")
    try text2.setMode("segmented")
    let text3 = try createTextNode("Baz")
    try text.select(0, 0)
    try block.append(text, text2, text3)
    try getRoot().append(block)
  }
  return editor
}

/// The test block of `init()`.
private func firstBlock() throws -> ElementNode {
  try #require(try getRoot().getFirstChild() as? ElementNode)
}

/// "a" in bold and "b" styled `color:green;`, in a paragraph.
private func initialState() throws {
  try getRoot().clear().append(
    createParagraphNode().append(
      createTextNode("a").toggleFormat("bold"),
      createTextNode("b").setStyle("color:green;")))
}

struct SpliceCase: Sendable, CustomTestStringConvertible {
  let name: String
  let start: Int
  let deleteCount: Int
  let deleteOnly: Bool
  let expectedText: String
  var testDescription: String { name }

  static let plainText: [SpliceCase] = [
    SpliceCase(
      name: "Do nothing", start: 0, deleteCount: 0, deleteOnly: true, expectedText: "FooBarBaz"),
    SpliceCase(
      name: "Insert in the beginning", start: 0, deleteCount: 0, deleteOnly: false,
      expectedText: "QuxQuuzFooBarBaz"),
    SpliceCase(
      name: "Insert in the middle", start: 1, deleteCount: 0, deleteOnly: false,
      expectedText: "FooQuxQuuzBarBaz"),
    SpliceCase(
      name: "Insert in the end", start: 3, deleteCount: 0, deleteOnly: false,
      expectedText: "FooBarBazQuxQuuz"),
    SpliceCase(
      name: "Delete in the beginning", start: 0, deleteCount: 1, deleteOnly: true,
      expectedText: "BarBaz"),
    SpliceCase(
      name: "Delete in the middle", start: 1, deleteCount: 1, deleteOnly: true,
      expectedText: "FooBaz"),
    SpliceCase(
      name: "Delete in the end", start: 2, deleteCount: 1, deleteOnly: true,
      expectedText: "FooBar"),
    SpliceCase(name: "Delete all", start: 0, deleteCount: 3, deleteOnly: true, expectedText: ""),
    SpliceCase(
      name: "Replace in the beginning", start: 0, deleteCount: 1, deleteOnly: false,
      expectedText: "QuxQuuzBarBaz"),
    SpliceCase(
      name: "Replace in the middle", start: 1, deleteCount: 1, deleteOnly: false,
      expectedText: "FooQuxQuuzBaz"),
    SpliceCase(
      name: "Replace in the end", start: 2, deleteCount: 1, deleteOnly: false,
      expectedText: "FooBarQuxQuuz"),
    SpliceCase(
      name: "Replace all", start: 0, deleteCount: 3, deleteOnly: false, expectedText: "QuxQuuz"),
  ]
}

/// The nodes of the nested-elements splice cases, by name.
enum NestedNode: String, Sendable {
  case text1, nestedText1, nestedBlock2, nestedText2, block
}

struct NestedSpliceCase: Sendable, CustomTestStringConvertible {
  let name: String
  let start: Int
  let deleteCount: Int
  let deleteOnly: Bool
  let expectedText: String
  /// Where both points end up: the node, the offset and the point type.
  let expected: (NestedNode, Int, String)
  var testDescription: String { name }

  static let all: [NestedSpliceCase] = [
    NestedSpliceCase(
      name: "Do nothing", start: 1, deleteCount: 0, deleteOnly: true,
      expectedText: "FooWiz\n\nFuz\n\nBar", expected: (.nestedText1, 1, "text")),
    NestedSpliceCase(
      name: "Delete selected element (selection moves to the previous)", start: 1,
      deleteCount: 1, deleteOnly: true, expectedText: "FooFuz\n\nBar",
      expected: (.text1, 3, "text")),
    NestedSpliceCase(
      name: "Replace selected element (selection moves to the previous)", start: 1,
      deleteCount: 1, deleteOnly: false, expectedText: "FooQuxQuuzFuz\n\nBar",
      expected: (.text1, 3, "text")),
    NestedSpliceCase(
      name: "Delete selected with previous element (selection moves to the next)", start: 0,
      deleteCount: 2, deleteOnly: true, expectedText: "Fuz\n\nBar",
      // Upstream expects nestedText2:0:text, which the selection becomes
      // only after a round trip through the DOM selection (applyDOMRange
      // normalizes it). The headless JS editor leaves the point that splice
      // sets, nestedBlock2:0:element, which is what the model does here.
      expected: (.nestedBlock2, 0, "element")),
    NestedSpliceCase(
      name: "Delete selected with all siblings (selection moves up to the element)", start: 0,
      deleteCount: 4, deleteOnly: true, expectedText: "", expected: (.block, 0, "element")),
  ]
}

@Suite struct LexicalElementNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > should return and object conforming to the expected schema"
    ))
  func exportJSONSchema() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let json = try createTestElementNode().exportJSON()
      #expect(
        json.stringify()
          == #"{"children":[],"direction":null,"format":"","indent":0,"type":"test_block","version":1}"#
      )
      #expect(json["textFormat"] == nil)
      #expect(json["textStyle"] == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > serializes only the first TextNode style and format"
    ))
  func exportJSONFirstTextFormat() throws {
    let editor = try elementNodeEditor()
    try update(editor, initialState)
    try editor.read {
      let root = try #require(editor.toJSON()["editorState"]?.objectValue?["root"]?.objectValue)
      let paragraph = try #require(root["children"]?.arrayValue?.first)
      #expect(
        paragraph.stringify()
          == #"{"children":[{"detail":0,"format":1,"mode":"normal","style":"","text":"a","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"color:green;","text":"b","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"","type":"paragraph","version":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > exportJSON() > serializes the same way without a root element"
    ))
  func exportJSONHeadless() throws {
    let editor = try elementNodeEditor()
    try update(editor, initialState)
    let headless = try createEditor()
    try headless.update(discrete: true, initialState)
    #expect(try headless.toJSON() == editor.toJSON())
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getChildren() > no children"
    ))
  func getChildrenNone() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try createTestElementNode().getChildren().isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getChildren() > some children"
    ))
  func getChildrenSome() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try firstBlock().getChildren().count == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getAllTextNodes() > basic"
    ))
  func getAllTextNodesBasic() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try firstBlock().getAllTextNodes().count == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getAllTextNodes() > nested"
    ))
  func getAllTextNodesNested() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try createTestElementNode()
      let innerBlock = try createTestElementNode()
      let text = try createTextNode("Foo")
      try text.select(0, 0)
      let text2 = try createTextNode("Bar")
      let text3 = try createTextNode("Baz")
      let text4 = try createTextNode("Qux")
      try block.append(text, innerBlock, text4)
      try innerBlock.append(text2, text3)
      #expect(
        try block.getAllTextNodes().map(ObjectIdentifier.init)
          == [text, text2, text3, text4].map(ObjectIdentifier.init))
      let innerInnerBlock = try createTestElementNode()
      let text5 = try createTextNode("More")
      let text6 = try createTextNode("Stuff")
      try innerInnerBlock.append(text5, text6)
      try innerBlock.append(innerInnerBlock)
      #expect(
        try block.getAllTextNodes().map(ObjectIdentifier.init)
          == [text, text2, text3, text5, text6, text4].map(ObjectIdentifier.init))
      try getRoot().append(block)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getFirstChild() > basic"
    ))
  func getFirstChildBasic() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try firstBlock().getFirstChild()?.getTextContent() == "Foo")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getFirstChild() > empty"
    ))
  func getFirstChildEmpty() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try createTestElementNode().getFirstChild() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getLastChild() > basic"
    ))
  func getLastChildBasic() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try firstBlock().getLastChild()?.getTextContent() == "Baz")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getLastChild() > empty"
    ))
  func getLastChildEmpty() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try createTestElementNode().getLastChild() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > basic"
    ))
  func getTextContentBasic() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try firstBlock().getTextContent() == "FooBarBaz")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > empty"
    ))
  func getTextContentEmpty() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      #expect(try createTestElementNode().getTextContent() == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContent() > nested"
    ))
  func getTextContentNested() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try createTestElementNode()
      let innerBlock = try createTestElementNode()
      let text = try createTextNode("Foo")
      try text.select(0, 0)
      let text2 = try createTextNode("Bar")
      let text3 = try createTextNode("Baz")
      try text3.setMode("token")
      let text4 = try createTextNode("Qux")
      try block.append(text, innerBlock, text4)
      try innerBlock.append(text2, text3)
      #expect(try block.getTextContent() == "FooBarBaz\n\nQux")
      let innerInnerBlock = try createTestElementNode()
      let text5 = try createTextNode("More")
      try text5.setMode("token")
      let text6 = try createTextNode("Stuff")
      try innerInnerBlock.append(text5, text6)
      try innerBlock.append(innerInnerBlock)
      #expect(try block.getTextContent() == "FooBarBazMoreStuff\n\nQux")
      try getRoot().append(block)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContentSize() > basic"
    ))
  func getTextContentSizeBasic() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try firstBlock()
      #expect(try block.getTextContentSize() == block.getTextContent().utf16.count)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > getTextContentSize() > child node getTextContentSize() can be overridden and is then reflected when calling the same method on parent node"
    ))
  func getTextContentSizeOverridden() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try createTestElementNode()
      let text = try SizeOneTextNode()
      try text.setTextContent("Foo")
      try block.append(text)
      #expect(try block.getTextContentSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Do nothing"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the beginning"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the middle"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Insert in the end"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the beginning"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the middle"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete in the end"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Delete all"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the beginning"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the middle"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace in the end"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Plain text: Replace all"
    ),
    arguments: SpliceCase.plainText)
  func splicePlainText(_ testCase: SpliceCase) throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try firstBlock()
      try block.splice(
        testCase.start, testCase.deleteCount,
        testCase.deleteOnly ? [] : [createTextNode("Qux"), createTextNode("Quuz")])
      #expect(try block.getTextContent() == testCase.expectedText)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Do nothing"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Delete selected element (selection moves to the previous)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Replace selected element (selection moves to the previous)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Delete selected with previous element (selection moves to the next)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Nested elements: Delete selected with all siblings (selection moves up to the element)"
    ),
    arguments: NestedSpliceCase.all)
  func spliceNestedElements(_ testCase: NestedSpliceCase) throws {
    let editor = try elementNodeEditor()
    var keys: [NestedNode: NodeKey] = [:]
    try update(editor) { () throws in
      let block = try firstBlock()
      let text1 = try createTextNode("Foo")
      let text2 = try createTextNode("Bar")
      let nestedBlock1 = try createTestElementNode()
      let nestedText1 = try createTextNode("Wiz")
      try nestedBlock1.append(nestedText1)
      let nestedBlock2 = try createTestElementNode()
      let nestedText2 = try createTextNode("Fuz")
      try nestedBlock2.append(nestedText2)
      try block.clear()
      try block.append(text1, nestedBlock1, nestedBlock2, text2)
      try nestedText1.select(1, 1)
      #expect(try block.getTextContent() == "FooWiz\n\nFuz\n\nBar")
      keys = [
        .text1: text1.getKey(), .nestedText1: nestedText1.getKey(),
        .nestedBlock2: nestedBlock2.getKey(), .nestedText2: nestedText2.getKey(),
        .block: block.getKey(),
      ]
    }
    try update(editor) { () throws in
      try firstBlock().splice(
        testCase.start, testCase.deleteCount,
        testCase.deleteOnly ? [] : [createTextNode("Qux"), createTextNode("Quuz")])
    }
    try update(editor) { () throws in
      #expect(try firstBlock().getTextContent() == testCase.expectedText)
      guard let selection = try getSelection() as? RangeSelection else { return }
      let (node, offset, type) = testCase.expected
      for point in [selection.anchor, selection.focus] {
        #expect(point.key == keys[node])
        #expect(point.offset == offset)
        #expect(point.type.rawValue == type)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Running transforms for inserted nodes, their previous siblings and new siblings"
    ))
  func spliceRunsTransforms() throws {
    let editor = try elementNodeEditor()
    var transforms: Set<NodeKey> = []
    var expectedTransforms: [NodeKey] = []
    let removeTransform = try editor.registerNodeTransform(TextNode.self) { node in
      transforms.insert(node.getKey())
    }
    try update(editor) { () throws in
      let block = try firstBlock()
      let anotherBlock = try createTestElementNode()
      let text1 = try createTextNode("1")
      let text2 = try createTextNode("2")
      try text2.setMode("segmented")
      let text3 = try createTextNode("3")
      try anotherBlock.append(text1, text2, text3)
      try getRoot().append(anotherBlock)
      expectedTransforms += [
        text1.getKey(), text2.getKey(), text3.getKey(),
        try #require(try block.getChildAtIndex(0)).getKey(),
        try #require(try block.getChildAtIndex(1)).getKey(),
      ]
    }
    try update(editor) { () throws in
      let last = try #require(try getRoot().getLastChild() as? ElementNode)
      try firstBlock().splice(1, 0, [try #require(try last.getChildAtIndex(1))])
    }
    removeTransform()
    try update(editor) { () throws in
      #expect(try firstBlock().getTextContent() == "Foo2BarBaz")
      for key in expectedTransforms {
        #expect(transforms.contains(key))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Re-inserting the node after the range does not create a cycle"
    ))
  func spliceReinsertNoCycle() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try firstBlock()
      let children = try block.getChildren()
      let (first, second, third) = (children[0], children[1], children[2])
      try block.splice(1, 0, [second])
      #expect(try second.getNextSibling()?.getKey() == third.getKey())
      #expect(try block.getChildrenSize() == 3)
      #expect(try block.getChildrenKeys() == [first.getKey(), second.getKey(), third.getKey()])
      #expect(try block.getLastChild()?.getKey() == third.getKey())
      #expect(try block.getTextContent() == "FooBarBaz")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalElementNode.test.tsx::LexicalElementNode tests > splice > Replacing a node with a later sibling does not create a cycle"
    ))
  func spliceReplaceWithLaterSibling() throws {
    let editor = try elementNodeEditor()
    try update(editor) { () throws in
      let block = try firstBlock()
      let children = try block.getChildren()
      let (first, third) = (children[0], children[2])
      try block.splice(1, 1, [third])
      #expect(try third.getNextSibling() == nil)
      #expect(try block.getChildrenSize() == 2)
      #expect(try block.getChildrenKeys() == [first.getKey(), third.getKey()])
      #expect(try block.getLastChild()?.getKey() == third.getKey())
      #expect(try block.getTextContent() == "FooBaz")
    }
  }
}
