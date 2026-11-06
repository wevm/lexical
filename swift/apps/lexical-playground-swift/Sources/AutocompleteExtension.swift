import Foundation
import Lexical
import LexicalExtension
import LexicalSelection
import LexicalTextKit

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias GhostTextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias GhostTextView = LexicalUITextView
#endif

// Port of packages/lexical-playground/src/plugins/AutocompleteExtension: at
// the end of a word, a simulated server suggests the rest of it after
// 200 ms, drawn as grey ghost text after the caret; Tab or the right arrow
// (a swipe right on iOS) accepts it.
//
// JS appends the ghost as an unmanaged `<span>` inside the text node's DOM;
// the text view shows it the same way, as a `TextKitSuffix` after the
// node (`AutocompleteGhostHost`): in the text layout, so the text after it
// moves aside and a long ghost wraps, and outside the editor state and the
// history. The dictionaries are upstream's English and Korean ones, picked
// by `detectLanguage`; while an IME composes, a pause of
// `compositionIdleDebounce` looks up the word being composed.

/// `AutocompleteDictionary`.
struct AutocompleteDictionary: Sendable {
  /// The extension never queries a shorter prefix.
  let minPrefixLength: Int
  /// The rest of the suggested word for `prefix`, or `nil`.
  let query: @Sendable (String) -> String?
}

/// `createWordlistDictionary(words, {minPrefixLength, caseSensitive})`:
/// the earliest-listed word that starts with the prefix and is longer than
/// it. As upstream, the words are indexed in the order of their folded
/// UTF-16 text, so the words sharing a prefix are one block that a binary
/// search finds.
func createWordlistDictionary(
  _ words: [String], minPrefixLength: Int = 2, caseSensitive: Bool = false
) -> AutocompleteDictionary {
  let fold: @Sendable (String) -> [UInt16] = {
    Array((caseSensitive ? $0 : $0.lowercased()).utf16)
  }
  let folded = words.map(fold)
  let order = folded.indices.sorted {
    folded[$0].lexicographicallyPrecedes(folded[$1]) || (folded[$0] == folded[$1] && $0 < $1)
  }
  return AutocompleteDictionary(minPrefixLength: minPrefixLength) { prefix in
    let length = prefix.utf16.count
    guard length >= minPrefixLength else { return nil }
    let needle = fold(prefix)
    // The first position whose folded word is not before the needle.
    var (lo, hi) = (0, order.count)
    while lo < hi {
      let mid = (lo + hi) / 2
      if folded[order[mid]].lexicographicallyPrecedes(needle) { lo = mid + 1 } else { hi = mid }
    }
    var best: Int?
    for k in lo..<order.count {
      let index = order[k]
      guard folded[index].starts(with: needle) else { break }
      if words[index].utf16.count > length, best.map({ index < $0 }) ?? true { best = index }
    }
    return best.map { String(decoding: words[$0].utf16.dropFirst(length), as: UTF16.self) }
  }
}

/// `detectLanguage`: the script of the last code point that isn't
/// zero-width or a control: `ko` for Hangul, `ja` for kana and CJK
/// ideographs, `en` otherwise.
func detectLanguage(_ text: String) -> String {
  for scalar in text.unicodeScalars.reversed() {
    let cp = scalar.value
    if cp < 0x20 || (0x200B...0x200F).contains(cp) || (0x202A...0x202E).contains(cp)
      || cp == 0xFEFF
    {
      continue
    }
    if (0xAC00...0xD7AF).contains(cp) || (0x1100...0x11FF).contains(cp)
      || (0x3130...0x318F).contains(cp) || (0xA960...0xA97F).contains(cp)
    {
      return "ko"
    }
    if (0x3040...0x30FF).contains(cp) || (0x31F0...0x31FF).contains(cp)
      || (0x3400...0x4DBF).contains(cp) || (0x4E00...0x9FFF).contains(cp)
      || (0xF900...0xFAFF).contains(cp) || (0x20000...0x2FA1F).contains(cp)
    {
      return "ja"
    }
    return "en"
  }
  return "en"
}

