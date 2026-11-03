import Lexical
import LexicalA11y
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts
// and EditorModeAnnounceExtension.test.ts.

private func buildEditor(_ announcer: any AnyLexicalExtensionArgument) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "[root]", dependencies: [announcer, RichTextExtension]))
}

@Suite struct HistoryAnnounceExtensionTests {
  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > announces the default Undone / Redone messages on the dependency sink"
    ))
  func announcesDefaults() throws {
    let editor = try buildEditor(HistoryAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(readLiveRegion(root) == "Undone")
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(readLiveRegion(root) == "Redone")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > respects message overrides from configExtension"
    ))
  func respectsOverrides() throws {
    let editor = try buildEditor(
      configExtension(HistoryAnnounceExtension) {
        $0.redone = "Custom redo"
        $0.undone = "Custom undo"
      })
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(readLiveRegion(root) == "Custom undo")
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(readLiveRegion(root) == "Custom redo")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > reflects message signal changes at runtime"
    ))
  func reflectsSignals() throws {
    let editor = try buildEditor(HistoryAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, HistoryAnnounceExtension).output
    output.undone.value = "Reverted"
    output.redone.value = "Restored"
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(readLiveRegion(root) == "Reverted")
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(readLiveRegion(root) == "Restored")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > does not announce while disabled, and resumes when re-enabled"
    ))
  func respectsDisabled() throws {
    let editor = try buildEditor(HistoryAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, HistoryAnnounceExtension).output
    output.disabled.value = true
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(readLiveRegion(root) == "")
    output.disabled.value = false
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(readLiveRegion(root) == "Redone")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/HistoryAnnounceExtension.test.ts::HistoryAnnounceExtension > keeps the history command chain intact (returns false)"
    ))
  func keepsCommandChain() throws {
    let editor = try buildEditor(HistoryAnnounceExtension)
    defer { editor.dispose() }
    try mountRoot(editor)
    var reached = false
    _ = editor.registerCommand(
      UNDO_COMMAND,
      { _, _ in
        reached = true
        return false
      }, 1)
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(reached)
  }
}

@Suite struct EditorModeAnnounceExtensionTests {
  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > announces the default editable / read-only messages on the dependency sink"
    ))
  func announcesDefaults() throws {
    let editor = try buildEditor(EditorModeAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    try editor.setEditable(false)
    #expect(readLiveRegion(root) == "Editor is read-only")
    try editor.setEditable(true)
    #expect(readLiveRegion(root) == "Editor is editable")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > respects message overrides from configExtension"
    ))
  func respectsOverrides() throws {
    let editor = try buildEditor(
      configExtension(EditorModeAnnounceExtension) {
        $0.editable = "Now editing"
        $0.readOnly = "Locked"
      })
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    try editor.setEditable(false)
    #expect(readLiveRegion(root) == "Locked")
    try editor.setEditable(true)
    #expect(readLiveRegion(root) == "Now editing")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > reflects message signal changes at runtime"
    ))
  func reflectsSignals() throws {
    let editor = try buildEditor(EditorModeAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, EditorModeAnnounceExtension).output
    output.editable.value = "On"
    output.readOnly.value = "Off"
    try editor.setEditable(false)
    #expect(readLiveRegion(root) == "Off")
    try editor.setEditable(true)
    #expect(readLiveRegion(root) == "On")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/EditorModeAnnounceExtension.test.ts::EditorModeAnnounceExtension > does not announce while disabled, and resumes when re-enabled"
    ))
  func respectsDisabled() throws {
    let editor = try buildEditor(EditorModeAnnounceExtension)
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, EditorModeAnnounceExtension).output
    output.disabled.value = true
    try editor.setEditable(false)
    #expect(readLiveRegion(root) == "")
    output.disabled.value = false
    try editor.setEditable(true)
    #expect(readLiveRegion(root) == "Editor is editable")
  }
}
