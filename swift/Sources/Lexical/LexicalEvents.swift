// Input as JS sees it: DOM-shaped events (`KeyboardEvent`, `InputEvent`,
// `ClipboardEvent`, `DragEvent`, ...) for the commands that carry one, the
// modifier masks of LexicalUtils.ts, the shortcut matching of
// LexicalKeyboardShortcuts.ts, and the default KEY_DOWN_COMMAND handler of
// LexicalEvents.ts. The native views build these events from `NSEvent`,
// `UIKey`, the pasteboard and drag sessions, with the DOM `key` and `code`
// values, so handlers ported from JS read the same fields.

/// The platform the keyboard conventions follow (JS `IS_APPLE`): `true` on
/// Apple platforms. Tests may override it per thread with `withIsApple`,
/// since upstream's unit tests run as a non-Apple browser.
public var IS_APPLE: Bool {
  if let override = ThreadContext.current.isAppleOverride { return override }
  #if canImport(Darwin)
    return true
  #else
    return false
  #endif
}

/// Runs `body` with `IS_APPLE` set to `isApple` on this thread.
public func withIsApple<T>(_ isApple: Bool, _ body: () throws -> T) rethrows -> T {
  let context = ThreadContext.current
  let previous = context.isAppleOverride
  context.isAppleOverride = isApple
  defer { context.isAppleOverride = previous }
  return try body()
}

/// `KeyboardEventModifiers`: a `KeyboardEvent`, or anything with its `key`,
/// `code` and modifier fields.
public protocol KeyboardEventModifiers {
  var key: String { get }
  var code: String { get }
  var altKey: Bool { get }
  var ctrlKey: Bool { get }
  var metaKey: Bool { get }
  var shiftKey: Bool { get }
}

/// The `Event` fields handlers use. A class, so `preventDefault()` in one
/// handler is seen by the code that dispatched the event.
open class DOMEvent {
  public let type: String
  public let cancelable: Bool
  public let timeStamp: Double
  public private(set) var defaultPrevented = false
  public private(set) var propagationStopped = false

  public init(_ type: String, cancelable: Bool = true, timeStamp: Double = 0) {
    self.type = type
    self.cancelable = cancelable
    self.timeStamp = timeStamp
  }

  public func preventDefault() {
    if cancelable { defaultPrevented = true }
  }

  public func stopPropagation() { propagationStopped = true }
}

/// A DOM-shaped keyboard event.
public final class KeyboardEvent: DOMEvent, KeyboardEventModifiers {
  public let key: String
  public let code: String
  public let altKey: Bool
  public let ctrlKey: Bool
  public let metaKey: Bool
  public let shiftKey: Bool
  public let `repeat`: Bool
  public let isComposing: Bool

  public init(
    _ type: String = "keydown", key: String = "", code: String = "", altKey: Bool = false,
    ctrlKey: Bool = false, metaKey: Bool = false, shiftKey: Bool = false, repeat: Bool = false,
    isComposing: Bool = false, cancelable: Bool = true, timeStamp: Double = 0
  ) {
    self.key = key
    self.code = code
    self.altKey = altKey
    self.ctrlKey = ctrlKey
    self.metaKey = metaKey
    self.shiftKey = shiftKey
    self.repeat = `repeat`
    self.isComposing = isComposing
    super.init(type, cancelable: cancelable, timeStamp: timeStamp)
  }
}

/// The expected state of one modifier in a `KeyboardEventModifierMask`:
/// pressed (`true`), not pressed (`false`, the default), or `any`.
public enum ModifierExpectation: Equatable, Sendable, ExpressibleByBooleanLiteral {
  case pressed
  case released
  case any

  public init(booleanLiteral value: Bool) { self = value ? .pressed : .released }

  func matches(_ pressed: Bool) -> Bool {
    switch self {
    case .any: true
    case .pressed: pressed
    case .released: !pressed
    }
  }
}

/// `KeyboardEventModifierMask`, with `CONTROL_OR_OTHER_KEY`: which modifier
/// stands in for Control on the other platform (`CONTROL_OR_META` swaps
/// Control and Meta).
public struct KeyboardEventModifierMask: Equatable, Sendable {
  public enum ControlOrOther: Sendable { case metaKey, altKey }

  public var altKey: ModifierExpectation
  public var ctrlKey: ModifierExpectation
  public var metaKey: ModifierExpectation
  public var shiftKey: ModifierExpectation
  public var controlOrOther: ControlOrOther?

