import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import Testing

@testable import LexicalFile

// `@lexical/file` has no upstream unit tests, and the conformance vectors
// (vectors/file) cover the document format. These cover what the vectors
// can't: `Date.now()` for a missing `lastSaved`, and `importFile` and
// `exportFile` through the app's file hooks.

@MainActor
struct LexicalFileTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(name: "[root]", dependencies: [RichTextExtension, HistoryExtension]))
  }

  private func setText(_ editor: Editor, _ text: String) throws {
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
    }
  }

  /// A missing or 0 `lastSaved` is now, and a missing or empty `source` is
  /// "Lexical", as JS's `||` makes them.
  @Test func defaultsLastSavedToNowAndSourceToLexical() throws {
    let editor = try makeEditor()
    let before = (Date().timeIntervalSince1970 * 1000).rounded(.down)
    for lastSaved: Double? in [nil, 0] {
      let document = try serializedDocumentFromEditorState(
        editor.getEditorState(), source: "", lastSaved: lastSaved)
      #expect(document.lastSaved >= before)
      #expect(document.lastSaved == document.lastSaved.rounded(.down))
      #expect(document.source == "Lexical")
      #expect(document.version == lexicalUpstreamVersion)
    }
  }

  @Test func formatsTimesAsToISOString() {
    #expect(toISOString(0) == "1970-01-01T00:00:00.000Z")
    #expect(toISOString(1_762_401_787_158) == "2025-11-06T04:03:07.158Z")
    #expect(toISOString(951_782_400_005) == "2000-02-29T00:00:00.005Z")
  }

  /// `exportFile` names the file by `fileName`, or else by the time it
  /// records as `lastSaved`, and writes the document's JSON.
  @Test func exportsThroughTheHook() throws {
    let editor = try makeEditor()
    try setText(editor, "Exported")
    var saved: [(text: String, fileName: String)] = []
    try exportFile(editor, fileName: "notes", source: "Playground") { saved.append(($0, $1)) }
    try exportFile(editor) { saved.append(($0, $1)) }
    #expect(saved.map(\.fileName).first == "notes.lexical")
    let named = try #require(JSONValue.parse(saved[0].text).objectValue)
    #expect(named["source"] == "Playground")
    #expect(named["version"] == .string(lexicalUpstreamVersion))
    #expect(
      named["editorState"] == .object(try editor.getEditorState().toJSON()))
    let unnamed = try #require(JSONValue.parse(saved[1].text).objectValue)
    let lastSaved = try #require(unnamed["lastSaved"]?.numberValue)
    #expect(saved[1].fileName == toISOString(lastSaved) + ".lexical")
    #expect(unnamed["source"] == "Lexical")
  }

  /// `importFile` asks for `.lexical` files, sets the editor to the
  /// document and clears the history, so the import can't be undone into
  /// the previous document.
  @Test func importsThroughTheHookAndClearsHistory() throws {
    let source = try makeEditor()
    try setText(source, "Imported")
    let text = try serializedDocumentFromEditorState(source.getEditorState()).stringify()

    let editor = try makeEditor()
    try setText(editor, "Before")
    try setText(editor, "Before, edited")
    var accepted: String?
    importFile(editor) { accept, callback in
      accepted = accept
      try? callback(text)
    }
    #expect(accepted == ".lexical")
    #expect(try editor.read { try getRoot().getTextContent() } == "Imported")
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "Imported")
  }

  /// A file that isn't a document reaches the hook as an error and leaves
  /// the editor as it was.
  @Test func reportsAnInvalidFileToTheHook() throws {
    let editor = try makeEditor()
    try setText(editor, "Kept")
    var error: (any Error)?
    importFile(editor) { _, callback in
      do { try callback(#"{"root":{}}"#) } catch let e { error = e }
    }
    #expect(error != nil)
    #expect(try editor.read { try getRoot().getTextContent() } == "Kept")
  }
}
