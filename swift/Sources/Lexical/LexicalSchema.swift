// Port of packages/lexical/src/LexicalSchema.ts: the serialization schema a
// node declares in its config, which drives `exportJSON` (both forms) and
// `updateFromJSON`, and the schemas a NodeState can be parsed with.
//
// JS schemas parse to arbitrary JS values. Here a schema's domain is
// `JSONValue?` (`nil` is `undefined`), which is what every parsed value is
// written as, so one type-erased walk serves every node. A field's accessors
// convert between that and the node's typed storage. Values are immutable,
// so JS's freezing and caller-owned tracking have nothing to do, and where JS
// compares two containers by reference (`===`) the port compares them by
// content.

/// What a schema describes (`SerializationSchemaMeta`), for tooling that
/// introspects it, and for a union choosing a member.
public indirect enum SchemaMeta: Sendable {
  case string
  case number(integer: Bool, min: Double?, max: Double?, clamp: Bool)
  case boolean
  case enumeration([JSONValue?])
  case array(item: SerializationSchema)
  /// `arraySetValue`.
  case set(item: SerializationSchema)
  case nullable(SerializationSchema, defaultAsNull: Bool)
  case optional(SerializationSchema, omitDefault: Bool)
  case union([SerializationSchema])
  case raw
  case object([(key: String, schema: SerializationSchema)])
  case aliased(SerializationSchema, aliases: [String: JSONValue])
  case transform(SerializationSchema)

  /// The JS `meta.kind`.
  public var kind: String {
    switch self {
    case .string: "string"
    case .number: "number"
    case .boolean: "boolean"
    case .enumeration: "enum"
    case .array: "array"
    case .set: "set"
    case .nullable: "nullable"
    case .optional: "optional"
    case .union: "union"
    case .raw: "raw"
    case .object: "object"
    case .aliased: "aliased"
    case .transform: "transform"
    }
  }
}

/// A parser with its default: `SerializationSchema<T>`. Calling it parses,
/// as calling a JS schema does.
public final class SerializationSchema: Sendable {
  public let meta: SchemaMeta
  /// The value `parse(undefined)` returns, which the compact form omits.
  public let defaultValue: JSONValue?
  public let isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)?
  /// The values the schema accepts as already in its domain, when it says.
  public let accepts: (@Sendable (JSONValue?) -> Bool)?
  /// Whether `accepts` was derived by a combinator rather than declared.
  let acceptsIsDerived: Bool
  /// `$isCatchAll`, which JS caches per schema.
  let isCatchAll: Bool
  private let parser: @Sendable (JSONValue?) -> JSONValue?

  /// A schema of your own. An `accepts` given here is declared: it narrows
  /// what a union will choose this schema for.
  public convenience init(
    meta: SchemaMeta,
    defaultValue: JSONValue? = nil,
    isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)? = nil,
    accepts: (@Sendable (JSONValue?) -> Bool)? = nil,
    parse: @escaping @Sendable (JSONValue?) -> JSONValue?
  ) {
    self.init(
      parse, meta, defaultValue, isEqual, accepts: accepts, acceptsIsDerived: false)
  }

  /// `makeSchema`: a schema that states no default derives it from parsing
  /// `undefined`.
  init(
    _ parse: @escaping @Sendable (JSONValue?) -> JSONValue?, _ meta: SchemaMeta,
    _ defaultValue: JSONValue? = nil,
    _ isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)? = nil,
    accepts: (@Sendable (JSONValue?) -> Bool)? = nil, acceptsIsDerived: Bool = true
  ) {
    self.meta = meta
    self.parser = parse
    self.isEqual = isEqual
    self.accepts = accepts
    self.acceptsIsDerived = acceptsIsDerived
    self.defaultValue = defaultValue ?? parse(nil)
    self.isCatchAll = computeIsCatchAll(meta, declared: accepts != nil && !acceptsIsDerived)
  }

  /// Parses an untrusted value; never fails, substituting the default.
  public func parse(_ value: JSONValue?) -> JSONValue? { parser(value) }

  public func callAsFunction(_ value: JSONValue?) -> JSONValue? { parser(value) }

  /// `isSchemaEqual`.
  public func isSchemaEqual(_ a: JSONValue?, _ b: JSONValue?) -> Bool {
    a == b || (isEqual?(a, b) ?? false)
  }

  /// `isSchemaDefault`.
  public func isSchemaDefault(_ value: JSONValue?) -> Bool {
    isSchemaEqual(value, defaultValue)
  }

  /// `declaredAccepts`: the `accepts` a schema's author wrote, not one a
  /// combinator derived.
  public var declaredAccepts: (@Sendable (JSONValue?) -> Bool)? {
    acceptsIsDerived ? nil : accepts
  }

  /// `$acceptsValue`: whether `value` is in the schema's domain.
  public func acceptsValue(_ value: JSONValue?) -> Bool {
    if let accepts { return accepts(value) }
    return schemaMatch(self, value) != nil
  }
}

