// The `font` shorthand as the declaration of the upstream tests (jsdom)
// expands it: `properties/font.js`' `parse`, which the declaration's
// `cssText` setter and `setProperty("font", ...)` both run, with the
// longhand parsers it calls (`fontStyle`, `fontVariant`, `fontWeight`,
// `fontSize`, `lineHeight` and `fontFamily`'s `parse`). Pinned by the
// `css/declarations` vectors, quirks included: a numeric weight before the
// size is also read as the line height, and a `font-variant` keyword other
// than `normal` (`small-caps`) makes the value invalid.
//
// jsdom first checks the whole value against the `font` grammar with
// css-tree's lexer (`isValidPropertyValue`); `isValidFont` is that grammar
// written out for the values `parse` reads.

/// The longhands of `font`, in the order its setter writes them
/// (`shorthandFor`).
let fontLonghands = [
  "font-style", "font-variant", "font-weight", "font-size", "line-height", "font-family",
]

private let styleKeywords: Set<String> = ["normal", "italic", "oblique"]
private let weightKeywords: Set<String> = ["normal", "bold", "bolder", "lighter"]
private let stretchKeywords: Set<String> = [
  "normal", "ultra-condensed", "extra-condensed", "condensed", "semi-condensed", "semi-expanded",
  "expanded", "extra-expanded", "ultra-expanded",
]
private let sizeKeywords: Set<String> = [
  "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "xxx-large", "larger",
  "smaller", "math",
]
/// The keywords `font-variant` takes, which `font` refuses but for
/// `normal`.
private let variantKeywords: Set<String> = [
  "normal", "none", "small-caps", "all-small-caps", "petite-caps", "all-petite-caps", "unicase",
  "titling-caps", "common-ligatures", "no-common-ligatures", "discretionary-ligatures",
  "no-discretionary-ligatures", "historical-ligatures", "no-historical-ligatures", "contextual",
  "no-contextual", "historical-forms", "lining-nums", "oldstyle-nums", "proportional-nums",
  "tabular-nums", "diagonal-fractions", "stacked-fractions", "ordinal", "slashed-zero", "jis78",
  "jis83", "jis90", "jis04", "simplified", "traditional", "full-width", "proportional-width",
  "ruby", "sub", "super", "text", "emoji", "unicode",
]
private let systemFonts: Set<String> = [
  "caption", "icon", "menu", "message-box", "small-caption", "status-bar",
]
private let genericFamilies: Set<String> = [
  "serif", "sans-serif", "cursive", "fantasy", "monospace", "system-ui", "emoji", "math",
  "fangsong", "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded",
]
private let lengthUnits: Set<String> = [
  "px", "em", "rem", "ex", "rex", "ch", "rch", "cap", "rcap", "ic", "ric", "lh", "rlh", "vw", "vh",
  "vi", "vb", "vmin", "vmax", "svw", "svh", "svi", "svb", "svmin", "svmax", "lvw", "lvh", "lvi",
  "lvb", "lvmin", "lvmax", "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax", "cqw", "cqh", "cqi",
  "cqb", "cqmin", "cqmax", "cm", "mm", "q", "in", "pt", "pc",
]
private let angleUnits: Set<String> = ["deg", "grad", "rad", "turn"]

private func isKeyword(_ word: String, _ keywords: Set<String>) -> Bool {
  keywords.contains(asciiLowercased(word))
}

/// `hasVarFunc`.
func hasVarFunc(_ value: String) -> Bool { asciiLowercased(value).contains("var(") }

private func isMathFunction(_ word: String) -> Bool {
  let lower = asciiLowercased(word)
  return ["calc(", "min(", "max(", "clamp("].contains { lower.hasPrefix($0) } && word.hasSuffix(")")
}

/// A length or percentage that isn't negative, or a unitless `0`.
private func isLengthPercentage(_ word: String) -> Bool {
  guard let (n, unit) = parseCSSNumber(word), n >= 0 else { return false }
  return (unit.isEmpty && n == 0) || unit == "%" || isKeyword(unit, lengthUnits)
}

private func unitlessNumber(_ word: String) -> Double? {
  guard let (n, unit) = parseCSSNumber(word), unit.isEmpty else { return nil }
  return n
}

/// `fontStyle.parse(part)`.
private func parseStyle(_ part: String) -> String? {
  isKeyword(part, styleKeywords) ? asciiLowercased(part) : nil
}

/// `fontVariant.parse(part)`, as far as `font` reads it.
private func parseVariant(_ part: String) -> String? {
  isKeyword(part, variantKeywords) ? asciiLowercased(part) : nil
}

