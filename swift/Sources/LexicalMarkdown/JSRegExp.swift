import Foundation
import Lexical

// JS regular expressions for the markdown port. Upstream's transformers are
// written as JS `RegExp`s, and their sources and flags are public API (custom
// transformers pass their own, and import rebuilds them with `new RegExp(
// regExp.source, flags)`). Swift `Regex` has no lookbehind, and its anchors
// and word boundaries see only the searched substring, so these run on ICU
// through `NSRegularExpression`, which matches over UTF-16 as JS does, with
// transparent bounds for `lastIndex`. The JS source is translated to ICU
// syntax first (see `translateJSRegExpSource`).

/// The characters JS `\s` matches.
private let JS_SPACE =
  #"\t\n\x{0B}\f\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}"#
private let JS_WORD = "A-Za-z0-9_"
private let WORD_BOUNDARY =
  "(?:(?<=[\(JS_WORD)])(?![\(JS_WORD)])|(?<![\(JS_WORD)])(?=[\(JS_WORD)]))"
private let NOT_WORD_BOUNDARY =
  "(?:(?<=[\(JS_WORD)])(?=[\(JS_WORD)])|(?<![\(JS_WORD)])(?![\(JS_WORD)]))"

/// `/source/flags`, as a JS `RegExp`.
public final class JSRegExp: @unchecked Sendable, CustomStringConvertible {
  /// `regExp.source`.
  public let source: String
  /// `regExp.flags`.
  public let flags: String
  /// `regExp.lastIndex`, in UTF-16 code units; used by `exec` and `test`
  /// when the regex is global or sticky.
  public var lastIndex = 0
  let regex: NSRegularExpression
  /// The name of each capture group, by group number minus one.
  let groupNames: [String?]

  /// `new RegExp(source, flags)`. Throws on a source ICU cannot compile.
  public init(validating source: String, _ flags: String = "") throws {
    self.source = source
    self.flags = String(flags.sorted())
    let (pattern, names) = translateJSRegExpSource(source, flags: flags)
    var options: NSRegularExpression.Options = []
    if flags.contains("i") { options.insert(.caseInsensitive) }
    regex = try NSRegularExpression(pattern: pattern, options: options)
    groupNames = names
  }

  /// `/source/flags`, for a source known to be valid.
  public convenience init(_ source: String, _ flags: String = "") {
    do {
      try self.init(validating: source, flags)
    } catch {
      preconditionFailure("Invalid regular expression /\(source)/\(flags): \(error)")
    }
  }

  public var global: Bool { flags.contains("g") }
  public var sticky: Bool { flags.contains("y") }
  public var description: String { "/\(source)/\(flags)" }

  /// `regExp.exec(string)`.
  public func exec(_ string: String) -> JSRegExpMatch? {
    let usesLastIndex = global || sticky
    let start = usesLastIndex ? lastIndex : 0
    guard let match = firstMatch(string, from: start) else {
      if usesLastIndex { lastIndex = 0 }
      return nil
    }
    if usesLastIndex { lastIndex = match.index + match.length }
    return match
  }

  /// `regExp.test(string)`.
  public func test(_ string: String) -> Bool { exec(string) != nil }

  /// The first match at or after the UTF-16 offset `start`, with JS
  /// semantics for anchors and lookbehind (the text before `start` is
  /// visible to them), and only at `start` for a sticky regex.
  func firstMatch(_ string: String, from start: Int) -> JSRegExpMatch? {
    let length = string.utf16.count
    guard start >= 0, start <= length else { return nil }
    var options: NSRegularExpression.MatchingOptions = [
      .withTransparentBounds, .withoutAnchoringBounds,
    ]
    if sticky { options.insert(.anchored) }
    guard
      let result = regex.firstMatch(
        in: string, options: options, range: NSRange(location: start, length: length - start))
    else { return nil }
    return JSRegExpMatch(result, string, groupNames)
  }
}

/// A `RegExpExecArray`: `[0]` the match, then the groups (`nil` for one that
/// did not participate, JS `undefined`), with `index` in UTF-16 code units.
public struct JSRegExpMatch: CustomStringConvertible {
  public let groups: [String?]
  /// The UTF-16 range of the match and of each group.
  public let ranges: [Range<Int>?]
  /// `match.groups`, by name.
  public let namedGroups: [String: String?]
  /// `match.index`.
  public let index: Int
  /// `match.input`.
  public let input: String