/// `undeclared`, which in JS also refuses a wrapped schema that names an
/// accessor; in Swift accessors live on `SchemaField`, so only the set check
/// remains.
private func undeclared(_ combinator: String, _ inner: SerializationSchema) throws {
  try notNestedSet(combinator, inner)
}

private func notNestedSet(_ combinator: String, _ inner: SerializationSchema?) throws {
  if case .set? = inner?.meta {
    throw LexicalError(
      "\(combinator): arraySetValue can only be the whole value of a createState, not part of another schema or a node property"
    )
  }
}

private func isPlainObject(_ value: JSONValue?) -> Bool {
  if case .object? = value { return true }
  return false
}

private func hasUndeclaredKey(
  _ source: JSONObject, _ fields: [(key: String, schema: SerializationSchema)]
) -> Bool {
  source.keys.contains { key in !fields.contains { $0.key == key } }
}

private func computeIsCatchAll(_ meta: SchemaMeta, declared: Bool) -> Bool {
  if declared { return false }
  switch meta {
  case .raw: return true
  case .union(let members): return !members.isEmpty && members.allSatisfy(\.isCatchAll)
  case .nullable(let inner, _), .optional(let inner, _), .transform(let inner),
    .aliased(let inner, _):
    return inner.isCatchAll
  default: return false
  }
}

private let FIT_WHOLE = 1
private let FIT_VIA_CATCH_ALL = 2
private let FIT_COERCIBLE = 3
private let FIT_NONE = 4

/// `$fitOf`: how well `value` fits `schema`, from whole to none.
private func fitOf(_ schema: SerializationSchema, _ value: JSONValue?) -> Int {
  let declared = schema.declaredAccepts
  let narrowed = declared != nil
  if let declared, !declared(value) {
    return FIT_NONE
  }
  let fit = metaFitOf(schema, value, narrowed)
  return narrowed && fit == FIT_NONE ? FIT_COERCIBLE : fit
}

private func metaFitOf(
  _ schema: SerializationSchema, _ value: JSONValue?, _ narrowed: Bool
)
  -> Int
{
  switch schema.meta {
  case .raw:
    return narrowed ? FIT_WHOLE : FIT_VIA_CATCH_ALL
  case .array(let item):
    guard case .array(let items)? = value else { return FIT_NONE }
    var worst = FIT_WHOLE
    for element in items {
      worst = max(worst, fitOf(item, element))
    }
    return worst >= FIT_COERCIBLE ? FIT_COERCIBLE : worst
  case .object(let fields):
    guard case .object(let source)? = value else { return FIT_NONE }
    if !narrowed && hasUndeclaredKey(source, fields) {
      return FIT_NONE
    }
    var worst = FIT_WHOLE
    for key in source.keys {
      if let field = fields.first(where: { $0.key == key }) {
        worst = max(worst, fitOf(field.schema, source[key]))
      }
    }
    return worst >= FIT_COERCIBLE ? FIT_COERCIBLE : worst
  case .union(let members):
    return bestUnionMember(members, value).fit
  case .aliased(let inner, let aliases):
    if case .string(let s)? = value, aliases[s] != nil { return FIT_WHOLE }
    return fitOf(inner, value)
  case .nullable(let inner, _):
    return value == nil || value == .null ? FIT_WHOLE : fitOf(inner, value)
  case .optional(let inner, _):
    return value == nil ? FIT_WHOLE : fitOf(inner, value)
  case .transform(let inner):
    return fitOf(inner, value)
  default:
    return narrowed || schema.acceptsValue(value) ? FIT_WHOLE : FIT_NONE
  }
}

