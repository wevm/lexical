import Lexical
import Testing

/// Replays `vectors/serialization/schema.json`: each `LexicalSchema.ts`
/// value builder, built here from the JS that names it, parses a table of
/// inputs to the value JS parsed and agrees on `isSchemaDefault`; a
/// declaration JS refuses is refused with its message. And
/// `vectors/serialization/schema-node.json`: `TestSchemaNode` imported from
/// the input and from each form writes both forms byte for byte, and a
/// clone made in a later update writes the legacy form again.
@Suite(.enabled(if: Vectors.available))
struct SchemaTests {
  struct NodeCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let input: String
    let legacy: String
    let compact: String
    var testDescription: String { name }
  }

  static let cases: [JSONValue] = (try? Vectors.loadJSON("serialization/schema")) ?? []
  static let nodeCases: [NodeCase] = (try? Vectors.load("serialization/schema-node")) ?? []

  /// `SCHEMAS` and `REFUSED_SCHEMAS` in serialization.test.ts.
  static func build(_ name: String) throws -> SerializationSchema {
    switch name {
    case "aliasedValue(enumValue(['number', 'bullet']), {ol: 'number', ul: 'bullet'})":
      return try aliasedValue(enumValue(["number", "bullet"]), ["ol": "number", "ul": "bullet"])
    case "arraySetValue(enumValue(['a', 'b']))": return try arraySetValue(enumValue(["a", "b"]))
    case "arraySetValue(stringValue())": return try arraySetValue(stringValue())
    case "arrayValue(numberValue())": return try arrayValue(numberValue())
    case "booleanValue()": return booleanValue()
    case "booleanValue(true)": return booleanValue(true)
    case "enumValue(['a', 'b', 1])": return try enumValue(["a", "b", .int(1)])
    case "enumValue(['ltr', 'rtl', null], null)":
      return try enumValue(["ltr", "rtl", .null], default: .some(.null))
    case "nullable(objectValue({n: nullable(booleanValue())}))":
      return try nullable(objectValue([("n", nullable(booleanValue()))]))
    case "nullable(stringValue('x'))": return try nullable(stringValue("x"))
    case "nullable(stringValue('x'), {defaultAsNull: true})":
      return try nullable(stringValue("x"), defaultAsNull: true)
    case "numberValue()": return try numberValue()
    case "numberValue(0, {clamp: true, max: 1, min: -1})":
      return try numberValue(0, min: -1, max: 1, clamp: true)
    case "numberValue(0, {integer: true})": return try numberValue(0, integer: true)
    case "numberValue(1, {clamp: true, integer: true, max: 3.5, min: 0.5})":
      return try numberValue(1, integer: true, min: 0.5, max: 3.5, clamp: true)
    case "numberValue(5, {max: 10, min: 0})": return try numberValue(5, min: 0, max: 10)
    case "objectValue({a: numberValue(), b: optional(stringValue())})":
      return try objectValue([("a", numberValue()), ("b", optional(stringValue()))])
    case "optional(numberValue(1))": return try optional(numberValue(1))
    case "optional(numberValue(1), {omitDefault: true})":
      return try optional(numberValue(1), omitDefault: true)
    case "rawValue()": return rawValue()
    case "stringValue('x')": return stringValue("x")
    case "stringValue()": return stringValue()
    case "transformValue(stringValue(), s => s + '!')":
      return try transformValue(stringValue()) { .string(($0?.stringValue ?? "") + "!") }
    case "unionValue([booleanValue(), stringValue('d')], 'd')":
      return try unionValue([booleanValue(), stringValue("d")], default: .some("d"))
    case "unionValue([numberValue(), rawValue()])":
      return try unionValue([numberValue(), rawValue()])
    case "unionValue([numberValue(), stringValue()])":
      return try unionValue([numberValue(), stringValue()])
    case "arraySetValue(numberValue())": return try arraySetValue(numberValue())
    case "arrayValue(arraySetValue(stringValue()))":
      return try arrayValue(arraySetValue(stringValue()))
    case "enumValue([])": return try enumValue([])
    case "enumValue(['a'], 'b')": return try enumValue(["a"], default: .some("b"))
    case "numberValue(0, {integer: true, max: 1.8, min: 1.2})":
      return try numberValue(0, integer: true, min: 1.2, max: 1.8)
    case "numberValue(0, {max: 1, min: 5})": return try numberValue(0, min: 5, max: 1)
    case "objectValue({toString: numberValue()})":
      return try objectValue([("toString", numberValue())])
    case "unionValue([])": return try unionValue([])
    default: throw LexicalError("no Swift schema for \(name)")
    }
  }

  /// `JSON.stringify(value)` as the vector records it: `nil` for
  /// `undefined`.
  static func recorded(_ value: JSONValue?) -> String? { value?.stringify() }

  @Test(arguments: cases.indices)
  func builder(_ index: Int) throws {
    let vector = try #require(Self.cases[index].objectValue)
    let name = try #require(vector["schema"]?.stringValue)
    if let error = vector["error"]?.stringValue {
      #expect(throws: LexicalError(error), "\(name)") { try Self.build(name) }
      return
    }
    let schema = try Self.build(name)
    #expect(Self.recorded(schema.defaultValue) == vector["defaultValue"]?.stringValue, "\(name)")
    for result in try #require(vector["results"]?.arrayValue) {
      let result = try #require(result.objectValue)
      let input = result["input"]
      let parsed = schema.parse(input)
      let shown = Self.recorded(input) ?? "undefined"
      #expect(
        Self.recorded(parsed) == result["parsed"]?.stringValue, "\(name) parsing \(shown)")
      #expect(
        schema.isSchemaDefault(parsed) == result["isDefault"]?.boolValue,
        "\(name) isSchemaDefault of \(shown)")
    }
  }

  @Test(arguments: nodeCases)
  func node(_ vector: NodeCase) throws {
    let editor = try createEditor(CreateEditorArgs(nodes: [.klass(TestSchemaNode.self)]))
    for input in [vector.input, vector.legacy, vector.compact] {
      let editorState = try editor.parseEditorState(input)
      #expect(try editorState.toJSON().stringify() == vector.legacy)
      #expect(try editorState.toJSON(true).stringify() == vector.compact)
    }
    try editor.setEditorState(editor.parseEditorState(vector.input))
    try editor.update(discrete: true) {
      guard let node = try getRoot().getFirstChild() else { throw LexicalError("no node") }
      _ = try node.getWritable()
    }
    #expect(try editor.getEditorState().toJSON().stringify() == vector.legacy)
  }
}
