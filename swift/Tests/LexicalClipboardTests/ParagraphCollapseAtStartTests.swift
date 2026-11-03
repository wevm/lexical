import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts.

private func createEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "[paragraph-collapse-at-start]", dependencies: [RichTextExtension]))
  try editor.setRootElement(TestRootElement())
  return editor
}

/// The work Backspace does: KEY_BACKSPACE_COMMAND -> DELETE_CHARACTER_COMMAND.
private func backspace(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    if let selection = try getSelection() as? RangeSelection {
      try selection.deleteCharacter(true)
    }
  }
}

private func blocks(_ editor: Editor) throws -> [String] {
  try editor.read { try getRoot().getChildren().map { try $0.getTextContent() } }
}

/// `first` and a "world" paragraph, with the caret at the start of `first`.
private func setUp(_ editor: Editor, _ first: @escaping () throws -> ParagraphNode) throws {
  try editor.update(discrete: true) {
    let firstParagraph = try first()
    _ = try getRoot().clear().append(
      firstParagraph, createParagraphNode().append(createTextNode("world")))
    _ = try firstParagraph.selectStart()
  }
}

@Suite struct ParagraphCollapseAtStartTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > keeps a first paragraph whose content follows a blank text node"
    ))
  func keepsContentAfterBlankText() throws {
    let editor = try createEditor()
    defer { editor.dispose() }
    try setUp(editor) {
      try createParagraphNode().append(
        createTextNode("  "), createTextNode("hello").toggleFormat("bold"))
    }
    try backspace(editor)
    #expect(try blocks(editor) == ["  hello", "world"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > keeps a first paragraph whose only other child is a line break"
    ))
  func keepsLineBreak() throws {
    let editor = try createEditor()
    defer { editor.dispose() }
    try setUp(editor) {
      try createParagraphNode().append(createTextNode(" "), createLineBreakNode())
    }
    try backspace(editor)
    #expect(try editor.read { try getRoot().getChildrenSize() } == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > control: a blank first paragraph is still collapsed away"
    ))
  func blankFirstParagraphCollapses() throws {
    let editor = try createEditor()
    defer { editor.dispose() }
    try setUp(editor) { try createParagraphNode().append(createTextNode("  ")) }
    try backspace(editor)
    #expect(try blocks(editor) == ["world"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ParagraphCollapseAtStart.test.ts::ParagraphNode.collapseAtStart > control: an empty first paragraph is still collapsed away"
    ))
  func emptyFirstParagraphCollapses() throws {
    let editor = try createEditor()
    defer { editor.dispose() }
    try setUp(editor) { try createParagraphNode() }
    try backspace(editor)
    #expect(try blocks(editor) == ["world"])
  }
}
