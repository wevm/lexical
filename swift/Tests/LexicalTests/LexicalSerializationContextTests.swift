import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalSerializationContext.test.ts. Upstream's
// `buildEditorFromExtensions` with `$initialEditorState` is an editor whose
// first update sets the document.

private func children(_ node: JSONObject) -> [JSONObject] {
  node["children"]?.arrayValue?.compactMap(\.objectValue) ?? []
}

private func compactEditor() throws -> Editor {
  let editor = try createEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
  try editor.update(discrete: true) {
    try getRoot().clear().append(
      try createParagraphNode().append(
        try createTextNode("plain"), try createTextNode("bold").setFormat("bold")))
  }
  return editor
}

private func rootJSON(_ editor: Editor, compact: Bool = false) throws -> JSONObject {
  try editor.read { try editor.getEditorState().toJSON(compact)["root"]!.objectValue! }
}

/// An editor whose document is one paragraph reading `hi`.
private func hiEditor(
  nodes: [LexicalNodeConfig] = [], _ initial: (() throws -> Void)? = nil
)
  throws -> Editor
{
  let editor = try createEditor(
    CreateEditorArgs(namespace: "", nodes: nodes, onError: { throw $0 }))
  try editor.update(discrete: true) {
    if let initial {
      try initial()
    } else {
      try getRoot().clear().append(try createParagraphNode().append(try createTextNode("hi")))
    }
  }
  return editor
}

nonisolated(unsafe) private var probeSeen: [Bool] = []

private final class ProbeNode: TextNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ProbeNode.self, type: "serialization-probe", extends: TextNode.self,
      json: [
        // Not import-only: a derived property is skipped by the compact walk
        // without its getter being called.
        SchemaField(
          "probe", rawValue(), of: ProbeNode.self,
          get: { _ in
            probeSeen.append(isCompactExport())
            return .bool(isCompactExport())
          }, set: { _, _ in })
      ])
  }
}

