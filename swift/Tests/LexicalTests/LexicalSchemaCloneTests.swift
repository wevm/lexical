import LexicalTesting
import Testing

@testable import Lexical

// Port of the `a clone carries the fields the schema declares`, `review
// fixes` and `numberValue accepts a stringified number` suites of
// LexicalSchema.test.ts. `SchemaField(_:_:field:)` is `withField`.

private func editorWith(_ nodes: [LexicalNode.Type]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

/// Creates a node, writes it, then makes it writable in a second update, so
/// what comes back is a clone.
private func writeThenRewrite<T: ElementNode>(
  _ editor: Editor, _ klass: T.Type, _ write: @escaping (T) -> Void
) throws -> T {
  var key = ""
  try editor.update(discrete: true) {
    let node = try create(klass)
    write(node)
    try getRoot().clear().append(node)
    key = node.getKey()
  }
  var cloned: T!
  try editor.update(discrete: true) { () throws in
    cloned = try getRoot().getFirstChild()!.getWritable() as? T
    #expect(cloned.getKey() == key)
  }
  return cloned
}

private final class CalloutNode: ElementNode {
  var __label = ""
  var __level = 1
  var __tone = "info"
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CalloutNode.self, type: "callout", extends: ElementNode.self,
      json: [
        SchemaField("label", stringValue(), field: \CalloutNode.__label),
        SchemaField(
          "level", numberValue(1, integer: true, min: 1, max: 3), field: \CalloutNode.__level),
        SchemaField("tone", enumValue(["info", "warning", "danger"]), field: \CalloutNode.__tone),
      ])
  }
}

private class CarryBaseNode: ElementNode {
  var __base = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CarryBaseNode.self, type: "carry-base", extends: ElementNode.self,
      json: [SchemaField("base", stringValue(), field: \CarryBaseNode.__base)])
  }
}

private final class CarryDerivedNode: CarryBaseNode {
  var __derived = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CarryDerivedNode.self, type: "carry-derived", extends: CarryBaseNode.self,
      json: [SchemaField("derived", stringValue(), field: \CarryDerivedNode.__derived)])
  }
}

private final class AfterCloneCalls: @unchecked Sendable {
  var count = 0
}
private let accessorNodeCalls = AfterCloneCalls()

private final class AccessorNode: ElementNode {
  var __ids: [String] = []
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      AccessorNode.self, type: "carry-accessor", extends: ElementNode.self,
      json: [
        SchemaField(
          "ids", arrayValue(stringValue()), of: AccessorNode.self,
          get: { try $0.getIds().schemaValue },
          set: { node, value in try node.setIds([String](schemaValue: value) ?? []) })
      ])
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    accessorNodeCalls.count += 1
    __ids = (prevNode as! AccessorNode).__ids
  }
  func getIds() throws -> [String] { try getLatest().__ids }
  @discardableResult
  func setIds(_ ids: [String]) throws -> Self {
    let writable = try getWritable()
    writable.__ids = ids
    return writable
  }
}

private final class Writes: @unchecked Sendable {
  var values: [String] = []
  var recording = false
}
private let storeNodeWrites = Writes()

private class StoreNode: ElementNode {
  var __shared = "" {
    didSet { if storeNodeWrites.recording { storeNodeWrites.values.append(__shared) } }
  }
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      StoreNode.self, type: "assign-once-base", extends: ElementNode.self,
      json: [SchemaField("shared", stringValue(), field: \StoreNode.__shared)])
  }
}

private final class RestateNode: StoreNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      RestateNode.self, type: "assign-once-derived", extends: StoreNode.self,
      json: [SchemaField("shared", stringValue("other"), field: \RestateNode.__shared)])
  }
}

private class ReBaseNode: ElementNode {
  var __shared = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ReBaseNode.self, type: "order-base", extends: ElementNode.self,
      json: [SchemaField("shared", stringValue(), field: \ReBaseNode.__shared)])
  }
}

private final class ReDerivedNode: ReBaseNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ReDerivedNode.self, type: "order-derived", extends: ReBaseNode.self,
      json: [SchemaField("shared", stringValue("fallback"), field: \ReDerivedNode.__shared)])
  }
}

