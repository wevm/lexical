import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/keyDownDispatchParity.ts, run
// by LexicalEventsKeyDown.test.ts (IS_APPLE=false) and
// LexicalEventsKeyDownApple.test.ts (IS_APPLE=true): the default
// KEY_DOWN_COMMAND handler dispatches what the legacy predicate chain would.

private enum Expected {
  case event
  case value(String)
}

private struct ExpectedDispatch {
  let command: String
  let payload: Expected
  let preventsDefault: Bool
}

private func referenceKeyDown(_ event: KeyboardEvent, _ isApple: Bool) -> ExpectedDispatch? {
  func m(_ key: String, _ mask: KeyboardEventModifierMask) -> Bool {
    isExactShortcutMatch(event, key, mask)
  }
  func dispatched(
    _ command: String, _ payload: Expected, _ preventsDefault: Bool = false
  )
    -> ExpectedDispatch
  {
    ExpectedDispatch(command: command, payload: payload, preventsDefault: preventsDefault)
  }
  let shiftAny = KeyboardEventModifierMask(shiftKey: .any)
  let ctrlOnly = KeyboardEventModifierMask(ctrlKey: true)
  let metaOnly = KeyboardEventModifierMask(metaKey: true)
  if m("ArrowRight", shiftAny) {
    return dispatched("KEY_ARROW_RIGHT_COMMAND", .event)
  } else if isApple ? m("ArrowRight", shiftAny.with(metaKey: true)) : m("End", shiftAny) {
    return dispatched("MOVE_TO_END", .event)
  } else if m("ArrowLeft", shiftAny) {
    return dispatched("KEY_ARROW_LEFT_COMMAND", .event)
  } else if isApple ? m("ArrowLeft", shiftAny.with(metaKey: true)) : m("Home", shiftAny) {
    return dispatched("MOVE_TO_START", .event)
  } else if m("ArrowUp", KeyboardEventModifierMask(altKey: .any, shiftKey: .any)) {
    return dispatched("KEY_ARROW_UP_COMMAND", .event)
  } else if m("ArrowDown", KeyboardEventModifierMask(altKey: .any, shiftKey: .any)) {
    return dispatched("KEY_ARROW_DOWN_COMMAND", .event)
  } else if m(
    "Enter", KeyboardEventModifierMask(altKey: .any, ctrlKey: .any, metaKey: .any, shiftKey: true))
  {
    return dispatched("KEY_ENTER_COMMAND", .event)
  } else if event.key == " " {
    return dispatched("KEY_SPACE_COMMAND", .event)
  } else if isApple && m("o", ctrlOnly) {
    return dispatched("INSERT_LINE_BREAK_COMMAND", .value("true"), true)
  } else if m("Enter", KeyboardEventModifierMask(altKey: .any, ctrlKey: .any, metaKey: .any)) {
    return dispatched("KEY_ENTER_COMMAND", .event)
  } else if m("Backspace", shiftAny) || (isApple && m("h", ctrlOnly)) {
    return event.key == "Backspace"
      ? dispatched("KEY_BACKSPACE_COMMAND", .event)
      : dispatched("DELETE_CHARACTER_COMMAND", .value("true"), true)
  } else if event.key == "Escape" {
    return dispatched("KEY_ESCAPE_COMMAND", .event)
  } else if m("Delete", KeyboardEventModifierMask()) || (isApple && m("d", ctrlOnly)) {
    return event.key == "Delete"
      ? dispatched("KEY_DELETE_COMMAND", .event)
      : dispatched("DELETE_CHARACTER_COMMAND", .value("false"), true)
  } else if m("Backspace", CONTROL_OR_ALT) {
    return dispatched("DELETE_WORD_COMMAND", .value("true"), true)
  } else if m("Delete", CONTROL_OR_ALT) {
    return dispatched("DELETE_WORD_COMMAND", .value("false"), true)
  } else if isApple && m("Backspace", metaOnly) {
    return dispatched("DELETE_LINE_COMMAND", .value("true"), true)
  } else if isApple && (m("Delete", metaOnly) || m("k", ctrlOnly)) {
    return dispatched("DELETE_LINE_COMMAND", .value("false"), true)
  } else if m("b", CONTROL_OR_META) {
    return dispatched("FORMAT_TEXT_COMMAND", .value("bold"), true)
  } else if m("u", CONTROL_OR_META) {
    return dispatched("FORMAT_TEXT_COMMAND", .value("underline"), true)
  } else if m("i", CONTROL_OR_META) {
    return dispatched("FORMAT_TEXT_COMMAND", .value("italic"), true)
  } else if m("Tab", shiftAny) {
    return dispatched("KEY_TAB_COMMAND", .event)
  } else if m("z", CONTROL_OR_META) {
    return dispatched("UNDO_COMMAND", .value("()"), true)
  } else if isApple
    ? m("z", KeyboardEventModifierMask(metaKey: true, shiftKey: true))
    : m("y", ctrlOnly) || m("z", KeyboardEventModifierMask(ctrlKey: true, shiftKey: true))
  {
    return dispatched("REDO_COMMAND", .value("()"), true)
  } else if m("a", CONTROL_OR_META) {
    return dispatched("SELECT_ALL_COMMAND", .event, true)
  }
  // Copy and cut need a non-range previous selection; there is none.
  return nil
}