/// `$bestUnionMember`: the first member that fits best, where a catch-all
/// ranks below any member that fits on its own terms.
private func bestUnionMember(
  _ members: [SerializationSchema], _ value: JSONValue?
) -> (
  fit: Int, member: SerializationSchema?
) {
  var member: SerializationSchema?
  var bestRank = FIT_NONE
  var bestFit = FIT_NONE
  for candidate in members {
    let fit = fitOf(candidate, value)
    let rank = fit < FIT_COERCIBLE && candidate.isCatchAll ? FIT_COERCIBLE : fit
    if rank < bestRank {
      member = candidate
      bestRank = rank
      bestFit = fit
      if rank == FIT_WHOLE { break }
    }
  }
  return (bestFit, member)
}

/// `$schemaMatch`: the parsed value when `value` is in the schema's domain.
/// Without `accepts`, a value is in it when it does not parse to the
/// default, or is the default.
private func schemaMatch(_ schema: SerializationSchema, _ value: JSONValue?) -> JSONValue?? {
  if schema.accepts != nil {
    return schema.acceptsValue(value) ? .some(schema(value)) : nil
  }
  let parsed = schema(value)
  return !schema.isSchemaDefault(parsed) || value == schema.defaultValue ? .some(parsed) : nil
}

/// `stringValue(defaultValue)`.
public func stringValue(_ defaultValue: String = "") -> SerializationSchema {
  SerializationSchema(
    { value in
      if case .string? = value { return value }
      return .string(defaultValue)
    }, .string,
    accepts: { value in
      if case .string? = value { return true }
      return false
    })
}

/// `numberValue(defaultValue, {min, max, integer, clamp})`. An integer
/// domain rounds its bounds inward; an empty domain is refused.
public func numberValue(
  _ defaultValue: Double = 0, integer: Bool = false, min: Double? = nil, max: Double? = nil,
  clamp: Bool = false
) throws -> SerializationSchema {
  let lower = integer ? min.map { $0.rounded(.up) } : min
  let upper = integer ? max.map { $0.rounded(.down) } : max
  if let lower, let upper, lower > upper {
    let rounded =
      integer && (lower != min || upper != max)
      ? " (rounded inward from \(jsNumberString(min!))..\(jsNumberString(max!)) because the domain is integers)"
      : ""
    throw LexicalError(
      "numberValue: the domain is empty; min \(jsNumberString(lower)) is above max \(jsNumberString(upper))\(rounded)"
    )
  }
  @Sendable func coerce(_ value: JSONValue?) -> JSONValue? {
    if case .string(let s)? = value, isJSONNumber(s) { return .number(Double(s)!) }
    return value
  }
  @Sendable func numeric(_ value: JSONValue?) -> Double? {
    guard case .number(let n)? = value, n.isFinite, !integer || n.rounded(.towardZero) == n
    else { return nil }
    return n
  }
  @Sendable func inDomain(_ value: JSONValue?) -> Double? {
    guard let n = numeric(value), lower.map({ n >= $0 }) ?? true, upper.map({ n <= $0 }) ?? true
    else { return nil }
    return n
  }
  return SerializationSchema(
    { value in
      let parsed = coerce(value)
      if clamp, let n = numeric(parsed) {
        if let lower, n < lower { return .number(lower) }
        if let upper, n > upper { return .number(upper) }
        return .number(n)
      }
      return .number(inDomain(parsed) ?? defaultValue)
    }, .number(integer: integer, min: lower, max: upper, clamp: clamp),
    accepts: { value in
      clamp ? numeric(coerce(value)) != nil : inDomain(coerce(value)) != nil
    })
}

