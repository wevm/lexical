import LexicalTesting
import Testing

@testable import Lexical

// Port of the `defaults and untrusted input`, `the export and parse shapes
// differ only where parsing is looser`, `withField compiles to direct field
// access`, `a field stands in for its accessor only while nobody overrides
// it` and `reference-typed defaults compact by content` suites of
// LexicalSchema.test.ts.

private func editorWith(_ nodes: [LexicalNode.Type]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

@Suite struct LexicalSchemaDefaultsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a declared fallback survives a member that accepts undefined"
    ))
  func declaredFallback() throws {
    #expect(try unionValue([rawValue(), numberValue()], default: 5).defaultValue == 5)
    #expect(
      try unionValue([optional(numberValue()), enumValue(["inherit"])], default: "inherit")
        .defaultValue == "inherit")
    #expect(try enumValue([nil, "middle", "bottom"], default: "middle").defaultValue == "middle")
    #expect(
      try unionValue([optional(numberValue()), stringValue()], default: .some(nil)).defaultValue
        == nil)
    _ = try unionValue([numberValue(), stringValue()], default: .some(nil))
    #expect(try enumValue(["middle", nil], default: .some(nil)).defaultValue == nil)
    #expect(try enumValue(["middle", nil]).defaultValue == "middle")
    #expect {
      try enumValue(["middle", "bottom"], default: .some(nil))
    } throws: { "\($0)".contains("enumValue: the default value is not one of the values") }
    #expect {
      try enumValue(["a", "b"], default: "c")
    } throws: { "\($0)".contains("enumValue: the default value is not one of the values") }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a schema shape describes what a property parses to, not what it reads"
    ))
  func schemaShapeDefault() throws {
    let schema = try objectValue([("count", numberValue()), ("label", stringValue())])
    #expect(schema.defaultValue == json(#"{"count":0,"label":""}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a union accepts exactly what its members accept"
    ))
  func unionAcceptsNaN() throws {
    let schema = try unionValue([enumValue([.number(.nan), 1]), numberValue()], default: 1)
    guard case .number(let n)? = schema(.number(.nan)) else {
      Issue.record("not a number")
      return
    }
    #expect(n.isNaN)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a reference-typed default cannot be mutated into every node"
    ))
  func defaultIsAValue() throws {
    let schema = try arrayValue(stringValue())
    var leaked = schema.defaultValue?.arrayValue ?? []
    leaked.append("leak")
    #expect(schema.defaultValue == json("[]"))
    var parsed = schema(json(#"["a"]"#))?.arrayValue ?? []
    parsed.append("b")
    #expect(parsed == ["a", "b"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::defaults and untrusted input > a field named for an Object.prototype member is refused"
    ))
  func objectPrototypeFieldRefused() throws {
    #expect {
      try objectValue([("toString", rawValue())])
    } throws: { "\($0)".contains(#"objectValue: "toString" is a member of Object.prototype"#) }
    #expect {
      try objectValue([("__proto__", rawValue())])
    } throws: { "\($0)".contains(#""__proto__" is not a valid field name"#) }
    let schema = try objectValue([("label", rawValue())])
    #expect(schema(json("{}")) == json("{}"))
    #expect(schema(json(#"{"label":"mine"}"#)) == json(#"{"label":"mine"}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::the export and parse shapes differ only where parsing is looser > exportJSON(true) type-checks as the value it actually returns"
    ))
  func compactExportHasNoVersion() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createParagraphNode()
      #expect(try node.exportJSON(true).has("version") == false)
      #expect(try node.exportJSON()["version"] == 1)
    }
  }
}

/// A node whose `label` property is its `__label` field, with accessors
/// that count their calls.
private final class FieldNode: ElementNode {
  var __label = "default"
  var calls = 0
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      FieldNode.self, type: "field-node", extends: ElementNode.self,
      json: [
        SchemaField(
          "label", stringValue("default"), of: FieldNode.self, get: { .string($0.__label) },
          set: { $0.__label = $1?.stringValue ?? "default" })
      ])
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __label = (prevNode as! FieldNode).__label
  }
  func setLabel(_ label: String) throws -> Self {
    let writable = try getWritable()
    writable.calls += 1
    writable.__label = "via-setter:\(label)"
    return writable
  }
  func getLabel() throws -> String { "via-getter:\(try getLatest().__label)" }
}

private final class LoudTextNode: TextNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(LoudTextNode.self, type: "loud-text", extends: TextNode.self)
  }
  override func getStyle() throws -> String { try super.getStyle() + ";loud" }
  override func setStyle(_ style: String) throws -> Self { try super.setStyle(style + ";set") }
}

private final class QuietTextNode: TextNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(QuietTextNode.self, type: "quiet-text", extends: TextNode.self)
  }
}

private final class WrappedElementNode: ElementNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(WrappedElementNode.self, type: "wrapped-element", extends: ElementNode.self)
  }
  override func getTextFormat() throws -> Int { try super.getTextFormat() | 1 }
}

