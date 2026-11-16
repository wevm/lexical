import Lexical

// Port of packages/lexical-react/src/LexicalTypeaheadMenuPluginUtils.ts and
// the platform-neutral types of shared/LexicalMenu.tsx.

/// `MenuTextMatch`: where a trigger matched the text before the caret. The
/// `leadOffset` (UTF-16) where the match starts, the query after the
/// trigger (`matchingString`), and the text, trigger included, that an
/// option replaces (`replaceableString`).
public struct MenuTextMatch: Equatable, Sendable {
  public var leadOffset: Int
  public var matchingString: String
  public var replaceableString: String

  public init(leadOffset: Int, matchingString: String, replaceableString: String) {
    self.leadOffset = leadOffset
    self.matchingString = matchingString
    self.replaceableString = replaceableString
  }
}

/// `TriggerFn`: whether the text before the caret opens a menu.
public typealias TriggerFn = (_ text: String, _ editor: Editor) -> MenuTextMatch?

/// `PUNCTUATION`: the characters (as a character class fragment) that end a
/// typeahead query.
public let PUNCTUATION = #"\.,\+\*\?\$\@\|#{}\(\)\^\-\[\]\\/!%'"~=<>_:;"#

/// `SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`: dispatched while a menu is
/// open to scroll the option at `index` into view. A view that draws the
/// menu handles it.
public let SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND = LexicalCommand<
  (index: Int, option: MenuOption)
>("SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND")

/// One item of a character class of a JS `RegExp` without the `u` flag: a
/// range of UTF-16 code units, or an escape class (`\s`, `\d`, `\w`, each
/// negated when `true`).
private enum ClassItem {
  case range(UInt16, UInt16)
  case whitespace(Bool)
  case digit(Bool)
  case word(Bool)
}

private func isWordUnit(_ unit: UInt16) -> Bool {
  switch unit {
  case 0x30...0x39, 0x41...0x5A, 0x61...0x7A, 0x5F: true
  default: false
  }
}

/// A character class, `[...]` or `[^...]`, over UTF-16 code units.
private struct CharClass {
  let negated: Bool
  private(set) var items: [ClassItem] = []

  /// The class whose body is `body` (what is between `[` or `[^` and
  /// `]`), as a JS `RegExp` without the `u` flag reads it: escapes,
  /// `\s`/`\d`/`\w` and their negations, and `a-b` ranges.
  init(_ body: String, negated: Bool) {
    self.negated = negated
    let units = Array(body.utf16)
    let backslash = UInt16(UInt8(ascii: "\\")), dash = UInt16(UInt8(ascii: "-"))
    func atom(_ i: inout Int) -> ClassItem {
      let unit = units[i]
      i += 1
      guard unit == backslash, i < units.count else { return .range(unit, unit) }
      let escaped = units[i]
      i += 1
      func hex(_ n: Int) -> UInt16? {
        guard i + n <= units.count,
          let value = UInt16(String(decoding: units[i..<i + n], as: UTF16.self), radix: 16)
        else { return nil }
        i += n
        return value
      }
      let value: UInt16
      switch Unicode.Scalar(escaped).map(Character.init) {
      case "s": return .whitespace(false)
      case "S": return .whitespace(true)
      case "d": return .digit(false)
      case "D": return .digit(true)
      case "w": return .word(false)
      case "W": return .word(true)
      case "n": value = 0x0A
      case "r": value = 0x0D
      case "t": value = 0x09
      case "v": value = 0x0B
      case "f": value = 0x0C
      case "b": value = 0x08
      case "0": value = 0
      case "x": value = hex(2) ?? escaped
      case "u": value = hex(4) ?? escaped
      // An identity escape.
      default: value = escaped
      }
      return .range(value, value)
    }
    var i = 0
    while i < units.count {
      let first = atom(&i)
      // `a-b`, unless the `-` ends the class or a side is a class.
      if case .range(let low, _) = first, i + 1 < units.count, units[i] == dash {
        var j = i + 1
        if case .range(let high, _) = atom(&j) {
          items.append(.range(low, high))
          i = j
          continue
        }
      }
      items.append(first)
    }
  }

  func matches(_ unit: UInt16) -> Bool {
    let found = items.contains { item in
      switch item {
      case .range(let low, let high): low <= unit && unit <= high
      case .whitespace(let not): isJSWhitespace(unit) != not
      case .digit(let not): (0x30...0x39).contains(unit) != not
      case .word(let not): isWordUnit(unit) != not
      }
    }
    return found != negated
  }
}

/// `useBasicTypeaheadTriggerMatch`: a trigger function for one trigger
/// character (`@`, `/`) after whitespace, an open parenthesis or the start
/// of the text, followed by `minLength` to `maxLength` characters that are
/// not punctuation (nor whitespace, unless `allowWhitespace`). JS's
/// `RegExp`, `(^|\s|\()([trigger]((?:[^trigger punctuation \s]){0,maxLength}))$`,
/// has no `u` flag, so it is matched here as JS matches it: over UTF-16
/// code units, with JS's `\s`, and the trigger and punctuation read as the
/// bodies of character classes.
public func basicTypeaheadTriggerMatch(
  _ trigger: String, minLength: Int = 1, maxLength: Int = 75,
  punctuation: String = PUNCTUATION, allowWhitespace: Bool = false
) -> TriggerFn {
  let validCharsSuffix = allowWhitespace ? "" : #"\s"#
  let triggerClass = CharClass(trigger, negated: false)
  let validChars = CharClass(trigger + punctuation + validCharsSuffix, negated: true)
  return { text, _ in
    let units = Array(text.utf16)
    // `match[3]`'s start for a trigger at `at`: the rest of the text, when
    // every unit of it is valid and there are at most `maxLength`.
    func queryAfter(_ at: Int) -> Int? {
      guard at < units.count, triggerClass.matches(units[at]) else { return nil }
      let rest = units[(at + 1)...]
      return rest.count <= maxLength && rest.allSatisfy(validChars.matches) ? at + 1 : nil
    }
    // `exec`: the first index a match starts at; at each, the alternatives
    // of group 1 in order: `^`, `\s`, `\(`.
    var found: (index: Int, lead: Int, query: Int)?
    for index in units.indices {
      if index == 0, let query = queryAfter(0) {
        found = (index, 0, query)
        break
      }
      let unit = units[index]
      if isJSWhitespace(unit) || unit == UInt16(UInt8(ascii: "(")),
        let query = queryAfter(index + 1)
      {
        found = (index, 1, query)
        break
      }
    }
    guard let found, units.count - found.query >= minLength else { return nil }
    return MenuTextMatch(
      leadOffset: found.index + found.lead,
      matchingString: String(decoding: units[found.query...], as: UTF16.self),
      replaceableString: String(decoding: units[(found.index + found.lead)...], as: UTF16.self))
  }
}