  init(_ result: NSTextCheckingResult, _ input: String, _ names: [String?]) {
    let ns = input as NSString
    var groups: [String?] = []
    var ranges: [Range<Int>?] = []
    for i in 0..<result.numberOfRanges {
      let range = result.range(at: i)
      if range.location == NSNotFound {
        groups.append(nil)
        ranges.append(nil)
      } else {
        groups.append(ns.substring(with: range))
        ranges.append(range.location..<(range.location + range.length))
      }
    }
    var named: [String: String?] = [:]
    for (i, name) in names.enumerated() {
      if let name, i + 1 < groups.count { named[name] = groups[i + 1] }
    }
    self.groups = groups
    self.ranges = ranges
    self.namedGroups = named
    self.index = result.range.location
    self.input = input
  }

  /// A match built from parts (for `handleImportAfterStartMatch` and tests).
  public init(groups: [String?], index: Int, input: String) {
    self.groups = groups
    self.ranges = groups.map { _ in nil }
    self.namedGroups = [:]
    self.index = index
    self.input = input
  }

  private init(
    groups: [String?], ranges: [Range<Int>?], namedGroups: [String: String?], index: Int,
    input: String
  ) {
    self.groups = groups
    self.ranges = ranges
    self.namedGroups = namedGroups
    self.index = index
    self.input = input
  }

  /// A copy of this match for `input`, which is what is left of the text
  /// it was found in after `distance` code units.
  public func moved(_ input: String, _ distance: Int) -> JSRegExpMatch {
    JSRegExpMatch(
      groups: groups,
      ranges: ranges.map { $0.map { ($0.lowerBound - distance)..<($0.upperBound - distance) } },
      namedGroups: namedGroups, index: index - distance, input: input)
  }

  /// `match[i]`, `nil` past the end, as JS reads `undefined`.
  public subscript(_ i: Int) -> String? { i >= 0 && i < groups.count ? groups[i] : nil }
  /// `match.length`.
  public var count: Int { groups.count }
  /// The matched text, `match[0]`.
  public var text: String { groups[0] ?? "" }
  /// The UTF-16 length of `match[0]`.
  public var length: Int { text.utf16.count }
  public var description: String { "\(groups) at \(index)" }
}

extension String {
  /// `string.match(regExp)` for a non-global regex: `regExp.exec(string)`
  /// from the start.
  public func jsMatch(_ regExp: JSRegExp) -> JSRegExpMatch? {
    regExp.firstMatch(self, from: 0)
  }

  /// `string.match(regExp)` for a global regex: every match's text, or
  /// `nil` when there is none.
  public func jsMatchAllTexts(_ regExp: JSRegExp) -> [String]? {
    let all = jsMatchAll(regExp).map(\.text)
    return all.isEmpty ? nil : all
  }

  /// `string.matchAll(regExp)`: every match, advancing past empty ones.
  public func jsMatchAll(_ regExp: JSRegExp) -> [JSRegExpMatch] {
    var out: [JSRegExpMatch] = []
    var start = 0
    let length = utf16.count
    while start <= length, let match = regExp.firstMatch(self, from: start) {
      out.append(match)
      let end = match.index + match.length
      start = end == match.index ? advanceStringIndex(end, regExp) : end
    }
    return out
  }

  /// `string.search(regExp)`.
  public func jsSearch(_ regExp: JSRegExp) -> Int {
    regExp.firstMatch(self, from: 0)?.index ?? -1
  }

  /// `string.replace(regExp, replacer)`: every match when the regex is
  /// global, else the first.
  public func jsReplace(
    _ regExp: JSRegExp, _ replacer: (JSRegExpMatch) throws -> String
  )
    rethrows -> String
  {
    let matches = regExp.global ? jsMatchAll(regExp) : jsMatch(regExp).map { [$0] } ?? []
    if matches.isEmpty { return self }
    let units = Array(utf16)
    var out: [UInt16] = []
    var last = 0
    for match in matches {
      out += units[last..<match.index]
      out += try replacer(match).utf16
      last = match.index + match.length
    }
    out += units[last...]
    return String(decoding: out, as: UTF16.self)
  }

  /// `string.replace(regExp, template)`, with `$&`, `$1`…`$99`, `$<name>`
  /// and `$$`.
  public func jsReplace(_ regExp: JSRegExp, template: String) -> String {
    jsReplace(regExp) { expandReplacementTemplate(template, $0) }
  }

  /// `string.split(separator)` over UTF-16, so that `"\r\n"` (one Swift
  /// Character) still splits on `"\n"`.
  public func jsSplit(_ separator: String) -> [String] {
    let units = Array(utf16)
    let sep = Array(separator.utf16)
    if sep.isEmpty { return units.map { String(decoding: [$0], as: UTF16.self) } }
    var out: [String] = []
    var start = 0
    var i = 0
    while i + sep.count <= units.count {
      if units[i..<(i + sep.count)].elementsEqual(sep) {
        out.append(String(decoding: units[start..<i], as: UTF16.self))
        i += sep.count
        start = i
      } else {
        i += 1
      }
    }
    out.append(String(decoding: units[start...], as: UTF16.self))
    return out
  }

