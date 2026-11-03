import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts.
// Upstream runs as a non-Apple browser (jsdom), so these tests run with
// `withIsApple(false)`.

private struct Event: KeyboardEventModifiers {
  var key: String
  var code: String
  var altKey: Bool
  var ctrlKey: Bool
  var metaKey: Bool
  var shiftKey: Bool
}

private func makeEvent(_ key: String, _ code: String, _ bits: Int) -> Event {
  Event(
    key: key, code: code, altKey: bits & 1 != 0, ctrlKey: bits & 2 != 0, metaKey: bits & 4 != 0,
    shiftKey: bits & 8 != 0)
}

private struct Match: KeyboardShortcutMatch {
  var key: String
  var modifiers = KeyboardEventModifierMask()
  var unshiftedKey: String?
  var name = ""
}

private func keyboardEvent(
  key: String, altKey: Bool = false, ctrlKey: Bool = false, metaKey: Bool = false,
  shiftKey: Bool = false
) -> KeyboardEvent {
  KeyboardEvent(key: key, altKey: altKey, ctrlKey: ctrlKey, metaKey: metaKey, shiftKey: shiftKey)
}

private let ctrl = KeyboardEventModifierMask(ctrlKey: true)

private typealias Listener = (LexicalCommand<KeyboardEvent>, (KeyboardEvent) -> Bool)

private func registerListeners(
  _ listeners: [Listener]
) -> (
  Editor, EmptyConfig, ExtensionRegisterState<Void, Void>
) throws -> () -> Void {
  { editor, _, _ in
    mergeRegister(
      listeners.map { command, listener in
        editor.registerCommand(command, { event, _ in listener(event) }, COMMAND_PRIORITY_EDITOR)
      })
  }
}

private func buildTestEditor(
  _ shortcuts: NamedKeyboardShortcuts, _ listeners: [Listener]
) throws
  -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: "keyboard-shortcuts-test",
      dependencies: [configExtension(KeyboardShortcutsExtension) { $0.shortcuts = shortcuts }],
      register: registerListeners(listeners)))
}

private func buildLayeredEditor(
  _ layers: [Partial<KeyboardShortcutsConfig>], _ listeners: [Listener]
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "layered-test",
      dependencies: layers.map { configExtension(KeyboardShortcutsExtension, $0) },
      register: registerListeners(listeners)))
}

private func shortcutsLayer(
  _ shortcuts: NamedKeyboardShortcuts
) -> Partial<
  KeyboardShortcutsConfig
> {
  Partial { $0.shortcuts = shortcuts }
}

/// Commands whose listeners append their name to `calls`.
private final class CommandRecorder {
  var calls: [String] = []
  var listeners: [Listener] = []

  /// A Ctrl+K shortcut for a fresh command that records `name` when handled.
  func ctrlKShortcut(_ name: String, _ handled: Bool = true) -> KeyboardShortcut {
    let command = createCommand("recorder/\(name)", payload: KeyboardEvent.self)
    listeners.append(
      (
        command,
        { [unowned self] _ in
          self.calls.append(name)
          return handled
        }
      ))
    return KeyboardShortcut(key: "k", modifiers: ctrl, command: command)
  }
}

private func ctrlK() -> KeyboardEvent { keyboardEvent(key: "k", ctrlKey: true) }

private func output(_ editor: Editor) throws -> KeyboardShortcutsOutput {
  try getExtensionDependencyFromEditor(editor, KeyboardShortcutsExtension).output
}

@Suite struct CompileKeyboardShortcutsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::compileKeyboardShortcuts > matches exactly the events that isExactShortcutMatch matches"
    ))
  func matchesLikeIsExactShortcutMatch() throws {
    let eventKeys: [(String, String)] = [
      ("b", "KeyB"), ("B", "KeyB"), ("б", "KeyB"), ("1", "Digit1"), ("!", "Digit1"),
      ("١", "Digit1"), ("Enter", "Enter"), (",", "Comma"), ("[", "BracketLeft"), ("z", "KeyZ"),
    ]
    let shortcuts: [(String, KeyboardEventModifierMask)] = [
      ("b", ctrl), ("B", KeyboardEventModifierMask(metaKey: true)),
      ("1", KeyboardEventModifierMask(altKey: true, ctrlKey: true)),
      ("Enter", KeyboardEventModifierMask(shiftKey: .any)), (",", ctrl),
      ("[", KeyboardEventModifierMask()),
      ("z", KeyboardEventModifierMask(ctrlKey: true, shiftKey: .any)),
    ]
    for (key, modifiers) in shortcuts {
      let compiled = try compileKeyboardShortcuts([Match(key: key, modifiers: modifiers)])
      for (eventKey, eventCode) in eventKeys {
        for bits in 0..<16 {
          let event = makeEvent(eventKey, eventCode, bits)
          #expect(
            (compiled.match(event) != nil) == isExactShortcutMatch(event, key, modifiers),
            "key=\(key) modifiers=\(modifiers) event=\(event)")
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::compileKeyboardShortcuts > matches returns all matching shortcuts in insertion order"
    ))
  func matchesInInsertionOrder() throws {
    let first = Match(key: "k", modifiers: ctrl, name: "first")
    let second = Match(
      key: "K", modifiers: KeyboardEventModifierMask(ctrlKey: true, shiftKey: .any), name: "second")
    let other = Match(key: "j", modifiers: ctrl, name: "other")
    let compiled = try compileKeyboardShortcuts([first, second, other])
    #expect(compiled.matches(makeEvent("k", "KeyK", 2)).map(\.name) == ["first", "second"])
    #expect(compiled.matches(makeEvent("K", "KeyK", 2 | 8)).map(\.name) == ["second"])
    #expect(compiled.matches(makeEvent("k", "KeyK", 0)).isEmpty)
  }
}

@Suite struct FormatKeyboardShortcutTests {
  private func format(_ shortcut: Match, _ isApple: Bool) -> [String] {
    formatKeyboardShortcut(shortcut, FormatKeyboardShortcutOptions(isApple: isApple))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > CONTROL_OR_META"
    ))
  func controlOrMeta() {
    withIsApple(false) {
      #expect(format(Match(key: " ", modifiers: CONTROL_OR_META), true) == ["\u{2318}", "Space"])
      #expect(format(Match(key: " ", modifiers: CONTROL_OR_META), false) == ["Ctrl", "Space"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > CONTROL_OR_ALT"
    ))
  func controlOrAlt() {
    withIsApple(false) {
      #expect(format(Match(key: " ", modifiers: CONTROL_OR_ALT), true) == ["\u{2325}", "Space"])
      #expect(format(Match(key: " ", modifiers: CONTROL_OR_ALT), false) == ["Ctrl", "Space"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'k', modifiers: [Object] }) -> apple: ⇧+⌘+K other: Shift+Meta+K"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'q', modifiers: [Object] }) -> apple: ⌃+⇧+Q other: Ctrl+Shift+Q"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'q', modifiers: [Object] }) -> apple: ⇧+⌘+Q other: Ctrl+Shift+Q"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'Backspace', …(1) }) -> apple: ⌥+⌫ other: Ctrl+Backspace"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: ' ' }) -> apple: Space other: Space"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formatKeyboardShortcut({ key: 'ArrowLeft', …(1) }) -> apple: ← other: ArrowLeft"
    ),
    arguments: [0, 1, 2, 3, 4, 5])
  func formatsBothPlatforms(index: Int) {
    withIsApple(false) {
      let cases: [(Match, String, String)] = [
        (
          Match(key: "k", modifiers: KeyboardEventModifierMask(metaKey: true, shiftKey: true)),
          "⇧+⌘+K", "Shift+Meta+K"
        ),
        (
          Match(key: "q", modifiers: KeyboardEventModifierMask(ctrlKey: true, shiftKey: true)),
          "\u{2303}+⇧+Q", "Ctrl+Shift+Q"
        ),
        (Match(key: "q", modifiers: CONTROL_OR_META.with(shiftKey: true)), "⇧+⌘+Q", "Ctrl+Shift+Q"),
        (Match(key: "Backspace", modifiers: CONTROL_OR_ALT), "\u{2325}+\u{232B}", "Ctrl+Backspace"),
        (Match(key: " "), "Space", "Space"),
        (
          Match(key: "ArrowLeft", modifiers: KeyboardEventModifierMask(shiftKey: .any)), "\u{2190}",
          "ArrowLeft"
        ),
      ]
      let (shortcut, apple, other) = cases[index]
      #expect(
        [true, false].map { format(shortcut, $0).joined(separator: "+") } == [apple, other])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::formatKeyboardShortcut > formats platform conventions"
    ))
  func platformConventions() {
    let shortcut = Match(
      key: "k", modifiers: KeyboardEventModifierMask(metaKey: true, shiftKey: true))
    #expect(format(shortcut, true).joined(separator: "+") == "⇧+⌘+K")
    #expect(format(shortcut, false).joined(separator: "+") == "Shift+Meta+K")
    #expect(
      format(
        Match(key: "q", modifiers: KeyboardEventModifierMask(ctrlKey: true, shiftKey: true)), true
      )
      .joined(separator: "+") == "⌃+⇧+Q")
    #expect(
      format(
        Match(key: "0", modifiers: KeyboardEventModifierMask(altKey: true, ctrlKey: true)), false
      )
      .joined(separator: "+") == "Ctrl+Alt+0")
    #expect(format(Match(key: " "), false).joined(separator: "+") == "Space")
    #expect(
      format(Match(key: "ArrowLeft", modifiers: KeyboardEventModifierMask(shiftKey: .any)), false)
        .joined(separator: "+") == "ArrowLeft")
  }
}

