import Lexical
import LexicalExtension
import LexicalHistory
import LexicalPlainText
import LexicalRichText
import Testing

// Port of packages/lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts.

// Use a constant clock so that every change falls inside the merge window
// (delay). @lexical/history classifies PASTE_TAG/CUT_TAG updates as their own
// undo boundary (see #8609).
private func createHistoryEditor(
  _ textExtension: (any AnyLexicalExtensionArgument)? = nil
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: (textExtension.map { [$0] } ?? []) + [
        configExtension(HistoryExtension) {
          $0.delay = 1000
          $0.now = { 0 }
        }
      ],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          let paragraph = try createParagraphNode()
          try getRoot().append(paragraph)
          try paragraph.selectEnd()
        })))
}

private func type(_ text: String) throws {
  if let selection = try getSelection() as? RangeSelection {
    try selection.insertText(text)
  }
}

private func clipboardEvent(_ type: String, _ text: String) -> ClipboardEvent {
  let clipboardData = DataTransfer()
  clipboardData.setData("text/plain", text)
  return ClipboardEvent(type, clipboardData: clipboardData)
}

/// Whether one update while `dispatch` runs (and the microtasks after it,
/// which upstream flushes with a timer) carried `tag`.
private func someUpdateTagged(
  _ editor: Editor, _ tag: UpdateTag, _ dispatch: () throws -> Void
) throws -> Bool {
  var tagged = false
  let unregister = editor.registerUpdateListener { payload in
    if payload.tags.has(tag) { tagged = true }
  }
  defer { unregister() }
  try dispatch()
  drainMicrotasks()
  return tagged
}

@Suite struct HistorySnapshotCutPasteTests {
  // Recognition lives in @lexical/history: any update tagged PASTE_TAG or
  // CUT_TAG is classified as `OTHER`, which both keeps it from merging into
  // the preceding keystrokes and (by becoming `prevChangeType`) keeps the
  // following keystrokes from merging into it.
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::@lexical/history treats PASTE_TAG as an undo boundary (#8609) > a tagged single-character change is isolated between keystrokes"
    ),
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::@lexical/history treats CUT_TAG as an undo boundary (#8609) > a tagged single-character change is isolated between keystrokes"
    ),
    arguments: [PASTE_TAG, CUT_TAG])
  func taggedChangeIsIsolated(tag: UpdateTag) throws {
    let editor = try createHistoryEditor()
    defer { editor.dispose() }

    // Type three characters that merge into a single history entry.
    try editor.update(discrete: true) { try type("a") }
    try editor.update(discrete: true) { try type("b") }
    try editor.update(discrete: true) { try type("c") }

    // An in-place single-character insert would normally merge into the run of
    // typing; the tag forces it into its own entry.
    try editor.update(tag: tag, discrete: true) { try type("X") }

    // Following keystrokes do not merge into the tagged entry either.
    try editor.update(discrete: true) { try type("d") }

    // Undo removes only the trailing keystroke...
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "abcX")

    // ...then the tagged change, leaving the earlier typing intact.
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "abc")
  }

  // Each handler tags its clipboard update so the recognition above engages.
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich text clipboard handlers tag their updates (#8609) > paste tags the update with PASTE_TAG"
    ),
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::plain text clipboard handlers tag their updates (#8609) > paste tags the update with PASTE_TAG"
    ),
    arguments: ["rich text", "plain text"])
  func pasteTagsUpdate(name: String) throws {
    let editor = try createHistoryEditor(
      name == "rich text" ? RichTextExtension : PlainTextExtension)
    defer { editor.dispose() }
    let pasteTagged = try someUpdateTagged(editor, PASTE_TAG) {
      try editor.dispatchCommand(PASTE_COMMAND, .clipboard(clipboardEvent("paste", "X")))
    }
    #expect(pasteTagged == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich text clipboard handlers tag their updates (#8609) > cut tags the update with CUT_TAG"
    ),
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::plain text clipboard handlers tag their updates (#8609) > cut tags the update with CUT_TAG"
    ),
    arguments: ["rich text", "plain text"])
  func cutTagsUpdate(name: String) throws {
    let editor = try createHistoryEditor(
      name == "rich text" ? RichTextExtension : PlainTextExtension)
    defer { editor.dispose() }
    let cutTagged = try someUpdateTagged(editor, CUT_TAG) {
      try editor.update(discrete: true) { try type("abc") }
      try editor.update(discrete: true) { try getRoot().getAllTextNodes()[0].select(1, 3) }
      try editor.dispatchCommand(CUT_COMMAND, .clipboard(clipboardEvent("cut", "")))
    }
    #expect(cutTagged == true)
  }

  // End-to-end coverage of the user-facing bug from #8609: undoing a paste
  // should not also undo the typing before it. Only a short rich-text paste,
  // which inserts in place, would otherwise merge into the keystrokes.
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/HistorySnapshotCutPaste.test.ts::rich-text paste is isolated from the preceding typing (#8609) > a single undo after a paste keeps the text typed before it"
    ))
  func richTextPasteIsIsolated() throws {
    let editor = try createHistoryEditor(RichTextExtension)
    defer { editor.dispose() }

    try editor.update(discrete: true) { try type("a") }
    try editor.update(discrete: true) { try type("b") }
    try editor.update(discrete: true) { try type("c") }

    try editor.dispatchCommand(PASTE_COMMAND, .clipboard(clipboardEvent("paste", "X")))
    #expect(try editor.read { try getRoot().getTextContent() } == "abcX")

    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "abc")
  }
}
