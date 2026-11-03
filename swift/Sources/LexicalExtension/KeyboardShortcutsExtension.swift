import Lexical

/// `FormatKeyboardShortcutOptions`.
public struct FormatKeyboardShortcutOptions {
  public var isApple: Bool?
  public var separator: String?

  public init(isApple: Bool? = nil, separator: String? = nil) {
    self.isApple = isApple
    self.separator = separator
  }
}

private let UNIVERSAL_KEYS: [String: String] = [" ": "Space"]

private let APPLE_KEYS: [String: String] = UNIVERSAL_KEYS.merging(
  [
    "Alt": "\u{2325}", "ArrowDown": "\u{2193}", "ArrowLeft": "\u{2190}",
    "ArrowRight": "\u{2192}", "ArrowUp": "\u{2191}", "Backspace": "\u{232B}",
    "CapsLock": "\u{21EA}", "Ctrl": "\u{2303}", "Delete": "\u{2326}", "End": "\u{2198}",
    "Enter": "\u{21A9}", "Escape": "\u{238B}", "Home": "\u{2196}", "Meta": "\u{2318}",
    "PageDown": "\u{21DF}", "PageUp": "\u{21DE}", "Shift": "\u{21E7}", "Tab": "\u{21E5}",
  ]
) { $1 }

private let SHIFT_APPLE_KEYS: [String: String] = APPLE_KEYS.merging(["Tab": "\u{21E4}"]) { $1 }

/// `formatKeyboardShortcut(shortcut, options)`: the shortcut's modifier and
/// key names, with the platform's symbols on Apple platforms.
public func formatKeyboardShortcut(
  _ shortcut: some KeyboardShortcutMatch,
  _ options: FormatKeyboardShortcutOptions = FormatKeyboardShortcutOptions()
) -> [String] {
  let isApple = options.isApple ?? IS_APPLE
  let key = shortcut.key
  let unshiftedKey = shortcut.unshiftedKey
  let modifiers = keyboardEventMaskForPlatform(shortcut.modifiers, isApple)
  var segments: [String] = []
  let keyNames =
    isApple ? (modifiers.shiftKey == .pressed ? SHIFT_APPLE_KEYS : APPLE_KEYS) : UNIVERSAL_KEYS
  let modifierNames: [(ModifierExpectation, Bool, String)] = [
    (modifiers.ctrlKey, false, "Ctrl"), (modifiers.altKey, false, "Alt"),
    (modifiers.shiftKey, true, "Shift"), (modifiers.metaKey, false, "Meta"),
  ]
  for (expected, isShift, name) in modifierNames where expected == .pressed {
    if isApple && isShift && unshiftedKey != nil && key.utf16.count == 1 {
      continue
    }
    segments.append(keyNames[name] ?? name)
  }
  if let name = keyNames[key] {
    segments.append(name)
  } else if !isApple, modifiers.shiftKey == .pressed, let unshiftedKey, !unshiftedKey.isEmpty {
    segments.append(unshiftedKey)
  } else if key.utf16.count == 1 {
    segments.append(key.uppercased())
  } else {
    segments.append(key)
  }
  return segments
}

/// `NamedKeyboardShortcuts`: shortcut bindings by name, in the order they
/// were added. A `nil` value (JS `null`) disables the name.
public struct NamedKeyboardShortcuts: ExpressibleByDictionaryLiteral {
  public private(set) var entries: [(name: String, shortcuts: [KeyboardShortcut]?)] = []

  public init() {}

  public init(dictionaryLiteral elements: (String, [KeyboardShortcut]?)...) {
    for (name, shortcuts) in elements { self[name] = shortcuts }
  }

  /// Whether `name` is present (as a binding or as `nil`).
  public func has(_ name: String) -> Bool { entries.contains { $0.name == name } }

  /// The bindings of `name`; reading `nil` means absent or disabled.
  public subscript(name: String) -> [KeyboardShortcut]? {
    get { entries.first { $0.name == name }?.shortcuts ?? nil }
    set { set(name, newValue) }
  }

  /// Sets `name` to `shortcuts`, or to `nil` to disable it, keeping its
  /// place when present.
  public mutating func set(_ name: String, _ shortcuts: [KeyboardShortcut]?) {
    if let index = entries.firstIndex(where: { $0.name == name }) {
      entries[index].shortcuts = shortcuts
    } else {
      entries.append((name, shortcuts))
    }
  }
}

/// `KeyboardShortcutsConfig`.
public struct KeyboardShortcutsConfig {
  public var disabled: Bool
  public var priority: Int
  public var shortcuts: NamedKeyboardShortcuts

  public init(
    disabled: Bool = false, priority: Int = COMMAND_PRIORITY_NORMAL,
    shortcuts: NamedKeyboardShortcuts = NamedKeyboardShortcuts()
  ) {
    self.disabled = disabled
    self.priority = priority
    self.shortcuts = shortcuts
  }
}

func registerKeyboardShortcuts(
  _ editor: Editor, _ shortcuts: [KeyboardShortcut], _ priority: Int
) throws -> () -> Void {
  let compiled = try compileKeyboardShortcuts(shortcuts)
  return editor.registerCommand(
    KEY_DOWN_COMMAND,
    { [unowned editor] event, fromEditor in
      var selection: BaseSelection??
      for shortcut in compiled.matches(event) {
        if editor !== fromEditor && !shortcut.bubbleFromNestedEditors {
          continue
        }
        if let disabled = shortcut.disabled {
          if selection == nil {
            selection = .some(try getSelection())
          }
          if try disabled(selection!, fromEditor) {
            continue
          }
        }
        let next = { try fromEditor.dispatchCommand(shortcut.command, event) }
        if let dispatch = shortcut.dispatch {
          if try dispatch(shortcut.command, event, next, fromEditor) { return true }
        } else if try next() {
          return true
        }
      }
      return false
    }, priority)
}

func mergeNamedShortcuts(
  _ config: NamedKeyboardShortcuts, _ overrides: NamedKeyboardShortcuts?
) -> NamedKeyboardShortcuts {
  guard let overrides else { return config }
  var dest = overrides
  for (name, shortcuts) in config.entries where !dest.has(name) {
    dest.set(name, shortcuts)
  }
  return dest
}

/// The output of `KeyboardShortcutsExtension`: the config as signals, so
/// shortcuts can be remapped and disabled at runtime.
public typealias KeyboardShortcutsOutput = NamedSignals<KeyboardShortcutsConfig>

/// `KeyboardShortcutsExtension`: dispatches the command of each shortcut
/// that matches a key-down, in name order, until one is handled.
public let KeyboardShortcutsExtension = defineExtension(
  name: "@lexical/extension/KeyboardShortcuts", config: KeyboardShortcutsConfig(),
  mergeConfig: { config, overrides in
    var merged = shallowMergeConfig(config, overrides)
    merged.shortcuts = mergeNamedShortcuts(config.shortcuts, overrides.shortcuts)
    return merged
  },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      guard !output.disabled.value else { return nil }
      let shortcuts = output.shortcuts.value.entries.flatMap { $0.shortcuts ?? [] }
      return try registerKeyboardShortcuts(editor, shortcuts, output.priority.value)
    }
  })
