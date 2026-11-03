/// The `Record<string, string>` of CSS properties that
/// `getStyleObjectFromCSS` returns: properties keep the order they were
/// first set in, as a JS object's string keys do.
public struct StyleObject: Equatable, Sequence, ExpressibleByDictionaryLiteral {
  public private(set) var keys: [String] = []
  private var values: [String: String] = [:]

  public init() {}

  public init(dictionaryLiteral elements: (String, String)...) {
    for (key, value) in elements { self[key] = value }
  }

  public var count: Int { keys.count }
  public var isEmpty: Bool { keys.isEmpty }

  /// Reads a property; setting `nil` deletes it, and setting an existing
  /// property keeps its position.
  public subscript(key: String) -> String? {
    get { values[key] }
    set {
      if let newValue {
        if values.updateValue(newValue, forKey: key) == nil { keys.append(key) }
      } else if values.removeValue(forKey: key) != nil {
        keys.removeAll { $0 == key }
      }
    }
  }

  public func makeIterator() -> some IteratorProtocol<(key: String, value: String)> {
    keys.lazy.map { (key: $0, value: values[$0]!) }.makeIterator()
  }

  /// Compares the properties, not their order, as `toEqual` does.
  public static func == (lhs: StyleObject, rhs: StyleObject) -> Bool { lhs.values == rhs.values }
}

/// `getStyleObjectFromCSS(css)`: parses inline CSS text into its
/// properties. Comments are dropped; `:` and `;` inside quotes or
/// parentheses are not delimiters.
public func getStyleObjectFromCSS(_ css: String) -> StyleObject {
  var styles = StyleObject()
  let units = Array(css.utf16)
  if units.isEmpty { return styles }
  let colon = UInt16(UInt8(ascii: ":"))
  let semicolon = UInt16(UInt8(ascii: ";"))
  let slash = UInt16(UInt8(ascii: "/"))
  let star = UInt16(UInt8(ascii: "*"))
  let backslash = UInt16(UInt8(ascii: "\\"))
  let doubleQuote = UInt16(UInt8(ascii: "\""))
  let singleQuote = UInt16(UInt8(ascii: "'"))
  let openParen = UInt16(UInt8(ascii: "("))
  let closeParen = UInt16(UInt8(ascii: ")"))

  var currentProperty: [UInt16] = []
  var currentValue: [UInt16] = []
  var currentQuote: UInt16? = nil
  var inComment = false
  var isEscaped = false
  var isParsingValue = false
  var parenthesisDepth = 0
  var chunkStart = -1
  let length = units.count

  func flush(_ end: Int) {
    if chunkStart != -1 {
      if isParsingValue {
        currentValue += units[chunkStart..<end]
      } else {
        currentProperty += units[chunkStart..<end]
      }
      chunkStart = -1
    }
  }
  func commit() {
    let property = String(decoding: jsTrim(currentProperty[...]), as: UTF16.self)
    let value = String(decoding: jsTrim(currentValue[...]), as: UTF16.self)
    if !property.isEmpty && !value.isEmpty {
      styles[property] = value
    }
  }

  var i = 0
  while i < length {
    defer { i += 1 }
    let char = units[i]
    let next: UInt16? = i + 1 < length ? units[i + 1] : nil
    if inComment {
      if char == star && next == slash {
        inComment = false
        i += 1
      }
      continue
    }
    if isEscaped {
      if chunkStart == -1 { chunkStart = i }
      isEscaped = false
      continue
    }
    if let quote = currentQuote {
      if chunkStart == -1 { chunkStart = i }
      if char == backslash {
        isEscaped = true
      } else if char == quote {
        currentQuote = nil
      }
      continue
    }
    if char == slash && next == star {
      flush(i)
      inComment = true
      i += 1
      continue
    }
    if char == doubleQuote || char == singleQuote {
      if chunkStart == -1 { chunkStart = i }
      currentQuote = char
      continue
    }
    if char == openParen {
      if chunkStart == -1 { chunkStart = i }
      parenthesisDepth += 1
      continue
    }
    if char == closeParen {
      if chunkStart == -1 { chunkStart = i }
      parenthesisDepth = max(0, parenthesisDepth - 1)
      continue
    }
    if !isParsingValue && char == colon && parenthesisDepth == 0 {
      if chunkStart != -1 {
        currentProperty += units[chunkStart..<i]
        chunkStart = -1
      }
      isParsingValue = true
      continue
    }
    if char == semicolon && parenthesisDepth == 0 {
      flush(i)
      commit()
      currentProperty = []
      currentValue = []
      isParsingValue = false
      continue
    }
    if chunkStart == -1 { chunkStart = i }
  }
  flush(length)
  commit()
  return styles
}
