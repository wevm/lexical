import Lexical
import LexicalTextKit
import Testing

// The typeable slice without a view: native input through the controller,
// checked against the model and the rendered text.

private func makeController() throws -> TextKitInputController {
  let editor = try createEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
  registerTextKitSliceEditing(editor)
  return TextKitInputController(editor: editor)
}

private func json(_ controller: TextKitInputController) throws -> String {
  try controller.editor.getEditorState().toJSON(true).stringify()
}

@Suite struct TextKitInputTests {
  @Test func typesAndRenders() throws {
    let c = try makeController()
    var rendered: [String] = []
    c.onRender = { document, _ in rendered.append(document.string) }
    try c.insertText("Hello")
    try c.insertText(" world")
    #expect(c.document.string == "Hello world")
    #expect(try c.selectedRange()! == (11, 0))
    #expect(rendered.last == "Hello world")
    try c.insertNewline()
    try c.insertText("Next")
    #expect(c.document.string == "Hello world\nNext")
    #expect(try c.selectedRange()! == (16, 0))
  }

  @Test func deletesWithinTextThroughTheMovement() throws {
    let c = try makeController()
    try c.insertText("ab😀c")
    try c.deleteBackward()
    #expect(c.document.string == "ab😀")
    // The emoji is one grapheme of two UTF-16 units.
    try c.deleteBackward()
    #expect(c.document.string == "ab")
    try c.setSelectedRange(location: 0, length: 0)
    try c.deleteForward()
    #expect(c.document.string == "b")
    try c.insertText("one two")
    try c.deleteWordBackward()
    #expect(c.document.string == "one b")
  }

  @Test func mergesParagraphsOnBackspace() throws {
    let c = try makeController()
    try c.insertText("a\nb")
    #expect(c.document.string == "a\nb")
    try c.setSelectedRange(location: 2, length: 0)
    try c.deleteBackward()
    #expect(c.document.string == "ab")
    #expect(try c.selectedRange()! == (1, 0))
  }

  @Test func replacesTheSelectionAndFormats() throws {
    let c = try makeController()
    try c.insertText("Hello world")
    try c.setSelectedRange(location: 6, length: 5)
    try c.toggleFormat("bold")
    #expect(c.document.runs.map(\.format) == [0, 1])
    try c.setSelectedRange(location: 0, length: 5)
    try c.insertText("Bye")
    #expect(c.document.string == "Bye world")
    #expect(try json(c).contains(#"{"type":"text","format":1,"text":"world"}"#))
  }

  @Test func lineBreaksAndArrowKeys() throws {
    let c = try makeController()
    try c.insertText("ab")
    try c.insertLineBreak()
    try c.insertText("cd")
    #expect(c.document.string == "ab\u{2028}cd")
    try c.moveSelection(.move, isBackward: true, granularity: .character)
    #expect(try c.selectedRange()! == (4, 0))
    try c.moveSelection(.extend, isBackward: true, granularity: .lineboundary)
    #expect(try c.selectedRange()! == (3, 1))
    try c.moveSelection(.move, isBackward: true, granularity: .character)
    #expect(try c.selectedRange()! == (3, 0))
    try c.moveSelection(.move, isBackward: true, granularity: .character)
    #expect(try c.selectedRange()! == (2, 0))
  }

  @Test func mapsLocationsToPoints() throws {
    let c = try makeController()
    try c.insertText("ab\ncd")
    let document = c.document
    try c.editor.read { () throws in
      let paragraphs = try getRoot().getChildren()
      let first = try #require(try (paragraphs[0] as? ElementNode)?.getFirstChild())
      let second = try #require(try (paragraphs[1] as? ElementNode)?.getFirstChild())
      #expect(document.point(at: 1) == (first.getKey(), 1, .text))
      #expect(document.point(at: 3) == (second.getKey(), 0, .text))
      #expect(document.location(paragraphs[1].getKey(), 0, .element) == 3)
      #expect(document.location("root", 2, .element) == 5)
    }
  }
}
