/// A JSON value, with objects that keep their keys in insertion order, the way
/// a JS object does. `stringify()` writes exactly the bytes `JSON.stringify`
/// writes for the same value, which the byte-exact serialization relies on.
public enum JSONValue: Equatable, Sendable {
  case null
  case bool(Bool)
  case number(Double)
  case string(String)
  case array([JSONValue])
  case object(JSONObject)

  public static func int(_ value: Int) -> JSONValue { .number(Double(value)) }

  public var stringValue: String? {
    if case .string(let s) = self { return s }
    return nil
  }

  public var numberValue: Double? {
    if case .number(let n) = self { return n }
    return nil
  }

  public var boolValue: Bool? {
    if case .bool(let b) = self { return b }
    return nil
  }

  public var arrayValue: [JSONValue]? {
    if case .array(let a) = self { return a }
    return nil
  }

  public var objectValue: JSONObject? {
    if case .object(let o) = self { return o }
    return nil
  }

  public var isNull: Bool {
    if case .null = self { return true }
    return false
  }

  /// `JSON.stringify(value)`.
  public func stringify() -> String {
    var out = ""
    write(to: &out)
    return out
  }

  func write(to out: inout String) {
    switch self {
    case .null: out += "null"
    case .bool(let b): out += b ? "true" : "false"
    case .number(let n): out += n.isFinite ? jsNumberToString(n) : "null"
    case .string(let s): writeJSONString(s, to: &out)
    case .array(let items):
      out += "["
      for (i, item) in items.enumerated() {
        if i > 0 { out += "," }
        item.write(to: &out)
      }
      out += "]"
    case .object(let object):
      object.write(to: &out)
    }
  }

  /// `JSON.parse(text)`.
  public static func parse(_ text: String) throws -> JSONValue {
    var parser = JSONParser(text)
    return try parser.parseDocument()
  }
}

extension JSONValue: ExpressibleByStringLiteral, ExpressibleByIntegerLiteral,
  ExpressibleByFloatLiteral, ExpressibleByBooleanLiteral, ExpressibleByNilLiteral,
  ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral
{
  public init(stringLiteral value: String) { self = .string(value) }
  public init(integerLiteral value: Int) { self = .number(Double(value)) }
  public init(floatLiteral value: Double) { self = .number(value) }
  public init(booleanLiteral value: Bool) { self = .bool(value) }
  public init(nilLiteral: ()) { self = .null }
  public init(arrayLiteral elements: JSONValue...) { self = .array(elements) }
  public init(dictionaryLiteral elements: (String, JSONValue)...) {
    var object = JSONObject()
    for (key, value) in elements { object[key] = value }
    self = .object(object)
  }
}

/// A JSON object whose keys keep insertion order, like a JS object with
/// string keys. A key can hold `nil`, the JS `undefined`: it keeps its
/// position, and `stringify` leaves it out, as `JSON.stringify` does. Setting
/// a key that is already present keeps its position.
public struct JSONObject: Equatable, Sendable, Sequence {
  // In JS order. Most objects are a node's handful of properties, which a
  // linear scan finds sooner than a hash table, and without allocating one;
  // larger objects also get `index`.
  private var entries: [(key: String, value: JSONValue?)] = []
  private var index: [String: Int]?

  private static let indexThreshold = 16

  public init() {}

  public init(_ entries: [(String, JSONValue)]) {
    for (key, value) in entries { self[key] = value }
  }

  /// The keys, in the order a JS object lists them, `undefined` ones too.
  public var keys: [String] { entries.map(\.key) }

  private func position(of key: String) -> Int? {
    if let index { return index[key] }
    return entries.firstIndex { $0.key == key }
  }

  private mutating func append(_ key: String, _ value: JSONValue?) {
    entries.append((key, value))
    if index != nil {
      index![key] = entries.count - 1
    } else if entries.count > Self.indexThreshold {
      reindex()
    }
  }

  private mutating func reindex() {
    guard entries.count > Self.indexThreshold else {
      index = nil
      return
    }
    var index: [String: Int] = [:]
    index.reserveCapacity(entries.count)
    for (i, entry) in entries.enumerated() { index[entry.key] = i }
    self.index = index
  }

