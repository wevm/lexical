import LexicalTesting
import Testing

@testable import Lexical

// Port of the `LexicalSchema value schemas` suite and the first
// `updateFromJSON tolerates partial and out-of-domain JSON` tests of
// LexicalSchema.test.ts. A schema's domain is `JSONValue?`, where `nil` is
// `undefined`.

/// Parses JSON text, for test inputs.
func json(_ text: String) -> JSONValue {
  try! JSONValue.parse(text)
}

/// Parses a JSON object, for test inputs.
func jsonObject(_ text: String) -> JSONObject {
  json(text).objectValue!
}

@Suite struct LexicalSchemaValueTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > stringValue"
    ))
  func string() {
    let parse = stringValue()
    #expect(parse("hello") == "hello")
    #expect(parse("") == "")
    #expect(parse(nil) == "")
    #expect(parse(.null) == "")
    #expect(parse(42) == "")
    #expect(parse(json("{}")) == "")
    #expect(stringValue("fallback")(nil) == "fallback")
    #expect(stringValue("fallback")(42) == "fallback")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > numberValue"
    ))
  func number() throws {
    let parse = try numberValue()
    #expect(parse(42) == 42)
    #expect(parse(0) == 0)
    #expect(parse(-1.5) == -1.5)
    #expect(parse("42") == 42)
    #expect(parse(nil) == 0)
    #expect(parse("forty-two") == 0)
    #expect(parse(.number(.nan)) == 0)
    #expect(parse(.number(.infinity)) == 0)
    #expect(parse(.number(-.infinity)) == 0)
    #expect(try numberValue(1)(nil) == 1)
    #expect(try numberValue(1)(.number(.nan)) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > booleanValue"
    ))
  func boolean() {
    let parse = booleanValue()
    #expect(parse(true) == true)
    #expect(parse(false) == false)
    #expect(parse(nil) == false)
    #expect(parse(0) == false)
    #expect(parse("true") == false)
    #expect(booleanValue(true)(nil) == true)
    #expect(booleanValue(true)(1) == true)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > enumValue"
    ))
  func enumeration() throws {
    let parse = try enumValue(["normal", "token", "segmented"])
    #expect(parse("token") == "token")
    #expect(parse("bogus") == "normal")
    #expect(parse(nil) == "normal")
    #expect(parse(42) == "normal")
    #expect(try enumValue(["normal", "token"], default: "token")("bogus") == "token")
    let direction = try enumValue([.null, "ltr", "rtl"])
    #expect(direction("rtl") == "rtl")
    #expect(direction("bogus") == .null)
    #expect(throws: LexicalError.self) { try enumValue([]) }
    #expect {
      try enumValue([])
    } throws: { error in
      "\(error)".contains("must not be empty")
    }
    #expect(direction(.null) == .null)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > nullable"
    ))
  func nullableValue() throws {
    let parse = try nullable(stringValue())
    #expect(parse("noopener") == "noopener")
    #expect(parse("") == "")
    #expect(parse(.null) == .null)
    #expect(parse(nil) == .null)
    #expect(parse(42) == "")
    let parseDirection = try nullable(enumValue(["ltr", "rtl"]))
    #expect(parseDirection("rtl") == "rtl")
    #expect(parseDirection(.null) == .null)
    #expect(parseDirection(nil) == .null)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > optional"
    ))
  func optionalValue() throws {
    let parse = try optional(numberValue())
    #expect(parse(120) == 120)
    #expect(parse(nil) == nil)
    #expect(parse(.null) == 0)
    #expect(parse("nope") == 0)
    let parseLanguage = try optional(nullable(stringValue()))
    #expect(parseLanguage(nil) == nil)
    #expect(parseLanguage(.null) == .null)
    #expect(parseLanguage("js") == "js")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > arrayValue"
    ))
  func array() throws {
    let parse = try arrayValue(stringValue())
    #expect(parse(json(#"["a","b"]"#)) == json(#"["a","b"]"#))
    #expect(parse(json("[]")) == json("[]"))
    #expect(parse(json(#"["a",42,null]"#)) == json(#"["a","",""]"#))
    #expect(parse(nil) == json("[]"))
    #expect(parse("nope") == json("[]"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > objectValue validates and fully populates"
    ))
  func object() throws {
    let parse = try objectValue([
      ("count", numberValue()), ("tags", arrayValue(stringValue())), ("title", stringValue()),
    ])
    #expect(
      parse(json(#"{"count":2,"tags":["x"],"title":"hi"}"#))
        == json(#"{"count":2,"tags":["x"],"title":"hi"}"#))
    #expect(
      parse(json(#"{"count":"bad","extra":true}"#)) == json(#"{"count":0,"tags":[],"title":""}"#))
    #expect(parse(nil) == json(#"{"count":0,"tags":[],"title":""}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::LexicalSchema value schemas > the default is recoverable via parse(undefined)"
    ))
  func defaultFromUndefined() throws {
    #expect(stringValue()(nil) == "")
    #expect(try numberValue(7)(nil) == 7)
    #expect(booleanValue(true)(nil) == true)
    #expect(try enumValue(["a", "b"], default: "b")(nil) == "b")
    #expect(try nullable(stringValue())(nil) == .null)
  }
}

/// A document with one paragraph holding `children`.
private func paragraphDocument(_ children: String) -> String {
  #"{"root":{"children":[{"children":["# + children
    + #"],"type":"paragraph","version":1}],"type":"root","version":1}}"#
}

@Suite struct LexicalSchemaUpdateFromJSONTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode applies defaults when properties are missing"
    ))
  func textNodeDefaults() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("initial")
      _ = try node.updateFromJSON(JSONObject())
      #expect(try node.getTextContent() == "")
      #expect(try node.getFormat() == 0)
      #expect(try node.getDetail() == 0)
      #expect(try node.getMode() == "normal")
      #expect(try node.getStyle() == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode rejects out-of-domain values"
    ))
  func textNodeRejectsOutOfDomain() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("initial")
      _ = try node.updateFromJSON(
        jsonObject(#"{"detail":"nope","format":null,"mode":"bogus","style":42,"text":99}"#))
      #expect(try node.getTextContent() == "")
      #expect(try node.getFormat() == 0)
      #expect(try node.getDetail() == 0)
      #expect(try node.getMode() == "normal")
      #expect(try node.getStyle() == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode preserves valid values"
    ))
  func textNodePreservesValid() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("initial")
      _ = try node.updateFromJSON(
        jsonObject(
          #"{"detail":0,"format":1,"mode":"token","style":"color: red","text":"hello"}"#))
      #expect(try node.getTextContent() == "hello")
      #expect(try node.getFormat() == 1)
      #expect(try node.getMode() == "token")
      #expect(try node.getStyle() == "color: red")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > parseEditorState round-trips compact node JSON"
    ))
  func parseCompactJSON() throws {
    let editor = try createTestEditor()
    let editorState = try editor.parseEditorState(
      paragraphDocument(#"{"text":"hello","type":"text","version":1}"#))
    try editorState.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 1)
      #expect(try textNodes[0].getTextContent() == "hello")
      #expect(try textNodes[0].getFormat() == 0)
      #expect(try textNodes[0].getDetail() == 0)
      #expect(try textNodes[0].getMode() == "normal")
      #expect(try textNodes[0].getStyle() == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TextNode accepts the legacy string names for format and detail"
    ))
  func textNodeLegacyNames() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("x")
      _ = try node.updateFromJSON(jsonObject(#"{"detail":"directionless","format":"bold"}"#))
      #expect(try node.hasFormat("bold"))
      #expect(try node.isDirectionless())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TabNode ignores JSON for its fixed properties"
    ))
  func tabNodeFixedProperties() throws {
    let editor = try createTestEditor()
    for bad in [
      #","mode":"token""#, #","detail":0"#, #","text":"xyz""#,
      #","detail":"directionless","mode":"segmented","text":"""#,
    ] {
      let state = try editor.parseEditorState(
        paragraphDocument(#"{"type":"tab","version":1"# + bad + "}"))
      try state.read { () throws in
        let tab = try #require(try getRoot().getAllTextNodes().first)
        #expect(isTabNode(tab))
        #expect(try tab.getTextContent() == "\t")
        #expect(try tab.isUnmergeable())
        #expect(try tab.getMode() == "normal")
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > TabNode imports fully compact JSON"
    ))
  func tabNodeCompact() throws {
    let editor = try createTestEditor()
    let editorState = try editor.parseEditorState(
      paragraphDocument(
        #"{"type":"tab","version":1},{"detail":"bogus","text":"xyz","type":"tab","version":1}"#))
    try editorState.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 2)
      for node in textNodes {
        #expect(isTabNode(node))
        #expect(try node.getTextContent() == "\t")
        #expect(try node.isUnmergeable())
      }
    }
  }
}

@Suite struct LexicalSchemaDomainTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > min rejects values below the bound"
    ))
  func minBound() throws {
    let span = try numberValue(1, min: 1)
    #expect(span(3) == 3)
    #expect(span(0) == 1)
    #expect(span(-4) == 1)
    #expect(span(nil) == 1)
    #expect(span.defaultValue == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > integer rejects fractional values"
    ))
  func integerDomain() throws {
    let span = try numberValue(1, integer: true, min: 1)
    #expect(span(2) == 2)
    #expect(span(2.5) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > max rejects values above the bound"
    ))
  func maxBound() throws {
    let pct = try numberValue(0, min: 0, max: 100)
    #expect(pct(100) == 100)
    #expect(pct(101) == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > clamp brings an out-of-range value to the nearest bound"
    ))
  func clamp() throws {
    let indent = try numberValue(0, integer: true, min: 0, max: 128, clamp: true)
    #expect(indent(5) == 5)
    #expect(indent(1e6) == 128)
    #expect(indent(-4) == 0)
    #expect(indent("1e6") == 128)
    #expect(indent(1.5) == 0)
    #expect(indent(.number(.infinity)) == 0)
    #expect(indent("banana") == 0)
    #expect(indent(nil) == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > an integer domain rounds its bounds inward"
    ))
  func integerBoundsRoundInward() throws {
    let parse = try numberValue(1, integer: true, min: 0.5, max: 2.5, clamp: true)
    #expect(parse(0) == 1)
    #expect(parse(3) == 2)
    #expect(parse(parse(0)) == parse(0))
    #expect(parse(parse(3)) == parse(3))
    guard case .number(let integer, let min, let max, let clamp) = parse.meta else {
      Issue.record("not a number schema")
      return
    }
    #expect(integer && clamp && min == 1 && max == 2)
    let strict = try numberValue(1, integer: true, min: 0.5, max: 2.5)
    #expect(strict(0) == 1)
    #expect(strict(1) == 1)
    #expect(strict(2) == 2)
    #expect(strict(3) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > a fractional domain keeps its bounds"
    ))
  func fractionalBounds() throws {
    guard
      case .number(let integer, let min, let max, let clamp) = try numberValue(
        1, min: 0.5, max: 2.5
      ).meta
    else {
      Issue.record("not a number schema")
      return
    }
    #expect(!integer && !clamp && min == 0.5 && max == 2.5)
    #expect(try numberValue(1, min: 0.5, max: 2.5, clamp: true)(3) == 2.5)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > an empty domain is refused where it is declared"
    ))
  func emptyDomainRefused() {
    #expect {
      try numberValue(0, min: 2, max: 1)
    } throws: { "\($0)".contains("the domain is empty; min 2 is above max 1") }
    #expect {
      try numberValue(0, integer: true, min: 0.5, max: 0.7)
    } throws: { "\($0)".contains("rounded inward from 0.5..0.7") }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > clamp widens what the domain accepts"
    ))
  func clampWidensAccepts() throws {
    #expect(try numberValue(0, max: 10, clamp: true).accepts!(99) == true)
    #expect(try numberValue(0, max: 10).accepts!(99) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > numberValue domain options > the domain is recorded on meta"
    ))
  func domainOnMeta() throws {
    if case .number(let integer, let min, let max, let clamp) = try numberValue(
      1, integer: true, min: 1
    ).meta {
      #expect(integer && !clamp && min == 1 && max == nil)
    } else {
      Issue.record("not a number schema")
    }
    if case .number(let integer, let min, let max, let clamp) = try numberValue(
      0, max: 8, clamp: true
    ).meta {
      #expect(!integer && clamp && min == nil && max == 8)
    } else {
      Issue.record("not a number schema")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > treats a default-valued input as absent"
    ))
  func omitDefaultAbsent() throws {
    let width = try optional(numberValue(), omitDefault: true)
    #expect(width(120) == 120)
    #expect(width(0) == nil)
    #expect(width("nonsense") == nil)
    #expect(width(nil) == nil)
    #expect(width.defaultValue == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > matches the `value || undefined` idiom it replaces"
    ))
  func omitDefaultIdiom() throws {
    let width = try optional(numberValue(), omitDefault: true)
    for value: JSONValue? in [0, 120, -3, nil] {
      let idiom: JSONValue? = value == 0 ? nil : value
      #expect(width(value) == idiom)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > optional({omitDefault}) > without the option a default-valued input is kept"
    ))
  func withoutOmitDefault() throws {
    #expect(try optional(numberValue())(0) == 0)
  }

  private static func dimension() throws -> SerializationSchema {
    try unionValue([numberValue(), enumValue(["inherit"])], default: "inherit")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > accepts a value from either member"
    ))
  func unionEitherMember() throws {
    let dimension = try Self.dimension()
    #expect(dimension(640) == 640)
    #expect(dimension("inherit") == "inherit")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > falls back when no member accepts the value"
    ))
  func unionFallback() throws {
    let dimension = try Self.dimension()
    #expect(dimension("banana") == "inherit")
    #expect(dimension(nil) == "inherit")
    #expect(dimension.defaultValue == "inherit")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > defaults to the first member default when none is given"
    ))
  func unionFirstMemberDefault() throws {
    #expect(try unionValue([numberValue(7), enumValue(["x"])])(.null) == 7)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > records its members on meta"
    ))
  func unionMeta() throws {
    #expect(try Self.dimension().meta.kind == "union")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > unionValue > defers equality to whichever member has one"
    ))
  func unionEquality() throws {
    let tags = try unionValue([arrayValue(stringValue()), enumValue(["all"])])
    #expect(tags.isSchemaEqual(json(#"["a"]"#), json(#"["a"]"#)))
    #expect(tags.isSchemaEqual(json(#"["a"]"#), json(#"["b"]"#)) == false)
    #expect(tags.isSchemaEqual("all", "all"))
    #expect(tags.isSchemaEqual("all", json(#"["a"]"#)) == false)
    let dimension = try Self.dimension()
    #expect(dimension.isSchemaEqual(640, 640))
    #expect(dimension.isSchemaEqual(640, "inherit") == false)
  }

  private static func format() throws -> SerializationSchema {
    let nameToBit: [String: JSONValue] = ["bold": 1, "italic": 2]
    return try transformValue(
      unionValue([numberValue(), enumValue(["bold", "italic"])], default: 0)
    ) { value in
      if case .string(let s)? = value { return nameToBit[s] }
      return value
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > normalizes accepted values into the target domain"
    ))
  func transformNormalizes() throws {
    let format = try Self.format()
    #expect(format(4) == 4)
    #expect(format("bold") == 1)
    #expect(format("italic") == 2)
    #expect(format("junk") == 0)
    #expect(format(nil) == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > defaultValue is the transformed inner default"
    ))
  func transformDefault() throws {
    #expect(try Self.format().defaultValue == 0)
    let upper = try transformValue(stringValue("a")) {
      $0?.stringValue.map { .string($0.uppercased()) }
    }
    #expect(upper.defaultValue == "A")
    #expect(upper("bc") == "BC")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > transformValue > meta names the transform and holds the inner schema"
    ))
  func transformMeta() throws {
    let inner = stringValue()
    let t = try transformValue(inner) { $0?.stringValue.map { .int($0.utf16.count) } }
    guard case .transform(let held) = t.meta else {
      Issue.record("not a transform schema")
      return
    }
    #expect(held === inner)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > rawValue > passes an unvalidated value through"
    ))
  func raw() {
    let raw = rawValue()
    let value = json(#"{"editorState":{"root":{}}}"#)
    #expect(raw(value) == value)
    #expect(raw(nil) == nil)
    #expect(raw.defaultValue == nil)
    #expect(raw.meta.kind == "raw")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > a getter returning undefined omits its property"
    ))
  func getterUndefinedOmits() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode()
      try paragraph.append(createTextNode("x").setFormat(1))
      #expect(try paragraph.getSerializedTextFormat() == nil)
      #expect(try paragraph.getSerializedTextStyle() == nil)
      let styled = try createParagraphNode().setTextStyle("color: red")
      #expect(try styled.getSerializedTextStyle() == "color: red")
      #expect(try styled.exportJSON()["textStyle"] == "color: red")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > a property is read through the getter its schema names"
    ))
  func propertyReadThroughGetter() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("hi").exportJSON()
      #expect(text["text"] == "hi" && text["type"] == "text")
      let paragraph = try createParagraphNode().exportJSON()
      #expect(paragraph["format"] == "" && paragraph["type"] == "paragraph")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::updateFromJSON tolerates partial and out-of-domain JSON > exportJSON is written from the schema > export round-trips through import for every schema property"
    ))
  func exportImportRoundTrip() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let node = try createTextNode("round trip").setFormat(1).setStyle("color: red").setMode(
        "token")
      let exported = try node.exportJSON()
      let restored = try createTextNode("").updateFromJSON(exported)
      #expect(try restored.exportJSON() == exported)
    }
  }
}
