import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts.

private func setUpEditor(
  _ dependency: any AnyLexicalExtensionArgument = ClearEditorExtension
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "clear-editor-test", dependencies: [dependency],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          try getRoot().append(try createParagraphNode().append(try createTextNode("content")))
        })))
}

@Suite struct ClearEditorExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > CLEAR_EDITOR_COMMAND empties the document"
    ))
  func emptiesTheDocument() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.dispatchCommand(CLEAR_EDITOR_COMMAND)
    try editor.read { () throws in #expect(try getRoot().getTextContent() == "") }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > a configured $onClear replaces the default behavior"
    ))
  func configuredOnClear() throws {
    let editor = try setUpEditor(
      configExtension(ClearEditorExtension) {
        $0.onClear = {
          let root = try getRoot()
          try root.clear()
          try root.append(try createParagraphNode().append(try createTextNode("cleared")))
        }
      })
    defer { editor.dispose() }
    try editor.dispatchCommand(CLEAR_EDITOR_COMMAND)
    try editor.read { () throws in #expect(try getRoot().getTextContent() == "cleared") }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClearEditorExtension.test.ts::ClearEditorExtension > $onClear runs inline when dispatched from inside an update"
    ))
  func onClearRunsInline() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update {
      try editor.dispatchCommand(CLEAR_EDITOR_COMMAND)
      let root = try getRoot()
      try root.clear()
      try root.append(try createParagraphNode().append(try createTextNode("replacement")))
    }
    try editor.read { () throws in #expect(try getRoot().getTextContent() == "replacement") }
  }
}