/// A listener that counts its calls and returns `result`.
private final class Spy {
  var calls: [KeyboardEvent] = []
  let result: Bool
  init(_ result: Bool) { self.result = result }
  func callAsFunction(_ event: KeyboardEvent) -> Bool {
    calls.append(event)
    return result
  }
}

@Suite struct RegisterKeyboardShortcutsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > dispatches the matched shortcut command with the event as payload"
    ))
  func dispatchesMatchedCommand() throws {
    try withIsApple(false) {
      let BOLD = createCommand("test/BOLD", payload: KeyboardEvent.self)
      let ITALIC = createCommand("test/ITALIC", payload: KeyboardEvent.self)
      let bold = Spy(true)
      let italic = Spy(true)
      let editor = try buildTestEditor(
        [
          "BOLD": [KeyboardShortcut(key: "b", modifiers: ctrl, command: BOLD)],
          "ITALIC": [KeyboardShortcut(key: "i", modifiers: ctrl, command: ITALIC)],
        ], [(BOLD, bold.callAsFunction), (ITALIC, italic.callAsFunction)])
      defer { editor.dispose() }
      let event = keyboardEvent(key: "b", ctrlKey: true)
      #expect(try editor.dispatchCommand(KEY_DOWN_COMMAND, event))
      #expect(bold.calls.count == 1)
      #expect(bold.calls[0] === event)
      #expect(italic.calls.isEmpty)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b"))
      #expect(bold.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > falls through to the next matching shortcut when a dispatch is unhandled"
    ))
  func fallsThroughWhenUnhandled() throws {
    try withIsApple(false) {
      let SKIPPED = createCommand("test/SKIPPED", payload: KeyboardEvent.self)
      let HANDLED = createCommand("test/HANDLED", payload: KeyboardEvent.self)
      let skipped = Spy(false)
      let handled = Spy(true)
      let editor = try buildTestEditor(
        [
          "SKIPPED": [KeyboardShortcut(key: "k", modifiers: ctrl, command: SKIPPED)],
          "HANDLED": [KeyboardShortcut(key: "k", modifiers: ctrl, command: HANDLED)],
        ], [(SKIPPED, skipped.callAsFunction), (HANDLED, handled.callAsFunction)])
      defer { editor.dispose() }
      #expect(try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK()))
      #expect(skipped.calls.count == 1)
      #expect(handled.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > skips shortcuts whose $disabled predicate returns true"
    ))
  func skipsDisabled() throws {
    try withIsApple(false) {
      let DISABLED = createCommand("test/DISABLED", payload: KeyboardEvent.self)
      let ENABLED = createCommand("test/ENABLED", payload: KeyboardEvent.self)
      let disabled = Spy(true)
      let enabled = Spy(true)
      var disabledCalls = 0
      let editor = try buildTestEditor(
        [
          "DISABLED": [
            KeyboardShortcut(
              key: "k", modifiers: ctrl, command: DISABLED,
              disabled: { _, _ in
                disabledCalls += 1
                return true
              })
          ],
          "ENABLED": [KeyboardShortcut(key: "k", modifiers: ctrl, command: ENABLED)],
        ], [(DISABLED, disabled.callAsFunction), (ENABLED, enabled.callAsFunction)])
      defer { editor.dispose() }
      #expect(try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK()))
      #expect(disabledCalls == 1)
      #expect(disabled.calls.isEmpty)
      #expect(enabled.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > falls through when $dispatch returns false"
    ))
  func fallsThroughWhenDispatchReturnsFalse() throws {
    try withIsApple(false) {
      let SKIPPED = createCommand("test/DISPATCH_SKIP", payload: KeyboardEvent.self)
      let HANDLED = createCommand("test/DISPATCH_HANDLED", payload: KeyboardEvent.self)
      let handled = Spy(true)
      let editor = try buildTestEditor(
        [
          "SKIPPED": [
            KeyboardShortcut(
              key: "k", modifiers: ctrl, command: SKIPPED, dispatch: { _, _, _, _ in false })
          ],
          "HANDLED": [KeyboardShortcut(key: "k", modifiers: ctrl, command: HANDLED)],
        ], [(HANDLED, handled.callAsFunction)])
      defer { editor.dispose() }
      #expect(try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK()))
      #expect(handled.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::registerKeyboardShortcuts > $dispatch middleware wraps the command dispatch"
    ))
  func dispatchMiddlewareWraps() throws {
    try withIsApple(false) {
      let WRAPPED = createCommand("test/WRAPPED", payload: KeyboardEvent.self)
      var order: [String] = []
      var editorRef: Editor?
      let editor = try buildTestEditor(
        [
          "WRAPPED": [
            KeyboardShortcut(
              key: "k", modifiers: ctrl, command: WRAPPED,
              dispatch: { command, _, next, editor2 in
                #expect(command === WRAPPED)
                #expect(editor2 === editorRef)
                order.append("before")
                let handled = try next()
                order.append("after")
                return handled
              })
          ]
        ],
        [
          (
            WRAPPED,
            { _ in
              order.append("listener")
              return true
            }
          )
        ])
      editorRef = editor
      defer { editor.dispose() }
      #expect(try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK()))
      #expect(order == ["before", "listener", "after"])
    }
  }
}