  /// `string.indexOf(search, from)` over UTF-16.
  public func jsIndexOf(_ search: String, _ from: Int = 0) -> Int {
    let units = Array(utf16)
    let needle = Array(search.utf16)
    var i = Swift.max(0, from)
    while i + needle.count <= units.count {
      if units[i..<(i + needle.count)].elementsEqual(needle) { return i }
      i += 1
    }
    return -1
  }

  /// `string.lastIndexOf(search, from)` over UTF-16.
  public func jsLastIndexOf(_ search: String, _ from: Int? = nil) -> Int {
    let units = Array(utf16)
    let needle = Array(search.utf16)
    var i = Swift.min(from ?? units.count, units.count - needle.count)
    while i >= 0 {
      if units[i..<(i + needle.count)].elementsEqual(needle) { return i }
      i -= 1
    }
    return -1
  }

  /// `text[index]`: the UTF-16 code unit at `index` as a string, or `nil`
  /// (JS `undefined`).
  public func jsCharAt(_ index: Int) -> String? {
    utf16Unit(at: index).map { String(decoding: [$0], as: UTF16.self) }
  }

  /// `string.length`.
  public var jsLength: Int { utf16.count }

  /// `string.slice(start, end)`.
  public func jsSubstring(_ start: Int, _ end: Int? = nil) -> String { jsSlice(start, end) }

  /// `string.startsWith(search)` and `endsWith`, comparing code units.
  public func jsStartsWith(_ search: String) -> Bool { utf16.starts(with: search.utf16) }
  public func jsEndsWith(_ search: String) -> Bool {
    utf16.reversed().starts(with: search.utf16.reversed())
  }

  /// `string.codePointAt(index)`.
  public func jsCodePointAt(_ index: Int) -> UInt32? {
    guard let unit = utf16Unit(at: index) else { return nil }
    if UTF16.isLeadSurrogate(unit), let next = utf16Unit(at: index + 1),
      UTF16.isTrailSurrogate(next)
    {
      return 0x10000 + ((UInt32(unit) - 0xD800) << 10) + (UInt32(next) - 0xDC00)
    }
    return UInt32(unit)
  }

  /// The index after `index` when skipping an empty match: one code unit,
  /// or one code point for a `u` regex.
  fileprivate func advanceStringIndex(_ index: Int, _ regExp: JSRegExp) -> Int {
    if regExp.flags.contains("u") || regExp.flags.contains("v"),
      let unit = utf16Unit(at: index), UTF16.isLeadSurrogate(unit),
      let next = utf16Unit(at: index + 1), UTF16.isTrailSurrogate(next)
    {
      return index + 2
    }
    return index + 1
  }
}

/// `GetSubstitution`: expands `$&`, `$n`, `$nn`, `$<name>` and `$$`.
private func expandReplacementTemplate(_ template: String, _ match: JSRegExpMatch) -> String {
  let t = Array(template.utf16)
  var out: [UInt16] = []
  let dollar = UInt16(UInt8(ascii: "$"))
  var i = 0
  func isDigit(_ u: UInt16) -> Bool { u >= 0x30 && u <= 0x39 }
  while i < t.count {
    let c = t[i]
    guard c == dollar, i + 1 < t.count else {
      out.append(c)
      i += 1
      continue
    }
    let n = t[i + 1]
    if n == dollar {
      out.append(dollar)
      i += 2
    } else if n == UInt16(UInt8(ascii: "&")) {
      out += match.text.utf16
      i += 2
    } else if n == UInt16(UInt8(ascii: "`")) {
      out += match.input.jsSlice(0, match.index).utf16
      i += 2
    } else if n == UInt16(UInt8(ascii: "'")) {
      out += match.input.jsSlice(match.index + match.length).utf16
      i += 2
    } else if isDigit(n) {
      let groupCount = match.count - 1
      var digits = Int(n - 0x30)
      var used = 2
      if i + 2 < t.count, isDigit(t[i + 2]) {
        let two = digits * 10 + Int(t[i + 2] - 0x30)
        if two >= 1 && two <= groupCount {
          digits = two
          used = 3
        }
      }
      if digits >= 1 && digits <= groupCount {
        out += (match[digits] ?? "").utf16
        i += used
      } else {
        out.append(c)
        i += 1
      }
    } else if n == UInt16(UInt8(ascii: "<")), !match.namedGroups.isEmpty,
      let close = t[(i + 2)...].firstIndex(of: UInt16(UInt8(ascii: ">")))
    {
      let name = String(decoding: t[(i + 2)..<close], as: UTF16.self)
      out += ((match.namedGroups[name] ?? nil) ?? "").utf16
      i = close + 1
    } else {
      out.append(c)
      i += 1
    }
  }
  return String(decoding: out, as: UTF16.self)
}

