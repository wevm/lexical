import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts.

private let selectionSerializationExtension = defineExtension(
  name: "[selection-serialization]", dependencies: [RichTextExtension],
  editorConfig: InitialEditorConfig(
    initialEditorState: .update { _ in
      _ = try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("plain"), createTextNode("secret").setFormat("bold")))
    }))

private func buildRootEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[root]", dependencies: [selectionSerializationExtension],
      editorConfig: InitialEditorConfig(namespace: "", onError: { error, _ in throw error })))
}

private func selectionJSON(_ compact: Bool = false) throws -> [JSONObject] {
  let editor = try buildRootEditor()
  defer { editor.dispose() }
  var nodes: [JSONObject] = []
  try editor.update(discrete: true) {
    _ = try selectAll()
    nodes = try withCompactExport(compact) {
      try generateJSONFromSelectedNodes(editor, getSelection()).nodes
    }
  }
  return nodes
}

/// `toEqual`/`toMatchObject` compare keys regardless of order.
private func entries(_ object: JSONObject) -> [String: JSONValue] {
  Dictionary(uniqueKeysWithValues: object.map { ($0.key, $0.value) })
}

// A RangeSelection over a paragraph's contents yields its text nodes at the
// top level of the payload, with no paragraph wrapper.
@Suite struct SelectionSerializationContextTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > legacy is the default"
    ))
  func legacyIsDefault() throws {
    let plain = try #require(try selectionJSON().first)
    let expected: [String: JSONValue] = [
      "detail": 0, "format": 0, "mode": "normal", "style": "", "text": "plain", "type": "text",
      "version": 1,
    ]
    let actual = entries(plain)
    for (key, value) in expected {
      #expect(actual[key] == value, "\(key)")
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > compact applies to a selection export too"
    ))
  func compactApplies() throws {
    let nodes = try selectionJSON(true)
    #expect(
      nodes.map(entries) == [
        ["text": "plain", "type": "text"], ["format": 1, "text": "secret", "type": "text"],
      ])
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::selection export honors the serialization context > the legacy form can be forced at a call site"
    ))
  func legacyForced() throws {
    // Whatever an editor is configured to do, a caller that needs the old
    // format can ask for it around any export.
    let plain = try #require(try selectionJSON(false).first)
    #expect(plain["version"] == 1)
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SelectionSerializationContext.test.ts::the serialized nodes parse back as a document without a cast"
    ))
  func parsesBack() throws {
    let editor = try buildRootEditor()
    defer { editor.dispose() }
    var nodes: [JSONObject] = []
    try editor.update(discrete: true) {
      // The paragraph itself, so the payload is a subtree a root can hold.
      let selection = createNodeSelection()
      selection.add(try getRoot().getFirstChildOrThrow().getKey())
      nodes = try generateJSONFromSelectedNodes(editor, selection).nodes
    }
    let state = try editor.parseEditorState(
      JSONObject([
        (
          "root",
          .object(
            JSONObject([
              ("children", .array(nodes.map { .object($0) })), ("direction", .null),
              ("format", ""), ("indent", 0), ("type", "root"), ("version", 1),
            ]))
        )
      ]))
    #expect(try state.read { try getRoot().getTextContent() } == "plainsecret")
  }
}