private let GRID_KEYS: [(String, String)] = [
  ("ArrowRight", "ArrowRight"), ("ArrowLeft", "ArrowLeft"), ("ArrowUp", "ArrowUp"),
  ("ArrowDown", "ArrowDown"), ("Enter", "Enter"), (" ", "Space"), ("Backspace", "Backspace"),
  ("Delete", "Delete"), ("Escape", "Escape"), ("Tab", "Tab"), ("a", "KeyA"), ("b", "KeyB"),
  ("c", "KeyC"), ("d", "KeyD"), ("h", "KeyH"), ("i", "KeyI"), ("k", "KeyK"), ("o", "KeyO"),
  ("q", "KeyQ"), ("u", "KeyU"), ("x", "KeyX"), ("y", "KeyY"), ("z", "KeyZ"),
  // Non-Latin layouts: event.key is non-ASCII, matching falls back to code
  ("и", "KeyB"), ("я", "KeyZ"),
]

private final class Recorder {
  var recorded: [(command: String, payload: Any)] = []
}

private func observe<P>(
  _ editor: Editor, _ recorder: Recorder, _ name: String, _ command: LexicalCommand<P>
) -> () -> Void {
  editor.registerCommand(
    command,
    { payload, _ in
      recorder.recorded.append((name, payload))
      return true
    }, COMMAND_PRIORITY_CRITICAL)
}

private func describe(_ payload: Any) -> String {
  if let event = payload as? KeyboardEvent { return "event:\(ObjectIdentifier(event))" }
  if payload is Void { return "()" }
  return "\(payload)"
}