@Suite struct LexicalSchemaCloneTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a node that declares only fields needs no afterCloneFrom"
    ))
  func fieldsNeedNoAfterCloneFrom() throws {
    let editor = try editorWith([CalloutNode.self])
    let cloned = try writeThenRewrite(editor, CalloutNode.self) { node in
      node.__label = "Heads up"
      node.__level = 3
      node.__tone = "danger"
    }
    try editor.read { () throws in
      let exported = try cloned.exportJSON()
      #expect(exported["label"] == "Heads up")
      #expect(exported["level"] == 3)
      #expect(exported["tone"] == "danger")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > each class carries its own fields and delegates the rest"
    ))
  func eachClassCarriesItsFields() throws {
    let editor = try editorWith([CarryBaseNode.self, CarryDerivedNode.self])
    let cloned = try writeThenRewrite(editor, CarryDerivedNode.self) { node in
      node.__base = "from the base"
      node.__derived = "from the subclass"
    }
    #expect(cloned.__base == "from the base")
    #expect(cloned.__derived == "from the subclass")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a hand-written afterCloneFrom is left alone"
    ))
  func handWrittenAfterCloneFrom() throws {
    let editor = try editorWith([AccessorNode.self])
    let cloned = try writeThenRewrite(editor, AccessorNode.self) { node in
      node.__ids = ["a", "b"]
    }
    #expect(cloned.__ids == ["a", "b"])
    #expect(accessorNodeCalls.count > 0)
  }

  // JS also checks which classes get a synthesized `afterCloneFrom`; Swift
  // carries the fields in `LexicalNode.afterCloneFrom`, so only the
  // assign-once behavior is ported.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > a re-declared field is assigned once, by the class that owns it"
    ))
  func redeclaredFieldAssignedOnce() throws {
    let editor = try editorWith([StoreNode.self, RestateNode.self])
    try editor.update(discrete: true) {
      let node = try create(RestateNode.self)
      let prev = try create(RestateNode.self)
      prev.__shared = "carried"
      storeNodeWrites.recording = true
      defer { storeNodeWrites.recording = false }
      try node.afterCloneFrom(prev)
    }
    #expect(storeNodeWrites.values == ["carried"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::a clone carries the fields the schema declares > what a base class carries does not depend on registration order"
    ))
  func registrationOrderIndependent() throws {
    let editor = try editorWith([ReDerivedNode.self, ReBaseNode.self])
    #expect(
      try writeThenRewrite(editor, ReBaseNode.self) { $0.__shared = "base value" }.__shared
        == "base value")
    #expect(
      try writeThenRewrite(editor, ReDerivedNode.self) { $0.__shared = "derived value" }.__shared
        == "derived value")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > equality is total: an unvalidated getter result cannot throw"
    ))
  func equalityIsTotal() throws {
    let ids = try arrayValue(stringValue())
    let point = try objectValue([("x", numberValue())])
    for value: JSONValue? in [.null, nil, 7, "x"] {
      #expect(ids.isSchemaDefault(value) == false)
      #expect(point.isSchemaDefault(value) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > optional and nullable keep the inner content comparison"
    ))
  func optionalNullableInnerComparison() throws {
    let ids = try arrayValue(stringValue())
    #expect(try optional(ids, omitDefault: true)(json("[]")) == nil)
    #expect(try nullable(ids, defaultAsNull: true)(json("[]")) == .null)
    #expect(try optional(ids).isEqual!(nil, nil))
    #expect(try optional(ids).isEqual!(nil, json("[]")) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > a union parses undefined to the fallback it declared"
    ))
  func unionUndefinedFallback() throws {
    let schema = try unionValue(
      [optional(numberValue()), enumValue(["inherit"])], default: "inherit")
    #expect(schema.defaultValue == "inherit")
    #expect(schema(nil) == "inherit")
    #expect(schema(640) == 640)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::review fixes > a state built from a schema adopts its equality and default"
    ))
  func stateAdoptsSchema() throws {
    let idsState = createState("ids", schema: try arrayValue(stringValue()))
    #expect(idsState.isEqual(json("[]"), idsState.defaultValue))
    #expect(idsState.isEqual(json(#"["a"]"#), json("[]")) == false)
    var leaked = idsState.defaultValue?.arrayValue ?? []
    leaked.append("leak")
    #expect(idsState.defaultValue == json("[]"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > a string spelled as a JSON number is converted"
    ))
  func stringifiedNumber() throws {
    let parse = try numberValue(0)
    #expect(parse("2") == 2)
    #expect(parse("0") == 0)
    #expect(parse("1e3") == 1000)
    #expect(parse("1E3") == 1000)
    #expect(parse("1e-3") == 0.001)
    #expect(parse("2.5") == 2.5)
    #expect(parse("-4") == -4)
    #expect(parse("-0.5") == -0.5)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > only the JSON grammar is read"
    ))
  func onlyJSONGrammar() throws {
    let parse = try numberValue(7)
    for value in ["0x10", "0b11", "0o17", "1_000", "+1", "05", ".5", "5.", " 2 ", "\n2", "2n"] {
      #expect(parse(.string(value)) == 7, "\(value)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > anything that does not read as a finite number is out of domain"
    ))
  func nonFiniteOutOfDomain() throws {
    let parse = try numberValue(7)
    for value: JSONValue in ["", "   ", "abc", "Infinity", "NaN", true, .null, []] {
      #expect(parse(value) == 7, "\(value)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the bounds apply to the converted value"
    ))
  func boundsOnConverted() throws {
    let span = try numberValue(1, integer: true, min: 1)
    #expect(span("2") == 2)
    #expect(span("0") == 1)
    #expect(span("2.5") == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the reported domain is still numbers"
    ))
  func reportedDomainNumbers() throws {
    #expect(try numberValue().meta.kind == "number")
    #expect(try numberValue(3).defaultValue == 3)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > a union accepts through a converting member"
    ))
  func unionConvertingMember() throws {
    let schema = try unionValue([numberValue(), enumValue(["auto"])], default: "auto")
    #expect(schema("2") == 2)
    #expect(schema(2) == 2)
    #expect(schema("auto") == "auto")
    #expect(schema("banana") == "auto")
    #expect(try transformValue(schema) { $0 == "auto" ? 0 : $0 }("auto") == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSchema.test.ts::numberValue accepts a stringified number > the union reads a stringified number for a real node property"
    ))
  func unionStringifiedNodeProperty() throws {
    let names: [String: JSONValue] = ["bold": 1, "italic": 2]
    let format = try transformValue(
      unionValue([numberValue(), enumValue(["bold", "italic"])], default: 0)
    ) { value in
      if case .string(let s)? = value { return names[s] }
      return value
    }
    #expect(format("1") == 1)
    #expect(format(1) == 1)
    #expect(format("bold") == 1)
    #expect(format("junk") == 0)
  }
}
