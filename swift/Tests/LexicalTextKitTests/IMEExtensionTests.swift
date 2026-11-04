import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTextKit
import Testing

// IMEExtension has no upstream tests. These drive it as a view does: marked
// text through TextKitInputController, which reports the start and end of
// the composition as COMPOSITION_START_COMMAND and COMPOSITION_END_COMMAND.

private func setUp() throws -> (Editor, TextKitInputController, IMEOutput) {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "ime-test", dependencies: [RichTextExtension, IMEExtension]))
  let controller = TextKitInputController(editor: editor)
  try editor.update(discrete: true) {
    let paragraph = try createParagraphNode()
    try getRoot().clear().append(paragraph)
    try paragraph.select()
  }
  return (editor, controller, try getExtensionDependencyFromEditor(editor, IMEExtension).output)
}

@Suite struct IMEExtensionTests {
  @Test func tracksACompositionInExistingText() throws {
    let (editor, controller, ime) = try setUp()
    defer { editor.dispose() }
    try controller.insertText("ab")
    let textKey = try editor.read { try #require(try getRoot().getFirstDescendant()).getKey() }
    #expect(ime.compositionKey.peek() == nil)

    try controller.setMarkedText("に", selectedRange: TextKitRange(location: 1, length: 0))
    #expect(ime.compositionKey.peek() == textKey)
    #expect(ime.composingTextNode.peek()?.getKey() == textKey)

    try controller.unmarkText()
    #expect(ime.compositionKey.peek() == nil)
    #expect(ime.composingTextNode.peek() == nil)
    #expect(try editor.read { try getRoot().getTextContent() } == "abに")
  }

  @Test func resolvesTheTextNodeOfACompositionStartedOnAnElement() throws {
    let (editor, controller, ime) = try setUp()
    defer { editor.dispose() }
    let paragraphKey = try editor.read { try #require(try getRoot().getFirstChild()).getKey() }

    try controller.setMarkedText("にほ", selectedRange: TextKitRange(location: 2, length: 0))
    // The composition started on the empty paragraph; its text node is the
    // one the first marked text went into.
    #expect(ime.compositionKey.peek() == paragraphKey)
    let composing = try #require(ime.composingTextNode.peek())
    #expect(try editor.read { try composing.getTextContent() } == "にほ")

    try controller.unmarkText()
    #expect(ime.composingTextNode.peek() == nil)
  }
}