/// `extractTrailingWord`: the last run of non-whitespace, after trailing
/// whitespace (JS's Unicode `\s`, which includes NBSP and U+3000).
func extractTrailingWord(_ text: String) -> String {
  let isSpace = { (s: Unicode.Scalar) in s.properties.isWhitespace || s == "\u{FEFF}" }
  var scalars = Substring(text).unicodeScalars
  while let last = scalars.last, isSpace(last) { scalars.removeLast() }
  var start = scalars.endIndex
  while start > scalars.startIndex, !isSpace(scalars[scalars.index(before: start)]) {
    start = scalars.index(before: start)
  }
  return String(scalars[start...])
}

/// `$search`: the word before a collapsed caret at the end of a simple
/// text node.
private func search(_ selection: BaseSelection?) throws -> (Bool, String) {
  guard let selection = selection as? RangeSelection, selection.isCollapsed(),
    let node = try selection.getNodes().first as? TextNode, try node.isSimpleText(),
    try isAtNodeEnd(selection.anchor)
  else { return (false, "") }
  let text = try node.getTextContent()
  let word = text.split(separator: " ", omittingEmptySubsequences: false).last ?? ""
  return word.isEmpty ? (false, "") : (true, String(word))
}

/// Latency (seconds) of the simulated autocomplete server.
private let QUERY_LATENCY: TimeInterval = 0.2

private func formatSuggestionText(_ suggestion: String) -> String {
  #if os(iOS)
    "\(suggestion) (SWIPE \u{2B95})"
  #else
    "\(suggestion) (TAB)"
  #endif
}

/// `defaultDictionaries`: English (upstream's `dictionaries/english.ts`,
/// as `autocomplete-english.txt`) and Korean (`dictionaries/korean.ts`,
/// as `autocomplete-korean.txt`, 217 KB), each loaded and indexed on first
/// use, by the language `detectLanguage` gives.
enum AutocompleteDictionaries {
  static let english = load("autocomplete-english", minPrefixLength: 4)
  static let korean = load("autocomplete-korean", minPrefixLength: 2)

  static func dictionary(_ language: String) -> AutocompleteDictionary? {
    switch language {
    case "en": english
    case "ko": korean
    default: nil
    }
  }

  private static func load(_ name: String, minPrefixLength: Int) -> AutocompleteDictionary {
    let words =
      Bundle.main.url(forResource: name, withExtension: "txt")
      .flatMap { try? String(contentsOf: $0, encoding: .utf8) }?
      .split(separator: "\n").map(String.init) ?? []
    return createWordlistDictionary(words, minPrefixLength: minPrefixLength)
  }
}

/// A callback the extension, whose state lives on the main thread, hands
/// to the main queue.
private final class MainThreadCallback: @unchecked Sendable {
  let callback: () -> Void
  init(_ callback: @escaping () -> Void) { self.callback = callback }
}

/// Where the ghost is drawn: after the text of `key`.
struct AutocompleteGhost: Equatable {
  let key: NodeKey
  let text: String
}

struct AutocompleteConfig {
  var disabled = false
  /// `compositionIdleDebounceMs`, in seconds: while an IME composes, a
  /// suggestion is looked up after this long without a change to the
  /// marked text (0 turns it off).
  var compositionIdleDebounce: TimeInterval = 0.2
}

struct AutocompleteOutput {
  let disabled: Signal<Bool>
  /// The ghost to draw, `syncGhost`'s argument.
  let ghost: Signal<AutocompleteGhost?>
}

