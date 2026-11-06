import Foundation
import Lexical
import LexicalCodeCore
import LexicalList
import LexicalMenu
import LexicalRichText
import LexicalSelection
import Observation

// The composer's two typeahead menus, "@" for people and "/" for commands.
// `LexicalMenu` keeps each menu's state and its keys (arrows, Enter, Tab,
// Escape); the app draws the options, here as a list above the composer,
// the way chat apps show suggestions.

/// One row the suggestion list draws.
struct Suggestion: Identifiable, Equatable {
  let id: String
  let title: String
  let detail: String
  let symbol: String
}

/// What the view draws for an open menu: its rows and the highlighted
/// one. Clicks and the pointer go back to the menu.
@MainActor
@Observable
final class SuggestionsModel {
  private(set) var rows: [Suggestion] = []
  private(set) var selectedIndex: Int?
  @ObservationIgnored var choose: (Int) -> Void = { _ in }
  @ObservationIgnored var highlight: (Int) -> Void = { _ in }

  var isOpen: Bool { !rows.isEmpty }

  /// Follows `menu`, drawing each option with `row`.
  func follow<Option: MenuOption>(_ menu: MenuState<Option>, row: @escaping (Option) -> Suggestion)
  {
    menu.onChange = { [weak self, weak menu] in
      guard let self, let menu else { return }
      rows = menu.resolution == nil ? [] : menu.options.map(row)
      selectedIndex = menu.selectedIndex
    }
    choose = { [weak menu] index in
      guard let menu, menu.options.indices.contains(index) else { return }
      menu.setHighlightedIndex(index)
      menu.selectOptionAndCleanUp(menu.options[index])
    }
    highlight = { [weak menu] index in menu?.setHighlightedIndex(index) }
  }
}

/// The menus' priority: above the composer's Enter-to-send handler, so
/// Enter chooses a suggestion while a menu is open.
let menuPriority = COMMAND_PRIORITY_NORMAL

// MARK: Mentions

let people = [
  "Ada Lovelace", "Alan Turing", "Barbara Liskov", "Donald Knuth", "Edsger Dijkstra",
  "Grace Hopper", "Katherine Johnson", "Ken Thompson", "Margaret Hamilton", "Radia Perlman",
]

final class PersonOption: MenuOption {
  let name: String

  init(name: String) {
    self.name = name
    super.init(name)
    title = name
  }
}

/// "@" and the start of a name opens a menu of the people whose names
/// contain it; choosing one replaces the query with a `MentionNode`.
@MainActor
func registerMentionsMenu(_ editor: Editor, _ model: SuggestionsModel) -> () -> Void {
  let trigger = basicTypeaheadTriggerMatch("@", minLength: 0, maxLength: 30)
  var menuRef: MenuState<PersonOption>?
  let (menu, unregister) = registerTypeaheadMenu(
    editor, triggerFn: trigger,
    onQueryChange: { query in
      MainActor.assumeIsolated {
        guard let query else { return }
        let matches = people.filter { query.isEmpty || $0.localizedCaseInsensitiveContains(query) }
        menuRef?.setOptions(matches.prefix(5).map { PersonOption(name: $0) })
      }
    },
    onSelectOption: { (option: PersonOption, nodeToReplace, closeMenu, _) in
      let mention = try createMentionNode(option.name)
      try nodeToReplace?.replace(mention)
      // A space after the mention, as chat apps add, so typing goes on.
      let space = try createTextNode(" ")
      try mention.insertAfter(space)
      try space.select()
      closeMenu()
    },
    commandPriority: menuPriority)
  menuRef = menu
  model.follow(menu) { option in
    Suggestion(
      id: option.key, title: option.name, detail: "", symbol: "person.crop.circle.fill")
  }
  return unregister
}

// MARK: Slash commands

final class CommandOption: MenuOption {
  let detail: String
  let symbol: String
  let run: () throws -> Void

  init(_ name: String, _ detail: String, _ symbol: String, run: @escaping () throws -> Void) {
    self.detail = detail
    self.symbol = symbol
    self.run = run
    super.init(name)
    title = name
  }
}

@MainActor
private func slashCommands(_ editor: Editor) -> [CommandOption] {
  [
    CommandOption("shrug", "Append ¯\\_(ツ)_/¯", "face.smiling") {
      try getSelection()?.insertText("¯\\_(ツ)_/¯")
    },
    CommandOption("code", "Start a code block", "chevron.left.forwardslash.chevron.right") {
      try setBlocksType(getSelection()) { try createCodeNode() }
    },
    CommandOption("quote", "Start a quote", "text.quote") {
      try setBlocksType(getSelection()) { try createQuoteNode() }
    },
    CommandOption("list", "Start a bulleted list", "list.bullet") {
      _ = try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND, ())
    },
  ]
}

/// "/" at the start of a word opens a menu of commands, filtered by what
/// follows it; choosing one removes the query and runs the command.
@MainActor
func registerSlashMenu(_ editor: Editor, _ model: SuggestionsModel) -> () -> Void {
  let trigger = basicTypeaheadTriggerMatch("/", minLength: 0, maxLength: 20)
  let commands = slashCommands(editor)
  var menuRef: MenuState<CommandOption>?
  let (menu, unregister) = registerTypeaheadMenu(
    editor, triggerFn: trigger,
    onQueryChange: { query in
      MainActor.assumeIsolated {
        guard let query else { return }
        menuRef?.setOptions(
          commands.filter { query.isEmpty || $0.key.hasPrefix(query.lowercased()) })
      }
    },
    onSelectOption: { (option: CommandOption, nodeToReplace, closeMenu, _) in
      try nodeToReplace?.remove()
      try option.run()
      closeMenu()
    },
    commandPriority: menuPriority)
  menuRef = menu
  model.follow(menu) { option in
    Suggestion(
      id: option.key, title: "/" + option.key, detail: option.detail, symbol: option.symbol)
  }
  return unregister
}
