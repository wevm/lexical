import LexicalTesting
import Testing

@testable import Lexical

// Port of the union, catch-all, container and comparison suites of
// LexicalSchema.test.ts: how a union picks the member that fits a value
// best. A JS `undefined` property is a key holding `nil` (see `obj`).

/// An object whose entries may hold `nil`, the JS `undefined`.
private func obj(_ entries: (String, JSONValue?)...) -> JSONValue {
  var object = JSONObject()
  for (key, value) in entries { object.set(key, value) }
  return .object(object)
}

/// `value => \`n${value}\``.
@Sendable private func prefixN(_ value: JSONValue?) -> JSONValue? {
  .string("n" + (value ?? .null).stringify())
}

/// `value => value.split(',')`.
@Sendable private func splitCommas(_ value: JSONValue?) -> JSONValue? {
  .array(
    (value?.stringValue ?? "").split(separator: ",", omittingEmptySubsequences: false).map {
      .string(String($0))
    })
}

private final class Counter: @unchecked Sendable {
  var count = 0
}

/// A copy of `base` whose `accepts` is declared, as `Object.assign(fn,
/// base, {accepts})` makes in JS.
private func declaring(
  _ base: SerializationSchema, accepts: @escaping @Sendable (JSONValue?) -> Bool,
  parse: (@Sendable (JSONValue?) -> JSONValue?)? = nil
) -> SerializationSchema {
  SerializationSchema(
    meta: base.meta, defaultValue: base.defaultValue, isEqual: base.isEqual, accepts: accepts,
    parse: parse ?? { base($0) })
}

@Suite struct LexicalSchemaUnionMemberTests {
  private static func dimension() throws -> SerializationSchema {
    try unionValue([numberValue(), enumValue(["inherit"])], default: "inherit")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a value a member normalizes into its default is still that member"
    ))
  func normalizedIntoDefault() throws {
    let dimension = try Self.dimension()
    #expect(dimension("0") == 0)
    #expect(dimension("-0") == .number(-0.0))
    #expect(dimension(0) == 0)
    #expect(dimension("banana") == "inherit")
    #expect(dimension("inherit") == "inherit")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > an alias whose target is the inner default is still that member"
    ))
  func aliasToInnerDefault() throws {
    let legacy = try aliasedValue(stringValue(), ["none": ""])
    #expect(legacy("none") == "")
    #expect(try unionValue([legacy], default: "fallback")("none") == "")
    #expect(try unionValue([legacy], default: "fallback")(42) == "fallback")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a comparator never sees values its own member did not produce"
    ))
  func comparatorOwnValues() throws {
    let byId = try transformValue(
      objectValue([("id", numberValue()), ("tag", stringValue())]), { $0 }
    ) { a, b in a?.objectValue?["id"] == b?.objectValue?["id"] }
    let plain = try objectValue([("v", numberValue())])
    let union = try unionValue([byId, plain])
    #expect(union.isEqual!(json(#"{"v":1}"#), json(#"{"v":2}"#)) == false)
    #expect(
      union.isEqual!(json(#"{"id":1,"tag":"a"}"#), json(#"{"id":1,"tag":"b"}"#)) == false)
    #expect(byId.isSchemaEqual(json(#"{"id":1,"tag":"a"}"#), json(#"{"id":1,"tag":"b"}"#)))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a wrapper carries its inner membership into the union"
    ))
  func wrapperCarriesMembership() throws {
    let number = try numberValue()
    let width = try unionValue([number, enumValue(["inherit"])], default: "inherit")
    #expect(width("0") == 0)
    #expect(width(640) == 640)
    #expect(width("banana") == "inherit")
    let transformed = try unionValue(
      [transformValue(numberValue()) { $0 }, enumValue(["inherit"])], default: "inherit")
    #expect(transformed("0") == 0)
    let nilable = try unionValue(
      [nullable(numberValue()), enumValue(["inherit"])], default: "inherit")
    #expect(nilable("0") == 0)
    let maybe = try unionValue(
      [optional(numberValue()), enumValue(["inherit"])], default: "inherit")
    #expect(maybe("0") == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a union is itself a member, so nesting one keeps its domain"
    ))
  func nestedUnion() throws {
    let inner = try unionValue([numberValue(), enumValue(["inherit"])], default: 0)
    #expect(inner.accepts != nil)
    let outer = try unionValue([inner, stringValue("zzz")], default: "zzz")
    #expect(outer("0") == 0)
    #expect(outer(640) == 640)
    #expect(outer("inherit") == "inherit")
    #expect(outer("banana") == "banana")
    #expect(try unionValue([nullable(inner), stringValue("zzz")], default: "zzz")("0") == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > optional lifts membership for undefined only, nullable for both"
    ))
  func optionalNullableMembership() throws {
    let bare = try Self.dimension()
    let maybe = try unionValue(
      [optional(numberValue()), enumValue(["inherit"])], default: "inherit")
    #expect(bare(.null) == "inherit")
    #expect(maybe(.null) == "inherit")
    #expect(
      try unionValue([nullable(numberValue()), enumValue(["inherit"])], default: "inherit")(.null)
        == .null)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a union defers equality to the member that recognizes the pair"
    ))
  func unionDefersEquality() throws {
    let ids = try unionValue([arrayValue(stringValue()), enumValue(["all"])])
    #expect(ids.isSchemaDefault(json("[]")))
    #expect(ids.isSchemaDefault(json(#"["a"]"#)) == false)
    #expect(ids.isSchemaEqual(json(#"["a","b"]"#), json(#"["a","b"]"#)))
    #expect(ids.isSchemaEqual(json(#"["a"]"#), json(#"["b"]"#)) == false)
    #expect(ids.isSchemaEqual("all", "all"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union member knows its own domain > a bounded member rejects what falls outside its bounds"
    ))
  func boundedMember() throws {
    let span = try unionValue([numberValue(1, integer: true, min: 1)], default: 1)
    #expect(span("4") == 4)
    #expect(span("0") == 1)
    #expect(span("1.5") == 1)
  }

  // A JSON array has no holes; `null` elements go through the item schema.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a sparse array is parsed, not passed through > holes go through the item schema like any other element"
    ))
  func sparseArray() throws {
    let schema = try arrayValue(stringValue())
    #expect(schema(json("[null,null,null]")) == json(#"["","",""]"#))
    #expect(schema(json(#"["a",42,null]"#)) == json(#"["a","",""]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::objectValue refuses a __proto__ field in every build > because the parse would reparent its result instead of writing it"
    ))
  func protoFieldRefused() {
    #expect {
      try objectValue([("__proto__", stringValue())])
    } throws: { "\($0)".contains("__proto__") }
  }
}