  /// The value for `key`, or `nil` when it is missing or `undefined`.
  public subscript(key: String) -> JSONValue? {
    get { position(of: key).flatMap { entries[$0].value } }
    set { set(key, newValue) }
  }

  /// Sets `key` to `value`, where `nil` is `undefined`.
  public mutating func set(_ key: String, _ value: JSONValue?) {
    if let i = position(of: key) {
      entries[i].value = value
    } else if let index = arrayIndex(key) {
      // A JS object lists array-index keys first, in ascending order.
      let position =
        entries.firstIndex { other in arrayIndex(other.key).map { $0 > index } ?? true }
        ?? entries.count
      entries.insert((key, value), at: position)
      if self.index != nil || entries.count > Self.indexThreshold { reindex() }
    } else {
      append(key, value)
    }
  }

  /// Whether `key` is present (`key in object`), even when `undefined`.
  public func has(_ key: String) -> Bool { position(of: key) != nil }

  /// `delete object[key]`.
  public mutating func remove(_ key: String) {
    if let i = position(of: key) {
      entries.remove(at: i)
      if index != nil { reindex() }
    }
  }

  /// The entries `JSON.stringify` writes: every key whose value is defined.
  public func makeIterator() -> IndexingIterator<[(key: String, value: JSONValue)]> {
    entries.compactMap { entry in entry.value.map { (key: entry.key, value: $0) } }
      .makeIterator()
  }

  public static func == (lhs: JSONObject, rhs: JSONObject) -> Bool {
    Array(lhs).elementsEqual(Array(rhs)) { $0.key == $1.key && $0.value == $1.value }
  }

  func write(to out: inout String) {
    out += "{"
    var first = true
    for (key, value) in entries {
      guard let value else { continue }
      if !first { out += "," }
      first = false
      writeJSONString(key, to: &out)
      out += ":"
      value.write(to: &out)
    }
    out += "}"
  }

  public func stringify() -> String {
    var out = ""
    write(to: &out)
    return out
  }
}

/// `JSON.stringify`'s string quoting: `"`, `\` and C0 controls are escaped,
/// using the short forms where JSON has them, and everything else is written
/// as is.
/// The value of `key` when it is a JS array index: a canonical decimal
/// below 2³² − 1.
private func arrayIndex(_ key: String) -> UInt32? {
  guard let first = key.utf8.first, first >= 0x30, first <= 0x39,
    key == "0" || first != 0x30, key.utf8.count <= 10,
    key.utf8.allSatisfy({ $0 >= 0x30 && $0 <= 0x39 }),
    let value = UInt64(key), value < 0xFFFF_FFFF
  else {
    return nil
  }
  return UInt32(value)
}

func writeJSONString(_ s: String, to out: inout String) {
  out += "\""
  var run = s.unicodeScalars.startIndex
  let scalars = s.unicodeScalars
  var i = scalars.startIndex
  while i < scalars.endIndex {
    let v = scalars[i].value
    let escape: String?
    switch v {
    case 0x22: escape = "\\\""
    case 0x5C: escape = "\\\\"
    case 0x08: escape = "\\b"
    case 0x0C: escape = "\\f"
    case 0x0A: escape = "\\n"
    case 0x0D: escape = "\\r"
    case 0x09: escape = "\\t"
    case 0..<0x20:
      let hex = String(v, radix: 16)
      escape = "\\u" + String(repeating: "0", count: 4 - hex.count) + hex
    default: escape = nil
    }
    if let escape {
      out += Substring(scalars[run..<i])
      out += escape
      i = scalars.index(after: i)
      run = i
    } else {
      i = scalars.index(after: i)
    }
  }
  // Appending to `out.unicodeScalars` would copy all of `out` each time.
  out += Substring(scalars[run..<scalars.endIndex])
  out += "\""
}