  public init(
    altKey: ModifierExpectation = false, ctrlKey: ModifierExpectation = false,
    metaKey: ModifierExpectation = false, shiftKey: ModifierExpectation = false,
    controlOrOther: ControlOrOther? = nil
  ) {
    self.altKey = altKey
    self.ctrlKey = ctrlKey
    self.metaKey = metaKey
    self.shiftKey = shiftKey
    self.controlOrOther = controlOrOther
  }

  /// `{...mask, shiftKey: ...}` and the like.
  public func with(
    altKey: ModifierExpectation? = nil, ctrlKey: ModifierExpectation? = nil,
    metaKey: ModifierExpectation? = nil, shiftKey: ModifierExpectation? = nil
  ) -> Self {
    var mask = self
    if let altKey { mask.altKey = altKey }
    if let ctrlKey { mask.ctrlKey = ctrlKey }
    if let metaKey { mask.metaKey = metaKey }
    if let shiftKey { mask.shiftKey = shiftKey }
    return mask
  }

  subscript(other: ControlOrOther) -> ModifierExpectation {
    get { other == .metaKey ? metaKey : altKey }
    set {
      if other == .metaKey { metaKey = newValue } else { altKey = newValue }
    }
  }
}

/// `CONTROL_OR_META`: Command on Apple platforms, Control elsewhere.
public var CONTROL_OR_META: KeyboardEventModifierMask {
  KeyboardEventModifierMask(
    ctrlKey: IS_APPLE ? false : true, metaKey: IS_APPLE ? true : false, controlOrOther: .metaKey)
}

/// `CONTROL_OR_ALT`: Option on Apple platforms, Control elsewhere.
public var CONTROL_OR_ALT: KeyboardEventModifierMask {
  KeyboardEventModifierMask(
    altKey: IS_APPLE ? true : false, ctrlKey: IS_APPLE ? false : true, controlOrOther: .altKey)
}

/// `keyboardEventMaskForPlatform(mask, isApple)`: the mask as it would be
/// on the other platform, when it is a control-or-other mask.
public func keyboardEventMaskForPlatform(
  _ mask: KeyboardEventModifierMask, _ isApple: Bool
) -> KeyboardEventModifierMask {
  guard let otherKey = mask.controlOrOther, isApple != IS_APPLE else { return mask }
  var result = mask
  result.ctrlKey = mask[otherKey]
  result[otherKey] = mask.ctrlKey
  return result
}

/// `isModifierMatch(event, mask)`.
public func isModifierMatch(
  _ event: some KeyboardEventModifiers, _ mask: KeyboardEventModifierMask
) -> Bool {
  mask.altKey.matches(event.altKey) && mask.ctrlKey.matches(event.ctrlKey)
    && mask.shiftKey.matches(event.shiftKey) && mask.metaKey.matches(event.metaKey)
}

private func isASCIIKey(_ key: String) -> Bool {
  key.utf16.count == 1 && key.utf16.first! <= 127
}

/// `isExactShortcutMatch(event, expectedKey, mask)`: whether `event` is
/// `expectedKey` (case-insensitive) with exactly the modifiers of `mask`,
/// falling back to `event.code` for non-ASCII layouts.
public func isExactShortcutMatch(
  _ event: some KeyboardEventModifiers, _ expectedKey: String,
  _ mask: KeyboardEventModifierMask
) -> Bool {
  if !isModifierMatch(event, mask) { return false }
  if event.key.lowercased() == expectedKey.lowercased() { return true }
  if expectedKey.utf16.count > 1 { return false }
  if isASCIIKey(event.key) { return false }
  if event.code.hasPrefix("Digit"), expectedKey.count == 1,
    expectedKey.unicodeScalars.allSatisfy({ ("0"..."9").contains($0) })
  {
    return event.code == "Digit\(expectedKey)"
  }
  return event.code == "Key" + expectedKey.uppercased()
}

public func isModifier(_ event: some KeyboardEventModifiers) -> Bool {
  event.ctrlKey || event.shiftKey || event.altKey || event.metaKey
}

public func isBackspace(_ event: some KeyboardEventModifiers) -> Bool {
  event.key == "Backspace"
}

public func isEscape(_ event: some KeyboardEventModifiers) -> Bool { event.key == "Escape" }

public func isDelete(_ event: some KeyboardEventModifiers) -> Bool { event.key == "Delete" }