private func shortcutWith(
  _ command: LexicalCommand<KeyboardEvent>, _ key: String, _ modifiers: KeyboardEventModifierMask
) -> [KeyboardShortcut] {
  [KeyboardShortcut(key: key, modifiers: modifiers, command: command)]
}

private func buildExtensionEditor(
  _ shortcuts: NamedKeyboardShortcuts, _ listeners: [Listener],
  _ overlay: NamedKeyboardShortcuts? = nil
) throws -> Editor {
  try buildLayeredEditor(
    overlay.map { [shortcutsLayer(shortcuts), shortcutsLayer($0)] } ?? [shortcutsLayer(shortcuts)],
    listeners)
}

@Suite struct KeyboardShortcutsExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > dispatches configured shortcuts"
    ))
  func dispatchesConfigured() throws {
    try withIsApple(false) {
      let BOLD = createCommand("ext/BOLD", payload: KeyboardEvent.self)
      let bold = Spy(true)
      let editor = try buildExtensionEditor(
        ["bold": shortcutWith(BOLD, "b", ctrl)], [(BOLD, bold.callAsFunction)])
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b", ctrlKey: true))
      #expect(bold.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > overlays merge by name: add, remap, and disable"
    ))
  func overlaysMergeByName() throws {
    try withIsApple(false) {
      let BOLD = createCommand("overlay/BOLD", payload: KeyboardEvent.self)
      let ITALIC = createCommand("overlay/ITALIC", payload: KeyboardEvent.self)
      let CUSTOM = createCommand("overlay/CUSTOM", payload: KeyboardEvent.self)
      let bold = Spy(true)
      let italic = Spy(true)
      let custom = Spy(true)
      let editor = try buildExtensionEditor(
        ["bold": shortcutWith(BOLD, "b", ctrl), "italic": shortcutWith(ITALIC, "i", ctrl)],
        [
          (BOLD, bold.callAsFunction), (ITALIC, italic.callAsFunction),
          (CUSTOM, custom.callAsFunction),
        ],
        [
          "bold": shortcutWith(BOLD, "b", KeyboardEventModifierMask(ctrlKey: true, shiftKey: true)),
          "custom": shortcutWith(CUSTOM, "m", KeyboardEventModifierMask(altKey: true)),
          "italic": nil,
        ])
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b", ctrlKey: true))
      #expect(bold.calls.isEmpty)
      _ = try editor.dispatchCommand(
        KEY_DOWN_COMMAND, keyboardEvent(key: "b", ctrlKey: true, shiftKey: true))
      #expect(bold.calls.count == 1)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "i", ctrlKey: true))
      #expect(italic.calls.isEmpty)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "m", altKey: true))
      #expect(custom.calls.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension > shortcuts can be remapped and disabled at runtime through the output signals"
    ))
  func remappedAtRuntime() throws {
    try withIsApple(false) {
      let BOLD = createCommand("runtime/BOLD", payload: KeyboardEvent.self)
      let bold = Spy(true)
      let editor = try buildExtensionEditor(
        ["bold": shortcutWith(BOLD, "b", ctrl)], [(BOLD, bold.callAsFunction)])
      defer { editor.dispose() }
      let output = try output(editor)
      var shortcuts = output.shortcuts.value
      shortcuts["bold"] = shortcutWith(BOLD, "b", KeyboardEventModifierMask(metaKey: true))
      output.shortcuts.value = shortcuts
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b", ctrlKey: true))
      #expect(bold.calls.isEmpty)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b", metaKey: true))
      #expect(bold.calls.count == 1)
      output.disabled.value = true
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, keyboardEvent(key: "b", metaKey: true))
      #expect(bold.calls.count == 1)
    }
  }
}

@Suite struct KeyboardShortcutsTableMergeTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a later layer replaces the mapping it overrides"
    ))
  func laterLayerReplaces() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("base", false)]]),
          shortcutsLayer(["bold": [rec.ctrlKShortcut("override", false)]]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["override"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > only the last layer to configure a name survives"
    ))
  func lastLayerSurvives() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("first", false)]]),
          shortcutsLayer(["bold": [rec.ctrlKShortcut("second", false)]]),
          shortcutsLayer(["bold": [rec.ctrlKShortcut("third", false)]]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["third"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > null disables the name"
    ))
  func nullDisables() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("base", false)]]),
          shortcutsLayer(["bold": nil]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls.isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > an array replaces the mapping with several bindings"
    ))
  func arrayReplaces() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let replaced = rec.ctrlKShortcut("replaced", false)
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("base", false)]]),
          shortcutsLayer(["bold": [replaced]]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["replaced"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > an empty array disables the name like null does"
    ))
  func emptyArrayDisables() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("base", false)]]),
          shortcutsLayer(["bold": []]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls.isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a name mapped to an array registers every entry in order"
    ))
  func arrayRegistersInOrder() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer([
            "bold": [rec.ctrlKShortcut("first", false), rec.ctrlKShortcut("second", false)]
          ])
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["first", "second"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > names added by a later layer are matched before existing names"
    ))
  func laterNamesFirst() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["aaa": [rec.ctrlKShortcut("aaa", false)]]),
          shortcutsLayer(["zzz": [rec.ctrlKShortcut("zzz", false)]]),
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["zzz", "aaa"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > a layer that configures no shortcuts leaves the table alone"
    ))
  func layerWithoutShortcuts() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          shortcutsLayer(["bold": [rec.ctrlKShortcut("base")]]),
          Partial { $0.priority = COMMAND_PRIORITY_CRITICAL },
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["base"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension shortcut table merge > arrays assigned to the runtime signal are flattened too"
    ))
  func runtimeArraysFlattened() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let base = rec.ctrlKShortcut("base", false)
      let runtimeFirst = rec.ctrlKShortcut("runtimeFirst", false)
      let runtimeSecond = rec.ctrlKShortcut("runtimeSecond", false)
      let editor = try buildLayeredEditor([shortcutsLayer(["bold": [base]])], rec.listeners)
      defer { editor.dispose() }
      try output(editor).shortcuts.value = ["bold": [runtimeFirst, runtimeSecond]]
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["runtimeFirst", "runtimeSecond"])
    }
  }
}

@Suite struct KeyboardShortcutsPriorityTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > defaults to COMMAND_PRIORITY_NORMAL, ahead of $handleKeyDown"
    ))
  func defaultsToNormal() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [shortcutsLayer(["bold": [rec.ctrlKShortcut("bold")]])], rec.listeners)
      defer { editor.dispose() }
      let output = try output(editor)
      #expect(output.priority.value == COMMAND_PRIORITY_NORMAL)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["bold"])
      // The core $handleKeyDown at COMMAND_PRIORITY_EDITOR always reports
      // the event as handled, so a listener at that priority is not reached.
      rec.calls = []
      output.priority.value = COMMAND_PRIORITY_EDITOR
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls.isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > a configured priority is used for the KEY_DOWN_COMMAND listener"
    ))
  func configuredPriority() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          Partial {
            $0.priority = COMMAND_PRIORITY_LOW
            $0.shortcuts = ["bold": [rec.ctrlKShortcut("bold")]]
          }
        ], rec.listeners)
      defer { editor.dispose() }
      let keyDown = editor.registerCommand(
        KEY_DOWN_COMMAND,
        { _, _ in
          rec.calls.append("high")
          return true
        }, COMMAND_PRIORITY_HIGH)
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["high"])
      rec.calls = []
      try output(editor).priority.value = COMMAND_PRIORITY_CRITICAL
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls == ["bold"])
      keyDown()
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > the last layer to configure a priority wins"
    ))
  func lastPriorityWins() throws {
    let editor = try buildLayeredEditor(
      [
        Partial { $0.priority = COMMAND_PRIORITY_LOW },
        Partial { $0.priority = COMMAND_PRIORITY_CRITICAL },
      ], [])
    defer { editor.dispose() }
    #expect(try output(editor).priority.value == COMMAND_PRIORITY_CRITICAL)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension priority > disabled: true never registers the listener"
    ))
  func disabledNeverRegisters() throws {
    try withIsApple(false) {
      let rec = CommandRecorder()
      let editor = try buildLayeredEditor(
        [
          Partial {
            $0.disabled = true
            $0.shortcuts = ["bold": [rec.ctrlKShortcut("bold")]]
          }
        ], rec.listeners)
      defer { editor.dispose() }
      _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(rec.calls.isEmpty)
    }
  }
}

private let NESTED_BOLD = createCommand("nested/BOLD", payload: KeyboardEvent.self)

private final class NestedEditors {
  var calls: [String] = []
  var parentEditor: Editor!
  var childEditor: Editor!

  init(_ shortcuts: NamedKeyboardShortcuts, priority: Int? = nil) throws {
    func recorder(
      _ name: String
    ) -> (
      Editor, EmptyConfig, ExtensionRegisterState<Void, Void>
    ) throws -> () -> Void {
      { [unowned self] editor, _, _ in
        editor.registerCommand(
          NESTED_BOLD,
          { _, _ in
            self.calls.append("bold@\(name)")
            return true
          }, COMMAND_PRIORITY_EDITOR)
      }
    }
    parentEditor = try buildEditorFromExtensions(
      defineExtension(
        name: "parent",
        dependencies: [
          configExtension(KeyboardShortcutsExtension) {
            if let priority { $0.priority = priority }
            $0.shortcuts = shortcuts
          }
        ], register: recorder("parent")))
    childEditor = try parentEditor.read {
      try buildEditorFromExtensions(
        defineExtension(
          name: "child", dependencies: [NestedEditorExtension], register: recorder("child")))
    }
  }

  func dispose() {
    childEditor.dispose()
    parentEditor.dispose()
  }
}

private func boldShortcut(
  bubbleFromNestedEditors: Bool = false,
  disabled: ((BaseSelection?, Editor) throws -> Bool)? = nil,
  dispatch: (
    (LexicalCommand<KeyboardEvent>, KeyboardEvent, () throws -> Bool, Editor) throws -> Bool
  )? = nil
) -> [KeyboardShortcut] {
  [
    KeyboardShortcut(
      key: "k", modifiers: ctrl, command: NESTED_BOLD, disabled: disabled, dispatch: dispatch,
      bubbleFromNestedEditors: bubbleFromNestedEditors)
  ]
}

@Suite struct KeyboardShortcutsNestedEditorTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > events bubbled up from a nested editor are ignored by default"
    ))
  func bubbledIgnoredByDefault() throws {
    try withIsApple(false) {
      let editors = try NestedEditors(["bold": boldShortcut()])
      defer { editors.dispose() }
      _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(editors.calls.isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > events from the registering editor are dispatched without the flag"
    ))
  func ownEventsDispatched() throws {
    try withIsApple(false) {
      let editors = try NestedEditors(["bold": boldShortcut()])
      defer { editors.dispose() }
      _ = try editors.parentEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(editors.calls == ["bold@parent"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > bubbleFromNestedEditors dispatches on the originating editor"
    ))
  func bubbleDispatchesOnOrigin() throws {
    try withIsApple(false) {
      let editors = try NestedEditors(["bold": boldShortcut(bubbleFromNestedEditors: true)])
      defer { editors.dispose() }
      _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(editors.calls == ["bold@child"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > a non-bubbling shortcut does not shadow a bubbling one on the same key"
    ))
  func nonBubblingDoesNotShadow() throws {
    try withIsApple(false) {
      let IGNORED = createCommand("nested/IGNORED", payload: KeyboardEvent.self)
      let ignored = Spy(true)
      let editors = try NestedEditors([
        "ignored": [KeyboardShortcut(key: "k", modifiers: ctrl, command: IGNORED)],
        "bold": boldShortcut(bubbleFromNestedEditors: true),
      ])
      defer { editors.dispose() }
      let cleanup = editors.parentEditor.registerCommand(
        IGNORED, { event, _ in ignored(event) }, COMMAND_PRIORITY_EDITOR)
      _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(ignored.calls.isEmpty)
      #expect(editors.calls == ["bold@child"])
      cleanup()
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > $disabled and $dispatch receive the originating editor"
    ))
  func predicatesReceiveOrigin() throws {
    try withIsApple(false) {
      var disabledEditors: [Editor] = []
      var dispatchEditors: [Editor] = []
      let editors = try NestedEditors([
        "bold": boldShortcut(
          bubbleFromNestedEditors: true,
          disabled: { _, editor in
            disabledEditors.append(editor)
            return false
          },
          dispatch: { _, _, next, editor in
            dispatchEditors.append(editor)
            return try next()
          })
      ])
      defer { editors.dispose() }
      _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(editors.calls == ["bold@child"])
      #expect(disabledEditors.count == 1)
      #expect(disabledEditors.first === editors.childEditor)
      #expect(dispatchEditors.count == 1)
      #expect(dispatchEditors.first === editors.childEditor)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > $disabled is not consulted for events that will not bubble"
    ))
  func disabledNotConsultedWithoutBubble() throws {
    try withIsApple(false) {
      var disabledCalls = 0
      let editors = try NestedEditors([
        "bold": boldShortcut(disabled: { _, _ in
          disabledCalls += 1
          return false
        })
      ])
      defer { editors.dispose() }
      _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
      #expect(editors.calls.isEmpty)
      #expect(disabledCalls == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/KeyboardShortcutsExtension.test.ts::KeyboardShortcutsExtension nested editors > bubbling requires a priority above COMMAND_PRIORITY_EDITOR"
    ))
  func bubblingNeedsPriority() throws {
    try withIsApple(false) {
      let bubbling: NamedKeyboardShortcuts = ["bold": boldShortcut(bubbleFromNestedEditors: true)]
      for priority in [COMMAND_PRIORITY_LOW, COMMAND_PRIORITY_NORMAL, COMMAND_PRIORITY_CRITICAL] {
        let editors = try NestedEditors(bubbling, priority: priority)
        _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
        #expect(editors.calls == ["bold@child"], "priority \(priority)")
        editors.dispose()
      }
      for priority in [COMMAND_PRIORITY_BEFORE_EDITOR, COMMAND_PRIORITY_EDITOR] {
        let editors = try NestedEditors(bubbling, priority: priority)
        _ = try editors.childEditor.dispatchCommand(KEY_DOWN_COMMAND, ctrlK())
        #expect(editors.calls.isEmpty, "priority \(priority)")
        editors.dispose()
      }
    }
  }
}
