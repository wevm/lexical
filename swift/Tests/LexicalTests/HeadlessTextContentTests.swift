import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/HeadlessTextContent.test.ts. Upstream spies on
// `RootNode.prototype.getTextContent`; here the paragraphs are a replacement
// class that counts how often their text is read, which every read of the
// document text goes through.

nonisolated(unsafe) private var textReads = 0

private final class CountingParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CountingParagraph.self, type: "counting-paragraph", extends: ParagraphNode.self)
  }
  override func getTextContent() throws -> String {
    textReads += 1
    return try super.getTextContent()
  }
}

private func headlessEditor(_ paragraphs: Int) throws -> Editor {
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [
        .klass(CountingParagraph.self),
        .replacement(
          LexicalNodeReplacement(
            replace: ParagraphNode.self, with: { _ in try CountingParagraph() },
            withKlass: CountingParagraph.self)),
      ], onError: { throw $0 }))
  try editor.update(discrete: true) {
    for i in 0..<paragraphs {
      try getRoot().append(try createParagraphNode().append(try createTextNode("para \(i)")))
    }
  }
  return editor
}

private func firstText(_ paragraph: LexicalNode?) throws -> TextNode {
  guard let text = try (paragraph as? ElementNode)?.getFirstChild() as? TextNode else {
    throw LexicalError("expected a paragraph with text")
  }
  return text
}

/// Prepends "x" to the text of paragraph `index`.
private func edit(_ editor: Editor, _ index: Int) throws {
  try editor.update(discrete: true) {
    let text = try firstText(try getRoot().getChildAtIndex(index))
    _ = try text.setTextContent("x" + (try text.getTextContent()))
  }
}

@Suite(.serialized) struct HeadlessTextContentTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HeadlessTextContent.test.ts::text content listeners in a headless editor > a commit computes no document text while nothing listens"
    ))
  func noTextWithoutListeners() throws {
    let editor = try headlessEditor(100)
    textReads = 0
    for i in 0..<20 { try edit(editor, i) }
    #expect(textReads == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HeadlessTextContent.test.ts::text content listeners in a headless editor > a listener still gets each change of the text, and only those"
    ))
  func listenerGetsEachChange() throws {
    let editor = try headlessEditor(3)
    var texts: [String] = []
    let unregister = editor.registerTextContentListener { texts.append($0) }
    try edit(editor, 1)
    #expect(texts == ["para 0\n\nxpara 1\n\npara 2"])
    // A change that leaves the text as it was.
    try editor.update(discrete: true) {
      _ = try firstText(try getRoot().getFirstChild()).toggleFormat("bold")
    }
    #expect(texts.count == 1)
    unregister()
    textReads = 0
    try edit(editor, 2)
    #expect(texts.count == 1)
    #expect(textReads == 0)
  }
}