private final class TagsNode: ElementNode {
  var __tags: [String] = []
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TagsNode.self, type: "tags-node", extends: ElementNode.self,
      json: [
        SchemaField(
          "tags", arrayValue(stringValue()), of: TagsNode.self,
          get: { .array($0.__tags.map(JSONValue.string)) },
          set: { node, value in node.__tags = value?.arrayValue?.compactMap(\.stringValue) ?? [] })
      ])
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __tags = (prevNode as! TagsNode).__tags
  }
}

@Suite struct LexicalSchemaFieldTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::withField compiles to direct field access > the field is read and written directly, not through the methods"
    ))
  func fieldReadDirectly() throws {
    let editor = try editorWith([FieldNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try #require(
        try FieldNode.importJSON(jsonObject(#"{"label":"hello","type":"field-node"}"#))
          as? FieldNode)
      #expect(node.__label == "hello")
      #expect(node.calls == 0)
      #expect(try node.exportJSON()["label"] == "hello")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::withField compiles to direct field access > an absent property still parses to the schema default"
    ))
  func absentPropertyDefault() throws {
    let editor = try editorWith([FieldNode.self])
    try editor.update(discrete: true) { () throws in
      let bare = try #require(
        try FieldNode.importJSON(jsonObject(#"{"type":"field-node"}"#)) as? FieldNode)
      #expect(bare.__label == "default")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > an overridden accessor is consulted in both directions"
    ))
  func overriddenAccessorConsulted() throws {
    let editor = try editorWith([LoudTextNode.self, QuietTextNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try create(LoudTextNode.self).setStyle("color: red")
      #expect(try node.getStyle() == "color: red;set;loud")
      #expect(try node.exportJSON()["style"] == "color: red;set;loud")
      let parsed = try create(LoudTextNode.self).updateFromJSON(
        jsonObject(#"{"style":"color: blue","text":"x"}"#))
      #expect(try parsed.getStyle() == "color: blue;set;loud")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > a property whose accessor is not overridden keeps the field path"
    ))
  func unoverriddenAccessorFieldPath() throws {
    let editor = try editorWith([LoudTextNode.self, QuietTextNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try create(LoudTextNode.self).setTextContent("hello")
      #expect(try node.exportJSON()["text"] == "hello")
      let quiet = try create(QuietTextNode.self).setStyle("color: red")
      #expect(try quiet.exportJSON()["style"] == "color: red")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > a spelled accessor is guarded alongside the one it wraps"
    ))
  func wrappedAccessorGuarded() throws {
    let editor = try editorWith([WrappedElementNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try create(WrappedElementNode.self).setTextFormat(2)
      #expect(try node.exportJSON()["textFormat"] == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a field stands in for its accessor only while nobody overrides it > setterTable and getterTable carry a property whose two forms differ"
    ))
  func modeTables() throws {
    let editor = try editorWith([LoudTextNode.self, QuietTextNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try create(QuietTextNode.self).setMode("segmented")
      #expect(try node.exportJSON()["mode"] == "segmented")
      let parsed = try create(QuietTextNode.self).updateFromJSON(
        jsonObject(#"{"mode":"token","text":"x"}"#))
      #expect(try parsed.getMode() == "token")
      #expect(try parsed.exportJSON()["mode"] == "token")
      let bogus = try create(QuietTextNode.self).updateFromJSON(
        jsonObject(#"{"mode":"nonsense","text":"x"}"#))
      #expect(try bogus.getMode() == "normal")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > an array equal to its default is dropped, a differing one is not"
    ))
  func arrayCompactsByContent() throws {
    let ids = try arrayValue(stringValue())
    #expect(ids.isEqual!(json("[]"), json("[]")))
    #expect(ids.isEqual!(json(#"["a"]"#), json(#"["a"]"#)))
    #expect(ids.isEqual!(json(#"["a"]"#), json(#"["b"]"#)) == false)
    #expect(ids.isEqual!(json(#"["a"]"#), json("[]")) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > an object compares by its declared fields"
    ))
  func objectComparesByFields() throws {
    let point = try objectValue([("x", numberValue()), ("y", numberValue())])
    #expect(point.isEqual!(json(#"{"x":0,"y":0}"#), json(#"{"x":0,"y":0}"#)))
    #expect(point.isEqual!(json(#"{"x":1,"y":0}"#), json(#"{"x":0,"y":0}"#)) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::reference-typed defaults compact by content > a node with an array property compacts it away when empty"
    ))
  func emptyArrayCompactsAway() throws {
    let editor = try editorWith([TagsNode.self])
    try editor.update(discrete: true) { () throws in
      let empty = try create(TagsNode.self)
      let full = try create(TagsNode.self)
      full.__tags = ["a"]
      try getRoot().append(empty, full)
    }
    let root = try #require(try editor.getEditorState().toJSON(true)["root"]?.objectValue)
    let children = try #require(root["children"]?.arrayValue)
    #expect(children[0].objectValue?.has("tags") == false)
    #expect(children[1].objectValue?["tags"] == json(#"["a"]"#))
    let tags = try editor.parseEditorState(JSONObject([("root", .object(root))])).read {
      try getRoot().getChildren().map { ($0 as! TagsNode).__tags }
    }
    #expect(tags == [[], ["a"]])
  }
}
