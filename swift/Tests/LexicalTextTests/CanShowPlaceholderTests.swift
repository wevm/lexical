import Lexical
import LexicalExtension
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts.

/// An inline decorator that contributes no text, e.g. an inline image or
/// equation chip. Its emptiness keeps the root's text content empty while
/// the document plainly is not empty.
private final class InlineDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InlineDecoratorNode.self, type: "inline_decorator_placeholder", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[can-show-placeholder]", nodes: { [.klass(InlineDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
}

@Suite struct CanShowPlaceholderTests {
  // A simple empty TextNode is removed during reconciliation, which would
  // move the decorator to index 0. `token` and `segmented` nodes are not
  // simple text, so they survive and keep a TextNode at index 0 — the shape
  // that exercises the scan of the remaining children.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when a decorator follows an empty token text node"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when a decorator follows an empty segmented text node"
    ),
    arguments: ["token", "segmented"])
  func decoratorAfterEmptyText(_ mode: String) throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("").setMode(mode), create(InlineDecoratorNode.self)))
    }
    try editor.read {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      // Guard the premise: the text node must have survived, otherwise the
      // decorator would sit at index 0 and the scan would be trivial.
      #expect(try paragraph.getChildrenSize() == 2)
      #expect(try getRoot().getTextContent() == "")
      #expect(try canShowPlaceholder(false) == false)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when the decorator is several positions along"
    ))
  func decoratorSeveralAlong() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("").setMode("token"), createTextNode("").setMode("token"),
          create(InlineDecoratorNode.self)))
    }
    #expect(try editor.read { try canShowPlaceholder(false) } == false)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still false when the decorator is first"
    ))
  func decoratorFirst() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          create(InlineDecoratorNode.self), createTextNode("").setMode("token")))
    }
    #expect(try editor.read { try canShowPlaceholder(false) } == false)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still true for an empty paragraph"
    ))
  func emptyParagraph() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode())
    }
    #expect(try editor.read { try canShowPlaceholder(false) } == true)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still true for a paragraph of empty text nodes"
    ))
  func paragraphOfEmptyText() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("").setMode("token"), createTextNode("").setMode("token")))
    }
    #expect(try editor.read { try canShowPlaceholder(false) } == true)
  }
}