/// `Number.prototype.toString()` for a finite double (ECMA-262
/// Number::toString), built on Swift's shortest round-trip digits.
func jsNumberToString(_ value: Double) -> String {
  if value == 0 { return "0" }
  if value < 0 { return "-" + jsNumberToString(-value) }
  // Swift's description has the same shortest digits as JS; only the layout
  // differs ("1e-07" vs "1e-7", "100.0" vs "100").
  let description = value.description
  var mantissa = Substring(description)
  var exponent = 0
  if let e = description.firstIndex(where: { $0 == "e" || $0 == "E" }) {
    mantissa = description[..<e]
    exponent = Int(description[description.index(after: e)...])!
  }
  var digits: String
  var pointPosition: Int
  if let dot = mantissa.firstIndex(of: ".") {
    digits = String(mantissa[..<dot]) + String(mantissa[mantissa.index(after: dot)...])
    pointPosition = mantissa.distance(from: mantissa.startIndex, to: dot)
  } else {
    digits = String(mantissa)
    pointPosition = digits.count
  }
  // Normalize to digits d1...dk with value 0.d1...dk × 10^n.
  while digits.count > 1 && digits.first == "0" {
    digits.removeFirst()
    pointPosition -= 1
  }
  while digits.count > 1 && digits.last == "0" {
    digits.removeLast()
  }
  let k = digits.count
  let n = pointPosition + exponent
  if k <= n && n <= 21 {
    return digits + String(repeating: "0", count: n - k)
  }
  if 0 < n && n <= 21 {
    let split = digits.index(digits.startIndex, offsetBy: n)
    return String(digits[..<split]) + "." + String(digits[split...])
  }
  if -6 < n && n <= 0 {
    return "0." + String(repeating: "0", count: -n) + digits
  }
  let e = n - 1
  let sign = e < 0 ? "-" : "+"
  let first = String(digits.first!)
  let rest = digits.dropFirst()
  return first + (rest.isEmpty ? "" : "." + rest) + "e" + sign + String(abs(e))
}

/// A `JSON.parse` over the text's UTF-8, which reads the same values as JS
/// reads its UTF-16; error positions are UTF-16 offsets, as in JS.
struct JSONParser {
  let bytes: [UInt8]
  var i = 0

  init(_ text: String) { bytes = Array(text.utf8) }

  mutating func parseDocument() throws -> JSONValue {
    skipWhitespace()
    let value = try parseValue()
    skipWhitespace()
    guard i == bytes.count else { throw error("Unexpected non-whitespace character after JSON") }
    return value
  }

  func error(_ message: String) -> LexicalError {
    let position = String(decoding: bytes[0..<min(i, bytes.count)], as: UTF8.self).utf16.count
    return LexicalError("JSON.parse: \(message) at position \(position)")
  }

  mutating func skipWhitespace() {
    while i < bytes.count {
      switch bytes[i] {
      case 0x20, 0x09, 0x0A, 0x0D: i += 1
      default: return
      }
    }
  }

  mutating func expect(_ literal: StaticString) throws {
    var ok = true
    literal.withUTF8Buffer { literal in
      for byte in literal {
        guard i < bytes.count, bytes[i] == byte else {
          ok = false
          return
        }
        i += 1
      }
    }
    guard ok else { throw error("Unexpected token") }
  }

  mutating func parseValue() throws -> JSONValue {
    guard i < bytes.count else { throw error("Unexpected end of JSON input") }
    switch bytes[i] {
    case 0x7B: return .object(try parseObject())
    case 0x5B: return .array(try parseArray())
    case 0x22: return .string(try parseString())
    case 0x74:
      try expect("true")
      return .bool(true)
    case 0x66:
      try expect("false")
      return .bool(false)
    case 0x6E:
      try expect("null")
      return .null
    default: return .number(try parseNumber())
    }
  }

  mutating func parseObject() throws -> JSONObject {
    i += 1
    skipWhitespace()
    if i < bytes.count, bytes[i] == 0x7D {
      i += 1
      return JSONObject()
    }
    // `set` keeps a repeated key's first position and orders array-index
    // keys, as `JSON.parse` does.
    var object = JSONObject()
    while true {
      skipWhitespace()
      guard i < bytes.count, bytes[i] == 0x22 else { throw error("Expected property name") }
      let key = try parseString()
      skipWhitespace()
      try expect(":")
      skipWhitespace()
      object.set(key, try parseValue())
      skipWhitespace()
      guard i < bytes.count else { throw error("Unexpected end of JSON input") }
      if bytes[i] == 0x2C {
        i += 1
        continue
      }
      try expect("}")
      return object
    }
  }

