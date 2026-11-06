import Lexical
import LexicalExtension
import SwiftUI

// The playground's keyboard shortcuts, ported from
// packages/lexical-playground/src/plugins/ShortcutsExtension: the key
// bindings (`SHORTCUT_BINDINGS`), each dispatching its own command through
// `KeyboardShortcutsExtension`, and the Keyboard Shortcuts panel
// (`ShortcutsHelpDialog`). They belong to the playground, not to Lexical.

private let CONTROL_OR_META_ALT = CONTROL_OR_META.with(altKey: true)
private let CONTROL_OR_META_SHIFT = CONTROL_OR_META.with(shiftKey: true)
private let CONTROL_SHIFT = KeyboardEventModifierMask(ctrlKey: true, shiftKey: true)

/// A shortcut of the web playground's panel, in its order: those
/// `ShortcutsExtension` handles, then the editor's own (`BUILTIN_SHORTCUT_BINDINGS`).
enum PlaygroundShortcut: String, CaseIterable, Identifiable {
  case normal = "NORMAL"
  case heading1 = "HEADING1"
  case heading2 = "HEADING2"
  case heading3 = "HEADING3"
  case numberedList = "NUMBERED_LIST"
  case bulletList = "BULLET_LIST"
  case checkList = "CHECK_LIST"
  case codeBlock = "CODE_BLOCK"
  case quote = "QUOTE"
  case addComment = "ADD_COMMENT"
  case increaseFontSize = "INCREASE_FONT_SIZE"
  case decreaseFontSize = "DECREASE_FONT_SIZE"
  case insertCodeBlock = "INSERT_CODE_BLOCK"
  case strikethrough = "STRIKETHROUGH"
  case lowercase = "LOWERCASE"
  case uppercase = "UPPERCASE"
  case capitalize = "CAPITALIZE"
  case centerAlign = "CENTER_ALIGN"
  case justifyAlign = "JUSTIFY_ALIGN"
  case leftAlign = "LEFT_ALIGN"
  case rightAlign = "RIGHT_ALIGN"
  case `subscript` = "SUBSCRIPT"
  case superscript = "SUPERSCRIPT"
  case indent = "INDENT"
  case outdent = "OUTDENT"
  case clearFormatting = "CLEAR_FORMATTING"
  case insertLink = "INSERT_LINK"
  case bold = "BOLD"
  case italic = "ITALIC"
  case underline = "UNDERLINE"
  case undo = "UNDO"
  case redo = "REDO"

  var id: String { rawValue }

  /// The key binding, as in `SHORTCUT_BINDINGS` (and
  /// `BUILTIN_SHORTCUT_BINDINGS`).
  var binding: (key: String, modifiers: KeyboardEventModifierMask, unshiftedKey: String?) {
    switch self {
    case .normal: ("0", CONTROL_OR_META_ALT, nil)
    case .heading1: ("1", CONTROL_OR_META_ALT, nil)
    case .heading2: ("2", CONTROL_OR_META_ALT, nil)
    case .heading3: ("3", CONTROL_OR_META_ALT, nil)
    case .numberedList: ("7", CONTROL_OR_META_SHIFT, nil)
    case .bulletList: ("8", CONTROL_OR_META_SHIFT, nil)
    case .checkList: ("9", CONTROL_OR_META_SHIFT, nil)
    case .codeBlock: ("c", CONTROL_OR_META_ALT, nil)
    case .quote: ("q", CONTROL_SHIFT, nil)
    case .addComment: ("m", CONTROL_OR_META_ALT, nil)
    case .increaseFontSize: (">", CONTROL_OR_META_SHIFT, ".")
    case .decreaseFontSize: ("<", CONTROL_OR_META_SHIFT, ",")
    case .insertCodeBlock: ("c", CONTROL_OR_META_SHIFT, nil)
    case .strikethrough: ("x", CONTROL_OR_META_SHIFT, nil)
    case .lowercase: ("1", CONTROL_SHIFT, nil)
    case .uppercase: ("2", CONTROL_SHIFT, nil)
    case .capitalize: ("3", CONTROL_SHIFT, nil)
    case .centerAlign: ("e", CONTROL_OR_META_SHIFT, nil)
    case .justifyAlign: ("j", CONTROL_OR_META_SHIFT, nil)
    case .leftAlign: ("l", CONTROL_OR_META_SHIFT, nil)
    case .rightAlign: ("r", CONTROL_OR_META_SHIFT, nil)
    case .subscript: (",", CONTROL_OR_META, nil)
    case .superscript: (".", CONTROL_OR_META, nil)
    case .indent: ("]", CONTROL_OR_META, nil)
    case .outdent: ("[", CONTROL_OR_META, nil)
    case .clearFormatting: ("\\", CONTROL_OR_META, nil)
    case .insertLink: ("k", CONTROL_OR_META, nil)
    case .bold: ("b", CONTROL_OR_META, nil)
    case .italic: ("i", CONTROL_OR_META, nil)
    case .underline: ("u", CONTROL_OR_META, nil)
    case .undo: ("z", CONTROL_OR_META, nil)
    case .redo: ("z", CONTROL_OR_META_SHIFT, nil)
    }
  }

