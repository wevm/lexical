import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts.

private func buildTestEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[root]", dependencies: [RichTextExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(TestShadowRootNode.self)] }))
}

private func paragraphTexts(_ node: ElementNode) throws -> [String] {
  try node.getChildren().map { try #require($0 as? ParagraphNode).getTextContent() }
}

@Suite struct LexicalRootSelectionPhantomParagraphTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > reuses an empty trailing paragraph instead of appending a new one"
    ))
  func reusesTrailingParagraph() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      _ = try root.append(createParagraphNode())
      try root.select(1, 1).insertText("x")
    }
    try editor.read { () throws in #expect(try paragraphTexts(getRoot()) == ["x"]) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > empty root still creates a new paragraph for the typed text"
    ))
  func emptyRootCreatesParagraph() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().select(0, 0).insertText("x")
    }
    try editor.read { () throws in #expect(try paragraphTexts(getRoot()) == ["x"]) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > reuses an empty trailing paragraph inside a shadow root"
    ))
  func reusesTrailingParagraphInShadowRoot() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let shadowRoot = try createTestShadowRootNode().append(createParagraphNode())
      _ = try getRoot().clear().append(shadowRoot)
      try shadowRoot.select(1, 1).insertText("x")
    }
    try editor.read { () throws in
      let shadowRoot = try #require(
        try getRoot().getFirstChildOrThrow() as? TestShadowRootNode, "Expected TestShadowRootNode")
      #expect(try paragraphTexts(shadowRoot) == ["x"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRootSelectionPhantomParagraph.test.ts::Typing at root + last-offset selection (no phantom paragraph) > falls back to a new paragraph when the trailing child is a DecoratorNode"
    ))
  func newParagraphAfterDecorator() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      _ = try root.append(
        createParagraphNode().append(createTextNode("first")),
        createTestDecoratorNode().setIsInline(false))
      try root.select(2, 2).insertText("x")
    }
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(try #require(children[0] as? ParagraphNode).getTextContent() == "first")
      #expect(children[1] is TestDecoratorNode)
      #expect(try #require(children[2] as? ParagraphNode).getTextContent() == "x")
    }
  }
}
