import LexicalTesting
import Testing

@testable import Lexical

// Port of the `a default is metadata`, `schema(undefined) is always the
// schema default`, `isEqual is total`, `a schema tracks what it accepts`,
// `the parse shape accepts what a parser actually accepts` and `a misspelled
// field name` suites of LexicalSchema.test.ts. Their type-level assertions
// have no Swift counterpart; the runtime parts are ported.

private class Deep1: ElementNode {
  var __p1 = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      Deep1.self, type: "deep-1", extends: ElementNode.self,
      json: [SchemaField("p1", stringValue(), field: \Deep1.__p1)])
  }
}

private class Deep2: Deep1 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep2.self, type: "deep-2", extends: Deep1.self)
  }
}

private class Deep3: Deep2 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep3.self, type: "deep-3", extends: Deep2.self)
  }
}

private class Deep4: Deep3 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep4.self, type: "deep-4", extends: Deep3.self)
  }
}

private class Deep5: Deep4 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep5.self, type: "deep-5", extends: Deep4.self)
  }
}

private class Deep6: Deep5 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep6.self, type: "deep-6", extends: Deep5.self)
  }
}

private class Deep7: Deep6 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep7.self, type: "deep-7", extends: Deep6.self)
  }
}

private class Deep8: Deep7 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep8.self, type: "deep-8", extends: Deep7.self)
  }
}

private class Deep9: Deep8 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep9.self, type: "deep-9", extends: Deep8.self)
  }
}

private class Deep10: Deep9 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep10.self, type: "deep-10", extends: Deep9.self)
  }
}

private class Deep11: Deep10 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep11.self, type: "deep-11", extends: Deep10.self)
  }
}

private class Deep12: Deep11 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep12.self, type: "deep-12", extends: Deep11.self)
  }
}

private class Deep13: Deep12 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep13.self, type: "deep-13", extends: Deep12.self)
  }
}

private class Deep14: Deep13 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep14.self, type: "deep-14", extends: Deep13.self)
  }
}

private class Deep15: Deep14 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep15.self, type: "deep-15", extends: Deep14.self)
  }
}

private class Deep16: Deep15 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep16.self, type: "deep-16", extends: Deep15.self)
  }
}

private class Deep17: Deep16 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep17.self, type: "deep-17", extends: Deep16.self)
  }
}

private class Deep18: Deep17 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep18.self, type: "deep-18", extends: Deep17.self)
  }
}

private class Deep19: Deep18 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep19.self, type: "deep-19", extends: Deep18.self)
  }
}

private class Deep20: Deep19 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep20.self, type: "deep-45", extends: Deep19.self)
  }
}

private class Deep21: Deep20 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep21.self, type: "deep-21", extends: Deep20.self)
  }
}

private class Deep22: Deep21 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep22.self, type: "deep-22", extends: Deep21.self)
  }
}

private class Deep23: Deep22 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep23.self, type: "deep-23", extends: Deep22.self)
  }
}

private class Deep24: Deep23 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep24.self, type: "deep-24", extends: Deep23.self)
  }
}

private class Deep25: Deep24 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep25.self, type: "deep-25", extends: Deep24.self)
  }
}

private class Deep26: Deep25 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep26.self, type: "deep-26", extends: Deep25.self)
  }
}

private class Deep27: Deep26 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep27.self, type: "deep-27", extends: Deep26.self)
  }
}

private class Deep28: Deep27 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep28.self, type: "deep-28", extends: Deep27.self)
  }
}

private class Deep29: Deep28 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep29.self, type: "deep-29", extends: Deep28.self)
  }
}

private class Deep30: Deep29 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep30.self, type: "deep-30", extends: Deep29.self)
  }
}

private class Deep31: Deep30 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep31.self, type: "deep-31", extends: Deep30.self)
  }
}

private class Deep32: Deep31 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep32.self, type: "deep-32", extends: Deep31.self)
  }
}

private class Deep33: Deep32 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep33.self, type: "deep-33", extends: Deep32.self)
  }
}

private class Deep34: Deep33 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep34.self, type: "deep-34", extends: Deep33.self)
  }
}