public func isSelectAll(_ event: some KeyboardEventModifiers) -> Bool {
  isExactShortcutMatch(event, "a", CONTROL_OR_META)
}

// MARK: - Keyboard commands

public let KEY_DOWN_COMMAND = LexicalCommand<KeyboardEvent>("KEYDOWN_COMMAND")
public let KEY_ARROW_RIGHT_COMMAND = LexicalCommand<KeyboardEvent>("KEY_ARROW_RIGHT_COMMAND")
public let MOVE_TO_END = LexicalCommand<KeyboardEvent>("MOVE_TO_END")
public let KEY_ARROW_LEFT_COMMAND = LexicalCommand<KeyboardEvent>("KEY_ARROW_LEFT_COMMAND")
public let MOVE_TO_START = LexicalCommand<KeyboardEvent>("MOVE_TO_START")
public let KEY_ARROW_UP_COMMAND = LexicalCommand<KeyboardEvent>("KEY_ARROW_UP_COMMAND")
public let KEY_ARROW_DOWN_COMMAND = LexicalCommand<KeyboardEvent>("KEY_ARROW_DOWN_COMMAND")
public let KEY_ENTER_COMMAND = LexicalCommand<KeyboardEvent?>("KEY_ENTER_COMMAND")
public let KEY_SPACE_COMMAND = LexicalCommand<KeyboardEvent>("KEY_SPACE_COMMAND")
public let KEY_BACKSPACE_COMMAND = LexicalCommand<KeyboardEvent>("KEY_BACKSPACE_COMMAND")
public let KEY_ESCAPE_COMMAND = LexicalCommand<KeyboardEvent>("KEY_ESCAPE_COMMAND")
public let KEY_DELETE_COMMAND = LexicalCommand<KeyboardEvent>("KEY_DELETE_COMMAND")
public let KEY_TAB_COMMAND = LexicalCommand<KeyboardEvent>("KEY_TAB_COMMAND")
public let SELECT_ALL_COMMAND = LexicalCommand<KeyboardEvent>("SELECT_ALL_COMMAND")
/// Deprecated in JS v0.31.0: use `KEY_DOWN_COMMAND` and check the modifiers.
public let KEY_MODIFIER_COMMAND = LexicalCommand<KeyboardEvent>("KEY_MODIFIER_COMMAND")

// MARK: - Keyboard shortcuts (LexicalKeyboardShortcuts.ts)

/// `KeyboardShortcutMatch`: a key and its modifiers.
public protocol KeyboardShortcutMatch {
  var key: String { get }
  var modifiers: KeyboardEventModifierMask { get }
  /// The key without Shift, for display (`unshiftedKey`).
  var unshiftedKey: String? { get }
}

/// `KeyboardShortcut`: a shortcut that dispatches `command` with the event.
public struct KeyboardShortcut: KeyboardShortcutMatch {
  public var key: String
  public var modifiers: KeyboardEventModifierMask
  public var unshiftedKey: String?
  public var command: LexicalCommand<KeyboardEvent>
  public var description: String?
  /// `$disabled(selection, editor)`: skip the shortcut when it returns true.
  public var disabled: ((BaseSelection?, Editor) throws -> Bool)?
  /// `$dispatch(command, event, $next, editor)`: replaces dispatching
  /// `command`; `next` dispatches it.
  public var dispatch:
    ((LexicalCommand<KeyboardEvent>, KeyboardEvent, () throws -> Bool, Editor) throws -> Bool)?
  public var bubbleFromNestedEditors: Bool

  public init(
    key: String, modifiers: KeyboardEventModifierMask = KeyboardEventModifierMask(),
    unshiftedKey: String? = nil, command: LexicalCommand<KeyboardEvent>,
    description: String? = nil,
    disabled: ((BaseSelection?, Editor) throws -> Bool)? = nil,
    dispatch: (
      (LexicalCommand<KeyboardEvent>, KeyboardEvent, () throws -> Bool, Editor) throws -> Bool
    )? = nil,
    bubbleFromNestedEditors: Bool = false
  ) {
    self.key = key
    self.modifiers = modifiers
    self.unshiftedKey = unshiftedKey
    self.command = command
    self.description = description
    self.disabled = disabled
    self.dispatch = dispatch
    self.bubbleFromNestedEditors = bubbleFromNestedEditors
  }
}

