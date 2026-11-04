import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts.
// Upstream arms the extension with a triple click's mousedown and then
// reports the browser's selection with a selectionchange event. A native
// view has synced its selection by the time it reports the click, so here
// the overselection is made first and the click (CLICK_COMMAND, with the
// click count as its detail) follows it. The tests about arming and
// disarming around DOM events stay dom-only.

private func setUpEditor(bold: Bool = false) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "normalize-triple-click-selection-test",
      dependencies: [NormalizeTripleClickSelectionExtension],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          for text in ["Paragraph 1", "Paragraph 2"] {
            let node = try createTextNode(text)
            if bold { try node.setFormat("bold").setStyle("color: red") }
            try getRoot().append(createParagraphNode().append(node))
          }
        }),
      register: { editor, _, _ in
        do { try editor.setRootElement(TestRootElement()) } catch { Issue.record(error) }
        return {}
      }))
}

/// What a triple click selects: all of paragraph 1 up to the start of
/// paragraph 2, as the view syncs it.
private func overselect(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let children = try getRoot().getChildren()
    let text = try #require((children[0] as? ElementNode)?.getFirstChild() as? TextNode)
    _ = try text.select(0, 0)
    try #require(try getSelection() as? RangeSelection).focus.set(
      children[1].getKey(), 0, .element)
  }
}

private func click(_ editor: Editor, _ detail: Int) throws {
  try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click", detail: detail))
}

private struct Focus: Equatable {
  var offset: Int
  var text: String
}

private func readFocus(_ editor: Editor) throws -> Focus? {
  try editor.read {
    guard let selection = try getSelection() as? RangeSelection else { return nil }
    return Focus(
      offset: selection.focus.offset, text: try selection.focus.getNode().getTextContent())
  }
}

private let fixed = Focus(offset: "Paragraph 1".utf16.count, text: "Paragraph 1")
private let overselected = Focus(offset: 0, text: "Paragraph 2")

@Suite struct NormalizeTripleClickSelectionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims the selection change that follows a triple click"
    ))
  func trimsTripleClick() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try overselect(editor)
    try click(editor, 3)
    #expect(try readFocus(editor) == fixed)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims a triple click on formatted text"
    ))
  func trimsFormattedText() throws {
    let editor = try setUpEditor(bold: true)
    defer { editor.dispose() }
    try overselect(editor)
    try click(editor, 3)
    #expect(try readFocus(editor) == fixed)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > trims after a fourth click"
    ))
  func trimsFourthClick() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try overselect(editor)
    try click(editor, 4)
    #expect(try readFocus(editor) == fixed)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > only trims the first selection change"
    ))
  func onlyTrimsOnce() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try overselect(editor)
    try click(editor, 3)
    try editor.update(discrete: true) { _ = try getRoot().selectStart() }
    try overselect(editor)
    #expect(try readFocus(editor) == overselected)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > a single or double click cancels it"
    ))
  func singleOrDoubleClickDoesNotTrim() throws {
    for detail in [1, 2] {
      let editor = try setUpEditor()
      defer { editor.dispose() }
      try overselect(editor)
      try click(editor, detail)
      #expect(try readFocus(editor) == overselected)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > a triple click outside the editor does not arm it"
    ))
  func tripleClickInAnotherEditor() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    let other = try setUpEditor()
    defer { other.dispose() }
    try overselect(editor)
    try click(other, 3)
    #expect(try readFocus(editor) == overselected)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeTripleClickSelection.test.ts::NormalizeTripleClickSelectionExtension > does nothing without a triple click"
    ))
  func nothingWithoutTripleClick() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try overselect(editor)
    #expect(try readFocus(editor) == overselected)
  }

  @Test func disabledDoesNotTrim() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "normalize-triple-click-selection-disabled",
        dependencies: [
          configExtension(NormalizeTripleClickSelectionExtension) { $0.disabled = true }
        ],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            for text in ["Paragraph 1", "Paragraph 2"] {
              try getRoot().append(createParagraphNode().append(createTextNode(text)))
            }
          })))
    defer { editor.dispose() }
    try overselect(editor)
    try click(editor, 3)
    #expect(try readFocus(editor) == overselected)
  }
}
