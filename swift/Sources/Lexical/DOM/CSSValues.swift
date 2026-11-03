// How the inline style declaration normalizes the values written to it.
// Upstream tests run on jsdom, whose declaration parses each value and
// serializes it back; exported HTML carries that form byte for byte. This
// follows it for the values Lexical and its tests write, pinned by the
// `css/values` vectors: hex and functional colors become `rgb()`/`rgba()`,
// numbers lose their redundant digits, a bare `0` length becomes `0px`,
// keywords are lowercased, `calc()` of a product is evaluated, commas are
// spaced and single-quoted strings are requoted. Values outside that are
// kept as written, with their whitespace collapsed; jsdom also rejects
// values it cannot parse, which this does not.

/// Bits of `cssPropertyTraitsByName`.
struct CSSPropertyTraits: OptionSet {
  let rawValue: UInt8
  /// Hex and functional colors serialize as `rgb()`/`rgba()`.
  static let color = CSSPropertyTraits(rawValue: 1)
  /// A bare `0` serializes as `0px`.
  static let zeroLength = CSSPropertyTraits(rawValue: 2)
  /// Identifiers keep their case (font and animation names).
  static let preserveCase = CSSPropertyTraits(rawValue: 4)
  /// The value is kept as written.
  static let verbatim = CSSPropertyTraits(rawValue: 8)
}

/// The declarations in inline CSS text, in order, a repeated property
/// keeping the position of its first declaration and its last value.
func parseCSSDeclarations(_ css: String) -> [(String, String)] {
  getStyleObjectFromCSS(css).map { ($0.key, $0.value) }
}

/// The value `name` serializes `value` as, or `nil` when the declaration
/// does not accept the property.
func normalizeCSSValue(_ name: String, _ value: String) -> String? {
  if name.hasPrefix("--") {
    let trimmed = jsTrim(value)
    return trimmed.isEmpty ? nil : trimmed
  }
  guard let raw = cssPropertyTraitsByName[name] else { return nil }
  let traits = CSSPropertyTraits(rawValue: raw)
  var out = ""
  var needsSpace = false
  for token in tokenizeCSSValue(value) {
    switch token {
    case .comma:
      out += ","
      needsSpace = true
    case .word(let word):
      if needsSpace || !out.isEmpty { out += " " }
      needsSpace = false
      out += traits.contains(.verbatim) ? word : normalizeCSSWord(word, traits)
    }
  }
  return out.isEmpty ? nil : out
}

private enum CSSToken {
  case word(String)
  case comma
}

/// Splits a value into words and commas at the top level; strings and
/// parenthesized groups stay inside their word.
private func tokenizeCSSValue(_ value: String) -> [CSSToken] {
  var tokens: [CSSToken] = []
  var word = ""
  var depth = 0
  var quote: Character? = nil
  var escaped = false
  func flush() {
    if !word.isEmpty { tokens.append(.word(word)) }
    word = ""
  }
  for char in value {
    if let current = quote {
      word.append(char)
      if escaped {
        escaped = false
      } else if char == "\\" {
        escaped = true
      } else if char == current {
        quote = nil
      }
      continue
    }
    switch char {
    case "\"", "'":
      quote = char
      word.append(char)
    case "(":
      depth += 1
      word.append(char)
    case ")":
      depth = max(0, depth - 1)
      word.append(char)
    case "," where depth == 0:
      flush()
      tokens.append(.comma)
    case " ", "\t", "\n", "\r", "\r\n", "\u{0C}":
      if depth == 0 { flush() } else { word.append(char) }
    default:
      word.append(char)
    }
  }
  flush()
  return tokens
}

private func normalizeCSSWord(_ word: String, _ traits: CSSPropertyTraits) -> String {
  guard let first = word.first else { return word }
  if first == "'" { return requoteCSSString(word) }
  if first == "\"" { return word }
  if traits.contains(.color), first == "#", let rgba = parseHexColor(word.dropFirst()) {
    return serializeColor(rgba, hexAlpha: true)
  }
  if let open = word.firstIndex(of: "("), word.last == ")" {
    let function = word[..<open].lowercased()
    let arguments = String(word[word.index(after: open)..<word.index(before: word.endIndex)])
    if traits.contains(.color), let rgba = parseColorFunction(function, arguments) {
      return serializeColor(rgba, hexAlpha: false)
    }
    if function == "calc", let result = evaluateCalcProduct(arguments) {
      return "calc(\(result))"
    }
    return word
  }
  if let (number, unit) = parseCSSNumber(word) {
    if unit.isEmpty && number == 0 && traits.contains(.zeroLength) { return "0px" }
    return formatCSSNumber(number) + unit.lowercased()
  }
  return traits.contains(.preserveCase) ? word : word.lowercased()
}

/// `'text'` as `"text"`.
private func requoteCSSString(_ word: String) -> String {
  var body = word.dropFirst()
  if body.last == "'" { body = body.dropLast() }
  var out = "\""
  var escaped = false
  for char in body {
    if escaped {
      if char != "'" { out.append("\\") }
      out.append(char)
      escaped = false
    } else if char == "\\" {
      escaped = true
    } else if char == "\"" {
      out += "\\\""
    } else {
      out.append(char)
    }
  }
  return out + "\""
}