private class PlainBlock: ElementNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(PlainBlock.self, type: "schema-plain-block", extends: ElementNode.self)
  }
}

private final class StyledBlock: PlainBlock {
  override class func config() throws -> NodeConfig? {
    NodeConfig(StyledBlock.self, type: "schema-styled-block", extends: PlainBlock.self)
  }
  override func getTextFormat() throws -> Int { IS_BOLD }
  override func getTextStyle() throws -> String { "color: red;" }
}

private final class MigratingText: TextNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(MigratingText.self, type: "schema-migrating-text", extends: TextNode.self)
  }
  override func updateFromJSON(_ serializedNode: JSONObject) throws -> Self {
    try super.updateFromJSON(serializedNode).setStyle("color: red;")
  }
}

private func editorWith(_ nodes: [LexicalNode.Type]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

@Suite struct LexicalSchemaDomainFitTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema does not take back an override the node already had > an overridden getTextFormat/getTextStyle still decides the export"
    ))
  func overriddenTextStyleExport() throws {
    let editor = try editorWith([PlainBlock.self, StyledBlock.self])
    try editor.update(discrete: true) { () throws in
      let styled = try create(StyledBlock.self).exportJSON()
      #expect(styled["textFormat"] == .int(IS_BOLD))
      #expect(styled["textStyle"] == "color: red;")
      let plain = try create(PlainBlock.self).exportJSON()
      #expect(plain["textFormat"] == nil)
      #expect(plain["textStyle"] == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema does not take back an override the node already had > a synthesized importJSON calls an overridden updateFromJSON"
    ))
  func importJSONCallsOverride() throws {
    let editor = try editorWith([MigratingText.self])
    try editor.update(discrete: true) { () throws in
      let imported = try #require(
        try MigratingText.importJSON(
          jsonObject(
            #"{"detail":0,"format":0,"mode":"normal","style":"","text":"hi","type":"schema-migrating-text","version":1}"#
          )) as? TextNode)
      #expect(try imported.getStyle() == "color: red;")
      #expect(try imported.getTextContent() == "hi")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > a union picks the wrapper whose inner schema recognizes the value"
    ))
  func unionPicksWrapper() throws {
    let schema = try unionValue(
      [nullable(stringValue(), defaultAsNull: true), enumValue(["auto"])], default: "auto")
    #expect(schema("") == .null)
    #expect(schema("left") == "left")
    #expect(schema("auto") == "auto")
    #expect(schema(42) == "auto")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > the same holds for optional and for a transformed domain"
    ))
  func optionalAndTransformedDomain() throws {
    let opt = try unionValue(
      [optional(stringValue(), omitDefault: true), enumValue(["auto"])], default: "auto")
    #expect(opt("") == nil)
    let toArray = try transformValue(stringValue(), splitCommas) { $0 == $1 }
    let union = try unionValue([toArray, enumValue(["auto"])], default: "auto")
    #expect(union("") == json(#"[""]"#))
    #expect(union("a,b") == json(#"["a","b"]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a wrapper answers for its inner schema’s domain > a wrapper still declines what its inner schema declines"
    ))
  func wrapperDeclines() throws {
    let schema = try unionValue([optional(numberValue()), enumValue([.null])], default: .null)
    #expect(schema(.null) == .null)
    #expect(schema(7) == 7)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a raw value validates nothing, so it declines nothing"
    ))
  func rawDeclinesNothing() throws {
    let raw = rawValue()
    #expect(raw.accepts!(nil))
    #expect(raw.accepts!("hi"))
    let union = try unionValue(
      [objectValue([("note", raw), ("title", stringValue())]), enumValue(["auto"])],
      default: "auto")
    #expect(union(obj(("note", nil), ("title", "keep me"))) == json(#"{"title":"keep me"}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > an enum listing undefined recognizes it, when it is the default"
    ))
  func enumUndefinedMember() throws {
    #expect(try enumValue([nil, "top"]).accepts!(nil))
    #expect(try enumValue(["top", nil]).accepts!(nil) == false)
    #expect {
      try enumValue(["top", "middle"], default: "unset")
    } throws: { "\($0)".contains("enumValue: the default value is not one of the values") }
    let union = try unionValue(
      [
        objectValue([("align", enumValue([nil, "top"])), ("id", stringValue())]),
        enumValue(["auto"]),
      ],
      default: "auto")
    #expect(union(obj(("align", nil), ("id", "x"))) == json(#"{"id":"x"}"#))
    #expect(union(obj(("align", "sideways"), ("id", "x"))) == json(#"{"id":"x"}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a constrained member still declines its own out-of-domain default"
    ))
  func constrainedDeclinesDefault() throws {
    #expect(try numberValue(0, min: 1).accepts!(0) == false)
    #expect(
      try unionValue([numberValue(0, min: 1), enumValue(["auto"])], default: "auto")(0) == "auto")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a schema declares the domain it reads, and only that > a transform describes what it reads, not what it writes"
    ))
  func transformReads() throws {
    let prefixed = try transformValue(numberValue(), prefixN)
    #expect(prefixed.accepts!("n0") == false)
    #expect(prefixed.accepts!(0))
    #expect(
      try unionValue([prefixed, aliasedValue(numberValue(), ["n0": 42])], default: 0)("n0") == 42)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a declaration means what it says > an explicit undefined default is a default"
    ))
  func explicitUndefinedDefault() throws {
    #expect(
      try unionValue([numberValue(), optional(stringValue())], default: .some(nil)).defaultValue
        == nil)
    #expect(try unionValue([numberValue(), optional(stringValue())]).defaultValue == 0)
  }
}

@Suite struct LexicalSchemaCatchAllTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > a raw field does not veto its enclosing container"
    ))
  func rawFieldNoVeto() throws {
    let value = json(#"[{"label":"keep label","note":"keep note"}]"#)
    #expect(
      try unionValue(
        [
          arrayValue(numberValue()),
          arrayValue(objectValue([("label", stringValue()), ("note", rawValue())])),
        ], default: json("[]"))(value) == value)
    let tags = json(#"{"note":"hi","tags":["red","blue"]}"#)
    #expect(
      try unionValue(
        [
          objectValue([("note", rawValue()), ("tags", arrayValue(numberValue()))]),
          objectValue([("note", rawValue()), ("tags", arrayValue(stringValue()))]),
        ], default: "x")(tags) == tags)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but a union that merely contains one is not itself a catch-all"
    ))
  func containingUnionNotCatchAll() throws {
    let value = json(#"["red","42"]"#)
    #expect(
      try unionValue(
        [
          arrayValue(numberValue()),
          unionValue([arrayValue(stringValue()), rawValue()], default: json("[]")),
        ], default: json("[]"))(value) == value)
    #expect(
      try unionValue(
        [
          unionValue([arrayValue(numberValue()), rawValue()], default: json("[]")),
          arrayValue(stringValue()),
        ], default: json("[]"))(value) == value)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a fit earned through a catch-all ranks below a real one"
    ))
  func catchAllFitRanksBelow() throws {
    #expect(
      try unionValue(
        [unionValue([numberValue(), rawValue()], default: 0), arrayValue(numberValue())],
        default: json("[]"))(json(#"["42"]"#)) == json("[42]"))
    #expect(
      try unionValue(
        [unionValue([enumValue(["auto"]), rawValue()], default: "auto"), numberValue()],
        default: 0)("640") == 640)
  }

  /// Wraps `numberValue()` in `depth` levels built by `wrap`, and counts the
  /// leaf's `accepts` calls for one parse.
  private func checks(
    depth: Int, _ wrap: (SerializationSchema) throws -> SerializationSchema,
    value: (JSONValue) -> JSONValue
  ) throws -> Int {
    let base = try numberValue()
    let counter = Counter()
    var schema = declaring(base) { value in
      counter.count += 1
      return base.accepts!(value)
    }
    var input: JSONValue = 1
    for _ in 0..<depth {
      schema = try wrap(schema)
      input = value(input)
    }
    counter.count = 0
    _ = schema(input)
    return counter.count
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and measuring a union costs one traversal, not two"
    ))
  func unionMeasuredOnce() throws {
    var counts: [Int] = []
    for depth in [4, 8, 12, 16] {
      let count = try checks(
        depth: depth,
        { try unionValue([objectValue([("value", $0)]), stringValue()], default: "") },
        value: { .object(JSONObject([("value", $0)])) })
      counts.append(count)
      #expect(count <= depth * 4)
    }
    #expect(counts[3] < counts[0] * 8)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a union field holding undefined is still the fit its members give it"
    ))
  func unionFieldUndefined() throws {
    let width = try unionValue(
      [optional(numberValue()), enumValue(["inherit"])], default: "inherit")
    let specific = try objectValue([("label", stringValue()), ("w", width)])
    let catchAll = try objectValue([("label", stringValue()), ("w", rawValue())])
    let value = obj(("label", "keep me"), ("w", nil))
    let expected = json(#"{"label":"keep me","w":"inherit"}"#)
    #expect(try unionValue([specific, catchAll], default: "x")(value) == expected)
    #expect(try unionValue([catchAll, specific], default: "x")(value) == expected)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and nesting unions directly costs no more than that"
    ))
  func nestedUnionsLinear() throws {
    var counts: [Int] = []
    for depth in [4, 8, 12, 16] {
      let count = try checks(
        depth: depth, { try unionValue([$0, stringValue()], default: "") }, value: { $0 })
      counts.append(count)
      #expect(count <= depth * 4)
    }
    #expect(counts[3] < counts[0] * 8)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but only if that member would really reach it"
    ))
  func onlyIfReached() throws {
    let viaFallback = try objectValue([
      ("tags", unionValue([arrayValue(numberValue()), rawValue()], default: json("[]")))
    ])
    let value = json(#"{"tags":["red","42"]}"#)
    #expect(viaFallback(value) == json(#"{"tags":[0,42]}"#))
    #expect(
      try unionValue(
        [viaFallback, objectValue([("tags", arrayValue(stringValue()))])], default: "x")(value)
        == value)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a catch-all inside a member is part of what that member fits"
    ))
  func catchAllInsideMember() throws {
    let value = json(#"{"a":"","b":[]}"#)
    #expect(
      try unionValue(
        [
          objectValue([("a", stringValue()), ("b", stringValue())]),
          objectValue([
            ("a", stringValue()), ("b", unionValue([stringValue(), rawValue()], default: "")),
          ]),
        ], default: "x")(value) == value)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a member that normalizes still wins by declaration order"
    ))
  func normalizingMemberOrder() throws {
    #expect(
      try unionValue([numberValue(), enumValue(["inherit"])], default: "inherit")("640") == 640)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and a catch-all is one wherever it is reached from"
    ))
  func catchAllAnywhere() throws {
    let value = json(#"["red","42"]"#)
    #expect(
      try unionValue(
        [
          unionValue([arrayValue(numberValue()), rawValue()], default: json("[]")),
          arrayValue(stringValue()),
        ], default: json("[]"))(value) == value)
    for wrapped in [
      try optional(rawValue()), try nullable(rawValue()),
      try unionValue([rawValue()], default: .some(nil)),
    ] {
      #expect(
        try unionValue([objectValue([("id", stringValue())]), wrapped], default: "x")(
          json(#"{"id":42}"#)) == json(#"{"id":""}"#))
      #expect(
        try unionValue([numberValue(), wrapped], default: 0)(json(#"{"odd":1}"#))
          == json(#"{"odd":1}"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > but a bare raw member still comes last, and still catches"
    ))
  func bareRawLast() throws {
    #expect(
      try unionValue([arrayValue(numberValue()), rawValue()], default: json("[]"))(
        json(#"[1,"x"]"#)) == json("[1,0]"))
    #expect(
      try unionValue([numberValue(), rawValue()], default: 0)(json(#"{"odd":1}"#))
        == json(#"{"odd":1}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a catch-all is a last resort, not a mismatch > and what a union reports is the member it will actually use"
    ))
  func unionReportsUsedMember() throws {
    let viaFallback = try objectValue([
      ("note", rawValue()),
      ("tags", unionValue([arrayValue(numberValue()), rawValue()], default: json("[]"))),
    ])
    let specific = try objectValue([("note", rawValue()), ("tags", arrayValue(stringValue()))])
    let value = json(#"{"note":"keep me","tags":["red","42"]}"#)
    #expect(try unionValue([viaFallback, specific], default: "x")(value) == value)
    #expect(try unionValue([specific, viaFallback], default: "x")(value) == value)
  }
}

@Suite struct LexicalSchemaContainerTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own"
    ))
  func declaredPredicateObject() throws {
    let fields: [(String, SerializationSchema)] = [
      ("kind", stringValue()), ("text", stringValue()),
    ]
    let general = try objectValue(fields)
    let specialValue = json(#"{"kind":"special","text":""}"#)
    let special = SerializationSchema(
      meta: general.meta, defaultValue: specialValue,
      accepts: { $0?.objectValue?["kind"] == "special" }, parse: { _ in specialValue })
    let parse = try unionValue([special, general], default: "x")
    #expect(
      parse(json(#"{"kind":"ordinary","text":"keep me"}"#))
        == json(#"{"kind":"ordinary","text":"keep me"}"#))
    #expect(parse(json(#"{"kind":"special","text":"anything"}"#)) == specialValue)
  }

  private func taggedUnion(_ base: SerializationSchema) throws -> SerializationSchema {
    @Sendable func accepts(_ value: JSONValue?) -> Bool {
      value?.stringValue?.hasPrefix("#") ?? false
    }
    let tagged = declaring(base, accepts: accepts) { value in
      accepts(value) ? .string(value!.stringValue!.uppercased()) : base.defaultValue
    }
    return try unionValue([tagged, stringValue()], default: "")
  }

  private func declaredPredicateKinds(_ kind: String) throws {
    let base: SerializationSchema =
      switch kind {
      case "raw": rawValue()
      case "nullable": try nullable(stringValue())
      case "optional": try optional(stringValue())
      case "transform": try transformValue(stringValue()) { $0 }
      case "aliased": try aliasedValue(stringValue(), ["#alias": "#aliased"])
      default: try unionValue([stringValue(), numberValue()], default: "")
      }
    let parse = try taggedUnion(base)
    #expect(parse("ordinary") == "ordinary")
    #expect(parse("#tag") == "#TAG")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: raw"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: nullable"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: optional"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: transform"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: aliased"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > but a declared predicate still says which values are its own: union"
    ))
  func declaredPredicateEachKind() throws {
    for kind in ["raw", "nullable", "optional", "transform", "aliased", "union"] {
      try declaredPredicateKinds(kind)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and naming an accessor on one neither silences it nor its original"
    ))
  func namingKeepsPredicate() throws {
    // Swift names accessors on a SchemaField, so only the predicate half
    // applies: the declared `accepts` is the one reported.
    let parse = try taggedUnion(nullable(stringValue()))
    #expect(parse("ordinary") == "ordinary")
    guard case .union(let members) = parse.meta else {
      Issue.record("not a union")
      return
    }
    #expect(members[0].declaredAccepts != nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and a predicate wider than its metadata is not overruled by it"
    ))
  func widerPredicate() throws {
    let items = try arrayValue(stringValue())
    let legacy = declaring(
      items,
      accepts: { value in
        switch value {
        case .array?, .string?: true
        default: false
        }
      },
      parse: { value in
        if case .string(let s)? = value { return splitCommas(.string(s)) }
        return items(value)
      })
    #expect(try unionValue([legacy, numberValue()], default: 0)("a,b") == json(#"["a","b"]"#))
    #expect(try unionValue([legacy, numberValue()], default: 0)(7) == 7)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and that holds for a wrapper or a union, not just a container"
    ))
  func predicateWrapperOrUnion() throws {
    @Sendable func accepts(_ value: JSONValue?) -> Bool {
      switch value {
      case nil, .number?: true
      case .string("inherit")?: true
      default: false
      }
    }
    for base in [try optional(numberValue()), try unionValue([numberValue()], default: 0)] {
      let width = declaring(base, accepts: accepts) { $0 == "inherit" ? "inherit" : base($0) }
      let parse = try unionValue([width, enumValue(["auto"])], default: "auto")
      #expect(parse("inherit") == "inherit")
      #expect(parse("auto") == "auto")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > an object with a transform field round-trips"
    ))
  func objectTransformField() throws {
    let shape = try objectValue([
      ("title", stringValue()), ("v", transformValue(numberValue(), prefixN)),
    ])
    #expect(
      try unionValue([shape, enumValue(["LOST"])], default: "LOST")(
        json(#"{"title":"keep me","v":"n5"}"#))
        == json(#"{"title":"keep me","v":"n0"}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and one whose field declines its own default"
    ))
  func fieldDeclinesDefault() throws {
    let value = json(#"{"label":"keep me","n":0}"#)
    #expect(
      try unionValue(
        [
          objectValue([("label", stringValue()), ("n", numberValue(0, min: 1))]),
          enumValue(["auto"]),
        ],
        default: "auto")(value) == value)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > and a nested object that gained a key keeps its siblings"
    ))
  func nestedObjectGainedKey() throws {
    #expect(
      try unionValue(
        [
          objectValue([("item", objectValue([("id", stringValue())])), ("title", stringValue())]),
          enumValue(["auto"]),
        ], default: "auto")(json(#"{"item":{"extra":1,"id":"a"},"title":"T"}"#))
        == json(#"{"item":{"id":"a"},"title":"T"}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a container schema answers for its shape, not its contents > while the strict pass still tells the variants apart"
    ))
  func strictPassVariants() throws {
    #expect(
      try unionValue(
        [objectValue([("x", numberValue())]), objectValue([("y", numberValue())])], default: "none")(
          json(#"{"y":1}"#)) == json(#"{"y":1}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > so a union and its own accepts cannot disagree"
    ))
  func unionAndAcceptsAgree() throws {
    let inner = try unionValue([optional(numberValue()), enumValue(["auto"])], default: "auto")
    let shape = try objectValue([("title", stringValue()), ("x", inner)])
    let value = obj(("title", "t"), ("x", nil))
    let outer = try unionValue([shape, enumValue(["none"])], default: "none")
    let accepted = shape.accepts!(value)
    #expect(outer(value) == (accepted ? shape(value) : "none"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > and wrapping a member does not change the union’s answer"
    ))
  func wrappingMemberSameAnswer() throws {
    let member = try objectValue([
      ("note", unionValue([optional(numberValue()), stringValue()], default: 0)),
      ("title", stringValue()),
    ])
    let value = obj(("note", nil), ("title", "keep me"))
    let answers = try [member, optional(member), nullable(member), transformValue(member) { $0 }]
      .map {
        try unionValue([$0, enumValue(["LOST"])], default: "LOST")(value)
      }
    #expect(Set(answers.map { ($0 ?? .null).stringify() }).count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a strict match never claims what the schema itself declines > and a union recognizes the value it defaults to"
    ))
  func unionRecognizesDefault() throws {
    let u = try unionValue([optional(stringValue()), numberValue()], default: .some(nil))
    #expect(u.accepts!(nil))
    let shape = try objectValue([("note", u), ("title", stringValue())])
    #expect(shape.accepts!(shape(json(#"{"title":"x"}"#))))
    #expect(
      try unionValue([shape, enumValue(["LOST"])], default: "LOST")(json(#"{"title":"x"}"#))
        == json(#"{"title":"x"}"#))
  }
}

@Suite struct LexicalSchemaArrayTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > one malformed element does not discard the good ones"
    ))
  func malformedElementKept() throws {
    #expect(
      try unionValue([arrayValue(stringValue()), enumValue(["none"])], default: "none")(
        json(#"["a",1]"#)) == json(#"["a",""]"#))
    let shape = try objectValue([("label", stringValue()), ("tags", arrayValue(numberValue()))])
    #expect(
      try unionValue([shape, enumValue(["auto"])], default: "auto")(
        json(#"{"label":"keep me","tags":[1,null]}"#)) == json(#"{"label":"keep me","tags":[1,0]}"#)
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > an array no element of which matches keeps its enclosing object"
    ))
  func noElementMatches() throws {
    let shape = try objectValue([("label", stringValue()), ("tags", arrayValue(numberValue()))])
    #expect(
      try unionValue([shape, enumValue(["auto"])], default: "auto")(
        json(#"{"label":"keep me","tags":["banana"]}"#)) == json(#"{"label":"keep me","tags":[0]}"#)
    )
    let doc = try objectValue([
      ("items", arrayValue(objectValue([("id", stringValue())]))), ("title", stringValue()),
    ])
    #expect(
      try unionValue([doc, enumValue(["auto"])], default: "auto")(
        json(#"{"items":[{"extra":1,"id":"a"}],"title":"T"}"#))
        == json(#"{"items":[{"id":"a"}],"title":"T"}"#))
    let rows = try arrayValue(transformValue(numberValue(), prefixN))
    #expect(
      try unionValue([rows, enumValue(["none"])], default: "none")(json(#"["n1","n2"]"#))
        == json(#"["n0","n0"]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > and a catch-all member never outranks one that owns the value"
    ))
  func catchAllNeverOutranks() throws {
    #expect(
      try unionValue([arrayValue(numberValue()), rawValue()], default: json("[]"))(
        json(#"[1,"x"]"#))
        == json("[1,0]"))
    #expect(rawValue().accepts!("anything"))
  }

  // A JSON array has no absent elements; `null` stands in for one.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > an absent element says nothing either way"
    ))
  func absentElement() throws {
    #expect(try arrayValue(stringValue()).accepts!(json("[null,null]")))
    #expect(try arrayValue(stringValue()).accepts!(json("[]")))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > a complete match beats a partial one, whichever comes first"
    ))
  func completeBeatsPartial() throws {
    #expect(
      try unionValue([arrayValue(numberValue()), arrayValue(stringValue())], default: json("[]"))(
        json(#"["red","42"]"#)) == json(#"["red","42"]"#))
    let tags = try unionValue(
      [
        arrayValue(objectValue([("label", stringValue())])),
        arrayValue(objectValue([("color", optional(stringValue())), ("label", stringValue())])),
      ], default: json("[]"))
    let value = json(#"[{"label":"a"},{"color":"red","label":"b"}]"#)
    #expect(tags(value) == value)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema keeps what it can rather than falling back > and still picks the variant whose items match"
    ))
  func picksMatchingItems() throws {
    #expect(
      try unionValue([arrayValue(numberValue()), arrayValue(stringValue())], default: json("[]"))(
        json(#"["red","blue"]"#)) == json(#"["red","blue"]"#))
    #expect(
      try unionValue(
        [
          arrayValue(objectValue([("x", numberValue())])),
          arrayValue(objectValue([("y", numberValue())])),
        ],
        default: json("[]"))(json(#"[{"y":1}]"#)) == json(#"[{"y":1}]"#))
    #expect(
      try unionValue(
        [
          objectValue([("tags", arrayValue(numberValue()))]),
          objectValue([("tags", arrayValue(stringValue()))]),
        ],
        default: "none")(json(#"{"tags":["red","blue"]}"#)) == json(#"{"tags":["red","blue"]}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > a union falls through to the array variant whose items match"
    ))
  func arrayVariantFallthrough() throws {
    let union = try unionValue([arrayValue(numberValue()), arrayValue(stringValue())])
    #expect(union(json(#"["red","blue"]"#)) == json(#"["red","blue"]"#))
    #expect(union(json("[1,2]")) == json("[1,2]"))
    let objects = try unionValue([
      arrayValue(objectValue([("x", numberValue())])),
      arrayValue(objectValue([("y", numberValue())])),
    ])
    #expect(objects(json(#"[{"y":1}]"#)) == json(#"[{"y":1}]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > and an enclosing object still picks the variant its items fit"
    ))
  func enclosingObjectPicks() throws {
    let strings = try objectValue([("tags", arrayValue(stringValue()))])
    let numbers = try objectValue([("tags", arrayValue(numberValue()))])
    #expect(strings.accepts!(json(#"{"tags":[1,2]}"#)))
    #expect(
      try unionValue([numbers, strings], default: "none")(json(#"{"tags":["a","b"]}"#))
        == json(#"{"tags":["a","b"]}"#))
    #expect(
      try unionValue([strings, numbers], default: "none")(json(#"{"tags":[1,2]}"#))
        == json(#"{"tags":[1,2]}"#))
    #expect(strings(json(#"{"tags":[1,2]}"#)) == json(#"{"tags":["",""]}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an array schema answers only for its own element domain > an empty array belongs to every array schema"
    ))
  func emptyArrayBelongs() throws {
    #expect(try arrayValue(numberValue()).accepts!(json("[]")))
    #expect(try arrayValue(objectValue([("x", numberValue())])).accepts!(json("[]")))
  }
}

@Suite struct LexicalSchemaContentTests {
  private static func toArray() throws -> SerializationSchema {
    try transformValue(stringValue(), splitCommas) { $0 == $1 }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > a transformed output compares by content"
    ))
  func transformedContent() throws {
    let union = try unionValue([Self.toArray()], default: json(#"[""]"#))
    #expect(union.isSchemaEqual(union("a,b"), union("a,b")))
    #expect(union.isSchemaEqual(union("a,b"), union("a,c")) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > through nullable and optional, whose default is a nil"
    ))
  func throughNullableOptional() throws {
    for member in [try nullable(Self.toArray()), try optional(Self.toArray())] {
      let union = try unionValue([member], default: .null)
      #expect(union.isSchemaEqual(json(#"["a","b"]"#), json(#"["a","b"]"#)))
      #expect(union.isSchemaEqual(json(#"["a","b"]"#), json(#"["a","c"]"#)) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > so a wrapper around it can still recognize its own default"
    ))
  func wrapperRecognizesDefault() throws {
    let schema = try optional(
      unionValue([Self.toArray()], default: json(#"[""]"#)), omitDefault: true)
    #expect(schema("") == nil)
    #expect(schema("a,b") == json(#"["a","b"]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a union compares by content, not by a member’s comparator > and two members’ values are never confused for each other"
    ))
  func membersNotConfused() throws {
    let toIds = try transformValue(stringValue()) { value in
      .array(
        (value?.stringValue ?? "").split(separator: ",", omittingEmptySubsequences: false).map {
          .object(JSONObject([("id", .number(Double($0) ?? .nan))]))
        })
    }
    let union = try unionValue([toIds, arrayValue(stringValue())], default: json("[]"))
    #expect(union.isSchemaEqual(json(#"["red"]"#), json(#"["blue"]"#)) == false)
    #expect(union.isSchemaEqual(json(#"["red"]"#), json(#"["red"]"#)))
    let byId = try transformValue(
      objectValue([("id", numberValue()), ("tag", stringValue())]), { $0 }
    ) { a, b in a?.objectValue?["id"] == b?.objectValue?["id"] }
    let numeric = try unionValue(
      [byId, objectValue([("id", numberValue()), ("tag", numberValue())])],
      default: json(#"{"id":0,"tag":0}"#))
    #expect(numeric.isSchemaEqual(json(#"{"id":1,"tag":5}"#), json(#"{"id":1,"tag":9}"#)) == false)
  }

  private static func pointOrLabel() throws -> (SerializationSchema, SerializationSchema) {
    let point = try objectValue([("x", numberValue())])
    let label = try objectValue([("label", stringValue())])
    return (point, try unionValue([point, label]))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > a union falls through to the variant whose keys match"
    ))
  func objectKeysMatch() throws {
    let (_, union) = try Self.pointOrLabel()
    #expect(union(json(#"{"label":"hello"}"#)) == json(#"{"label":"hello"}"#))
    #expect(union(json(#"{"x":5}"#)) == json(#"{"x":5}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > and declines what belongs to neither"
    ))
  func objectDeclinesNeither() throws {
    let (_, union) = try Self.pointOrLabel()
    for value in [json(#"{"x":"banana"}"#), json("[1,2]"), "str"] {
      #expect(union(value) == union.defaultValue)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > an empty object belongs to every variant"
    ))
  func emptyObjectEveryVariant() throws {
    let (_, union) = try Self.pointOrLabel()
    #expect(union(json("{}")) == json(#"{"x":0}"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > which is what lets a nested empty object survive a round trip"
    ))
  func nestedEmptyObjectRoundTrip() throws {
    let nested = try unionValue([
      enumValue(["auto"]),
      objectValue([
        ("label", stringValue()),
        ("options", objectValue([("color", optional(stringValue(), omitDefault: true))])),
      ]),
    ])
    let expected = json(#"{"label":"hello","options":{}}"#)
    let parsed = nested(json(#"{"label":"hello","options":{"color":""}}"#))
    #expect(parsed == expected)
    #expect(nested(try JSONValue.parse(parsed!.stringify())) == expected)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::an object schema answers only for its own shape > membership is the same question the parse asks"
    ))
  func objectMembership() throws {
    let (point, _) = try Self.pointOrLabel()
    #expect(point.accepts!(json(#"{"x":1}"#)))
    #expect(point.accepts!(json(#"{"label":"hello"}"#)) == false)
    #expect(point.accepts!(json(#"{"label":"hello","x":1}"#)) == false)
    #expect(point.accepts!(json("{}")))
    #expect(point.accepts!(json("[]")) == false)
  }
}

@Suite struct LexicalSchemaParseCountTests {
  private func leaf(_ counter: Counter) throws -> SerializationSchema {
    try transformValue(stringValue()) { value in
      counter.count += 1
      return value
    }
  }

  private func count(_ schema: SerializationSchema, _ value: JSONValue, _ counter: Counter) -> Int {
    counter.count = 0
    _ = schema(value)
    return counter.count
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > through an object, a union, and the wrappers"
    ))
  func parsedOnce() throws {
    let c = Counter()
    let a = json(#"{"a":"x"}"#)
    #expect(count(try objectValue([("a", leaf(c))]), a, c) == 1)
    #expect(count(try unionValue([enumValue(["auto"]), objectValue([("a", leaf(c))])]), a, c) == 1)
    #expect(count(try nullable(objectValue([("a", leaf(c))])), a, c) == 1)
    #expect(count(try optional(unionValue([objectValue([("a", leaf(c))])])), a, c) == 1)
    #expect(count(try aliasedValue(objectValue([("a", leaf(c))]), [:]), a, c) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > once per element of an array, not once per element per element"
    ))
  func oncePerElement() throws {
    let c = Counter()
    #expect(count(try arrayValue(leaf(c)), json(#"["a","b","c"]"#), c) == 3)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a value is parsed once per traversal > and stays linear as unions nest"
    ))
  func linearNesting() throws {
    let c = Counter()
    var schema = try objectValue([("a", leaf(c))])
    var value = json(#"{"a":"x"}"#)
    for _ in 0..<10 {
      schema = try unionValue([enumValue(["auto"]), objectValue([("inner", schema)])])
      value = .object(JSONObject([("inner", value)]))
    }
    #expect(count(schema, value, c) == 1)
  }
}