/// `booleanValue(defaultValue)`.
public func booleanValue(_ defaultValue: Bool = false) -> SerializationSchema {
  SerializationSchema(
    { value in
      if case .bool? = value { return value }
      return .bool(defaultValue)
    }, .boolean,
    accepts: { value in
      if case .bool? = value { return true }
      return false
    })
}

/// `enumValue(values, defaultValue)`: one of `values`, otherwise the
/// default, which is the first value unless given and must be one of them.
public func enumValue(
  _ values: [JSONValue?], default defaultValue: JSONValue?? = .none
) throws -> SerializationSchema {
  guard !values.isEmpty else {
    throw LexicalError(
      "enumValue: values must not be empty; an enum with no members admits no value")
  }
  let fallback: JSONValue? =
    switch defaultValue {
    case .none: values[0]
    case .some(let value): value
    }
  // A JS `Set` matches by SameValueZero, so `NaN` is a member of itself.
  @Sendable func allowed(_ value: JSONValue?) -> Bool {
    values.contains { member in
      if case .number(let a)? = member, case .number(let b)? = value, a.isNaN, b.isNaN {
        return true
      }
      return member == value
    }
  }
  guard allowed(fallback) else {
    throw LexicalError("enumValue: the default value is not one of the values")
  }
  return SerializationSchema(
    { value in
      value != nil && allowed(value) ? value : fallback
    }, .enumeration(values), fallback,
    accepts: { value in
      allowed(value) && (value != nil || fallback == nil)
    })
}

private func liftIsEqual(
  _ inner: SerializationSchema
) -> (
  @Sendable (JSONValue?, JSONValue?) -> Bool
)? {
  guard let isEqual = inner.isEqual else { return nil }
  return { a, b in
    a == nil || a == .null || b == nil || b == .null ? a == b : isEqual(a, b)
  }
}

/// `nullable(inner, {defaultAsNull})`.
public func nullable(
  _ inner: SerializationSchema, defaultAsNull: Bool = false
) throws -> SerializationSchema {
  try undeclared("nullable", inner)
  return SerializationSchema(
    { value in
      if value == nil || value == .null { return .null }
      let parsed = inner(value)
      return defaultAsNull && inner.isSchemaDefault(parsed) ? .null : parsed
    }, .nullable(inner, defaultAsNull: defaultAsNull), nil, liftIsEqual(inner),
    accepts: { value in value == nil || value == .null || inner.acceptsValue(value) })
}

/// `optional(inner, {omitDefault})`.
public func optional(
  _ inner: SerializationSchema, omitDefault: Bool = false
) throws -> SerializationSchema {
  try undeclared("optional", inner)
  return SerializationSchema(
    { value in
      if value == nil { return nil }
      let parsed = inner(value)
      return omitDefault && inner.isSchemaDefault(parsed) ? nil : parsed
    }, .optional(inner, omitDefault: omitDefault), nil, liftIsEqual(inner),
    accepts: { value in value == nil || inner.acceptsValue(value) })
}

/// `unionValue(members, defaultValue)`: the value as the member that fits it
/// best parses it, otherwise the default (the first member's unless given).
public func unionValue(
  _ members: [SerializationSchema], default defaultValue: JSONValue?? = .none
) throws -> SerializationSchema {
  guard !members.isEmpty else {
    throw LexicalError("unionValue: at least one member schema is required")
  }
  for member in members {
    try undeclared("unionValue", member)
  }
  let fallback: JSONValue? =
    switch defaultValue {
    case .none: members[0].defaultValue
    case .some(let value): value
    }
  return SerializationSchema(
    { value in
      if value == nil { return fallback }
      guard let member = bestUnionMember(members, value).member else { return fallback }
      return member(value)
    }, .union(members), fallback,
    // `$sameContent`, which is content equality on JSON values.
    { a, b in a == b },
    accepts: { value in
      (value != nil || fallback == nil) && members.contains { $0.acceptsValue(value) }
    })
}