/// A number with an optional unit: `+5px`, `1.50em`, `1e1px`, `.5`.
private func parseCSSNumber(_ word: String) -> (Double, String)? {
  let scalars = Array(word.unicodeScalars)
  var i = 0
  func isDigit(_ index: Int) -> Bool {
    index < scalars.count && ("0"..."9").contains(scalars[index])
  }
  if i < scalars.count, scalars[i] == "+" || scalars[i] == "-" { i += 1 }
  var digits = 0
  while isDigit(i) {
    i += 1
    digits += 1
  }
  if i < scalars.count, scalars[i] == ".", isDigit(i + 1) {
    i += 1
    while isDigit(i) {
      i += 1
      digits += 1
    }
  }
  guard digits > 0 else { return nil }
  if i < scalars.count, scalars[i] == "e" || scalars[i] == "E" {
    var j = i + 1
    if j < scalars.count, scalars[j] == "+" || scalars[j] == "-" { j += 1 }
    if isDigit(j) {
      while isDigit(j) { j += 1 }
      i = j
    }
  }
  let numberText = String(String.UnicodeScalarView(scalars[..<i]))
  let unit = String(String.UnicodeScalarView(scalars[i...]))
  guard
    unit.unicodeScalars.allSatisfy({
      $0 == "%" || ("a"..."z").contains($0) || ("A"..."Z").contains($0)
    }),
    let number = Double(numberText)
  else { return nil }
  return (number, unit)
}

private func formatCSSNumber(_ number: Double) -> String {
  number == 0 ? "0" : jsNumberToString(number)
}

/// `calc(n * length)` and `calc(length * n)` as their product.
private func evaluateCalcProduct(_ arguments: String) -> String? {
  let parts = arguments.split(whereSeparator: { $0 == " " || $0 == "\t" || $0 == "\n" })
  guard parts.count == 3, parts[1] == "*",
    let (a, unitA) = parseCSSNumber(String(parts[0])),
    let (b, unitB) = parseCSSNumber(String(parts[2])),
    unitA.isEmpty || unitB.isEmpty
  else { return nil }
  return formatCSSNumber(a * b) + (unitA + unitB).lowercased()
}

private struct RGBA {
  var r: Double
  var g: Double
  var b: Double
  var a: Double
}

private func parseHexColor(_ hex: Substring) -> RGBA? {
  let digits = hex.compactMap { $0.hexDigitValue }
  guard digits.count == hex.count else { return nil }
  let channels: [Int]
  switch digits.count {
  case 3, 4: channels = digits.map { $0 * 17 }
  case 6, 8:
    channels = stride(from: 0, to: digits.count, by: 2).map { digits[$0] * 16 + digits[$0 + 1] }
  default: return nil
  }
  return RGBA(
    r: Double(channels[0]), g: Double(channels[1]), b: Double(channels[2]),
    a: channels.count == 4 ? Double(channels[3]) / 255 : 1)
}

/// `rgb()`, `rgba()`, `hsl()` and `hsla()`, with commas or spaces and a
/// `/` alpha.
private func parseColorFunction(_ function: String, _ arguments: String) -> RGBA? {
  guard ["rgb", "rgba", "hsl", "hsla"].contains(function) else { return nil }
  var parts = arguments.split(whereSeparator: { $0 == "," || $0 == " " || $0 == "/" || $0 == "\t" })
    .map(String.init)
  guard parts.count == 3 || parts.count == 4 else { return nil }
  var alpha = 1.0
  if parts.count == 4 {
    let last = parts.removeLast()
    guard let (value, unit) = parseCSSNumber(last), unit.isEmpty || unit == "%" else { return nil }
    alpha = unit == "%" ? value / 100 : value
  }
  let values = parts.compactMap(parseCSSNumber)
  guard values.count == 3 else { return nil }
  let clampAlpha = min(1, max(0, alpha))
  if function.hasPrefix("rgb") {
    guard values.allSatisfy({ $0.1.isEmpty }) else { return nil }
    let channel = { (v: Double) in min(255, max(0, v.rounded())) }
    return RGBA(
      r: channel(values[0].0), g: channel(values[1].0), b: channel(values[2].0), a: clampAlpha)
  }
  let h =
    (values[0].0.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360)
    / 360
  let s = min(1, max(0, values[1].0 / 100))
  let l = min(1, max(0, values[2].0 / 100))
  let q = l < 0.5 ? l * (1 + s) : l + s - l * s
  let p = 2 * l - q
  func hue(_ t: Double) -> Double {
    var t = t
    if t < 0 { t += 1 }
    if t > 1 { t -= 1 }
    if t < 1 / 6 { return p + (q - p) * 6 * t }
    if t < 1 / 2 { return q }
    if t < 2 / 3 { return p + (q - p) * (2 / 3 - t) * 6 }
    return p
  }
  return RGBA(
    r: (hue(h + 1 / 3) * 255).rounded(), g: (hue(h) * 255).rounded(),
    b: (hue(h - 1 / 3) * 255).rounded(), a: clampAlpha)
}

private func serializeColor(_ color: RGBA, hexAlpha: Bool) -> String {
  let rgb = "\(Int(color.r)), \(Int(color.g)), \(Int(color.b))"
  if color.a >= 1 { return "rgb(\(rgb))" }
  var alpha = (color.a * 1000).rounded() / 1000
  if hexAlpha {
    // A byte alpha prints with two decimals when they round-trip to it.
    let byte = (color.a * 255).rounded()
    let twoDecimals = (color.a * 100).rounded() / 100
    if (twoDecimals * 255).rounded() == byte { alpha = twoDecimals }
  }
  return "rgba(\(rgb), \(formatCSSNumber(alpha)))"
}
