import Foundation
import Lexical
import LexicalMenu

// Port of packages/lexical-playground/src/plugins/EmojiPickerPlugin: a ":"
// and at least one character open a menu of up to 10 emoji whose alias or
// tag matches. `emoji-list.json` is upstream's `utils/emoji-list.ts` (the
// gemoji list) with only the fields the picker reads.

/// `EmojiOption`.
final class EmojiOption: MenuOption {
  let emoji: String
  let keywords: [String]

  init(_ title: String, emoji: String, keywords: [String] = []) {
    self.emoji = emoji
    self.keywords = keywords
    super.init(title)
    self.title = title
  }
}

/// `Emoji`, the fields of an `emoji-list.ts` entry the picker reads.
struct Emoji: Decodable {
  let emoji: String
  let aliases: [String]
  let tags: [String]
}

private let MAX_EMOJI_SUGGESTION_COUNT = 10

/// The emoji list, loaded once.
@MainActor
private let emojiOptions: [EmojiOption] = {
  guard let url = Bundle.main.url(forResource: "emoji-list", withExtension: "json"),
    let data = try? Data(contentsOf: url),
    let emojis = try? JSONDecoder().decode([Emoji].self, from: data)
  else { return [] }
  return emojis.map {
    EmojiOption("\($0.emoji) \($0.aliases[0])", emoji: $0.emoji, keywords: $0.aliases + $0.tags)
  }
}()

/// The options for `queryString`: those with a keyword that matches it as
/// a case-insensitive regular expression, at most 10. Upstream's filter
/// also tests the title, but `||` binds tighter than its `?:`, so only the
/// keywords decide; this keeps that. A query that is not a valid pattern
/// matches nothing, where JS throws.
func filterEmojiOptions(_ options: [EmojiOption], _ queryString: String?) -> [EmojiOption] {
  guard let queryString else { return Array(options.prefix(MAX_EMOJI_SUGGESTION_COUNT)) }
  guard let regex = try? NSRegularExpression(pattern: queryString, options: .caseInsensitive)
  else { return [] }
  var result: [EmojiOption] = []
  for option in options
  where option.keywords.contains(where: {
    regex.firstMatch(in: $0, range: NSRange($0.startIndex..., in: $0)) != nil
  }) {
    result.append(option)
    if result.count == MAX_EMOJI_SUGGESTION_COUNT { break }
  }
  return result
}

/// `EmojiPickerPlugin`.
@MainActor
func registerEmojiPicker(
  _ editor: Editor
) -> (
  host: TypeaheadMenuHost<EmojiOption>, unregister: () -> Void
) {
  var setOptions: ([EmojiOption]) -> Void = { _ in }
  let (menu, unregister) = registerTypeaheadMenu(
    editor,
    triggerFn: basicTypeaheadTriggerMatch(
      ":",
      // A bare ":" must not open the menu, or its Enter would take the
      // paragraph break.
      minLength: 1,
      punctuation: #"\.,\+\*\?\$\@\|#{}\(\)\^\[\]\\/!%'"~=<>:;"#),  // allow _ and -
    onQueryChange: { query in setOptions(filterEmojiOptions(emojiOptions, query)) },
    onSelectOption: { (option: EmojiOption, nodeToRemove, closeMenu, _) in
      guard let selection = try getSelection() as? RangeSelection else { return }
      try nodeToRemove?.remove()
      try selection.insertNodes([createTextNode(option.emoji)])
      closeMenu()
    })
  setOptions = { [weak menu] in menu?.setOptions($0) }
  let host = TypeaheadMenuHost(menu: menu) {
    TypeaheadMenuRow(id: $0.key, title: $0.title ?? "", symbol: nil, glyph: nil)
  }
  return (host, unregister)
}