private class Deep35: Deep34 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep35.self, type: "deep-35", extends: Deep34.self)
  }
}

private class Deep36: Deep35 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep36.self, type: "deep-36", extends: Deep35.self)
  }
}

private class Deep37: Deep36 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep37.self, type: "deep-37", extends: Deep36.self)
  }
}

private class Deep38: Deep37 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep38.self, type: "deep-38", extends: Deep37.self)
  }
}

private class Deep39: Deep38 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep39.self, type: "deep-39", extends: Deep38.self)
  }
}

private class Deep40: Deep39 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep40.self, type: "deep-40", extends: Deep39.self)
  }
}

private class Deep41: Deep40 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep41.self, type: "deep-41", extends: Deep40.self)
  }
}

private class Deep42: Deep41 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep42.self, type: "deep-42", extends: Deep41.self)
  }
}

private class Deep43: Deep42 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep43.self, type: "deep-43", extends: Deep42.self)
  }
}

private class Deep44: Deep43 {
  override class func config() throws -> NodeConfig? {
    NodeConfig(Deep44.self, type: "deep-44", extends: Deep43.self)
  }
}

private final class Deep45: Deep44 {
  var __p45 = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      Deep45.self, type: "deep-45", extends: Deep44.self,
      json: [SchemaField("p45", stringValue(), field: \Deep45.__p45)])
  }
}

private final class CaptionNode: DecoratorNode {
  var __caption = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CaptionNode.self, type: "caption-no-extends",
      json: [SchemaField("caption", stringValue(), field: \CaptionNode.__caption)])
  }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private class TagBase: ElementNode {
  var __tag = "a"
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TagBase.self, type: "tag-base", extends: ElementNode.self,
      json: [SchemaField("tag", enumValue(["a", "b", "c"]), field: \TagBase.__tag)])
  }
}

private final class TagSub: TagBase {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TagSub.self, type: "tag-sub", extends: TagBase.self,
      json: [
        SchemaField(
          "tag", aliasedValue(enumValue(["a", "b", "c"]), ["third": "c"]), field: \TagSub.__tag)
      ])
  }
}

private final class OptionalFieldNode: ElementNode {
  var __caption: String? = nil
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      OptionalFieldNode.self, type: "optional-field-node", extends: ElementNode.self,
      json: [
        SchemaField("caption", optional(stringValue()), field: \OptionalFieldNode.__caption)
      ])
  }
}

