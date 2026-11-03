import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalCustomTextContent.test.ts. Upstream
// registers both of its custom paragraphs under the type `custom-paragraph`,
// so the later one (`~"…"`, size delegated to getTextContent) is the one
// `$create` builds in both `describe.each` cases; each test here carries
// both ids. Swift cannot ask whether getTextContent is overridden, so the
// node overrides getTextContentSize to match (see DESIGN.md).

private final class CustomTextParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CustomTextParagraphNode.self, type: "custom-paragraph", extends: ParagraphNode.self)
  }
  override func getTextContent() throws -> String { "~\"\(try super.getTextContent())\"" }
  override func getTextContentSize() throws -> Int { 3 + (try super.getTextContentSize()) }
}

private func makeEditor() throws -> Editor {
  try createMountedTestEditor(nodes: [.klass(CustomTextParagraphNode.self)])
}

private func custom() throws -> CustomTextParagraphNode { try CustomTextParagraphNode() }

private func customRootText(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getTextContent() }
}

@Suite struct LexicalCustomTextContentTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"\" after reconciliation"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"\" after reconciliation #2"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"Hello\" after reconciliation"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"Hello\" after reconciliation #2"
    ), arguments: ["", "Hello"])
  func rootTextIncludesCustom(text: String) throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) {
      let paragraph = try custom()
      if text != "" {
        try paragraph.append(try createTextNode(text))
      }
      try getRoot().clear().append(paragraph)
      #expect(try paragraph.getTextContentSize() == paragraph.getTextContent().utf16.count)
      #expect(try getRoot().getTextContent() == "~\"\(text)\"")
    }
    try editor.read { () throws in
      #expect(try getRoot().getTextContent() == "~\"\(text)\"")
      #expect(try getRoot().getTextContentSize() == "~\"\(text)\"".utf16.count)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > updates custom text when only a descendant is dirty"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > updates custom text when only a descendant is dirty #2"
    ))
  func descendantDirty() throws {
    let editor = try makeEditor()
    var text: TextNode!
    try editor.update(discrete: true) {
      text = try createTextNode("Hello")
      try getRoot().clear().append(try custom().append(text))
    }
    var changes: [String] = []
    let unregister = editor.registerTextContentListener { changes.append($0) }
    defer { unregister() }
    try editor.update(discrete: true) { try text.setTextContent("World") }
    #expect(try customRootText(editor) == "~\"World\"")
    #expect(changes == ["~\"World\""])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves clean custom text when a following sibling changes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves clean custom text when a following sibling changes #2"
    ))
  func followingSiblingChanges() throws {
    let editor = try makeEditor()
    var text: TextNode!
    try editor.update(discrete: true) {
      text = try createTextNode("Last")
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("First")),
        try custom().append(try createTextNode("Custom")),
        try createParagraphNode().append(try createTextNode("Third")),
        try createParagraphNode().append(text))
    }
    try editor.update(discrete: true) { try text.setTextContent("Updated") }
    #expect(try customRootText(editor) == "First\n\n~\"Custom\"\n\nThird\n\nUpdated")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > recomputes a custom parent when its child suffix changes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > recomputes a custom parent when its child suffix changes #2"
    ))
  func childSuffixChanges() throws {
    let editor = try makeEditor()
    var paragraph: CustomTextParagraphNode!
    var last: TextNode!
    try editor.update(discrete: true) {
      paragraph = try custom()
      last = try createTextNode("d").toggleFormat("underline")
      try getRoot().clear().append(
        try paragraph.append(
          try createTextNode("a"), try createTextNode("b").toggleFormat("bold"),
          try createTextNode("c").toggleFormat("italic"), last))
    }
    try editor.update(discrete: true) { try last.setTextContent("D") }
    #expect(try customRootText(editor) == "~\"abcD\"")
    try editor.update(discrete: true) { try paragraph.append(try createTextNode("e")) }
    #expect(try customRootText(editor) == "~\"abcDe\"")
    try editor.update(discrete: true) { try paragraph.getLastChildOrThrow().remove() }
    #expect(try customRootText(editor) == "~\"abcD\"")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > restores custom text when an editor state is replaced"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > restores custom text when an editor state is replaced #2"
    ))
  func editorStateReplaced() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(try custom().append(try createTextNode("Original")))
    }
    let original = editor.getEditorState()
    try editor.update(discrete: true) { try getRoot().clear().append(try createParagraphNode()) }
    try editor.setEditorState(original)
    #expect(try customRootText(editor) == "~\"Original\"")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves the source text when an ancestor of a custom node moves"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves the source text when an ancestor of a custom node moves #2"
    ))
  func ancestorMoves() throws {
    let editor = try makeEditor()
    var target: TestElementNode!
    var moved: TestElementNode!
    var text: TextNode!
    try editor.update(discrete: true) {
      target = try createTestElementNode()
      text = try createTextNode("Original")
      moved = try createTestElementNode().append(try custom().append(text))
      try getRoot().clear().append(
        target,
        try createTestElementNode().append(
          try createParagraphNode().append(try createTextNode("First")),
          try createParagraphNode().append(try createTextNode("Second")),
          try createParagraphNode().append(try createTextNode("Third")), moved))
    }
    try editor.update(discrete: true) {
      #expect(try moved.getTextContentSize() == moved.getTextContent().utf16.count)
      try target.append(moved)
      try text.setTextContent("Changed text")
    }
    try editor.read { () throws in
      #expect(try moved.getTextContentSize() == moved.getTextContent().utf16.count)
      #expect(try target.getTextContentSize() == target.getTextContent().utf16.count)
      #expect(try getRoot().getTextContent() == "~\"Changed text\"\n\nFirst\n\nSecond\n\nThird")
      #expect(try getRoot().getTextContentSize() == getRoot().getTextContent().utf16.count)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > includes slots in custom text when they change or are removed"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > includes slots in custom text when they change or are removed #2"
    ))
  func slotsInCustomText() throws {
    let editor = try makeEditor()
    var paragraph: CustomTextParagraphNode!
    var title: TextNode!
    try editor.update(discrete: true) {
      paragraph = try custom().append(try createTextNode("Body"))
      title = try createTextNode("Title")
      _ = try setSlot(paragraph, "title", try createParagraphNode().append(title))
      try getRoot().clear().append(paragraph)
    }
    #expect(try customRootText(editor) == "~\"TitleBody\"")
    try editor.update(discrete: true) { try title.setTextContent("Updated") }
    try editor.read { () throws in
      #expect(try getRoot().getTextContent() == "~\"UpdatedBody\"")
      #expect(try paragraph.getTextContentSize() == paragraph.getTextContent().utf16.count)
      #expect(try getRoot().getTextContentSize() == getRoot().getTextContent().utf16.count)
    }
    try editor.update(discrete: true) { try removeSlot(paragraph, "title") }
    #expect(try customRootText(editor) == "~\"Body\"")
  }
}