/// `aliasedValue(inner, aliases)`: also accepts the string names in `aliases`.
public func aliasedValue(
  _ inner: SerializationSchema, _ aliases: [String: JSONValue]
) throws -> SerializationSchema {
  try undeclared("aliasedValue", inner)
  return SerializationSchema(
    { value in
      if case .string(let s)? = value, let alias = aliases[s] { return alias }
      return inner(value)
    }, .aliased(inner, aliases: aliases), inner.defaultValue, inner.isEqual,
    accepts: { value in
      if case .string(let s)? = value, aliases[s] != nil { return true }
      return inner.acceptsValue(value)
    })
}

/// `transformValue(inner, transform, {isEqual})`: parses with `inner`, then
/// maps the result. `isEqual` is only for a transform to a container.
public func transformValue(
  _ inner: SerializationSchema, _ transform: @escaping @Sendable (JSONValue?) -> JSONValue?,
  isEqual: (@Sendable (JSONValue?, JSONValue?) -> Bool)? = nil
) throws -> SerializationSchema {
  try undeclared("transformValue", inner)
  let defaultValue = transform(inner.defaultValue)
  let schema = SerializationSchema(
    { value in transform(inner(value)) }, .transform(inner), defaultValue, isEqual,
    accepts: { value in inner.acceptsValue(value) })
  if isEqual != nil {
    switch schema.defaultValue {
    case .array?, .object?: break
    default:
      throw LexicalError(
        "transformValue: isEqual compares reference-typed values, but this schema parses to \(jsTypeof(schema.defaultValue)). Two primitives that are not === are two different serialized values; declaring them equal loses one of them in the compact form."
      )
    }
  }
  return schema
}

/// `rawValue()`: the JSON as is.
public func rawValue() -> SerializationSchema {
  SerializationSchema({ $0 }, .raw, accepts: { _ in true })
}

/// `arrayValue(item)`: each element parsed by `item`; anything but an array
/// is the empty array. An element `item` parses to `undefined` is written as
/// `null`, as `JSON.stringify` writes it.
public func arrayValue(_ item: SerializationSchema) throws -> SerializationSchema {
  try undeclared("arrayValue", item)
  return SerializationSchema(
    { value in
      guard case .array(let items)? = value else { return .array([]) }
      return .array(items.map { item($0) ?? .null })
    }, .array(item: item), nil,
    { a, b in
      guard case .array(let a)? = a, case .array(let b)? = b, a.count == b.count else {
        return false
      }
      return zip(a, b).allSatisfy { item.isSchemaEqual($0, $1) }
    },
    accepts: { value in
      if case .array? = value { return true }
      return false
    })
}

/// JS `<` on strings: by UTF-16 code unit.
private func jsLess(_ a: String, _ b: String) -> Bool {
  a.utf16.lexicographicallyPrecedes(b.utf16)
}

private func jsSame(_ a: String, _ b: String) -> Bool { a.utf16.elementsEqual(b.utf16) }

/// `arraySetValue(item)`: a set of strings, written as one sorted array
/// without duplicates; members `item` does not accept are dropped. Only the
/// whole value of a `createState`.
public func arraySetValue(_ item: SerializationSchema) throws -> SerializationSchema {
  try undeclared("arraySetValue", item)
  guard case .string? = item.defaultValue else {
    throw LexicalError("arraySetValue: the item schema must produce strings")
  }
  return SerializationSchema(
    { value in
      guard case .array(let items)? = value else { return .array([]) }
      var members: [String] = []
      for member in items where item.acceptsValue(member) {
        if case .string(let s)? = item(member) { members.append(s) }
      }
      members.sort(by: jsLess)
      var result: [JSONValue] = []
      for (i, member) in members.enumerated() where i == 0 || !jsSame(member, members[i - 1]) {
        result.append(.string(member))
      }
      return .array(result)
    }, .set(item: item), nil,
    // `isArraySetEqual`: element by element.
    { a, b in a == b },
    accepts: { value in
      if case .array? = value { return true }
      return false
    })
}

