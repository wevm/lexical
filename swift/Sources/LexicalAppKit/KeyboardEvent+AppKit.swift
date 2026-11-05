#if os(macOS)
  import AppKit
  import Lexical

  // An `NSEvent` key press as the DOM `KeyboardEvent` a browser on macOS
  // reports: `key` is the named key or the typed character, `code` the
  // physical key (for non-Latin layouts).

  /// DOM `code` values of the ANSI virtual key codes.
  private let codes: [UInt16: String] = [
    0x00: "KeyA", 0x01: "KeyS", 0x02: "KeyD", 0x03: "KeyF", 0x04: "KeyH", 0x05: "KeyG",
    0x06: "KeyZ", 0x07: "KeyX", 0x08: "KeyC", 0x09: "KeyV", 0x0B: "KeyB", 0x0C: "KeyQ",
    0x0D: "KeyW", 0x0E: "KeyE", 0x0F: "KeyR", 0x10: "KeyY", 0x11: "KeyT", 0x12: "Digit1",
    0x13: "Digit2", 0x14: "Digit3", 0x15: "Digit4", 0x16: "Digit6", 0x17: "Digit5",
    0x18: "Equal", 0x19: "Digit9", 0x1A: "Digit7", 0x1B: "Minus", 0x1C: "Digit8",
    0x1D: "Digit0", 0x1E: "BracketRight", 0x1F: "KeyO", 0x20: "KeyU", 0x21: "BracketLeft",
    0x22: "KeyI", 0x23: "KeyP", 0x24: "Enter", 0x25: "KeyL", 0x26: "KeyJ", 0x27: "Quote",
    0x28: "KeyK", 0x29: "Semicolon", 0x2A: "Backslash", 0x2B: "Comma", 0x2C: "Slash",
    0x2D: "KeyN", 0x2E: "KeyM", 0x2F: "Period", 0x30: "Tab", 0x31: "Space", 0x32: "Backquote",
    0x33: "Backspace", 0x35: "Escape", 0x4C: "NumpadEnter", 0x73: "Home", 0x74: "PageUp",
    0x75: "Delete", 0x77: "End", 0x79: "PageDown", 0x7B: "ArrowLeft", 0x7C: "ArrowRight",
    0x7D: "ArrowDown", 0x7E: "ArrowUp",
  ]

  /// The DOM `key` of keys that don't type a character.
  private let namedKeys: [UInt16: String] = [
    0x24: "Enter", 0x4C: "Enter", 0x30: "Tab", 0x31: " ", 0x33: "Backspace", 0x35: "Escape",
    0x73: "Home", 0x74: "PageUp", 0x75: "Delete", 0x77: "End", 0x79: "PageDown",
    0x7B: "ArrowLeft", 0x7C: "ArrowRight", 0x7D: "ArrowDown", 0x7E: "ArrowUp",
  ]

  extension KeyboardEvent {
    /// The `keydown` of a key press, or `nil` for an event that isn't one.
    public convenience init?(_ event: NSEvent) {
      guard event.type == .keyDown else { return nil }
      let flags = event.modifierFlags
      let key: String
      if let named = namedKeys[event.keyCode] {
        key = named
      } else if flags.contains(.command) {
        // Safari's `key` with Command: Option applies and Shift doesn't
        // (⇧⌘7 is "7", ⌥⌘1 is "¡", ⇧⌘. is "."), which is the event's
        // `characters`.
        key = event.characters ?? ""
      } else if flags.contains(.control) {
        // With Control alone, Shift applies (⌃⇧1 is "!", ⌃⇧Q is "Q").
        key = event.charactersIgnoringModifiers ?? ""
      } else {
        key = event.characters ?? ""
      }
      self.init(
        "keydown", key: key, code: codes[event.keyCode] ?? "", altKey: flags.contains(.option),
        ctrlKey: flags.contains(.control), metaKey: flags.contains(.command),
        shiftKey: flags.contains(.shift), repeat: event.isARepeat,
        timeStamp: event.timestamp * 1000)
    }
  }
#endif