/// Whether the editor's text view has the focus (`isEditorFocused`). The
/// extension runs on the main thread, as the view does.
private func isEditorFocused(_ editor: Editor) -> Bool {
  nonisolated(unsafe) let editor = editor
  return MainActor.assumeIsolated {
    guard let view = editor.getRootElement() as? GhostTextView else { return false }
    #if os(macOS)
      return view.window?.firstResponder === view
    #else
      return view.isFirstResponder
    #endif
  }
}

/// Runs `body` on the main queue after `delay`, unless cancelled first.
private func schedule(_ delay: TimeInterval, _ body: @escaping () -> Void) -> DispatchWorkItem {
  let callback = MainThreadCallback(body)
  let item = DispatchWorkItem { MainActor.assumeIsolated { callback.callback() } }
  DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
  return item
}

/// `AutocompleteExtension`.
let AutocompleteExtension = defineExtension(
  name: "@lexical/playground/autocomplete", config: AutocompleteConfig(),
  build: { _, config, _ in
    AutocompleteOutput(disabled: signal(config.disabled), ghost: signal(nil))
  },
  register: { editor, config, state in
    let output = state.getOutput()
    var activeTextNodeKey: NodeKey?
    var lastMatch: String?
    var lastSuggestion: String?
    // Stands in for the AbortController: a query answers only while it is
    // the latest.
    var generation = 0
    var pendingCompositionTimer: DispatchWorkItem?

    func clearPendingCompositionTimer() {
      pendingCompositionTimer?.cancel()
      pendingCompositionTimer = nil
    }

    func syncGhost(_ key: NodeKey?, _ text: String?) {
      let ghost = key.flatMap { key in text.map { AutocompleteGhost(key: key, text: $0) } }
      if output.ghost.peek() != ghost { output.ghost.value = ghost }
    }

    func dismiss() {
      activeTextNodeKey = nil
      lastMatch = nil
      lastSuggestion = nil
      generation += 1
      syncGhost(nil, nil)
    }

    /// `query`: the dictionary's answer for `prefix`, after the simulated
    /// server's latency, while `ref` is the latest query.
    func query(_ prefix: String, _ ref: Int, _ answer: @escaping (String?) -> Void) {
      let language = detectLanguage(prefix)
      _ = schedule(QUERY_LATENCY) {
        guard generation == ref else { return }
        answer(AutocompleteDictionaries.dictionary(language)?.query(prefix))
      }
    }

    /// The text node the IME composes in: the one the caret is in while
    /// it composes, as `IMEExtension` finds it, but read each time, since
    /// a change to the marked text can replace the node
    /// (`composingTextNode` keeps the node it started in).
    func composingKey() -> NodeKey? {
      guard editor.isComposing() else { return nil }
      return try? editor.read(.latest) {
        try ((getSelection() as? RangeSelection)?.anchor.getNode() as? TextNode)?.getKey()
      }
    }

    /// The composing node's text as the IME has it so far. The marked text
    /// is in the model, as a browser writes it into the DOM.
    func compositionText(_ key: NodeKey) -> String? {
      try? editor.read(.latest) { try (getNodeByKey(key) as? TextNode)?.getTextContent() }
    }

    /// `tryCompositionSuggestion`: the composition went idle; suggest the
    /// rest of the word it ends in.
    func tryCompositionSuggestion() {
      pendingCompositionTimer = nil
      guard let key = composingKey(), let text = compositionText(key), !text.isEmpty
      else { return }
      let prefix = extractTrailingWord(text)
      guard !prefix.isEmpty, prefix != lastMatch else { return }
      generation += 1
      syncGhost(nil, nil)
      lastMatch = prefix
      lastSuggestion = nil
      activeTextNodeKey = nil
      let ref = generation
      query(prefix, ref) { newSuggestion in
        // The user may have typed past the prefix while the query ran.
        guard let newSuggestion, composingKey() == key,
          let text = compositionText(key), extractTrailingWord(text) == prefix
        else { return }
        activeTextNodeKey = key
        lastSuggestion = newSuggestion
        syncGhost(key, formatSuggestionText(newSuggestion))
      }
    }

    /// `onCompositionUpdateDOM`: each change to the marked text restarts
    /// the idle wait. The ghost stays until the next lookup replaces it,
    /// so a Tab right after a pause still has it.
    func onCompositionUpdate() {
      let debounce = config.compositionIdleDebounce
      guard debounce > 0 else { return }
      clearPendingCompositionTimer()
      pendingCompositionTimer = schedule(debounce) { tryCompositionSuggestion() }
    }

    func applyAsyncSuggestion(_ newSuggestion: String?) {
      guard let newSuggestion, isEditorFocused(editor) else { return }
      try? editor.read(.latest) {
        let selection = try getSelection()
        let (hasMatch, match) = try search(selection)
        guard hasMatch, match == lastMatch, let selection = selection as? RangeSelection,
          let node = try selection.getNodes().first as? TextNode
        else { return }
        activeTextNodeKey = node.getKey()
        lastSuggestion = newSuggestion
        syncGhost(activeTextNodeKey, formatSuggestionText(newSuggestion))
      }
    }

    func handleUpdate(_ payload: UpdateListenerPayload) throws {
      guard isEditorFocused(editor) else {
        dismiss()
        return
      }
      // While composing, the idle wait takes over; the update that ends
      // the composition (tagged COMPOSITION_END_TAG) comes back here.
      if !payload.tags.contains(COMPOSITION_END_TAG) && editor.isComposing() {
        if !payload.dirtyLeaves.isEmpty { onCompositionUpdate() }
        return
      }
      try payload.editorState.read(editor: editor) {
        let selection = try getSelection()
        let (hasMatch, match) = try search(selection)
        guard hasMatch else {
          dismiss()
          return
        }
        if match == lastMatch {
          // Same prefix, but the cursor may have moved to another node
          // with it.
          if let node = try (selection as? RangeSelection)?.getNodes().first as? TextNode,
            node.getKey() != activeTextNodeKey
          {
            activeTextNodeKey = node.getKey()
            syncGhost(activeTextNodeKey, lastSuggestion.map(formatSuggestionText))
          }
          return
        }
        // A new prefix: clear the stale ghost while the query runs.
        generation += 1
        syncGhost(nil, nil)
        lastMatch = match
        lastSuggestion = nil
        activeTextNodeKey = nil
        query(match, generation) { applyAsyncSuggestion($0) }
      }
    }

    func commitSuggestion() throws -> Bool {
      guard let activeTextNodeKey, let lastSuggestion else { return false }
      guard let node = try getNodeByKey(activeTextNodeKey) as? TextNode else {
        dismiss()
        return false
      }
      // Accepted while the IME composes the node: the marked text is kept
      // as composed so far, with the suggestion after it, and the
      // composition ends there (the text view then drops its marked text),
      // so the IME does not merge its next syllable into the accepted word.
      if editor.isComposing(),
        try (getSelection() as? RangeSelection)?.anchor.key == activeTextNodeKey
      {
        try node.setTextContent(try node.getTextContent() + lastSuggestion)
        try setCompositionKey(nil)
        _ = try node.select()
        dismiss()
        return true
      }
      // The raw suggestion, without the "(TAB)" hint.
      _ = try node.spliceText(try node.getTextContentSize(), 0, lastSuggestion, moveSelection: true)
      dismiss()
      return true
    }

    func handleCommitCommand(_ event: KeyboardEvent) throws -> Bool {
      let didCommit = try commitSuggestion()
      if didCommit { event.preventDefault() }
      return didCommit
    }

    return try effect { () -> (() -> Void)? in
      if output.disabled.value { return nil }
      // Loads the wordlists off the main thread, ahead of the first query.
      DispatchQueue.global(qos: .utility).async {
        _ = AutocompleteDictionaries.english
        _ = AutocompleteDictionaries.korean
      }
      return mergeRegister([
        editor.registerUpdateListener { try handleUpdate($0) },
        editor.registerCommand(
          BLUR_COMMAND,
          { _, _ in
            dismiss(); return false
          }, COMMAND_PRIORITY_LOW),
        // A clean slate for the new prefix the composition types.
        editor.registerCommand(
          COMPOSITION_START_COMMAND,
          { _, _ in
            dismiss(); return false
          }, COMMAND_PRIORITY_LOW),
        // `onCompositionEndDOM`: the update this command makes is tagged
        // COMPOSITION_END_TAG and looks the committed word up.
        editor.registerCommand(
          COMPOSITION_END_COMMAND,
          { _, _ in
            clearPendingCompositionTimer(); return false
          }, COMMAND_PRIORITY_LOW),
        editor.registerCommand(
          KEY_TAB_COMMAND, { event, _ in try handleCommitCommand(event) }, COMMAND_PRIORITY_LOW),
        editor.registerCommand(
          KEY_ARROW_RIGHT_COMMAND, { event, _ in try handleCommitCommand(event) },
          COMMAND_PRIORITY_LOW),
        // `handleSwipeRight`: the swipe is not a command, so it commits in
        // an update of its own.
        editor.registerCommand(
          AUTOCOMPLETE_SWIPE_RIGHT_COMMAND, { _, _ in try commitSuggestion() },
          COMMAND_PRIORITY_LOW),
        clearPendingCompositionTimer,
        dismiss,
      ])
    }
  })