  /// The editor handles these itself (rich text and history).
  var isBuiltin: Bool { [.bold, .italic, .underline, .undo, .redo].contains(self) }

  /// The keys Safari reports for the same key press that the web's binding
  /// misses, so it matches here: Shift doesn't apply with Command (⇧⌘. is
  /// `.`, not `>`), and with Control it does (⌃⇧1 is `!` on a US layout).
  var otherKeys: [String] {
    switch self {
    case .increaseFontSize, .decreaseFontSize: binding.unshiftedKey.map { [$0] } ?? []
    case .lowercase: ["!"]
    case .uppercase: ["@"]
    case .capitalize: ["#"]
    default: []
    }
  }

  /// The command the shortcut dispatches with its key press.
  var command: LexicalCommand<KeyboardEvent> { shortcutCommands[self]! }

  /// The action's name in the panel, as `humanize` writes it.
  var title: String {
    let words = rawValue.lowercased().replacing("_", with: " ")
      .replacing(/(\d+)/, with: { " \($0.1)" })
    return words.prefix(1).uppercased() + words.dropFirst()
  }

  /// The shortcut's keys as the panel shows them (`formatKeyboardShortcut`),
  /// on Apple platforms or the others, where Redo is also Ctrl+Y.
  func keys(isApple: Bool) -> [String] {
    let (key, modifiers, unshiftedKey) =
      !isApple && self == .redo
      ? ("y", KeyboardEventModifierMask(ctrlKey: true), nil) : binding
    return formatKeyboardShortcut(
      Lexical.KeyboardShortcut(
        key: key, modifiers: modifiers, unshiftedKey: unshiftedKey, command: command),
      FormatKeyboardShortcutOptions(isApple: isApple))
  }

  /// The menu item's key equivalent. A shifted symbol is its own key there
  /// (⌘> rather than ⇧⌘.).
  var keyboardShortcut: SwiftUI.KeyboardShortcut? {
    let (key, mask, _) = binding
    guard let character = key.first else { return nil }
    var modifiers: EventModifiers = []
    if mask.controlOrOther != nil || mask.metaKey == .pressed { modifiers.insert(.command) }
    if mask.ctrlKey == .pressed && mask.controlOrOther == nil { modifiers.insert(.control) }
    if mask.altKey == .pressed { modifiers.insert(.option) }
    if mask.shiftKey == .pressed, character.isLetter || character.isNumber {
      modifiers.insert(.shift)
    }
    return SwiftUI.KeyboardShortcut(KeyEquivalent(character), modifiers: modifiers)
  }
}

private nonisolated(unsafe) let shortcutCommands = Dictionary(
  uniqueKeysWithValues: PlaygroundShortcut.allCases.map {
    ($0, createCommand("@lexical/playground/Shortcuts/\($0.rawValue)", payload: KeyboardEvent.self))
  })