private func editorWith(_ nodes: [LexicalNode.Type]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

@Suite struct LexicalSchemaCompositionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a default is metadata, so nothing hands out a mutable one > a union yields a fresh parse rather than a member default"
    ))
  func unionFreshParse() throws {
    let schema = try unionValue([arrayValue(stringValue())])
    #expect(schema(json(#"["a"]"#)) == json(#"["a"]"#))
    #expect(schema(json("[]")) == json("[]"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::schema(undefined) is always the schema default > enumValue reads an absent value as its default, not as a member"
    ))
  func enumAbsentIsDefault() throws {
    let schema = try enumValue([nil, "middle", "bottom"], default: "middle")
    #expect(schema.defaultValue == "middle")
    #expect(schema(nil) == "middle")
    #expect(schema("bottom") == "bottom")
    let leading = try enumValue([nil, "middle"])
    #expect(leading.defaultValue == nil)
    #expect(leading(nil) == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > an array with holes is not equal to a dense one of the same length"
    ))
  func holesNotEqual() throws {
    let schema = try arrayValue(stringValue())
    // A hole reads as `undefined`, which a JSON array writes as `null`.
    #expect(schema.isEqual!(json("[null,null,null]"), json(#"["a","b","c"]"#)) == false)
    #expect(schema.isEqual!(json(#"["a","b"]"#), json(#"["a","b"]"#)))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > an object carrying undeclared keys is not equal to the default"
    ))
  func undeclaredKeysNotEqual() throws {
    let schema = try objectValue([("x", numberValue())])
    #expect(schema.isEqual!(json(#"{"x":0}"#), schema.defaultValue))
    #expect(schema.isEqual!(json(#"{"extra":"keep","x":0}"#), schema.defaultValue) == false)
    #expect(schema.isEqual!(json("[]"), schema.defaultValue) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::isEqual is total over the values a getter can return > transformValue takes an isEqual for a reference-typed output"
    ))
  func transformIsEqual() throws {
    let plain = try transformValue(arrayValue(stringValue())) { $0 }
    #expect(plain.isEqual == nil)
    let compared = try transformValue(arrayValue(stringValue()), { $0 }) { a, b in
      a?.arrayValue?.count == b?.arrayValue?.count
    }
    #expect(compared.isEqual!(json("[]"), compared.defaultValue))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a primitive accepts what it parses, except where it reads more"
    ))
  func primitiveAccepts() throws {
    #expect(stringValue()("x") == "x")
    #expect(booleanValue()(true) == true)
    #expect(try numberValue()("2") == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > an alias table widens the accepted input, not the output"
    ))
  func aliasWidensInput() throws {
    let format = try aliasedValue(numberValue(), ["bold": 1, "italic": 2])
    #expect(format("bold") == 1)
    #expect(format("2") == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > the wrappers widen their inner the way they widen its output"
    ))
  func wrappersWiden() throws {
    let s = stringValue()
    #expect(s("x") == "x")
    #expect(try optional(s)(nil) == nil)
    #expect(try nullable(s)(.null) == .null)
    #expect(try arrayValue(numberValue())(json(#"["1",2]"#)) == json("[1,2]"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a union accepts what any member accepts"
    ))
  func unionAcceptsAnyMember() throws {
    let dimension = try unionValue([numberValue(), enumValue(["inherit"])], default: "inherit")
    #expect(dimension("640") == 640)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a node composes the input it accepts across its config chain"
    ))
  func paragraphComposedFields() {
    #expect(
      Set(getComposedSchemaFields(ParagraphNode.self).keys) == [
        "direction", "format", "indent", "textFormat", "textStyle",
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a chain deeper than any bound resolves, base included"
    ))
  func deepChain() {
    #expect(
      getComposedSchemaFields(Deep45.self).keys.sorted() == [
        "direction", "format", "indent", "p1", "p45", "textFormat", "textStyle",
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a node that omits extends keeps its own schema"
    ))
  func omitsExtends() {
    #expect(getComposedSchemaFields(CaptionNode.self)["caption"] != nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > a re-declared property is the subclass’s, not the intersection"
    ))
  func redeclaredProperty() {
    #expect(getComposedSchemaFields(TagSub.self)["tag"]?("third") == "c")
    #expect(getComposedSchemaFields(TagBase.self)["tag"]?("third") == "a")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema tracks what it accepts, not only what it parses to > an object accepts each property’s input, any of them absent"
    ))
  func objectAcceptsAbsent() throws {
    let point = try objectValue([("label", stringValue()), ("x", numberValue())])
    #expect(point(json(#"{"x":"3"}"#)) == json(#"{"label":"","x":3}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::the parse shape accepts what a parser actually accepts > a legacy alias and a stringified number type-check as input"
    ))
  func legacyAliasInput() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("hi")
      _ = try node.updateFromJSON(jsonObject(#"{"detail":"directionless","format":"bold"}"#))
      #expect(try node.getFormat() == IS_BOLD)
      #expect(try node.isDirectionless())
    }
  }

  // JS refuses the unknown key at the type level only; at runtime it is
  // ignored, as here.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::the parse shape accepts what a parser actually accepts > a property the node does not have is still refused"
    ))
  func unknownPropertyIgnored() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      _ = try createTextNode("hi").updateFromJSON(jsonObject(#"{"frmat":"bold"}"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a misspelled field name is caught in both directions > an initialized optional field is an own property"
    ))
  func optionalField() throws {
    let editor = try editorWith([OptionalFieldNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try create(OptionalFieldNode.self)
      let exported = try node.exportJSON()
      #expect(exported["caption"] == nil)
      #expect(exported["type"] == "optional-field-node")
      #expect(
        try node.updateFromJSON(jsonObject(#"{"caption":"hi"}"#)).exportJSON()["caption"] == "hi")
    }
  }
}
