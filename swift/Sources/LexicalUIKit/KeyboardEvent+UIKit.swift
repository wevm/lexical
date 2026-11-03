#if canImport(UIKit)
  import Lexical
  import UIKit

  // A hardware key press (`UIKey`) as the DOM `KeyboardEvent` Safari on iOS
  // reports: `key` is the named key or the typed character, `code` the
  // physical key.

  private let namedKeys: [UIKeyboardHIDUsage: (key: String, code: String)] = [
    .keyboardReturnOrEnter: ("Enter", "Enter"), .keypadEnter: ("Enter", "NumpadEnter"),
    .keyboardTab: ("Tab", "Tab"), .keyboardSpacebar: (" ", "Space"),
    .keyboardDeleteOrBackspace: ("Backspace", "Backspace"),
    .keyboardDeleteForward: ("Delete", "Delete"), .keyboardEscape: ("Escape", "Escape"),
    .keyboardLeftArrow: ("ArrowLeft", "ArrowLeft"),
    .keyboardRightArrow: ("ArrowRight", "ArrowRight"),
    .keyboardUpArrow: ("ArrowUp", "ArrowUp"), .keyboardDownArrow: ("ArrowDown", "ArrowDown"),
    .keyboardHome: ("Home", "Home"), .keyboardEnd: ("End", "End"),
    .keyboardPageUp: ("PageUp", "PageUp"), .keyboardPageDown: ("PageDown", "PageDown"),
  ]

  /// The DOM `code` of a letter or digit key.
  private func domCode(_ usage: UIKeyboardHIDUsage) -> String {
    let raw = usage.rawValue
    let a = UIKeyboardHIDUsage.keyboardA.rawValue
    let one = UIKeyboardHIDUsage.keyboard1.rawValue
    if (a...(a + 25)).contains(raw) {
      return "Key" + String(UnicodeScalar(UInt8(65 + raw - a)))
    }
    if (one...(one + 8)).contains(raw) { return "Digit\(raw - one + 1)" }
    if usage == .keyboard0 { return "Digit0" }
    return ""
  }

  extension KeyboardEvent {
    /// The `keydown` of a hardware key press.
    public convenience init(_ key: UIKey, timeStamp: Double = 0) {
      let flags = key.modifierFlags
      let named = namedKeys[key.keyCode]
      let character =
        flags.contains(.command) || flags.contains(.control)
        ? key.charactersIgnoringModifiers : key.characters
      self.init(
        "keydown", key: named?.key ?? character, code: named?.code ?? domCode(key.keyCode),
        altKey: flags.contains(.alternate), ctrlKey: flags.contains(.control),
        metaKey: flags.contains(.command), shiftKey: flags.contains(.shift),
        timeStamp: timeStamp)
    }
  }
#endif