/// `buildShortcuts()`: each binding with its command.
private func playgroundShortcutTable() -> NamedKeyboardShortcuts {
  var table = NamedKeyboardShortcuts()
  for shortcut in PlaygroundShortcut.allCases where !shortcut.isBuiltin {
    let (key, modifiers, unshiftedKey) = shortcut.binding
    table.set(
      shortcut.rawValue,
      ([key] + shortcut.otherKeys).map {
        Lexical.KeyboardShortcut(
          key: $0, modifiers: modifiers, unshiftedKey: unshiftedKey, command: shortcut.command)
      })
  }
  return table
}

/// `ShortcutsExtension`: contributes the key bindings to
/// `KeyboardShortcutsExtension`'s table. The model registers what each
/// command does.
let PlaygroundShortcutsExtension = defineExtension(
  name: "@lexical/playground-swift/Shortcuts",
  dependencies: [
    configExtension(KeyboardShortcutsExtension) { $0.shortcuts = playgroundShortcutTable() }
  ])

/// `ShortcutsHelpDialog`: each action and its shortcut, for macOS or for
/// Windows and Linux.
struct ShortcutsHelpView: View {
  @State private var isApple = true

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("Keyboard shortcuts").font(.title2.weight(.semibold))
      Picker("Keyboard layout", selection: $isApple) {
        Label("macOS", systemImage: "apple.logo").tag(true)
        Label("Windows / Linux", systemImage: "pc").tag(false)
      }
      .pickerStyle(.segmented)
      .labelsHidden()
      ScrollView {
        Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 8) {
          GridRow {
            Text("Action")
            Text("Shortcut")
          }
          .font(.headline)
          Divider()
          ForEach(PlaygroundShortcut.allCases) { shortcut in
            GridRow {
              Text(shortcut.title)
              HStack(spacing: 4) {
                ForEach(Array(shortcut.keys(isApple: isApple).enumerated()), id: \.offset) {
                  Text($0.element)
                    .font(.system(.callout, design: .monospaced))
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 4))
                }
              }
              .accessibilityElement(children: .combine)
            }
          }
        }
      }
    }
    .padding(20)
    .frame(minWidth: 380, minHeight: 420)
    .accessibilityIdentifier("keyboard-shortcuts")
  }
}

/// The shortcuts as menu items, so they show in the menu bar and work while
/// the editor isn't focused; while it is, the editor's key handling takes
/// them first.
struct PlaygroundCommands: Commands {
  @FocusedValue(\.playgroundModel) private var model
  #if os(macOS)
    @Environment(\.openWindow) private var openWindow
  #endif

  private func item(_ shortcut: PlaygroundShortcut, _ title: String? = nil) -> some View {
    Button(title ?? shortcut.title) { model?.perform(shortcut) }
      .keyboardShortcut(shortcut.keyboardShortcut)
      .disabled(model == nil)
  }

  var body: some Commands {
    CommandMenu("Format") {
      item(.normal)
      item(.heading1)
      item(.heading2)
      item(.heading3)
      item(.numberedList)
      item(.bulletList)
      item(.checkList)
      item(.codeBlock)
      item(.quote)
      Divider()
      item(.addComment)
      Divider()
      item(.bold)
      item(.italic)
      item(.underline)
      item(.strikethrough)
      item(.insertCodeBlock, "Inline code")
      item(.subscript)
      item(.superscript)
      item(.lowercase)
      item(.uppercase)
      item(.capitalize)
      Divider()
      item(.increaseFontSize)
      item(.decreaseFontSize)
      item(.leftAlign)
      item(.centerAlign)
      item(.rightAlign)
      item(.justifyAlign)
      item(.indent)
      item(.outdent)
      Divider()
      item(.clearFormatting)
      item(.insertLink)
    }
    CommandGroup(after: .textEditing) {
      // `FindReplaceExtension`'s ⌘F, for when the editor isn't focused.
      Button("Find and Replace…") { model?.findReplace.dispatch(TOGGLE_FIND_REPLACE_COMMAND) }
        .keyboardShortcut("f")
        .disabled(model == nil)
    }
    CommandGroup(replacing: .help) {
      Button("Keyboard Shortcuts") {
        #if os(macOS)
          openWindow(id: "keyboard-shortcuts")
        #else
          model?.showsShortcuts = true
        #endif
      }
    }
  }
}

extension FocusedValues {
  @Entry var playgroundModel: PlaygroundModel?
}
