import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts.

private let SEED_TEXT = "hello world"

private func initialEditorState() throws {
  let text = try createTextNode(SEED_TEXT)
  _ = try getRoot().append(createParagraphNode().append(text))
  _ = try text.select(0, SEED_TEXT.utf16.count)
}

private func buildSeededEditor(
  _ override: Partial<GetClipboardDataConfig> = Partial()
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[test-root]", dependencies: [configExtension(GetClipboardDataExtension, override)],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in try initialEditorState() })))
}

private func buildSeededEditor(
  exportMimeType: ExportMimeTypeConfig
) throws -> Editor {
  try buildSeededEditor(Partial { $0.exportMimeType = exportMimeType })
}

private final class Calls {
  var values: [String] = []
}

@Suite struct GetClipboardDataExtensionTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > produces default text/plain, text/html, and application/x-lexical-editor entries"
    ))
  func defaults() throws {
    let editor = try buildSeededEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      #expect(data["text/plain"] == SEED_TEXT)
      #expect(data["text/html"]?.contains(SEED_TEXT) == true)

      let lexicalPayload = try #require(data["application/x-lexical-editor"])
      let parsed = try #require(try JSONValue.parse(lexicalPayload).objectValue)
      #expect(parsed["namespace"]?.stringValue == editor._config.namespace)
      let nodes = try #require(parsed["nodes"]?.arrayValue)
      #expect(nodes.count > 0)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > lets an override replace the default output for an existing MIME type"
    ))
  func overrideReplaces() throws {
    let editor = try buildSeededEditor(exportMimeType: ["text/plain": [{ _, _ in "OVERRIDDEN" }]])
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      #expect(data["text/plain"] == "OVERRIDDEN")
      // Other MIME types still come from the defaults below the override.
      #expect(data["text/html"]?.contains(SEED_TEXT) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > falls through to the default handler when the override calls next()"
    ))
  func fallsThrough() throws {
    let editor = try buildSeededEditor(exportMimeType: [
      "text/plain": [{ _, next in "[wrapped] \(try next() ?? "")" }]
    ])
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      #expect(data["text/plain"] == "[wrapped] \(SEED_TEXT)")
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > omits a MIME type when its top handler returns null without calling next()"
    ))
  func omitsOnNil() throws {
    let editor = try buildSeededEditor(exportMimeType: ["text/html": [{ _, _ in nil }]])
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      #expect(data["text/html"] == nil)
      // Defaults for other MIME types are unaffected.
      #expect(data["text/plain"] == SEED_TEXT)
      #expect(data["application/x-lexical-editor"] != nil)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > registers a brand-new custom MIME type"
    ))
  func customMimeType() throws {
    let editor = try buildSeededEditor(exportMimeType: [
      "application/x-myformat": [
        { selection, _ in
          try selection.map {
            JSONObject([("text", .string(try $0.getTextContent())), ("v", 1)]).stringify()
          }
        }
      ]
    ])
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      #expect(
        data["application/x-myformat"]
          == JSONObject([("text", .string(SEED_TEXT)), ("v", 1)]).stringify())
      // Defaults are still produced.
      #expect(data["text/plain"] == SEED_TEXT)
      #expect(data["text/html"]?.contains(SEED_TEXT) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > runs higher-indexed handlers first within a single MIME stack"
    ))
  func higherIndexFirst() throws {
    let calls = Calls()
    let editor = try buildSeededEditor(exportMimeType: [
      "text/plain": [
        { _, next in
          calls.values.append("A")
          return "A(\(try next() ?? ""))"
        },
        { _, next in
          calls.values.append("B")
          return "B(\(try next() ?? ""))"
        },
      ]
    ])
    defer { editor.dispose() }
    try editor.read { () throws in
      let data = try getClipboardDataFromSelection()
      // Stack after merge: [defaultFn, A, B]; B runs first, calls next() ->
      // A, A calls next() -> defaultFn (returns SEED_TEXT).
      #expect(data["text/plain"] == "B(A(\(SEED_TEXT)))")
      #expect(calls.values == ["B", "A"])
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > returns the configured stack output for known MIME types"
    ))
  func exportKnownMimeTypes() throws {
    let editor = try buildSeededEditor(exportMimeType: [
      "application/x-myformat": [{ _, _ in "custom-payload" }]
    ])
    defer { editor.dispose() }
    try editor.read { () throws in
      #expect(try exportMimeTypeFromSelection("text/plain") == SEED_TEXT)
      #expect(try exportMimeTypeFromSelection("application/x-myformat") == "custom-payload")
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > returns null for MIME types that have no registered handler"
    ))
  func exportUnknownMimeType() throws {
    let editor = try buildSeededEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      #expect(try exportMimeTypeFromSelection("application/x-unknown") == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > $exportMimeTypeFromSelection > uses the default config when GetClipboardDataExtension is not built into the editor"
    ))
  func exportWithoutExtension() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[test-root]",
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in try initialEditorState() })))
    defer { editor.dispose() }
    try editor.read { () throws in
      #expect(try exportMimeTypeFromSelection("text/plain") == SEED_TEXT)
      #expect(try exportMimeTypeFromSelection("text/html")?.contains(SEED_TEXT) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/GetClipboardDataExtension.test.ts::GetClipboardDataExtension > setLexicalClipboardDataTransfer wires custom MIME types into the DataTransfer"
    ))
  func setDataTransfer() throws {
    let editor = try buildSeededEditor(exportMimeType: [
      "application/x-myformat": [{ _, _ in "custom-payload" }]
    ])
    defer { editor.dispose() }
    let dt = DataTransfer()
    try editor.read {
      setLexicalClipboardDataTransfer(dt, try getClipboardDataFromSelection())
    }
    #expect(dt.getData("text/plain") == SEED_TEXT)
    #expect(dt.getData("text/html").contains(SEED_TEXT))
    #expect(dt.getData("application/x-myformat") == "custom-payload")
  }
}
