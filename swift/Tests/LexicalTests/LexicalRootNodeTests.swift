import LexicalTesting
import Testing

@testable import Lexical

// Port of nodes/__tests__/unit/LexicalRootNode.test.ts, except createDOM and
// updateDOM, which are dom-only, and the __cachedText tests, which wait for
// the reconciler: it is what fills the cache, and a headless editor (JS or
// Swift) leaves it nil.

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
}