/// Sent by the ghost host when the text view is swiped right
/// (`addSwipeRightListener`).
let AUTOCOMPLETE_SWIPE_RIGHT_COMMAND = LexicalCommand<Void>("AUTOCOMPLETE_SWIPE_RIGHT_COMMAND")

/// Shows `AutocompleteOutput.ghost` in the editor's text view after the
/// end of its text node, as a `TextKitSuffix` of the theme's
/// `autocomplete` type (`.PlaygroundEditorTheme__autocomplete`, `#ccc`):
/// in the text layout, outside the editor state, as JS appends the ghost
/// `<span>` to the text node's DOM.
@MainActor
final class AutocompleteGhostHost: TypeaheadMenuAttaching {
  private let editor: Editor
  private weak var textView: GhostTextView?
  private var unsubscribe: (() -> Void)?
  #if os(iOS)
    private lazy var swipe = UISwipeGestureRecognizer(target: self, action: #selector(swiped))
  #endif

  init(editor: Editor) {
    self.editor = editor
    #if os(iOS)
      swipe.direction = .right
      // Typing and selecting go on as before.
      swipe.cancelsTouchesInView = false
    #endif
    let ghost = try? getExtensionDependencyFromEditor(editor, AutocompleteExtension).output.ghost
    // After the commit that changed it, outside the view's own rendering.
    unsubscribe = try? ghost?.subscribe { [weak self] _ in
      DispatchQueue.main.async { self?.show() }
    }
  }

  isolated deinit { unsubscribe?() }

  func attach(to root: AnyObject?) {
    #if os(iOS)
      textView?.removeGestureRecognizer(swipe)
    #endif
    try? textView?.controller.setSuffix(nil)
    textView = root as? GhostTextView
    guard let textView else { return }
    #if os(iOS)
      textView.addGestureRecognizer(swipe)
    #endif
    show()
  }

  #if os(iOS)
    @objc private func swiped() {
      _ = try? editor.dispatchCommand(AUTOCOMPLETE_SWIPE_RIGHT_COMMAND)
    }
  #endif

  private func show() {
    let ghost = try? getExtensionDependencyFromEditor(editor, AutocompleteExtension).output
      .ghost.peek()
    try? textView?.controller.setSuffix(
      ghost.map { TextKitSuffix(key: $0.key, text: $0.text, type: "autocomplete") })
  }
}
