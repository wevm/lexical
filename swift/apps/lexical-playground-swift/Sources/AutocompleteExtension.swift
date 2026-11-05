import Foundation
import Lexical
import LexicalExtension
import LexicalSelection

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias GhostTextView = LexicalNSTextView
  private typealias GhostLabel = NSTextField
#else
  import UIKit
  import LexicalUIKit
  private typealias GhostTextView = LexicalUITextView
  private typealias GhostLabel = UILabel
#endif

// Port of packages/lexical-playground/src/plugins/AutocompleteExtension: at
// the end of a word, a simulated server suggests the rest of it after
// 200 ms, drawn as grey ghost text after the caret; Tab or the right arrow
// (a swipe right on iOS) accepts it.
//
// JS appends the ghost as an unmanaged `<span>` inside the text node's DOM.
// A text view has no such child, so the ghost is a label over the text
// view at the end of the node (`AutocompleteGhostHost`), outside the
// editor state as on the web; text after the caret on the same line is
// covered rather than pushed aside (parity/deviations.json). Only the
// English dictionary is bundled (`autocomplete-english.txt`, upstream's
// `dictionaries/english.ts`); the Korean one and the IME
// composition-idle suggestions are follow-ups.

/// `AutocompleteDictionary`.
struct AutocompleteDictionary: Sendable {
  /// The extension never queries a shorter prefix.
  let minPrefixLength: Int
  /// The rest of the suggested word for `prefix`, or `nil`.
  let query: @Sendable (String) -> String?
}