private func getEventModifierBits(_ event: some KeyboardEventModifiers) -> Int {
  (event.altKey ? 1 : 0) | (event.ctrlKey ? 2 : 0) | (event.metaKey ? 4 : 0)
    | (event.shiftKey ? 8 : 0)
}

private func getMaskModifierBits(_ mask: KeyboardEventModifierMask) -> [Int] {
  var combos = [0]
  for (expected, bit) in [
    (mask.altKey, 1), (mask.ctrlKey, 2), (mask.metaKey, 4), (mask.shiftKey, 8),
  ] {
    switch expected {
    case .any: combos += combos.map { $0 | bit }
    case .pressed: combos = combos.map { $0 | bit }
    case .released: break
    }
  }
  return combos
}

/// `CompiledKeyboardShortcuts`: shortcuts indexed by key and code.
public final class CompiledKeyboardShortcuts<S: KeyboardShortcutMatch> {
  private var byKey: [String: [S]] = [:]
  private var byCode: [String: [S]] = [:]

  public init() {}

  @discardableResult
  public func add(_ shortcut: S) throws -> Self {
    let key = shortcut.key
    try invariant(!key.isEmpty, "KeyboardShortcutMatch: key must be non-empty")
    let lowerKey = key.lowercased()
    for bits in getMaskModifierBits(shortcut.modifiers) {
      byKey["\(bits):\(lowerKey)", default: []].append(shortcut)
      if key.utf16.count == 1 {
        let scalar = key.unicodeScalars.first!
        if ("0"..."9").contains(scalar) {
          byCode["\(bits):Digit\(key)", default: []].append(shortcut)
        } else if lowerKey.unicodeScalars.first.map({ ("a"..."z").contains($0) }) == true {
          byCode["\(bits):Key\(lowerKey.uppercased())", default: []].append(shortcut)
        }
      }
    }
    return self
  }

  public func matches(_ event: some KeyboardEventModifiers) -> [S] {
    let key = event.key
    if key.isEmpty { return [] }
    let bits = getEventModifierBits(event)
    var matches = byKey["\(bits):\(key.lowercased())"] ?? []
    if !byCode.isEmpty && !isASCIIKey(key) {
      if let byCode = byCode["\(bits):\(event.code)"] {
        matches += byCode
      }
    }
    return matches
  }

  public func match(_ event: some KeyboardEventModifiers) -> S? { matches(event).first }
}

/// `compileKeyboardShortcuts(shortcuts)`.
public func compileKeyboardShortcuts<S: KeyboardShortcutMatch>(
  _ shortcuts: some Sequence<S>
) throws -> CompiledKeyboardShortcuts<S> {
  let compiled = CompiledKeyboardShortcuts<S>()
  for shortcut in shortcuts {
    try compiled.add(shortcut)
  }
  return compiled
}

// MARK: - Other DOM-shaped events

/// One file of a `DataTransfer`.
public struct DataTransferFile: Equatable, Sendable {
  public var name: String
  public var type: String
  public var data: [UInt8]

  public init(name: String, type: String, data: [UInt8] = []) {
    self.name = name
    self.type = type
    self.data = data
  }
}

/// `DataTransfer`: the data of a clipboard or drag operation, by MIME type,
/// in the order it was set.
public final class DataTransfer {
  private var entries: [(type: String, data: String)] = []
  public var files: [DataTransferFile] = []
  public var dropEffect = "none"
  public var effectAllowed = "uninitialized"

  public init() {}

  /// The MIME types set so far, in order (`"Files"` last when there are
  /// files, as browsers report it).
  public var types: [String] { entries.map(\.type) + (files.isEmpty ? [] : ["Files"]) }

  /// `getData(type)`: the data for `type`, or `""`.
  public func getData(_ type: String) -> String {
    entries.first { $0.type == type.lowercased() }?.data ?? ""
  }

  public func setData(_ type: String, _ data: String) {
    let type = type.lowercased()
    if let index = entries.firstIndex(where: { $0.type == type }) {
      entries[index].data = data
    } else {
      entries.append((type, data))
    }
  }

  public func clearData(_ type: String? = nil) {
    if let type {
      entries.removeAll { $0.type == type.lowercased() }
    } else {
      entries.removeAll()
    }
  }
}

/// `ClipboardEvent`: copy, cut or paste, with its data.
public final class ClipboardEvent: DOMEvent {
  public let clipboardData: DataTransfer?