  mutating func parseArray() throws -> [JSONValue] {
    i += 1
    var items: [JSONValue] = []
    skipWhitespace()
    if i < bytes.count, bytes[i] == 0x5D {
      i += 1
      return items
    }
    while true {
      skipWhitespace()
      items.append(try parseValue())
      skipWhitespace()
      guard i < bytes.count else { throw error("Unexpected end of JSON input") }
      if bytes[i] == 0x2C {
        i += 1
        continue
      }
      try expect("]")
      return items
    }
  }

  mutating func parseString() throws -> String {
    i += 1
    let start = i
    // Most strings have no escape: their bytes are the string.
    while i < bytes.count {
      let b = bytes[i]
      if b == 0x22 {
        i += 1
        return String(decoding: bytes[start..<(i - 1)], as: UTF8.self)
      }
      if b == 0x5C || b < 0x20 { break }
      i += 1
    }
    // An escape: collect UTF-16 code units, which a `\u` escape names.
    var out = Array(String(decoding: bytes[start..<i], as: UTF8.self).utf16)
    var run = i
    func flush(_ end: Int) {
      if run < end { out += String(decoding: bytes[run..<end], as: UTF8.self).utf16 }
    }
    while true {
      guard i < bytes.count else { throw error("Unterminated string in JSON") }
      let b = bytes[i]
      switch b {
      case 0x22:
        flush(i)
        i += 1
        return String(decoding: out, as: UTF16.self)
      case 0x5C:
        flush(i)
        i += 1
        guard i < bytes.count else { throw error("Bad escaped character") }
        let e = bytes[i]
        i += 1
        switch e {
        case 0x22, 0x5C, 0x2F: out.append(UInt16(e))
        case 0x62: out.append(0x08)
        case 0x66: out.append(0x0C)
        case 0x6E: out.append(0x0A)
        case 0x72: out.append(0x0D)
        case 0x74: out.append(0x09)
        case 0x75:
          guard i + 4 <= bytes.count,
            let code = UInt16(String(decoding: bytes[i..<i + 4], as: UTF8.self), radix: 16)
          else { throw error("Bad Unicode escape") }
          out.append(code)
          i += 4
        default: throw error("Bad escaped character")
        }
        run = i
      case 0..<0x20:
        throw error("Bad control character in string literal")
      default:
        i += 1
      }
    }
  }

  mutating func parseNumber() throws -> Double {
    let start = i
    let isNegative = i < bytes.count && bytes[i] == 0x2D
    if isNegative { i += 1 }
    guard i < bytes.count, (0x30...0x39).contains(bytes[i]) else { throw error("Unexpected token") }
    let digitsStart = i
    if bytes[i] == 0x30 {
      i += 1
    } else {
      while i < bytes.count, (0x30...0x39).contains(bytes[i]) { i += 1 }
    }
    var isInteger = true
    if i < bytes.count, bytes[i] == 0x2E {
      isInteger = false
      i += 1
      guard i < bytes.count, (0x30...0x39).contains(bytes[i]) else {
        throw error("Unterminated fractional number")
      }
      while i < bytes.count, (0x30...0x39).contains(bytes[i]) { i += 1 }
    }
    if i < bytes.count, bytes[i] == 0x65 || bytes[i] == 0x45 {
      isInteger = false
      i += 1
      if i < bytes.count, bytes[i] == 0x2B || bytes[i] == 0x2D { i += 1 }
      guard i < bytes.count, (0x30...0x39).contains(bytes[i]) else {
        throw error("Exponent part is missing a number")
      }
      while i < bytes.count, (0x30...0x39).contains(bytes[i]) { i += 1 }
    }
    // An integer of up to 15 digits is exact as a double.
    if isInteger, i - digitsStart <= 15 {
      var value = 0
      for k in digitsStart..<i { value = value * 10 + Int(bytes[k] - 0x30) }
      return isNegative ? -Double(value) : Double(value)
    }
    return Double(String(decoding: bytes[start..<i], as: UTF8.self))!
  }
}