/// `fontWeight.parse(part)`: a keyword or a number from 1 to 1000.
private func parseWeight(_ part: String) -> String? {
  if isKeyword(part, weightKeywords) { return asciiLowercased(part) }
  guard let n = unitlessNumber(part), (1...1000).contains(n) else { return nil }
  return normalizeCSSValue("font-weight", part)
}

/// `fontSize.parse(part)`.
private func parseSize(_ part: String) -> String? {
  guard isKeyword(part, sizeKeywords) || isLengthPercentage(part) || isMathFunction(part) else {
    return nil
  }
  return normalizeCSSValue("font-size", part)
}

/// `lineHeight.parse(part)`.
private func parseLineHeight(_ part: String) -> String? {
  let valid =
    asciiLowercased(part) == "normal" || (unitlessNumber(part).map { $0 >= 0 } ?? false)
    || isLengthPercentage(part) || isMathFunction(part)
  return valid ? normalizeCSSValue("line-height", part) : nil
}

/// A CSS string token, whole: its contents.
private func stringBody(_ word: String) -> String? {
  guard let quote = word.first, quote == "\"" || quote == "'", word.count >= 2,
    word.last == quote
  else { return nil }
  let body = word.dropFirst().dropLast()
  return body.contains(quote) ? nil : String(body)
}

private func isIdent(_ word: String) -> Bool {
  var scalars = word.unicodeScalars.makeIterator()
  guard var first = scalars.next() else { return false }
  if first == "-" {
    guard let second = scalars.next() else { return false }
    if second == "-" { return true }
    first = second
  }
  func isASCIIAlpha(_ c: Unicode.Scalar) -> Bool {
    ("a"..."z").contains(c) || ("A"..."Z").contains(c)
  }
  guard isASCIIAlpha(first) || first == "_" || !first.isASCII else { return false }
  while let c = scalars.next() {
    guard isASCIIAlpha(c) || ("0"..."9").contains(c) || c == "-" || c == "_" || !c.isASCII else {
      return false
    }
  }
  return true
}

/// Whether `words` are one entry of a `font-family` list: a string, a
/// generic family, or a name of identifiers that aren't CSS-wide keywords,
/// `default` or (in a name of several) generic families.
private func isFamilyEntry(_ words: [String]) -> Bool {
  if words.isEmpty { return false }
  if words.count == 1, stringBody(words[0]) != nil || isKeyword(words[0], genericFamilies) {
    return true
  }
  return words.allSatisfy {
    isIdent($0) && !isKeyword($0, cssWideKeywords) && asciiLowercased($0) != "default"
      && !(words.count > 1 && isKeyword($0, genericFamilies))
  }
}

/// `fontFamily.parse(v)`: each entry of the list, a string or a name of
/// several identifiers quoted, joined with `, `; an empty entry makes it
/// empty.
private func parseFamily(_ value: String) -> String? {
  func quote(_ text: String) -> String {
    var out = "\""
    for c in text.unicodeScalars where c != "\\" {
      if c == "\"" { out += "\\" }
      out.unicodeScalars.append(c)
    }
    return out + "\""
  }
  var out: [String] = []
  for entry in splitTopLevel(value, ",") {
    if entry.isEmpty { return "" }
    let words = splitWords(entry)
    if words.count == 1 {
      if let body = stringBody(words[0]) {
        out.append(quote(body))
      } else if isIdent(words[0]) {
        out.append(words[0])
      } else {
        return nil
      }
    } else if words.allSatisfy(isIdent) {
      out.append(quote(words.joined(separator: " ")))
    } else {
      return nil
    }
  }
  return out.isEmpty ? nil : out.joined(separator: ", ")
}

private func isCSSSpace(_ c: Unicode.Scalar) -> Bool {
  c == " " || c == "\t" || c == "\n" || c == "\u{0C}" || c == "\r"
}

/// `value` without its leading and trailing white space.
private func trimCSS(_ value: String) -> String {
  let scalars = value.unicodeScalars
  guard let start = scalars.firstIndex(where: { !$0.properties.isWhitespace }) else { return "" }
  let end = scalars.lastIndex(where: { !$0.properties.isWhitespace })!
  return String(scalars[start...end])
}

/// Splits at `delimiter` outside strings and parentheses, trimming each
/// part (`splitValue` with a delimiter).
private func splitTopLevel(_ value: String, _ delimiter: Unicode.Scalar) -> [String] {
  var parts: [String] = []
  var current = String.UnicodeScalarView()
  var depth = 0
  var quote: Unicode.Scalar?
  func trimmed() -> String { trimCSS(String(current)) }
  for c in value.unicodeScalars {
    if let q = quote {
      if c == q { quote = nil }
      current.append(c)
      continue
    }
    switch c {
    case "\"", "'":
      quote = c
      current.append(c)
    case "(":
      depth += 1
      current.append(c)
    case ")":
      depth = max(depth - 1, 0)
      current.append(c)
    case delimiter where depth == 0:
      parts.append(trimmed())
      current = String.UnicodeScalarView()
    default: current.append(c)
    }
  }
  parts.append(trimmed())
  return parts
}