  public init(
    _ type: String, clipboardData: DataTransfer? = DataTransfer(), cancelable: Bool = true
  ) {
    self.clipboardData = clipboardData
    super.init(type, cancelable: cancelable)
  }
}

/// `InputEvent`: a `beforeinput` or `input` event.
public final class InputEvent: DOMEvent {
  public let inputType: String
  public let data: String?
  public let dataTransfer: DataTransfer?
  public let isComposing: Bool

  public init(
    _ type: String = "beforeinput", inputType: String, data: String? = nil,
    dataTransfer: DataTransfer? = nil, isComposing: Bool = false, cancelable: Bool = true
  ) {
    self.inputType = inputType
    self.data = data
    self.dataTransfer = dataTransfer
    self.isComposing = isComposing
    super.init(type, cancelable: cancelable)
  }
}

/// `CompositionEvent`: IME composition start or end.
public final class CompositionEvent: DOMEvent {
  public let data: String

  public init(_ type: String, data: String = "") {
    self.data = data
    super.init(type, cancelable: false)
  }
}

/// `MouseEvent` (and the `PointerEvent` fields Lexical reads), with the
/// node the view hit in place of the DOM target.
public class MouseEvent: DOMEvent, KeyboardEventModifiers {
  public let clientX: Double
  public let clientY: Double
  public let button: Int
  public let detail: Int
  public let altKey: Bool
  public let ctrlKey: Bool
  public let metaKey: Bool
  public let shiftKey: Bool
  /// The key of the node under the pointer, as the view resolves it.
  public let targetKey: NodeKey?
  public var key: String { "" }
  public var code: String { "" }

  public init(
    _ type: String = "click", clientX: Double = 0, clientY: Double = 0, button: Int = 0,
    detail: Int = 1, altKey: Bool = false, ctrlKey: Bool = false, metaKey: Bool = false,
    shiftKey: Bool = false, targetKey: NodeKey? = nil, cancelable: Bool = true
  ) {
    self.clientX = clientX
    self.clientY = clientY
    self.button = button
    self.detail = detail
    self.altKey = altKey
    self.ctrlKey = ctrlKey
    self.metaKey = metaKey
    self.shiftKey = shiftKey
    self.targetKey = targetKey
    super.init(type, cancelable: cancelable)
  }
}

/// `DragEvent`: a drag or drop, with its data.
public final class DragEvent: MouseEvent {
  public let dataTransfer: DataTransfer?

  public init(
    _ type: String, dataTransfer: DataTransfer? = DataTransfer(), clientX: Double = 0,
    clientY: Double = 0, targetKey: NodeKey? = nil
  ) {
    self.dataTransfer = dataTransfer
    super.init(type, clientX: clientX, clientY: clientY, targetKey: targetKey)
  }
}

/// `FocusEvent`.
public final class FocusEvent: DOMEvent {
  public init(_ type: String) { super.init(type, cancelable: false) }
}

/// `PasteCommandType`: `ClipboardEvent | InputEvent | KeyboardEvent`.
public enum PasteCommandType {
  case clipboard(ClipboardEvent)
  case input(InputEvent)
  case keyboard(KeyboardEvent)

  public var event: DOMEvent {
    switch self {
    case .clipboard(let e): e
    case .input(let e): e
    case .keyboard(let e): e
    }
  }
}

/// The payload of `COPY_COMMAND` and `CUT_COMMAND`:
/// `ClipboardEvent | KeyboardEvent` (or `nil`).
public enum ClipboardOrKeyboardEvent {
  case clipboard(ClipboardEvent)
  case keyboard(KeyboardEvent)

  public var event: DOMEvent {
    switch self {
    case .clipboard(let e): e
    case .keyboard(let e): e
    }
  }
}

public let CLICK_COMMAND = LexicalCommand<MouseEvent>("CLICK_COMMAND")
public let BEFORE_INPUT_COMMAND = LexicalCommand<InputEvent>("BEFORE_INPUT_COMMAND")
public let INPUT_COMMAND = LexicalCommand<InputEvent>("INPUT_COMMAND")
public let COMPOSITION_START_COMMAND = LexicalCommand<CompositionEvent>(
  "COMPOSITION_START_COMMAND")
