import Lexical
import LexicalTesting
import Testing

// The isExactShortcutMatch cases of LexicalUtils.test.ts, which construct
// DOM KeyboardEvents; `KeyboardEvent` carries the same fields here.

private let ctrl = KeyboardEventModifierMask(ctrlKey: true)

@Suite struct LexicalShortcutMatchTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches by event.key for single-letter"
    ))
  func matchesByKey() {
    #expect(isExactShortcutMatch(KeyboardEvent(key: "z", code: "KeyZ", ctrlKey: true), "z", ctrl))
    #expect(isExactShortcutMatch(KeyboardEvent(key: "Z", code: "KeyZ", ctrlKey: true), "z", ctrl))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches to event.key for ASCII remapped layout (English (US) Dvorak)"
    ))
  func asciiRemappedLayout() {
    let lower = KeyboardEvent(key: "x", code: "KeyB", ctrlKey: true)
    #expect(isExactShortcutMatch(lower, "x", ctrl))
    #expect(!isExactShortcutMatch(lower, "b", ctrl))
    let upper = KeyboardEvent(key: "X", code: "KeyB", ctrlKey: true)
    #expect(isExactShortcutMatch(upper, "x", ctrl))
    #expect(!isExactShortcutMatch(upper, "b", ctrl))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() fallback to event.code for single-letter in event.key via non-English layout"
    ))
  func nonEnglishLayout() {
    #expect(isExactShortcutMatch(KeyboardEvent(key: "я", code: "KeyZ", ctrlKey: true), "z", ctrl))
    #expect(isExactShortcutMatch(KeyboardEvent(key: "Я", code: "KeyZ", ctrlKey: true), "z", ctrl))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches special keys"
    ))
  func specialKeys() {
    for key in ["Enter", "Tab", "Delete"] {
      #expect(isExactShortcutMatch(KeyboardEvent(key: key, ctrlKey: true), key, ctrl))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isExactShortcutMatch() matches optional keys"
    ))
  func optionalKeys() {
    #expect(isExactShortcutMatch(KeyboardEvent(key: "a", ctrlKey: true), "a", ctrl))
    #expect(
      isExactShortcutMatch(
        KeyboardEvent(key: "a", shiftKey: true), "a", KeyboardEventModifierMask(shiftKey: true)))
    #expect(
      isExactShortcutMatch(
        KeyboardEvent(key: "a", metaKey: true), "a", KeyboardEventModifierMask(metaKey: true)))
    #expect(!isExactShortcutMatch(KeyboardEvent(key: "a"), "a", ctrl))
  }
}