/// Splits at whitespace outside strings and parentheses (`splitValue`).
private func splitWords(_ value: String) -> [String] {
  var words: [String] = []
  var current = String.UnicodeScalarView()
  var depth = 0
  var quote: Unicode.Scalar?
  for c in value.unicodeScalars {
    if let q = quote {
      if c == q { quote = nil }
      current.append(c)
      continue
    }
    switch c {
    case "\"", "'":
      quote = c
      current.append(c)
    case "(":
      depth += 1
      current.append(c)
    case ")":
      depth = max(depth - 1, 0)
      current.append(c)
    case let c where isCSSSpace(c) && depth == 0:
      if !current.isEmpty {
        words.append(String(current))
        current = String.UnicodeScalarView()
      }
    default: current.append(c)
    }
  }
  if !current.isEmpty { words.append(String(current)) }
  return words
}

/// Whether `value` matches the `font` grammar (css-tree's
/// `isValidPropertyValue("font", value)`):
/// `[ <font-style> || <font-variant-css2> || <font-weight> ||
/// <font-stretch-css3> ]? <font-size> [ / <line-height> ]? <font-family>`,
/// a system font, or a CSS-wide keyword.
private func isValidFont(_ value: String) -> Bool {
  let chunks = splitTopLevel(value, ",")
  let first = chunks[0]
  let families = chunks.dropFirst()
  if families.isEmpty {
    let words = splitWords(first)
    if words.count == 1, isKeyword(words[0], cssWideKeywords) || isKeyword(words[0], systemFonts) {
      return true
    }
  }
  if !families.allSatisfy({ isFamilyEntry(splitWords($0)) }) { return false }
  let halves = splitTopLevel(first, "/")
  let beforeSize: [String]
  let size: String
  let family: [String]
  switch halves.count {
  case 1:
    let words = splitWords(halves[0])
    guard let at = words.firstIndex(where: { parseSize($0) != nil }) else { return false }
    (beforeSize, size, family) = (Array(words[..<at]), words[at], Array(words[(at + 1)...]))
  case 2:
    let words = splitWords(halves[0])
    guard let last = words.last else { return false }
    let rest = splitWords(halves[1])
    guard let lineHeight = rest.first, parseLineHeight(lineHeight) != nil else { return false }
    (beforeSize, size, family) = (Array(words.dropLast()), last, Array(rest.dropFirst()))
  default: return false
  }
  if parseSize(size) == nil || !isFamilyEntry(family) { return false }
  // Each of style, variant, weight and stretch at most once, `normal`
  // standing for any of them.
  var (style, variant, weight, stretch, normals) = (false, false, false, false, 0)
  func take(_ seen: inout Bool) -> Bool {
    defer { seen = true }
    return !seen
  }
  var i = 0
  while i < beforeSize.count {
    let word = beforeSize[i]
    i += 1
    let ok: Bool
    if asciiLowercased(word) == "normal" {
      normals += 1
      ok = true
    } else if isKeyword(word, styleKeywords) {
      if asciiLowercased(word) == "oblique", i < beforeSize.count,
        let (_, unit) = parseCSSNumber(beforeSize[i]), isKeyword(unit, angleUnits)
      {
        i += 1
      }
      ok = take(&style)
    } else if asciiLowercased(word) == "small-caps" {
      ok = take(&variant)
    } else if parseWeight(word) != nil {
      ok = take(&weight)
    } else if isKeyword(word, stretchKeywords) {
      ok = take(&stretch)
    } else {
      ok = false
    }
    if !ok { return false }
  }
  return normals + [style, variant, weight, stretch].filter { $0 }.count <= 4
}