public let COMPOSITION_END_COMMAND = LexicalCommand<CompositionEvent>("COMPOSITION_END_COMMAND")
public let PASTE_COMMAND = LexicalCommand<PasteCommandType>("PASTE_COMMAND")
public let DROP_COMMAND = LexicalCommand<DragEvent>("DROP_COMMAND")
public let DRAGSTART_COMMAND = LexicalCommand<DragEvent>("DRAGSTART_COMMAND")
public let DRAGOVER_COMMAND = LexicalCommand<DragEvent>("DRAGOVER_COMMAND")
public let DRAGEND_COMMAND = LexicalCommand<DragEvent>("DRAGEND_COMMAND")
public let COPY_COMMAND = LexicalCommand<ClipboardOrKeyboardEvent?>("COPY_COMMAND")
public let CUT_COMMAND = LexicalCommand<ClipboardOrKeyboardEvent?>("CUT_COMMAND")
public let FOCUS_COMMAND = LexicalCommand<FocusEvent>("FOCUS_COMMAND")
public let BLUR_COMMAND = LexicalCommand<FocusEvent>("BLUR_COMMAND")

// MARK: - The default KEY_DOWN_COMMAND handler (LexicalEvents.ts)

/// `KeyDownShortcut`: a key-down binding of `$handleKeyDown`.
struct KeyDownShortcut: KeyboardShortcutMatch {
  let key: String
  let modifiers: KeyboardEventModifierMask
  var unshiftedKey: String? { nil }
  let onMatch: (KeyboardEvent, Editor) throws -> Void
}

/// The editor's input state that the key-down bindings set (JS
/// `editor._inputState`).
package struct InputState {
  package var isInsertLineBreak = false
  package var lastKeyCode: String?
}

private let ANY_MODIFIERS = KeyboardEventModifierMask(
  altKey: .any, ctrlKey: .any, metaKey: .any, shiftKey: .any)
private let CTRL_KEY = KeyboardEventModifierMask(ctrlKey: true)
private let META_KEY = KeyboardEventModifierMask(metaKey: true)
private let SHIFT_KEY_ANY = KeyboardEventModifierMask(shiftKey: .any)
private let ALT_SHIFT_KEY_ANY = KeyboardEventModifierMask(altKey: .any, shiftKey: .any)