private func runParity(_ isApple: Bool) throws {
  try withIsApple(isApple) {
    let recorder = Recorder()
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "keydown-parity-test",
        register: { editor2, _, _ in
          mergeRegister([
            observe(editor2, recorder, "KEY_ARROW_RIGHT_COMMAND", KEY_ARROW_RIGHT_COMMAND),
            observe(editor2, recorder, "KEY_ARROW_LEFT_COMMAND", KEY_ARROW_LEFT_COMMAND),
            observe(editor2, recorder, "KEY_ARROW_UP_COMMAND", KEY_ARROW_UP_COMMAND),
            observe(editor2, recorder, "KEY_ARROW_DOWN_COMMAND", KEY_ARROW_DOWN_COMMAND),
            observe(editor2, recorder, "MOVE_TO_END", MOVE_TO_END),
            observe(editor2, recorder, "MOVE_TO_START", MOVE_TO_START),
            observe(editor2, recorder, "KEY_ENTER_COMMAND", KEY_ENTER_COMMAND),
            observe(editor2, recorder, "KEY_SPACE_COMMAND", KEY_SPACE_COMMAND),
            observe(editor2, recorder, "INSERT_LINE_BREAK_COMMAND", INSERT_LINE_BREAK_COMMAND),
            observe(editor2, recorder, "KEY_BACKSPACE_COMMAND", KEY_BACKSPACE_COMMAND),
            observe(editor2, recorder, "KEY_DELETE_COMMAND", KEY_DELETE_COMMAND),
            observe(editor2, recorder, "KEY_ESCAPE_COMMAND", KEY_ESCAPE_COMMAND),
            observe(editor2, recorder, "KEY_TAB_COMMAND", KEY_TAB_COMMAND),
            observe(editor2, recorder, "DELETE_CHARACTER_COMMAND", DELETE_CHARACTER_COMMAND),
            observe(editor2, recorder, "DELETE_WORD_COMMAND", DELETE_WORD_COMMAND),
            observe(editor2, recorder, "DELETE_LINE_COMMAND", DELETE_LINE_COMMAND),
            observe(editor2, recorder, "FORMAT_TEXT_COMMAND", FORMAT_TEXT_COMMAND),
            observe(editor2, recorder, "UNDO_COMMAND", UNDO_COMMAND),
            observe(editor2, recorder, "REDO_COMMAND", REDO_COMMAND),
            observe(editor2, recorder, "SELECT_ALL_COMMAND", SELECT_ALL_COMMAND),
            observe(editor2, recorder, "COPY_COMMAND", COPY_COMMAND),
            observe(editor2, recorder, "CUT_COMMAND", CUT_COMMAND),
            observe(editor2, recorder, "KEY_MODIFIER_COMMAND", KEY_MODIFIER_COMMAND),
          ])
        }))
    defer { editor.dispose() }
    for (key, code) in GRID_KEYS {
      for bits in 0..<16 {
        let event = KeyboardEvent(
          key: key, code: code, altKey: bits & 1 != 0, ctrlKey: bits & 2 != 0,
          metaKey: bits & 4 != 0, shiftKey: bits & 8 != 0)
        recorder.recorded = []
        _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, event)
        let label =
          "key=\(key) code=\(code) alt=\(event.altKey) ctrl=\(event.ctrlKey) meta=\(event.metaKey) shift=\(event.shiftKey)"
        let expected = referenceKeyDown(event, isApple)
        let modifierDispatches = recorder.recorded.filter { $0.command == "KEY_MODIFIER_COMMAND" }
        let dispatches = recorder.recorded.filter { $0.command != "KEY_MODIFIER_COMMAND" }
        #expect(modifierDispatches.count == (bits == 0 ? 0 : 1), "\(label)")
        if let expected {
          #expect(dispatches.count == 1, "\(label)")
          #expect(dispatches.first?.command == expected.command, "\(label)")
          let expectedPayload: String
          switch expected.payload {
          case .event: expectedPayload = describe(event)
          case .value(let value): expectedPayload = value
          }
          #expect(dispatches.first.map { describe($0.payload) } == expectedPayload, "\(label)")
          #expect(event.defaultPrevented == expected.preventsDefault, "\(label)")
        } else {
          #expect(dispatches.isEmpty, "\(label)")
          #expect(!event.defaultPrevented, "\(label)")
        }
      }
    }
  }
}

@Suite struct KeyDownDispatchParityTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEventsKeyDown.test.ts::$handleKeyDown dispatch parity (IS_APPLE=false) > dispatches the same commands as the legacy predicate chain"
    ))
  func notApple() throws { try runParity(false) }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEventsKeyDownApple.test.ts::$handleKeyDown dispatch parity (IS_APPLE=true) > dispatches the same commands as the legacy predicate chain"
    ))
  func apple() throws { try runParity(true) }
}