/// Translates a JS regex source to ICU syntax with the same meaning: `\d`,
/// `\w`, `\s` and `\b` keep their ASCII (or JS whitespace) meaning, `$`
/// without `m` is the end of input only, `.` excludes only JS's line
/// terminators, a `[` or `&` inside a class is literal, and a `{` that is
/// not a quantifier is literal. Returns the pattern and each capture
/// group's name.
func translateJSRegExpSource(_ source: String, flags: String) -> (String, [String?]) {
  let s = Array(source.unicodeScalars)
  let dotAll = flags.contains("s")
  let multiline = flags.contains("m")
  var out = ""
  var names: [String?] = []
  var inClass = false
  var i = 0
  func at(_ j: Int) -> Unicode.Scalar? { j < s.count ? s[j] : nil }
  /// Whether `{` at `j` starts a `{n}`, `{n,}` or `{n,m}` quantifier;
  /// returns the index of its `}`.
  func quantifierEnd(_ j: Int) -> Int? {
    var k = j + 1
    var sawDigit = false
    while let c = at(k), ("0"..."9").contains(c) {
      sawDigit = true
      k += 1
    }
    guard sawDigit else { return nil }
    if at(k) == "," {
      k += 1
      while let c = at(k), ("0"..."9").contains(c) { k += 1 }
    }
    return at(k) == "}" ? k : nil
  }
  while i < s.count {
    let c = s[i]
    if c == "\\", let n = at(i + 1) {
      switch n {
      case "d": out += inClass ? "0-9" : "[0-9]"
      case "D": out += "[^0-9]"
      case "w": out += inClass ? JS_WORD : "[\(JS_WORD)]"
      case "W": out += "[^\(JS_WORD)]"
      case "s": out += inClass ? JS_SPACE : "[\(JS_SPACE)]"
      case "S": out += "[^\(JS_SPACE)]"
      case "b": out += inClass ? "\\x{08}" : WORD_BOUNDARY
      case "B": out += inClass ? "B" : NOT_WORD_BOUNDARY
      case "/": out += "/"
      case "0" where !(at(i + 2).map { ("0"..."9").contains($0) } ?? false):
        // `\0` not followed by a digit is NUL; ICU reads `\0` as octal.
        out += "\\x{0}"
      case "u" where at(i + 2) == "{":
        // `\u{1F600}` (u flag) is ICU's `\x{1F600}`.
        var k = i + 3
        var hex = ""
        while let h = at(k), h != "}" {
          hex.unicodeScalars.append(h)
          k += 1
        }
        out += "\\x{\(hex)}"
        i = k + 1
        continue
      default:
        out += "\\"
        out.unicodeScalars.append(n)
      }
      i += 2
      continue
    }
    if inClass {
      switch c {
      case "]":
        inClass = false
        out += "]"
      case "[": out += "\\["
      case "&": out += "\\&"
      case "-" where at(i + 1) == "-":
        out += "-\\-"
        i += 2
        continue
      default: out.unicodeScalars.append(c)
      }
      i += 1
      continue
    }
    switch c {
    case "[":
      if at(i + 1) == "]" {
        // `[]` matches nothing.
        out += "(?!)"
        i += 2
        continue
      }
      if at(i + 1) == "^", at(i + 2) == "]" {
        // `[^]` matches anything.
        out += "[\\x{0}-\\x{10FFFF}]"
        i += 3
        continue
      }
      inClass = true
      out += "["
      if at(i + 1) == "^" {
        out += "^"
        i += 1
      }
    // A `]` first in a class is literal in ICU but ends an empty class in
    // JS; JS has no such literal, so nothing to do.
    case "(":
      if at(i + 1) == "?" {
        if at(i + 2) == "<", let n = at(i + 3), n != "=", n != "!" {
          var k = i + 3
          var name = ""
          while let ch = at(k), ch != ">" {
            name.unicodeScalars.append(ch)
            k += 1
          }
          names.append(name)
        }
      } else {
        names.append(nil)
      }
      out += "("
    case "$": out += multiline ? "(?=[\\n\\r\\x{2028}\\x{2029}]|\\z)" : "\\z"
    case "^": out += multiline ? "(?:(?<=[\\n\\r\\x{2028}\\x{2029}])|\\A)" : "\\A"
    case ".": out += dotAll ? "[\\x{0}-\\x{10FFFF}]" : "[^\\n\\r\\x{2028}\\x{2029}]"
    case "{":
      if let end = quantifierEnd(i) {
        out += String(String.UnicodeScalarView(s[i...end]))
        i = end + 1
        continue
      }
      out += "\\{"
    case "}": out += "\\}"
    default: out.unicodeScalars.append(c)
    }
    i += 1
  }
  return (out, names)
}
