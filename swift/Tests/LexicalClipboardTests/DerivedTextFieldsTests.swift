import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/DerivedTextFields.test.ts.

// A paragraph whose stored textFormat (0) is not the format of its first
// text (bold), as a client that never rendered it writes it, and a second
// paragraph.
private let STATE = #"""
  {"root":{"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"color: red","text":"bold","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"plain","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}
  """#

private struct TextFields: Equatable {
  var format: Int
  var style: String
}

private func textFields(_ node: LexicalNode?) throws -> TextFields? {
  guard let element = node as? ElementNode else { return nil }
  return TextFields(format: try element.getTextFormat(), style: try element.getTextStyle())
}

private func firstParagraphTextFields() throws -> TextFields? {
  try textFields(getRoot().getFirstChild())
}

/// Italic and blue on the last paragraph's first text.
private func editLastParagraphText() throws {
  if let paragraph = try getRoot().getLastChild() as? ElementNode,
    let text = try paragraph.getFirstChild() as? TextNode
  {
    _ = try text.toggleFormat("italic")
    _ = try text.setStyle("color: blue")
  }
}

/// An editor showing STATE, set before the root element: its first render
/// creates the DOM and derives nothing.
private func setup() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "derived-text-fields", dependencies: [RichTextExtension],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
  try editor.setEditorState(editor.parseEditorState(STATE))
  try editor.setRootElement(TestRootElement())
  return editor
}

@Suite struct DerivedTextFieldsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > a full reconcile changes no element that the update did not change"
    ))
  func fullReconcileChangesNothing() throws {
    let editor = try setup()
    defer { editor.dispose() }
    #expect(try editor.read(firstParagraphTextFields) == TextFields(format: 0, style: ""))
    var calls: [([NodeKey], [NodeKey])] = []
    _ = editor.registerUpdateListener { payload in
      calls.append((payload.dirtyElements.keysArray, payload.dirtyLeaves.array))
    }
    try editor.update(discrete: true) { fullReconcile() }
    #expect(try editor.read(firstParagraphTextFields) == TextFields(format: 0, style: ""))
    try #require(calls.count == 1)
    // Only the root, which a full reconcile marks.
    #expect(calls[0].0 == ["root"])
    #expect(calls[0].1.isEmpty)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > a full reconcile with an edit derives them for the element it changes only"
    ))
  func fullReconcileWithEdit() throws {
    let editor = try setup()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      fullReconcile()
      try editLastParagraphText()
    }
    #expect(
      try editor.read { try getRoot().getChildren().map(textFields) } == [
        TextFields(format: 0, style: ""), TextFields(format: 2, style: "color: blue"),
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/DerivedTextFields.test.ts::derived textFormat and textStyle of an element > an element whose first text changes takes its format and style"
    ))
  func firstTextChange() throws {
    let editor = try setup()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try editLastParagraphText() }
    #expect(
      try editor.read { try textFields(getRoot().getLastChild()) }
        == TextFields(format: 2, style: "color: blue"))
  }
}