/// The `Object.prototype` members a serialized object inherits.
private let OBJECT_PROTOTYPE_MEMBERS: Set<String> = [
  "__defineGetter__", "__defineSetter__", "__lookupGetter__", "__lookupSetter__", "__proto__",
  "constructor", "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "toLocaleString",
  "toString", "valueOf",
]

/// `checkFieldNames`.
func checkFieldNames(
  _ combinator: String, _ keys: [String], _ schemas: [SerializationSchema]
)
  throws
{
  for (key, schema) in zip(keys, schemas) {
    if key == "__proto__" {
      throw LexicalError("\(combinator): \"__proto__\" is not a valid field name")
    }
    if OBJECT_PROTOTYPE_MEMBERS.contains(key) {
      throw LexicalError(
        "\(combinator): \"\(key)\" is a member of Object.prototype, which a serialized object inherits, so it cannot be told apart from a property the document never carried"
      )
    }
    try notNestedSet(combinator, schema)
  }
}

/// A JS property read on a JSON value, as `objectValue` reads its source.
private func jsProperty(_ value: JSONValue?, _ key: String) -> JSONValue? {
  switch value {
  case .object(let object)?: return object[key]
  case .array(let items)?:
    if key == "length" { return .int(items.count) }
    if let index = Int(key), String(index) == key, items.indices.contains(index) {
      return items[index]
    }
    return nil
  default: return nil
  }
}

/// `objectValue(fields)`: an object with exactly the declared fields, each
/// parsed by its schema.
public func objectValue(_ fields: [(String, SerializationSchema)]) throws -> SerializationSchema {
  let entries = fields.map { (key: $0.0, schema: $0.1) }
  try checkFieldNames("objectValue", entries.map(\.key), entries.map(\.schema))
  return SerializationSchema(
    { value in
      var result = JSONObject()
      for (key, schema) in entries {
        result.set(key, schema(jsProperty(value, key)))
      }
      return .object(result)
    }, .object(entries), nil,
    { a, b in
      guard case .object(let a)? = a, case .object(let b)? = b,
        !hasUndeclaredKey(a, entries), !hasUndeclaredKey(b, entries)
      else { return false }
      return entries.allSatisfy { $0.schema.isSchemaEqual(a[$0.key], b[$0.key]) }
    },
    accepts: { value in
      guard case .object(let object)? = value else { return false }
      return !hasUndeclaredKey(object, entries)
    })
}

/// JS `typeof` of a JSON value.
private func jsTypeof(_ value: JSONValue?) -> String {
  switch value {
  case nil: "undefined"
  case .null?, .array?, .object?: "object"
  case .bool?: "boolean"
  case .number?: "number"
  case .string?: "string"
  }
}

