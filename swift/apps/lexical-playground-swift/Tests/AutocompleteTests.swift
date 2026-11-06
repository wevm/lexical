import Testing

@testable import LexicalPlayground

/// The bundled dictionaries answer as the web playground's do for the same
/// prefixes (checked in Chromium against `AutocompleteExtension`).
@MainActor
struct AutocompleteTests {
  @Test func suggestsAsTheWebDoes() {
    #expect(detectLanguage("infor") == "en")
    #expect(detectLanguage("사용") == "ko")
    #expect(detectLanguage("かな\u{200B}") == "ja")
    #expect(AutocompleteDictionaries.dictionary("en")?.query("infor") == "mation")
    // English waits for four characters, Korean for two.
    #expect(AutocompleteDictionaries.dictionary("en")?.query("inf") == nil)
    #expect(AutocompleteDictionaries.dictionary("ko")?.query("사용") == "권")
    #expect(AutocompleteDictionaries.dictionary("ja") == nil)
  }

  @Test func picksTheEarliestListedWord() {
    let dictionary = createWordlistDictionary(["testing", "tea", "testimonials", "Test"])
    #expect(dictionary.query("TES") == "ting")
    #expect(dictionary.query("tea") == nil)
    #expect(dictionary.query("t") == nil)
  }
}

#if os(macOS)
  import AppKit
  import Lexical
  import LexicalAppKit
  import LexicalExtension
  import LexicalRichText
  import LexicalTextKit

  private typealias TextView = LexicalNSTextView
#else
  import Lexical
  import LexicalExtension
  import LexicalRichText
  import LexicalTextKit
  import LexicalUIKit
  import UIKit

  private typealias TextView = LexicalUITextView
#endif

/// The ghost during an IME composition (`tryCompositionSuggestion`), in a
/// focused text view: a pause in the marked text looks the composed word
/// up, and accepting it ends the composition without breaking it.
@MainActor
struct AutocompleteCompositionTests {
  private func makeView() throws -> (TextView, [AnyObject]) {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/AutocompleteTests",
        dependencies: [RichTextExtension, AutocompleteExtension]))
    #if os(macOS)
      let view = TextView(editor: editor, frame: NSRect(x: 0, y: 0, width: 400, height: 300))
      let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled],
        backing: .buffered, defer: false)
      window.contentView = view
      window.makeFirstResponder(view)
    #else
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
      let view = TextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      _ = view.becomeFirstResponder()
    #endif
    // Draws the ghost in the view, as the playground does.
    let host = AutocompleteGhostHost(editor: editor)
    host.attach(to: view)
    return (view, [window, host])
  }

  private func ghost(_ view: TextView) throws -> String? {
    try getExtensionDependencyFromEditor(view.editor, AutocompleteExtension).output.ghost.peek()?
      .text
  }

  private func text(_ view: TextView) -> String { view.controller.document.string }

  /// The marked text "사용", left alone past the idle wait and the lookup.
  private func compose(_ view: TextView) async throws {
    try view.controller.setMarkedText("ㅅ", selectedRange: TextKitRange(location: 1, length: 0))
    try view.controller.setMarkedText("사", selectedRange: TextKitRange(location: 1, length: 0))
    try view.controller.setMarkedText("사용", selectedRange: TextKitRange(location: 2, length: 0))
    #expect(try ghost(view) == nil)
    try await Task.sleep(for: .milliseconds(700))
  }

  @Test func suggestsWhileComposingAndAcceptsMidComposition() async throws {
    let (view, window) = try makeView()
    try await compose(view)
    #expect(view.editor.isComposing())
    #expect(try ghost(view)?.hasPrefix("권 ") == true)
    // The ghost is drawn after the marked text, outside the model.
    #expect(text(view).hasPrefix("사용권 "))
    #expect(try view.editor.read { try getRoot().getTextContent() } == "사용")
    // Tab while the IME still composes (the web's mid-composition path).
    #expect(try view.editor.dispatchCommand(KEY_TAB_COMMAND, KeyboardEvent(key: "Tab")))
    #expect(!view.editor.isComposing())
    #expect(view.controller.markedRange == nil)
    #expect(text(view) == "사용권")
    #expect(try ghost(view) == nil)
    // The next syllable starts a composition of its own after the word.
    try view.controller.setMarkedText("ㄱ", selectedRange: TextKitRange(location: 1, length: 0))
    try view.controller.unmarkText()
    #expect(text(view) == "사용권ㄱ")
    _ = window
  }

  @Test func keepsTheSuggestionWhenTheCompositionCommits() async throws {
    let (view, window) = try makeView()
    try await compose(view)
    // The IME commits the word (as Korean does on Tab) and then the key
    // reaches the editor.
    try view.controller.unmarkText()
    #expect(!view.editor.isComposing())
    try await Task.sleep(for: .milliseconds(500))
    #expect(try ghost(view)?.hasPrefix("권 ") == true)
    #expect(try view.controller.keyDown(KeyboardEvent(key: "Tab", code: "Tab")))
    #expect(text(view) == "사용권")
    _ = window
  }
}
