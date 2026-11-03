// Port of packages/lexical/src/LexicalSchema.ts: the serialization schema a
// node declares in its config, which drives `exportJSON` (both forms) and
// `updateFromJSON`.
//
// JS schemas parse to arbitrary JS values. Here a schema's domain is
// `JSONValue?` (`nil` is `undefined`), which is what every parsed value is
// written as, so one type-erased walk serves every node. A field's accessors
// convert between that and the node's typed storage.

/// What a schema describes, for tooling that introspects it.
public indirect enum SchemaMeta: Sendable, Equatable {
  case string
  case number(integer: Bool, min: Double?, max: Double?, clamp: Bool)
  case boolean
  case enumeration([JSONValue?])
  case nullable(SchemaMeta)
  case optional(SchemaMeta)
  case aliased(SchemaMeta, aliases: [String: JSONValue])
  case raw
}

/// A parser with its default: `SerializationSchema<T>`.
public struct SerializationSchema: Sendable {
  public let meta: SchemaMeta
  /// The value `parse(undefined)` returns, which the compact form omits.
  public let defaultValue: JSONValue?
  public let isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)?
  private let parser: @Sendable (JSONValue?) -> JSONValue?

  public init(
    meta: SchemaMeta,
    defaultValue: JSONValue?? = .none,
    isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)? = nil,
    parse: @escaping @Sendable (JSONValue?) -> JSONValue?
  ) {
    self.meta = meta
    self.parser = parse
    self.isEqual = isEqual
    // `makeSchema`: a schema that states no default derives it from
    // parsing `undefined`.
    switch defaultValue {
    case .none: self.defaultValue = parse(nil)
    case .some(let value): self.defaultValue = value
    }
  }

  /// Parses an untrusted value; never fails, substituting the default.
  public func parse(_ value: JSONValue?) -> JSONValue? { parser(value) }

  /// `isSchemaEqual`.
  public func isSchemaEqual(_ a: JSONValue?, _ b: JSONValue?) -> Bool {
    a == b || (isEqual?(a, b) ?? false)
  }

  /// `isSchemaDefault`.
  public func isSchemaDefault(_ value: JSONValue?) -> Bool {
    isSchemaEqual(value, defaultValue)
  }
}

/// `stringValue(defaultValue)`.
public func stringValue(_ defaultValue: String = "") -> SerializationSchema {
  SerializationSchema(meta: .string) { value in
    if case .string? = value { return value }
    return .string(defaultValue)
  }
}

/// `numberValue(defaultValue, options)`.
public func numberValue(
  _ defaultValue: Double = 0, integer: Bool = false, min: Double? = nil, max: Double? = nil,
  clamp: Bool = false
) -> SerializationSchema {
  let lower = integer ? min.map { $0.rounded(.up) } : min
  let upper = integer ? max.map { $0.rounded(.down) } : max
  @Sendable func coerce(_ value: JSONValue?) -> Double? {
    switch value {
    case .number(let n)?: return n
    case .string(let s)? where isJSONNumber(s): return Double(s)
    default: return nil
    }
  }
  @Sendable func isNumeric(_ n: Double?) -> Bool {
    guard let n, n.isFinite else { return false }
    return !integer || n.rounded(.towardZero) == n
  }
  return SerializationSchema(
    meta: .number(integer: integer, min: lower, max: upper, clamp: clamp)
  ) { value in
    let parsed = coerce(value)
    if clamp, isNumeric(parsed), let n = parsed {
      if let lower, n < lower { return .number(lower) }
      if let upper, n > upper { return .number(upper) }
      return .number(n)
    }
    if isNumeric(parsed), let n = parsed, lower.map({ n >= $0 }) ?? true,
      upper.map({ n <= $0 }) ?? true
    {
      return .number(n)
    }
    return .number(defaultValue)
  }
}

/// `booleanValue(defaultValue)`.
public func booleanValue(_ defaultValue: Bool = false) -> SerializationSchema {
  SerializationSchema(meta: .boolean) { value in
    if case .bool? = value { return value }
    return .bool(defaultValue)
  }
}