/// `buildKeyDownShortcuts()`. The bindings JS adds only for Chrome's text
/// replacement, iOS's virtual Shift key and inputs embedded in a node's DOM
/// have no native counterpart and are left out.
func buildKeyDownShortcuts() -> [KeyDownShortcut] {
  func dispatch(
    _ key: String, _ modifiers: KeyboardEventModifierMask,
    _ command: LexicalCommand<KeyboardEvent>
  ) -> KeyDownShortcut {
    KeyDownShortcut(key: key, modifiers: modifiers) { event, editor in
      _ = try Lexical.dispatchCommand(editor, command, event)
    }
  }
  func prevent<T>(
    _ key: String, _ modifiers: KeyboardEventModifierMask, _ command: LexicalCommand<T>,
    _ payload: T
  ) -> KeyDownShortcut {
    KeyDownShortcut(key: key, modifiers: modifiers) { event, editor in
      event.preventDefault()
      _ = try Lexical.dispatchCommand(editor, command, payload)
    }
  }
  func enter(_ modifiers: KeyboardEventModifierMask, _ isInsertLineBreak: Bool) -> KeyDownShortcut {
    KeyDownShortcut(key: "Enter", modifiers: modifiers) { event, editor in
      editor._inputState.isInsertLineBreak = isInsertLineBreak
      _ = try Lexical.dispatchCommand(editor, KEY_ENTER_COMMAND, event)
      if event.defaultPrevented {
        editor._inputState.isInsertLineBreak = false
      }
    }
  }
  func copyOrCut(
    _ key: String, _ command: LexicalCommand<ClipboardOrKeyboardEvent?>
  )
    -> KeyDownShortcut
  {
    KeyDownShortcut(key: key, modifiers: CONTROL_OR_META) { event, editor in
      // Only a RangeSelection can use the native cut and copy.
      let prevSelection = editor._editorState._selection
      if prevSelection != nil, !(prevSelection is RangeSelection) {
        event.preventDefault()
        _ = try Lexical.dispatchCommand(editor, command, .keyboard(event))
      }
    }
  }
  var shortcuts = [
    dispatch("ArrowRight", SHIFT_KEY_ANY, KEY_ARROW_RIGHT_COMMAND),
    dispatch("ArrowLeft", SHIFT_KEY_ANY, KEY_ARROW_LEFT_COMMAND),
    dispatch("ArrowUp", ALT_SHIFT_KEY_ANY, KEY_ARROW_UP_COMMAND),
    dispatch("ArrowDown", ALT_SHIFT_KEY_ANY, KEY_ARROW_DOWN_COMMAND),
    enter(ANY_MODIFIERS.with(shiftKey: true), true),
    enter(ANY_MODIFIERS.with(shiftKey: false), false),
    dispatch(" ", ANY_MODIFIERS, KEY_SPACE_COMMAND),
    dispatch("Backspace", SHIFT_KEY_ANY, KEY_BACKSPACE_COMMAND),
    dispatch("Escape", ANY_MODIFIERS, KEY_ESCAPE_COMMAND),
    dispatch("Delete", KeyboardEventModifierMask(), KEY_DELETE_COMMAND),
    prevent("Backspace", CONTROL_OR_ALT, DELETE_WORD_COMMAND, true),
    prevent("Delete", CONTROL_OR_ALT, DELETE_WORD_COMMAND, false),
    prevent("b", CONTROL_OR_META, FORMAT_TEXT_COMMAND, "bold"),
    prevent("u", CONTROL_OR_META, FORMAT_TEXT_COMMAND, "underline"),
    prevent("i", CONTROL_OR_META, FORMAT_TEXT_COMMAND, "italic"),
    dispatch("Tab", SHIFT_KEY_ANY, KEY_TAB_COMMAND),
    prevent("z", CONTROL_OR_META, UNDO_COMMAND, ()),
    prevent("z", CONTROL_OR_META.with(shiftKey: true), REDO_COMMAND, ()),
  ]
  if IS_APPLE {
    shortcuts += [
      prevent("o", CTRL_KEY, INSERT_LINE_BREAK_COMMAND, true),
      dispatch("ArrowLeft", SHIFT_KEY_ANY.with(metaKey: true), MOVE_TO_START),
      dispatch("ArrowRight", SHIFT_KEY_ANY.with(metaKey: true), MOVE_TO_END),
      prevent("h", CTRL_KEY, DELETE_CHARACTER_COMMAND, true),
      prevent("d", CTRL_KEY, DELETE_CHARACTER_COMMAND, false),
      prevent("Backspace", META_KEY, DELETE_LINE_COMMAND, true),
      prevent("Delete", META_KEY, DELETE_LINE_COMMAND, false),
      prevent("k", CTRL_KEY, DELETE_LINE_COMMAND, false),
    ]
  } else {
    shortcuts += [
      dispatch("Home", SHIFT_KEY_ANY, MOVE_TO_START),
      dispatch("End", SHIFT_KEY_ANY, MOVE_TO_END),
      prevent("y", CTRL_KEY, REDO_COMMAND, ()),
    ]
  }
  shortcuts += [
    KeyDownShortcut(key: "a", modifiers: CONTROL_OR_META) { event, editor in
      event.preventDefault()
      _ = try Lexical.dispatchCommand(editor, SELECT_ALL_COMMAND, event)
    },
    copyOrCut("c", COPY_COMMAND),
    copyOrCut("x", CUT_COMMAND),
  ]
  return shortcuts
}

/// `$handleKeyDown`: turns a key-down into the command it stands for.
/// Always reports the event as handled, as in JS.
func handleKeyDown(_ event: KeyboardEvent, _: Editor) throws -> Bool {
  let editor = try getActiveEditor()
  if event.key.isEmpty { return true }
  let isApple = IS_APPLE
  let compiled: CompiledKeyboardShortcuts<KeyDownShortcut>
  if let cached = editor._keyDownShortcuts, cached.isApple == isApple {
    compiled = cached.shortcuts
  } else {
    compiled = try compileKeyboardShortcuts(buildKeyDownShortcuts())
    editor._keyDownShortcuts = (isApple, compiled)
  }
  editor._inputState.lastKeyCode = event.key
  if let shortcut = compiled.match(event) {
    try shortcut.onMatch(event, editor)
  }
  if isModifier(event) {
    _ = try editor.dispatchCommand(KEY_MODIFIER_COMMAND, event)
  }
  return true
}

/// `registerDefaultCommandHandlers(editor)`: the handlers every editor
/// has. The input and composition handlers come with the native input
/// layer.
func registerDefaultCommandHandlers(_ editor: Editor) {
  _ = editor.registerCommand(KEY_DOWN_COMMAND, handleKeyDown, COMMAND_PRIORITY_EDITOR)
}