/// `createWordlistDictionary(words, {minPrefixLength, caseSensitive})`:
/// the earliest-listed word that starts with the prefix and is longer than
/// it. Upstream binary-searches a sorted index for the same answer; the
/// playground's list is short enough to scan.
func createWordlistDictionary(
  _ words: [String], minPrefixLength: Int = 2, caseSensitive: Bool = false
) -> AutocompleteDictionary {
  let fold: @Sendable (String) -> String = { caseSensitive ? $0 : $0.lowercased() }
  let folded = words.map(fold)
  return AutocompleteDictionary(minPrefixLength: minPrefixLength) { prefix in
    let length = prefix.utf16.count
    guard length >= minPrefixLength else { return nil }
    let needle = fold(prefix)
    for (i, word) in words.enumerated()
    where folded[i].hasPrefix(needle) && word.utf16.count > length {
      return String(decoding: word.utf16.dropFirst(length), as: UTF16.self)
    }
    return nil
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

/// The English wordlist, loaded once.
@MainActor
private let englishDictionary: AutocompleteDictionary = {
  let words =
    Bundle.main.url(forResource: "autocomplete-english", withExtension: "txt")
    .flatMap { try? String(contentsOf: $0, encoding: .utf8) }?
    .split(separator: "\n").map(String.init) ?? []
  return createWordlistDictionary(words, minPrefixLength: 4)
}()

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

/// `AutocompleteExtension`.
let AutocompleteExtension = defineExtension(
  name: "@lexical/playground/autocomplete", config: AutocompleteConfig(),
  build: { _, config, _ in
    AutocompleteOutput(disabled: signal(config.disabled), ghost: signal(nil))
  },
  register: { editor, _, state in
    let output = state.getOutput()
    var activeTextNodeKey: NodeKey?
    var lastMatch: String?
    var lastSuggestion: String?
    // Stands in for the AbortController: a query answers only while it is
    // the latest.
    var generation = 0

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

    func applyAsyncSuggestion(_ ref: Int, _ newSuggestion: String?) {
      guard generation == ref, let newSuggestion,
        isEditorFocused(editor)
      else { return }
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
      if !payload.tags.contains(COMPOSITION_END_TAG) && editor.isComposing() { return }
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
        let ref = generation
        let language = detectLanguage(match)
        let answer = MainThreadCallback {
          guard generation == ref else { return }
          let dictionary = language == "en" ? MainActor.assumeIsolated { englishDictionary } : nil
          applyAsyncSuggestion(ref, dictionary?.query(match))
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + QUERY_LATENCY) {
          MainActor.assumeIsolated { answer.callback() }
        }
      }
    }

    func commitSuggestion() throws -> Bool {
      guard let activeTextNodeKey, let lastSuggestion else { return false }
      guard let node = try getNodeByKey(activeTextNodeKey) as? TextNode else {
        dismiss()
        return false
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
      return mergeRegister([
        editor.registerUpdateListener { try handleUpdate($0) },
        editor.registerCommand(
          BLUR_COMMAND,
          { _, _ in
            dismiss(); return false
          }, COMMAND_PRIORITY_LOW),
        editor.registerCommand(
          COMPOSITION_START_COMMAND,
          { _, _ in
            dismiss(); return false
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
        dismiss,
      ])
    }
  })

/// Sent by the ghost host when the text view is swiped right
/// (`addSwipeRightListener`).
let AUTOCOMPLETE_SWIPE_RIGHT_COMMAND = LexicalCommand<Void>("AUTOCOMPLETE_SWIPE_RIGHT_COMMAND")

/// Draws `AutocompleteOutput.ghost` over the editor's text view, after the
/// end of its text node, in the text's font and `#ccc`
/// (`.PlaygroundEditorTheme__autocomplete`).
@MainActor
final class AutocompleteGhostHost: TypeaheadMenuAttaching {
  private let editor: Editor
  private let label = GhostLabel()
  private weak var textView: GhostTextView?
  private var unsubscribe: (() -> Void)?
  #if os(iOS)
    private lazy var swipe = UISwipeGestureRecognizer(target: self, action: #selector(swiped))
  #endif

  init(editor: Editor) {
    self.editor = editor
    #if os(macOS)
      label.isEditable = false
      label.isSelectable = false
      label.isBordered = false
      label.drawsBackground = false
      // `#ccc` in a light appearance; dimmed text in a dark one.
      label.textColor = .tertiaryLabelColor
      label.setAccessibilityIdentifier("autocomplete-ghost")
    #else
      label.textColor = .tertiaryLabel
      label.isUserInteractionEnabled = false
      swipe.direction = .right
      // Typing and selecting go on as before.
      swipe.cancelsTouchesInView = false
      label.accessibilityIdentifier = "autocomplete-ghost"
    #endif
    label.isHidden = true
    let ghost = try? getExtensionDependencyFromEditor(editor, AutocompleteExtension).output.ghost
    unsubscribe = try? ghost?.subscribe { [weak self] _ in
      DispatchQueue.main.async { self?.position() }
    }
  }

  isolated deinit { unsubscribe?() }

  func attach(to root: AnyObject?) {
    label.removeFromSuperview()
    #if os(iOS)
      textView?.removeGestureRecognizer(swipe)
    #endif
    textView = root as? GhostTextView
    guard let textView else { return }
    textView.addSubview(label)
    #if os(iOS)
      textView.addGestureRecognizer(swipe)
    #endif
    position()
  }

  #if os(iOS)
    @objc private func swiped() {
      _ = try? editor.dispatchCommand(AUTOCOMPLETE_SWIPE_RIGHT_COMMAND)
    }
  #endif

  private func position() {
    guard let textView,
      let ghost = try? getExtensionDependencyFromEditor(editor, AutocompleteExtension).output
        .ghost.peek(),
      let size = try? editor.read(
        .latest,
        {
          try (getNodeByKey(ghost.key) as? TextNode)?
            .getTextContentSize()
        }),
      let rect = textView.rect(
        from: createPoint(ghost.key, size, .text), to: createPoint(ghost.key, size, .text)),
      let location = textView.controller.document.location(
        for: createPoint(ghost.key, size, .text))
    else {
      label.isHidden = true
      return
    }
    #if os(macOS)
      let font =
        location > 0
        ? textView.textStorage?.attribute(.font, at: location - 1, effectiveRange: nil) as? NSFont
        : nil
      label.font = font ?? .systemFont(ofSize: 16)
      label.stringValue = ghost.text
    #else
      let font =
        location > 0
        ? textView.textStorage.attribute(.font, at: location - 1, effectiveRange: nil) as? UIFont
        : nil
      label.font = font ?? .systemFont(ofSize: 16)
      label.text = ghost.text
    #endif
    label.sizeToFit()
    // On the caret's line, centered on it, and cut off at the text
    // container's right edge, where the web's inline ghost would wrap.
    let height = label.frame.height
    #if os(macOS)
      let right = textView.bounds.maxX - textView.textContainerInset.width
      label.cell?.lineBreakMode = .byTruncatingTail
    #else
      let right = textView.bounds.maxX - textView.textContainerInset.right
      label.lineBreakMode = .byTruncatingTail
    #endif
    label.frame = CGRect(
      x: rect.maxX, y: rect.midY - height / 2,
      width: max(0, min(label.frame.width, right - rect.maxX)), height: height)
    label.isHidden = false
  }
}