/// `enumValue(values, defaultValue)`: one of `values` (compared as JS `===`),
/// otherwise the default, which is the first value unless given.
public func enumValue(
  _ values: [JSONValue?], default defaultValue: JSONValue?? = .none
)
  -> SerializationSchema
{
  let fallback: JSONValue? =
    switch defaultValue {
    case .none: values[0]
    case .some(let value): value
    }
  return SerializationSchema(meta: .enumeration(values), defaultValue: .some(fallback)) { value in
    if value != nil, values.contains(where: { $0 == value }) { return value }
    return fallback
  }
}

/// `aliasedValue(inner, aliases)`: also accepts the string names in `aliases`.
public func aliasedValue(
  _ inner: SerializationSchema, _ aliases: [String: JSONValue]
)
  -> SerializationSchema
{
  SerializationSchema(
    meta: .aliased(inner.meta, aliases: aliases), defaultValue: .some(inner.defaultValue),
    isEqual: inner.isEqual
  ) { value in
    if case .string(let s)? = value, let alias = aliases[s] { return alias }
    return inner.parse(value)
  }
}

/// `nullable(inner)`.
public func nullable(
  _ inner: SerializationSchema, defaultAsNull: Bool = false
)
  -> SerializationSchema
{
  SerializationSchema(meta: .nullable(inner.meta), isEqual: inner.isEqual) { value in
    if value == nil || value == .null { return .null }
    let parsed = inner.parse(value)
    return defaultAsNull && inner.isSchemaDefault(parsed) ? .null : parsed
  }
}

/// `optional(inner)`.
public func optional(
  _ inner: SerializationSchema, omitDefault: Bool = false
)
  -> SerializationSchema
{
  SerializationSchema(meta: .optional(inner.meta), isEqual: inner.isEqual) { value in
    if value == nil { return nil }
    let parsed = inner.parse(value)
    return omitDefault && inner.isSchemaDefault(parsed) ? nil : parsed
  }
}

/// `rawValue()`: the JSON as is.
public func rawValue() -> SerializationSchema {
  SerializationSchema(meta: .raw) { $0 }
}

/// The JSON number grammar, which `numberValue` reads strings with:
/// `/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/`.
func isJSONNumber(_ s: String) -> Bool {
  var parser = JSONParser(Array(s.utf16))
  guard (try? parser.parseNumber()) != nil else { return false }
  return parser.i == parser.units.count
}

/// One property of a node's serialization schema, with the accessors that
/// read it for export and apply it on import (`withField` / `withAccessors`).
public struct SchemaField {
  public let key: String
  public let schema: SerializationSchema
  /// Reads the property for export, in the schema's domain. `nil` declares it
  /// import-only (`getter: null`).
  public let get: ((LexicalNode) throws -> JSONValue?)?
  /// When present, the property is written only where it holds (and is not
  /// the default): `when` on a field getter.
  public let when: ((LexicalNode) throws -> Bool)?
  /// Applies a parsed value. `nil` declares the property derived on import
  /// (`setter: null`), which the compact form leaves out.
  public let set: ((LexicalNode, JSONValue?) throws -> Void)?

  public init<Node: LexicalNode>(
    _ key: String, _ schema: SerializationSchema, of _: Node.Type,
    get: ((Node) throws -> JSONValue?)?,
    when: ((Node) throws -> Bool)? = nil,
    set: ((Node, JSONValue?) throws -> Void)?
  ) {
    self.key = key
    self.schema = schema
    self.get = get.map { get in { try get($0 as! Node) } }
    self.when = when.map { when in { try when($0 as! Node) } }
    self.set = set.map { set in { try set($0 as! Node, $1) } }
  }

  /// Whether the compact form omits `value`: `undefined`, or the default.
  func isCompactDefault(_ value: JSONValue?) -> Bool {
    value == nil || schema.isSchemaDefault(value)
  }
}
