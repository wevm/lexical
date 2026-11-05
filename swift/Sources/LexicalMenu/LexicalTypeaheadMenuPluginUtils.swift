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

/// `useBasicTypeaheadTriggerMatch`: a trigger function for one trigger
/// character (`@`, `/`) after whitespace, an open parenthesis or the start
/// of the text, followed by `minLength` to `maxLength` characters that are
/// not punctuation (nor whitespace, unless `allowWhitespace`).
public func basicTypeaheadTriggerMatch(
  _ trigger: String, minLength: Int = 1, maxLength: Int = 75,
  punctuation: String = PUNCTUATION, allowWhitespace: Bool = false
) -> TriggerFn {
  let validCharsSuffix = allowWhitespace ? "" : #"\s"#
  let validChars = "[^" + trigger + punctuation + validCharsSuffix + "]"
  let source =
    #"(^|\s|\()("# + "[" + trigger + "]" + "((?:" + validChars + "){0," + String(maxLength) + "})"
    + ")$"
  // The source is built from the arguments, as JS builds its RegExp; an
  // invalid one (an unescaped trigger) matches nothing.
  let regex = try? Regex(source).matchingSemantics(.unicodeScalar)
  return { text, _ in
    guard let regex, let match = try? regex.firstMatch(in: text) else { return nil }
    let maybeLeadingWhitespace = match.output[1].substring.map(String.init) ?? ""
    let matchingString = match.output[3].substring.map(String.init) ?? ""
    guard matchingString.utf16.count >= minLength else { return nil }
    return MenuTextMatch(
      leadOffset: text.utf16.distance(from: text.startIndex, to: match.range.lowerBound)
        + maybeLeadingWhitespace.utf16.count,
      matchingString: matchingString,
      replaceableString: match.output[2].substring.map(String.init) ?? "")
  }
}
