import LexicalTesting
import Testing

@testable import Lexical

// Port of nodes/__tests__/unit/LexicalRootNode.test.ts, except createDOM and
// updateDOM, which are dom-only. The __cachedText tests mount the editor:
// the reconciler is what fills the cache.

/// `expectRootTextContentToBe`: the cached text, the text computed without
/// the cache, and `getTextContent()` all agree.
private func expectRootTextContentToBe(_ editor: Editor, _ text: String) throws {
  try editor.getEditorState().read { () throws in
    let root = try getRoot()
    #expect(root.__cachedText == text)
    // Read without the cache, as upstream does on a copy of the frozen root.
    let cachedText = root.__cachedText
    root.__cachedText = nil
    defer { root.__cachedText = cachedText }
    #expect(try root.getTextContent() == text)
  }
}

@Suite struct LexicalRootNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.constructor"
    ))
  func constructor() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let rootNode = try createRootNode()
      let other = try createRootNode()
      #expect(rootNode.__key == other.__key)
      #expect(try rootNode.exportJSON() == other.exportJSON())
      #expect(rootNode.getType() == "root")
      #expect(try rootNode.getTextContent() == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.exportJSON() should return and object conforming to the expected schema"
    ))
  func exportJSONSchema() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let json = try createRootNode().exportJSON()
      #expect(
        json.stringify()
          == #"{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}"#)
      #expect(json["textFormat"] == nil)
      #expect(json["textStyle"] == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.clone()"
    ))
  func clone() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let rootNode = try createRootNode()
      let rootNodeClone = try cloneWithProperties(rootNode)
      #expect(rootNodeClone !== rootNode)
      #expect(rootNodeClone.__key == rootNode.__key)
      #expect(rootNodeClone.__cachedText == rootNode.__cachedText)
      #expect(try rootNodeClone.exportJSON() == rootNode.exportJSON())
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.isAttached()"
    ))
  func isAttached() throws {
    let editor = try createTestEditor()
    var rootNode: RootNode?
    try update(editor) { rootNode = try createRootNode() }
    #expect(try #require(rootNode).isAttached())
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.isRootNode()"
    ))
  func isRootNodeGuard() throws {
    let editor = try createTestEditor()
    var rootNode: RootNode?
    try update(editor) { rootNode = try createRootNode() }
    let isRoot = Lexical.isRootNode(rootNode)
    #expect(isRoot)
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > Cached getTextContent with decorators"
    ))
  func cachedTextContentWithDecorators() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(createTestDecoratorNode())
    }
    #expect(try editor.read(.latest) { try getRoot().getTextContent() } == "Hello world")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.clear() to handle selection update"
    ))
  func clearUpdatesSelection() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("Hello")
      try paragraph.append(text)
      try text.select()
    }
    try update(editor) { try getRoot().clear() }
    try update(editor) {
      let root = try getRoot()
      guard let selection = try getSelection() as? RangeSelection else { return }
      #expect(try selection.anchor.getNode() === root)
      #expect(try selection.focus.getNode() === root)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode is selected when its selected child is removed"
    ))
  func selectedWhenSelectedChildRemoved() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("Hello")
      try paragraph.append(text)
      try text.select()
    }
    try update(editor) { try #require(try getRoot().getFirstChild()).remove() }
    try update(editor) {
      let root = try getRoot()
      guard let selection = try getSelection() as? RangeSelection else { return }
      #expect(try selection.anchor.getNode() === root)
      #expect(try selection.focus.getNode() === root)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode is not selected when all children are removed with no selection"
    ))
  func notSelectedWhenClearedWithoutSelection() throws {
    let editor = try createTestEditor()
    try update(editor) {
      #expect(try getSelection() == nil)
      try getRoot().append(createParagraphNode())
      #expect(try getSelection() == nil)
    }
    try update(editor) {
      #expect(try getSelection() == nil)
      try getRoot().clear()
      #expect(try getSelection() == nil)
    }
    try update(editor) { () throws in #expect(try getSelection() == nil) }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode __cachedText incremental update #8096"
    ))
  func cachedTextIncremental() throws {
    let editor = try createMountedTestEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("a")), try createParagraphNode(),
        try createParagraphNode().append(try createTextNode("b")))
    }
    try expectRootTextContentToBe(editor, "a\n\n\n\nb")
    try editor.update(discrete: true) {
      _ = try getRoot().selectEnd().insertRawText(".")
    }
    try expectRootTextContentToBe(editor, "a\n\n\n\nb.")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode __cachedText"
    ))
  func cachedText() throws {
    let editor = try createMountedTestEditor()
    try update(editor) { try getRoot().append(createParagraphNode()) }
    try expectRootTextContentToBe(editor, "")
    try update(editor) {
      let firstParagraph = try #require(try getRoot().getFirstChild() as? ElementNode)
      try firstParagraph.append(createTextNode("first line"))
    }
    try expectRootTextContentToBe(editor, "first line")
    try update(editor) { try getRoot().append(createParagraphNode()) }
    try expectRootTextContentToBe(editor, "first line\n\n")
    try update(editor) {
      let secondParagraph = try #require(try getRoot().getLastChild() as? ElementNode)
      try secondParagraph.append(createTextNode("second line"))
    }
    try expectRootTextContentToBe(editor, "first line\n\nsecond line")
    try update(editor) { try getRoot().append(createParagraphNode()) }
    try expectRootTextContentToBe(editor, "first line\n\nsecond line\n\n")
    try update(editor) {
      let thirdParagraph = try #require(try getRoot().getLastChild() as? ElementNode)
      try thirdParagraph.append(createTextNode("third line"))
    }
    try expectRootTextContentToBe(editor, "first line\n\nsecond line\n\nthird line")
    try update(editor) {
      let secondParagraph = try #require(try getRoot().getChildAtIndex(1) as? ElementNode)
      let secondParagraphText = try #require(try secondParagraph.getFirstChild() as? TextNode)
      try secondParagraphText.setTextContent("second line!")
    }
    try expectRootTextContentToBe(editor, "first line\n\nsecond line!\n\nthird line")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode __cachedText (empty paragraph)"
    ))
  func cachedTextEmptyParagraphs() throws {
    let editor = try createMountedTestEditor()
    try update(editor) { try getRoot().append(createParagraphNode(), createParagraphNode()) }
    try expectRootTextContentToBe(editor, "\n\n")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode __cachedText (inlines)"
    ))
  func cachedTextInlines() throws {
    let editor = try createMountedTestEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(
        createTextNode("a"), createTestElementNode(), createTextNode("b"),
        createTestInlineElementNode(), createTextNode("c"))
    }
    try expectRootTextContentToBe(editor, "a\n\nbc")
  }
}
