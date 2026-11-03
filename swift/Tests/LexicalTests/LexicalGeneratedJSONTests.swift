import Lexical
import LexicalTesting
import Testing

// Port of src/__tests__/unit/LexicalGeneratedJSON.test.ts. JS compiles
// per-class exporters and parsers from the schema as a speed-up; Swift
// has only the schema walk, so the tests of the generated code itself are
// skipped as deviations. The behavior they protect is ported here.

/// `Boxed` from "a property compared at run time omits exactly what the walk
/// omits", without its hand-written `generated` code.
private final class Boxed: LineBreakNode {
  var __box: JSONValue = .object(JSONObject([("w", .number(0))]))
  var __w: Double = 0
  override class func config() -> NodeConfig? {
    try! NodeConfig(
      Boxed.self, type: "run-time-compared-box", extends: LineBreakNode.self,
      json: [
        SchemaField("box", objectValue([("w", numberValue())]), field: \Boxed.__box),
        SchemaField("w", numberValue(), field: \Boxed.__w),
      ])
  }
}

nonisolated(unsafe) private let probeState = createState(
  "probeFlag", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let flatState = createState(
  "flatFlag", parse: { $0?.stringValue ?? "" })

private final class StatefulText: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      StatefulText.self, type: "stateful-text", extends: TextNode.self,
      stateConfigs: [RequiredNodeStateConfig(probeState)])
  }
}

private final class ReplacedText: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ReplacedText.self, type: "replaced-text", extends: TextNode.self)
  }
}

private final class FlatStateText: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FlatStateText.self, type: "flat-state-text", extends: TextNode.self,
      stateConfigs: [RequiredNodeStateConfig(flatState, flat: true)])
  }
}

private func object(_ text: String) -> JSONObject { json(text).objectValue! }

private func throwingEditor(_ nodes: [LexicalNodeConfig]) throws -> Editor {
  try createEditor(CreateEditorArgs(namespace: "", nodes: nodes, onError: { throw $0 }))
}

@Suite struct LexicalGeneratedJSONTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > an exportJSON override composes with its generated exporter"
    ))
  func paragraphKeyOrder() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode()
      try paragraph.append(try createTextNode("x").setFormat("bold"))
      let json = try paragraph.exportJSON()
      #expect(json["textFormat"] == .number(1))
      #expect(json["textStyle"] == .string(""))
      #expect(json["type"] == .string("paragraph"))
      #expect(
        json.keys == [
          "children", "direction", "format", "indent", "textFormat", "textStyle", "type", "version",
        ])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated exportJSON > NodeState still reaches the JSON through a generated exporter"
    ))
  func textWithoutStateHasNoStateKey() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      #expect(try createTextNode("hi").exportJSON().has(NODE_STATE_KEY) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the compact form is generated too > a property compared at run time omits exactly what the walk omits"
    ))
  func compactOmitsObjectAtDefault() throws {
    let editor = try throwingEditor([.klass(Boxed.self)])
    try editor.update(discrete: true) { () throws in
      let atDefault = try create(Boxed.self)
      atDefault.__box = .object(JSONObject([("w", .number(0))]))
      #expect(try atDefault.exportJSON(true).stringify() == #"{"type":"run-time-compared-box"}"#)
      let differs = try create(Boxed.self)
      differs.__box = .object(JSONObject([("w", .number(3))]))
      #expect(
        try differs.exportJSON(true).stringify()
          == #"{"type":"run-time-compared-box","box":{"w":3}}"#)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the compact form is generated too > it omits what parsing restores, and nothing else"
    ))
  func compactTextOmitsDefaults() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      #expect(try createTextNode("").exportJSON(true).stringify() == #"{"type":"text"}"#)
      #expect(
        try createTextNode("hi").setMode("token").exportJSON(true).stringify()
          == #"{"type":"text","mode":"token","text":"hi"}"#)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > NodeState in the JSON is applied"
    ))
  func nodeStateIsImported() throws {
    let editor = try throwingEditor([.klass(StatefulText.self)])
    try editor.update(discrete: true) {
      let node = try create(StatefulText.self)
      _ = try node.setTextContent("hi")
      _ = try setState(node, probeState, "kept")
      try getRoot().clear().append(try createParagraphNode().append(node))
    }
    let json = try editor.getEditorState().toJSON().stringify()
    #expect(json.contains("\"\(NODE_STATE_KEY)\""))
    try editor.parseEditorState(json).read { () throws in
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      #expect(try getState(try paragraph.getFirstChildOrThrow(), probeState) == "kept")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > a replacement of the class is imported as itself"
    ))
  func replacementImportedAsItself() throws {
    let editor = try throwingEditor([
      .klass(ReplacedText.self),
      .replacement(
        LexicalNodeReplacement(
          replace: TextNode.self, with: { try ReplacedText(($0 as! TextNode).__text) },
          withKlass: ReplacedText.self)),
    ])
    try editor.update(discrete: true) {
      let node = try createTextNode("hello").setStyle("color: red")
      #expect(node is ReplacedText)
      try getRoot().clear().append(try createParagraphNode().append(node))
    }
    let json = try editor.getEditorState().toJSON().stringify()
    try editor.parseEditorState(json).read { () throws in
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      let node = try paragraph.getFirstChildOrThrow()
      #expect(node is ReplacedText)
      #expect(try node.getTextContent() == "hello")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::the synthesized importJSON > flat NodeState is applied before the generated parser runs"
    ))
  func flatStateIsImported() throws {
    let editor = try throwingEditor([.klass(FlatStateText.self)])
    try editor.update(discrete: true) { () throws in
      let node = try FlatStateText.importJSON(
        object(
          #"{"flatFlag":"set","text":"hello","type":"flat-state-text","version":1}"#))
      #expect(try getState(node, flatState) == "set")
      #expect(try node.getTextContent() == "hello")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > a generated parser returns the node the walk would have"
    ))
  func paragraphUpdateFromJSON() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().clear().append(paragraph)
      let updated = try paragraph.getLatest().updateFromJSON(
        object(#"{"format":"center","indent":3}"#))
      #expect(updated === (try paragraph.getLatest()))
      #expect(try paragraph.getLatest().getFormatType() == "center")
      #expect(try paragraph.getLatest().getIndent() == 3)
      _ = try paragraph.getLatest().updateFromJSON(object(#"{"indent":-2}"#))
      #expect(try paragraph.getLatest().getIndent() == 0)
      _ = try paragraph.getLatest().updateFromJSON(object(#"{"indent":1.5}"#))
      #expect(try paragraph.getLatest().getIndent() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > a hostile key cannot reach Object.prototype through a table"
    ))
  func hostileKeysParseAsDefaults() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      for hostile in ["toString", "constructor", "hasOwnProperty", "__proto__", "valueOf"] {
        let node = try createTextNode("")
        var json = JSONObject()
        json["detail"] = .string(hostile)
        json["format"] = .string(hostile)
        json["mode"] = .string(hostile)
        json["style"] = .string("")
        json["text"] = .string("")
        _ = try node.updateFromJSON(json)
        #expect(try node.getFormat() == 0, "\(hostile)")
        #expect(try node.getDetail() == 0, "\(hostile)")
        #expect(try node.getMode() == "normal", "\(hostile)")
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalGeneratedJSON.test.ts::generated updateFromJSON > the legacy string spellings still parse"
    ))
  func legacyStringSpellings() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("")
      _ = try node.updateFromJSON(
        object(
          #"{"detail":"unmergeable","format":"bold","mode":"token","style":"","text":"x"}"#))
      #expect(try node.getFormat() == 1)
      #expect(try node.getDetail() == 2)
      #expect(try node.getMode() == "token")
    }
  }
}