@Suite(.serialized) struct LexicalSerializationContextTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > legacy is the default and is unchanged"
    ))
  func legacyIsDefault() throws {
    let root = try rootJSON(try compactEditor())
    let paragraph = children(root)[0]
    let texts = children(paragraph)
    #expect(root["version"] == 1)
    #expect(paragraph["direction"] == .null)
    #expect(paragraph["format"] == "")
    #expect(paragraph["indent"] == 0)
    #expect(paragraph["type"] == "paragraph")
    #expect(paragraph["version"] == 1)
    #expect(texts[0]["detail"] == 0)
    #expect(texts[0]["format"] == 0)
    #expect(texts[0]["mode"] == "normal")
    #expect(texts[0]["style"] == "")
    #expect(texts[0]["text"] == "plain")
    #expect(texts[0]["type"] == "text")
    #expect(texts[0]["version"] == 1)
    #expect(texts[1]["format"] == 1)
    #expect(texts[1]["text"] == "bold")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > compact omits version and every default-valued property"
    ))
  func compactOmitsDefaults() throws {
    let root = try rootJSON(try compactEditor(), compact: true)
    let paragraph = children(root)[0]
    let texts = children(paragraph)
    #expect(root.has("version") == false)
    #expect(Array(paragraph).map(\.key).sorted() == ["children", "type"])
    // `JSONObject ==` compares key order too, unlike `toEqual`.
    #expect(texts[0] == JSONObject([("type", "text"), ("text", "plain")]))
    #expect(texts[1] == JSONObject([("type", "text"), ("format", 1), ("text", "bold")]))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > compact JSON parses back to the same document as legacy JSON"
    ))
  func compactParsesBack() throws {
    let editor = try compactEditor()
    let legacy = try rootJSON(editor)
    let compact = try rootJSON(editor, compact: true)
    #expect(compact.stringify().count < legacy.stringify().count)
    let fromCompact = try editor.parseEditorState(JSONObject([("root", .object(compact))]))
    let fromLegacy = try editor.parseEditorState(JSONObject([("root", .object(legacy))]))
    for state in [fromCompact, fromLegacy] {
      let texts = try state.read {
        try getRoot().getAllTextNodes().map { "\(try $0.getTextContent()):\(try $0.getFormat())" }
      }
      #expect(texts == ["plain:0", "bold:1"])
    }
    #expect(try fromCompact.toJSON() == fromLegacy.toJSON())
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > a formatted first text child survives compaction (#7971)"
    ))
  func formattedFirstChildSurvives() throws {
    let editor = try compactEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("bold").setFormat("bold").setStyle("color: red")))
    }
    let legacy = try rootJSON(editor)
    let compact = try rootJSON(editor, compact: true)
    #expect(children(compact)[0]["textFormat"] == 1)
    #expect(children(compact)[0]["textStyle"] == "color: red")
    for root in [legacy, compact] {
      let state = try editor.parseEditorState(JSONObject([("root", .object(root))]))
      let result = try state.read { () throws -> String in
        let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
        return "\(try paragraph.getTextFormat()) \(try paragraph.getTextStyle())"
      }
      #expect(result == "1 color: red")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export > the form does not leak outside its callback"
    ))
  func formDoesNotLeak() throws {
    let editor = try compactEditor()
    _ = try rootJSON(editor, compact: true)
    #expect(children(try rootJSON(editor))[0]["version"] == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::compact export: slot hosts > a decorator host keeps its slots in both forms"
    ))
  func decoratorHostKeepsSlots() throws {
    let editor = try hiEditor(
      nodes: [.klass(TestDecoratorNode.self), .klass(TestShadowRootNode.self)]
    ) {
      let host = try createTestDecoratorNode().setIsInline(false)
      let slot = try createTestShadowRootNode()
      try slot.append(try createParagraphNode().append(try createTextNode("inside")))
      try getRoot().clear().append(host)
      try setSlot(host, "body", slot)
    }
    for compact in [false, true] {
      let host = children(try rootJSON(editor, compact: compact))[0]
      #expect(host["type"] == "test_decorator")
      #expect(host["$slots"]?.stringify().contains("inside") == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > an explicit form wins over the ambient one, and types as itself"
    ))
  func explicitFormWins() throws {
    let state = try hiEditor().getEditorState()
    #expect(try state.toJSON().stringify().contains("hi"))
    #expect(try state.toJSON()["root"]!.objectValue!.has("version"))
    #expect(try state.toJSON(true)["root"]!.objectValue!.has("version") == false)
    try withCompactExport(true) { () throws in
      #expect(try state.toJSON(false)["root"]!.objectValue!.has("version"))
      #expect(try state.toJSON(true)["root"]!.objectValue!.has("version") == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > no argument writes the legacy form whatever encloses it"
    ))
  func noArgumentWritesLegacy() throws {
    let state = try hiEditor().getEditorState()
    #expect(try state.toJSON()["root"]!.objectValue!.has("version"))
    try withCompactExport(true) { () throws in
      #expect(try state.toJSON()["root"]!.objectValue!.has("version"))
      #expect(try state.toJSON(true)["root"]!.objectValue!.has("version") == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::editorState.toJSON states its form at the call site > a nested editor follows the document containing it"
    ))
  func nestedEditorFollowsDocument() throws {
    let editor = try hiEditor()
    let version = { () throws -> Bool in
      try editor.read {
        try editor.toJSON()["editorState"]!.objectValue!["root"]!.objectValue!.has("version")
      }
    }
    #expect(try version())
    try withCompactExport(true) { () throws in #expect(try version() == false) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > the export walk establishes it, in both directions"
    ))
  func walkEstablishesForm() throws {
    let editor = try hiEditor()
    try editor.read {
      #expect(isCompactExport() == false)
      withCompactExport(true) { #expect(isCompactExport()) }
      withCompactExport(true) {
        withCompactExport(false) { #expect(isCompactExport() == false) }
        #expect(isCompactExport())
      }
      #expect(isCompactExport() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > a schema getter observes it, which is what it is for"
    ))
  func schemaGetterObservesForm() throws {
    probeSeen = []
    let editor = try hiEditor(nodes: [.klass(ProbeNode.self)]) {
      // A TextNode subclass with no text is removed by normalization.
      try getRoot().clear().append(
        try createParagraphNode().append(try create(ProbeNode.self).setTextContent("x")))
    }
    let state = editor.getEditorState()
    probeSeen = []
    _ = try state.toJSON(false)
    _ = try state.toJSON(true)
    #expect(probeSeen == [false, true])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSerializationContext.test.ts::$isCompactExport reports the walk, not a single node > a bare node.exportJSON(true) does not establish it"
    ))
  func bareExportJSONDoesNotEstablish() throws {
    let editor = try hiEditor()
    try editor.read { () throws in
      _ = try getRoot().getFirstChildOrThrow().exportJSON(true)
      #expect(isCompactExport() == false)
    }
  }
}