/// `font.parse(value)`: the longhands `value` sets, in the order JS's
/// result object holds them, or `nil` when it is invalid. `value` is
/// trimmed and not empty.
func parseFont(_ value: String) -> [(String, String)]? {
  let value = resolveCalc(value)
  guard isValidFont(value) else { return nil }
  let chunks = splitTopLevel(value, ",")
  let fontBlock = chunks[0]
  let families = chunks.dropFirst()
  let halves = splitTopLevel(fontBlock, "/")
  var font: [(String, String)] = [
    ("font-style", "normal"), ("font-variant", "normal"), ("font-weight", "normal"),
  ]
  func get(_ key: String) -> String? { font.first { $0.0 == key }?.1 }
  func set(_ key: String, _ value: String) {
    if let i = font.firstIndex(where: { $0.0 == key }) {
      font[i].1 = value
    } else {
      font.append((key, value))
    }
  }
  // The variant `font` refuses: any other than `normal` (JS accepts only
  // `small-cap`, which no keyword is).
  func refusesVariant(_ part: String) -> Bool {
    get("font-variant") == "normal" && (parseVariant(part).map { $0 != "normal" } ?? false)
  }
  var fontFamilies: [String] = []
  if halves.count == 2 {
    // JS splits the part after the slash at single spaces.
    var rest = trimCSS(halves[1]).split(separator: " ", omittingEmptySubsequences: false).map(
      String.init)
    let lineB = rest.removeFirst()
    if lineB.isEmpty || rest.isEmpty { return nil }
    guard let lineHeightB = parseLineHeight(lineB),
      let family = parseFamily(rest.joined(separator: " "))
    else { return nil }
    fontFamilies.append(family)
    for part in splitWords(trimCSS(halves[0])) where part != "normal" {
      if let parsed = parseSize(part) { set("font-size", parsed) }
      if get("font-style") == "normal", let parsed = parseStyle(part) { set("font-style", parsed) }
      if refusesVariant(part) { return nil }
      if get("font-weight") == "normal", let parsed = parseWeight(part) {
        set("font-weight", parsed)
      }
    }
    if get("font-size") == nil { return nil }
    set("line-height", lineHeightB)
  } else {
    let revParts = Array(splitWords(trimCSS(fontBlock)).reversed())
    if revParts.count == 1 {
      guard isKeyword(revParts[0], cssWideKeywords) else { return nil }
      let keyword = asciiLowercased(revParts[0])
      return fontLonghands.map { ($0, keyword) }
    }
    set("line-height", "normal")
    var revFontFamily: [String] = []
    var fontSizeA: String?
    for part in revParts {
      if fontSizeA != nil {
        if asciiLowercased(part) == "normal" { continue }
        let parsers: [(String, (String) -> String?)] = [
          ("font-style", parseStyle), ("font-weight", parseWeight),
          ("line-height", parseLineHeight),
        ]
        for (key, parse) in parsers where get(key) == "normal" {
          if let parsed = parse(part) { set(key, parsed) }
        }
        if refusesVariant(part) { return nil }
      } else if let parsed = parseSize(part) {
        fontSizeA = parsed
      } else {
        guard let parsed = parseFamily(part) else { return nil }
        revFontFamily.append(parsed)
      }
    }
    guard let fontSizeA,
      let familyValue = parseFamily(revFontFamily.reversed().joined(separator: " ")),
      !familyValue.isEmpty,
      let family = parseFamily(familyValue)
    else { return nil }
    set("font-size", fontSizeA)
    fontFamilies.append(family)
  }
  for entry in families {
    guard let parsed = parseFamily(entry), !parsed.isEmpty else { return nil }
    if !fontFamilies.contains(parsed) { fontFamilies.append(parsed) }
  }
  set("font-family", fontFamilies.joined(separator: ", "))
  return font
}

/// `resolveCalc(value)`: each `calc()` that evaluates replaced with its
/// result, before the value is split.
private func resolveCalc(_ value: String) -> String {
  let scalars = Array(value.unicodeScalars)
  let lower = Array(asciiLowercased(value).unicodeScalars)
  let calc = Array("calc(".unicodeScalars)
  var out = String.UnicodeScalarView()
  var i = 0
  var copied = 0
  while i + calc.count <= scalars.count {
    guard Array(lower[i..<(i + calc.count)]) == calc else {
      i += 1
      continue
    }
    let open = i + calc.count
    var depth = 1
    var close: Int?
    var j = open
    while j < scalars.count {
      if scalars[j] == "(" { depth += 1 }
      if scalars[j] == ")" {
        depth -= 1
        if depth == 0 {
          close = j
          break
        }
      }
      j += 1
    }
    guard let close else { break }
    out.append(contentsOf: scalars[copied..<i])
    let arguments = String(String.UnicodeScalarView(scalars[open..<close]))
    if let result = evaluateCalc(arguments) {
      out.append(contentsOf: "calc(\(result))".unicodeScalars)
    } else {
      out.append(contentsOf: scalars[i...close])
    }
    i = close + 1
    copied = i
  }
  out.append(contentsOf: scalars[copied...])
  return String(out)
}

/// The value the `font` setter stores for the longhands it wrote: each one
/// that isn't `normal`, once, the line height after a `/`.
func serializeFont(_ longhands: [(String, String)]) -> String {
  var parts: [String] = []
  var seen: Set<String> = []
  for key in fontLonghands {
    guard let value = longhands.first(where: { $0.0 == key })?.1 else { continue }
    if value != "normal" && seen.insert(value).inserted {
      parts.append(key == "line-height" ? "/ \(value)" : value)
    }
  }
  return parts.joined(separator: " ")
}