/// JS `String(n)` for the numbers an error message names.
private func jsNumberString(_ n: Double) -> String {
  JSONValue.number(n).stringify()
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
  /// Copies the field from the node a clone is made from, for a property
  /// declared with `field:` (`withField`), so its class needs no
  /// `afterCloneFrom`.
  let carry: ((LexicalNode, LexicalNode) -> Void)?

  public init<Node: LexicalNode>(
    _ key: String, _ schema: SerializationSchema, of _: Node.Type,
    get: ((Node) throws -> JSONValue?)?,
    when: ((Node) throws -> Bool)? = nil,
    set: ((Node, JSONValue?) throws -> Void)?
  ) throws {
    try notNestedSet("withAccessors", schema)
    self.key = key
    self.schema = schema
    self.get = get.map { get in { try get($0 as! Node) } }
    self.when = when.map { when in { try when($0 as! Node) } }
    self.set = set.map { set in { try set($0 as! Node, $1) } }
    self.carry = nil
  }

  /// `withField(schema, {field})`: the property is the stored field at
  /// `field`, read and written directly, and carried across a clone. `when`
  /// writes it only where it holds.
  public init<Node: LexicalNode, Value: SchemaFieldValue>(
    _ key: String, _ schema: SerializationSchema,
    field: ReferenceWritableKeyPath<Node, Value>, when: ((Node) throws -> Bool)? = nil
  ) throws {
    try notNestedSet("withField", schema)
    // An optional property is absent when it has no value; any other one
    // writes JSON `null`.
    let absent: JSONValue? =
      if case .optional = schema.meta { nil } else { .null }
    self.key = key
    self.schema = schema
    self.get = { node in (node as! Node)[keyPath: field].schemaValue ?? absent }
    self.when = when.map { when in { try when($0 as! Node) } }
    self.set = { node, value in
      if let value = Value(schemaValue: value) {
        (node as! Node)[keyPath: field] = value
      }
    }
    self.carry = { to, from in
      (to as! Node)[keyPath: field] = (from as! Node)[keyPath: field]
    }
  }

  /// Whether the compact form omits `value`: `undefined`, or the default.
  func isCompactDefault(_ value: JSONValue?) -> Bool {
    value == nil || schema.isSchemaDefault(value)
  }
}

/// A Swift type a `withField` property can be stored as, converted to and
/// from the schema's JSON domain.
public protocol SchemaFieldValue {
  /// The value as the schema's JSON; `nil` when it has none.
  var schemaValue: JSONValue? { get }
  /// The value a parsed JSON value stands for, or `nil` when it is not one.
  init?(schemaValue: JSONValue?)
}

extension String: SchemaFieldValue {
  public var schemaValue: JSONValue? { .string(self) }
  public init?(schemaValue: JSONValue?) {
    guard case .string(let s)? = schemaValue else { return nil }
    self = s
  }
}

extension Double: SchemaFieldValue {
  public var schemaValue: JSONValue? { .number(self) }
  public init?(schemaValue: JSONValue?) {
    guard case .number(let n)? = schemaValue else { return nil }
    self = n
  }
}

extension Int: SchemaFieldValue {
  public var schemaValue: JSONValue? { .int(self) }
  public init?(schemaValue: JSONValue?) {
    guard case .number(let n)? = schemaValue, let i = Int(exactly: n) else { return nil }
    self = i
  }
}

extension Bool: SchemaFieldValue {
  public var schemaValue: JSONValue? { .bool(self) }
  public init?(schemaValue: JSONValue?) {
    guard case .bool(let b)? = schemaValue else { return nil }
    self = b
  }
}

extension JSONValue: SchemaFieldValue {
  public var schemaValue: JSONValue? { self }
  public init?(schemaValue: JSONValue?) {
    guard let schemaValue else { return nil }
    self = schemaValue
  }
}

extension Optional: SchemaFieldValue where Wrapped: SchemaFieldValue {
  /// `nil` has no value: absent from the JSON, or `null` for a nullable.
  public var schemaValue: JSONValue? { self?.schemaValue }
  public init?(schemaValue: JSONValue?) {
    if schemaValue == nil || schemaValue == .null {
      self = .none
    } else if let wrapped = Wrapped(schemaValue: schemaValue) {
      self = .some(wrapped)
    } else {
      return nil
    }
  }
}

extension Array: SchemaFieldValue where Element: SchemaFieldValue {
  public var schemaValue: JSONValue? { .array(map { $0.schemaValue ?? .null }) }
  public init?(schemaValue: JSONValue?) {
    guard case .array(let items)? = schemaValue else { return nil }
    var result: [Element] = []
    for item in items {
      guard let element = Element(schemaValue: item) else { return nil }
      result.append(element)
    }
    self = result
  }
}
