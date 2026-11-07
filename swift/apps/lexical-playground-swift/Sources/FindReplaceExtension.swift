import Foundation
import Lexical
import LexicalExtension
import LexicalMarkdown
import LexicalTextKit
import LexicalUtils
import Observation
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias FindTextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias FindTextView = LexicalUITextView
#endif

// Port of packages/lexical-playground/src/plugins/FindReplaceExtension:
// search the document's text (as a plain string or a regular expression,
// with or without case), step through the matches, replace the current one
// or all of them, and highlight every match, the current one in orange.
// It belongs to the playground, as upstream's does, so its search logic is
// here rather than in the library.
//
// Matches are found in the root's text content with JS's `RegExp`
// semantics (`JSRegExp`), and mapped back to text nodes by their offsets in
// it. The web paints them with CSS custom highlights; here they are
// `TextKitInputController.highlights`.

/// A match in the root's text content, in UTF-16 offsets.
struct TextMatch: Equatable {
  var end: Int
  var matchText: String
  var start: Int
}

/// A text node's range in the root's text content.
struct OffsetEntry: Equatable {
  var globalEnd: Int
  var globalStart: Int
  var key: NodeKey
}

/// A match's points in the document.
struct MatchPoints: Equatable {
  var anchorKey: NodeKey
  var anchorOffset: Int
  var focusKey: NodeKey
  var focusOffset: Int
  var format: Int
}

/// `findMatches`: the non-empty matches of `searchTerm` in `text`.
func findMatches(
  _ text: String, _ searchTerm: String, _ caseSensitive: Bool, _ isRegex: Bool
)
  -> [TextMatch]
{
  if searchTerm.isEmpty || text.isEmpty { return [] }
  let pattern = isRegex ? searchTerm : escapeRegExp(searchTerm)
  guard let regex = try? JSRegExp(validating: pattern, "g" + (caseSensitive ? "" : "i")) else {
    return []
  }
  var matches: [TextMatch] = []
  while let match = regex.exec(text) {
    if match.length == 0 {
      regex.lastIndex += 1
      continue
    }
    matches.append(
      TextMatch(end: match.index + match.length, matchText: match.text, start: match.index))
  }
  return matches
}

/// `escapeRegExp`: `string` as a pattern that matches it literally.
private func escapeRegExp(_ string: String) -> String {
  string.jsReplace(JSRegExp(#"[.*+?^${}()|[\]\\]"#, "g"), template: #"\$&"#)
}

/// `expandReplacement`: `template` with `$1`, `$&` and the others filled in
/// from `matchText`'s match of `searchRegex`, or as is without one.
func expandReplacement(
  _ template: String, _ matchText: String, _ searchRegex: JSRegExp?
)
  -> String
{
  guard let searchRegex else { return template }
  return matchText.jsReplace(searchRegex, template: template)
}

/// `$buildOffsetMap`: each text node's range in the root's text content,
/// in document order. A block after another adds the two line breaks of
/// `getTextContent`.
func buildOffsetMap() throws -> [OffsetEntry] {
  var entries: [OffsetEntry] = []
  var offset = 0
  let iterator = try dfsWithSlotsIterator()
  while let dfsNode = try iterator.next() {
    let node = dfsNode.node
    if try isBlockElementNode(node.getPreviousSibling()) { offset += 2 }
    if let text = node as? TextNode {
      let length = try text.getTextContentSize()
      entries.append(
        OffsetEntry(globalEnd: offset + length, globalStart: offset, key: text.getKey()))
      offset += length
    } else if node is LineBreakNode {
      offset += 1
    } else if node is DecoratorNode {
      // A decorator's default text is its slots' text, which the walk
      // reaches after it. Upstream counts a decorator's own text only when
      // it overrides `getTextContent`; Swift cannot tell an override, so a
      // decorator with slots counts none and one without counts its text
      // (`""` by default). The two differ only for a decorator with slots
      // that overrides it, which upstream does not handle either.
      offset += try getSlotNames(node).isEmpty ? node.getTextContentSize() : 0
    }
  }
  return entries
}

/// `$resolveMatchToPoints`: the points of `match`, or `nil` when it starts
/// or ends between text nodes.
func resolveMatchToPoints(_ match: TextMatch, _ offsetMap: [OffsetEntry]) throws -> MatchPoints? {
  guard let anchorEntry = findEntryForOffset(offsetMap, match.start),
    let focusEntry = findEntryForOffset(offsetMap, match.end - 1)
  else { return nil }
  let isSingleNode = anchorEntry.key == focusEntry.key
  let anchorNode = try getNodeByKeyOrThrow(anchorEntry.key)
  let format = isSingleNode ? try (anchorNode as? TextNode)?.getFormat() ?? 0 : 0
  return MatchPoints(
    anchorKey: anchorEntry.key, anchorOffset: match.start - anchorEntry.globalStart,
    focusKey: focusEntry.key, focusOffset: match.end - focusEntry.globalStart, format: format)
}

/// The entry whose range holds `offset`, by binary search.
private func findEntryForOffset(_ offsetMap: [OffsetEntry], _ offset: Int) -> OffsetEntry? {
  var lo = 0
  var hi = offsetMap.count - 1
  while lo <= hi {
    let mid = (lo + hi) / 2
    let entry = offsetMap[mid]
    if offset < entry.globalStart {
      hi = mid - 1
    } else if offset >= entry.globalEnd {
      lo = mid + 1
    } else {
      return entry
    }
  }
  return nil
}

/// `$replaceMatch`: the match's text replaced, keeping its node's format
/// when it is in one node, else as text in the format of the match's
/// start.
func replaceMatch(
  _ points: MatchPoints, _ replacementText: String, _ match: TextMatch, _ searchRegex: JSRegExp?
) throws {
  let finalText =
    searchRegex.map { expandReplacement(replacementText, match.matchText, $0) }
    ?? replacementText
  guard let anchorNode = try getNodeByKeyOrThrow(points.anchorKey) as? TextNode,
    let focusNode = try getNodeByKeyOrThrow(points.focusKey) as? TextNode
  else { return }
  if points.anchorKey == points.focusKey {
    _ = try anchorNode.spliceText(
      points.anchorOffset, points.focusOffset - points.anchorOffset, finalText,
      moveSelection: true)
    return
  }
  let selection = try anchorNode.select(0, 0)
  try selection.setTextNodeRange(anchorNode, points.anchorOffset, focusNode, points.focusOffset)
  selection.format = points.format
  try selection.insertText(finalText)
}

/// `$replaceAllMatches`: every match replaced, the last first so the
/// offsets of the others hold. Returns how many were.
@discardableResult
func replaceAllMatches(
  _ matches: [TextMatch], _ offsetMap: [OffsetEntry], _ replacementText: String,
  _ searchRegex: JSRegExp?
) throws -> Int {
  var count = 0
  for match in matches.reversed() {
    if let points = try resolveMatchToPoints(match, offsetMap) {
      try replaceMatch(points, replacementText, match, searchRegex)
      count += 1
    }
  }
  return count
}

/// `buildSearchRegex`: the pattern a regex search replaces with, for its
/// `$1` and the others.
private func buildSearchRegex(_ searchTerm: String, _ caseSensitive: Bool) -> JSRegExp? {
  try? JSRegExp(validating: searchTerm, caseSensitive ? "" : "i")
}

let TOGGLE_FIND_REPLACE_COMMAND = LexicalCommand<Void>("TOGGLE_FIND_REPLACE_COMMAND")
let CLOSE_FIND_REPLACE_COMMAND = LexicalCommand<Void>("CLOSE_FIND_REPLACE_COMMAND")
let FIND_NEXT_COMMAND = LexicalCommand<Void>("FIND_NEXT_COMMAND")
let FIND_PREV_COMMAND = LexicalCommand<Void>("FIND_PREV_COMMAND")
let REPLACE_CURRENT_COMMAND = LexicalCommand<Void>("REPLACE_CURRENT_COMMAND")
let REPLACE_ALL_COMMAND = LexicalCommand<Void>("REPLACE_ALL_COMMAND")
let SET_SEARCH_TERM_COMMAND = LexicalCommand<String>("SET_SEARCH_TERM_COMMAND")
let SET_REPLACE_TERM_COMMAND = LexicalCommand<String>("SET_REPLACE_TERM_COMMAND")
let TOGGLE_CASE_SENSITIVE_COMMAND = LexicalCommand<Void>("TOGGLE_CASE_SENSITIVE_COMMAND")
let TOGGLE_REGEX_COMMAND = LexicalCommand<Void>("TOGGLE_REGEX_COMMAND")

/// The extension's named signals.
struct FindReplaceState {
  var caseSensitive = false
  var currentIndex = 0
  var isOpen = false
  var isRegex = false
  var replaceTerm = ""
  var searchTerm = ""
}

/// `FindReplaceExtension`'s output: its signals, and what they compute.
struct FindReplaceOutput {
  let named: NamedSignals<FindReplaceState>
  let cachedText: Signal<String>
  /// The matches, while the panel is open.
  let matches: ReadonlySignal<[TextMatch]>
  /// `currentIndex`, kept within the matches.
  let effectiveIndex: ReadonlySignal<Int>
  /// A regex search whose pattern does not compile.
  let regexError: ReadonlySignal<Bool>

  var caseSensitive: Signal<Bool> { named.caseSensitive }
  var currentIndex: Signal<Int> { named.currentIndex }
  var isOpen: Signal<Bool> { named.isOpen }
  var isRegex: Signal<Bool> { named.isRegex }
  var replaceTerm: Signal<String> { named.replaceTerm }
  var searchTerm: Signal<String> { named.searchTerm }
}

/// Paints the matches (`$updateHighlights`) and scrolls the current one
/// into view, or clears them.
private func updateHighlights(_ editor: Editor, _ matches: [TextMatch], _ currentIndex: Int) {
  var all: [TextKitHighlightRange] = []
  var current: [TextKitHighlightRange] = []
  if !matches.isEmpty {
    try? editor.getEditorState().read {
      let offsetMap = try buildOffsetMap()
      for (i, match) in matches.enumerated() {
        guard let points = try resolveMatchToPoints(match, offsetMap) else { continue }
        let range = TextKitHighlightRange(
          anchorKey: points.anchorKey, anchorOffset: points.anchorOffset,
          focusKey: points.focusKey, focusOffset: points.focusOffset)
        all.append(range)
        if i == currentIndex { current.append(range) }
      }
    }
  }
  nonisolated(unsafe) let editor = editor
  MainActor.assumeIsolated {
    guard let textView = editor.getRootElement() as? FindTextView else { return }
    let controller = textView.controller
    controller.highlights["find-replace-match"] =
      all.isEmpty
      ? nil
      : TextKitHighlight(
        ranges: all, backgroundColor: TextKitColor(red: 1, green: 1, blue: 0, alpha: 0.4),
        priority: 10)
    controller.highlights["find-replace-current"] =
      current.isEmpty
      ? nil
      : TextKitHighlight(
        ranges: current,
        backgroundColor: TextKitColor(red: 1, green: 165 / 255, blue: 0, alpha: 0.6),
        priority: 11)
    // `scrollRangeIntoView`.
    if let range = current.first,
      let start = controller.document.location(range.anchorKey, range.anchorOffset, .text),
      let end = controller.document.location(range.focusKey, range.focusOffset, .text)
    {
      textView.scrollRangeToVisible(NSRange(location: start, length: end - start))
    }
  }
}

/// `FindReplaceExtension`.
let FindReplaceExtension = defineExtension(
  name: "@lexical/playground/FindReplace",
  build: { editor, _, _ in
    let named = namedSignals(FindReplaceState())
    let cachedText = watchedSignal(
      { (try? editor.read(.latest) { try getRoot().getTextContent() }) ?? "" },
      { s in editor.registerTextContentListener { s.value = $0 } })
    let matches = computed { () -> [TextMatch] in
      guard named.isOpen.value else { return [] }
      return findMatches(
        cachedText.value, named.searchTerm.value, named.caseSensitive.value, named.isRegex.value)
    }
    let effectiveIndex = computed { () -> Int in
      let count = matches.value.count
      return count > 0 ? min(named.currentIndex.value, count - 1) : 0
    }
    let regexError = computed { () -> Bool in
      guard named.isRegex.value, !named.searchTerm.value.isEmpty else { return false }
      return (try? JSRegExp(validating: named.searchTerm.value)) == nil
    }
    return FindReplaceOutput(
      named: named, cachedText: cachedText, matches: matches, effectiveIndex: effectiveIndex,
      regexError: regexError)
  },
  register: { editor, _, state in
    let output = state.getOutput()
    func listen(
      _ command: LexicalCommand<Void>, _ handler: @escaping () throws -> Void
    )
      -> () -> Void
    {
      editor.registerCommand(
        command,
        { _, _ in
          try handler()
          return true
        }, COMMAND_PRIORITY_LOW)
    }
    func replaceRegex() -> JSRegExp? {
      output.isRegex.peek()
        ? buildSearchRegex(output.searchTerm.peek(), output.caseSensitive.peek()) : nil
    }
    let findMask = KeyboardEventModifierMask(altKey: true, metaKey: true)
    return mergeRegister(
      listen(TOGGLE_FIND_REPLACE_COMMAND) { output.isOpen.value = !output.isOpen.peek() },
      listen(CLOSE_FIND_REPLACE_COMMAND) { output.isOpen.value = false },
      listen(FIND_NEXT_COMMAND) {
        let count = output.matches.peek().count
        if count > 0 { output.currentIndex.value = (output.currentIndex.peek() + 1) % count }
      },
      listen(FIND_PREV_COMMAND) {
        let count = output.matches.peek().count
        if count > 0 {
          output.currentIndex.value = (output.currentIndex.peek() - 1 + count) % count
        }
      },
      listen(REPLACE_CURRENT_COMMAND) {
        let matches = output.matches.peek()
        guard !matches.isEmpty else { return }
        let match = matches[output.effectiveIndex.peek()]
        let replaceText = output.replaceTerm.peek()
        let regex = replaceRegex()
        try editor.update(discrete: true) {
          if let points = try resolveMatchToPoints(match, buildOffsetMap()) {
            try replaceMatch(points, replaceText, match, regex)
          }
        }
      },
      listen(REPLACE_ALL_COMMAND) {
        let matches = output.matches.peek()
        guard !matches.isEmpty else { return }
        let replaceText = output.replaceTerm.peek()
        let regex = replaceRegex()
        try editor.update(discrete: true) {
          try replaceAllMatches(matches, buildOffsetMap(), replaceText, regex)
        }
      },
      editor.registerCommand(
        SET_SEARCH_TERM_COMMAND,
        { term, _ in
          output.searchTerm.value = term
          output.currentIndex.value = 0
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        SET_REPLACE_TERM_COMMAND,
        { term, _ in
          output.replaceTerm.value = term
          return true
        }, COMMAND_PRIORITY_LOW),
      listen(TOGGLE_CASE_SENSITIVE_COMMAND) {
        output.caseSensitive.value = !output.caseSensitive.peek()
        output.currentIndex.value = 0
      },
      listen(TOGGLE_REGEX_COMMAND) {
        output.isRegex.value = !output.isRegex.peek()
        output.currentIndex.value = 0
      },
      editor.registerCommand(
        KEY_DOWN_COMMAND,
        { event, editor in
          if isExactShortcutMatch(event, "f", CONTROL_OR_META)
            || isExactShortcutMatch(event, "f", findMask)
          {
            event.preventDefault()
            try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
            return true
          }
          if output.isOpen.peek(),
            isExactShortcutMatch(event, "g", CONTROL_OR_META)
              || isExactShortcutMatch(event, "g", CONTROL_OR_META.with(shiftKey: true))
          {
            event.preventDefault()
            try editor.dispatchCommand(event.shiftKey ? FIND_PREV_COMMAND : FIND_NEXT_COMMAND)
            return true
          }
          return false
        }, COMMAND_PRIORITY_LOW),
      try effect {
        let matches = output.matches.value
        let index = output.effectiveIndex.value
        let open = output.isOpen.value
        updateHighlights(editor, open ? matches : [], index)
      },
      { updateHighlights(editor, [], 0) })
  })

// MARK: The panel

/// What `FindReplacePanel` shows, kept from the extension's signals.
@MainActor
@Observable
final class FindReplaceModel {
  private(set) var isOpen = false
  var searchTerm = ""
  var replaceTerm = ""
  private(set) var caseSensitive = false
  private(set) var isRegex = false
  private(set) var matchCount = 0
  private(set) var effectiveIndex = 0
  private(set) var regexError = false
  @ObservationIgnored private weak var editor: Editor?

  /// Follows the extension of `editor`.
  func register(_ editor: Editor) throws -> () -> Void {
    self.editor = editor
    let output = try getExtensionDependencyFromEditor(editor, FindReplaceExtension).output
    return try effect { [weak self] in
      let state = (
        output.isOpen.value, output.searchTerm.value, output.replaceTerm.value,
        output.caseSensitive.value, output.isRegex.value, output.matches.value.count,
        output.effectiveIndex.value, output.regexError.value
      )
      MainActor.assumeIsolated {
        guard let self else { return }
        self.isOpen = state.0
        if self.searchTerm != state.1 { self.searchTerm = state.1 }
        if self.replaceTerm != state.2 { self.replaceTerm = state.2 }
        self.caseSensitive = state.3
        self.isRegex = state.4
        self.matchCount = state.5
        self.effectiveIndex = state.6
        self.regexError = state.7
      }
    }
  }

  /// The count's text (`find-replace-count`).
  var countText: String {
    if regexError { return "Invalid regex" }
    return matchCount > 0 ? "\(effectiveIndex + 1) / \(matchCount)" : "No results"
  }

  func dispatch(_ command: LexicalCommand<Void>) {
    _ = try? editor?.dispatchCommand(command)
  }

  func setSearchTerm(_ term: String) {
    _ = try? editor?.dispatchCommand(SET_SEARCH_TERM_COMMAND, term)
  }

  func setReplaceTerm(_ term: String) {
    _ = try? editor?.dispatchCommand(SET_REPLACE_TERM_COMMAND, term)
  }

  /// Close: the panel goes and the editor takes the focus back.
  func close() {
    dispatch(CLOSE_FIND_REPLACE_COMMAND)
    try? editor?.focus()
  }
}

/// `FindReplacePanel`: the search and replace fields, the match count, the
/// case and regex toggles, previous and next, Replace, All and close.
struct FindReplacePanel: View {
  @Bindable var model: FindReplaceModel
  @FocusState private var isSearchFocused: Bool
  @Environment(\.dynamicTypeSize) private var dynamicTypeSize
  /// The buttons' 28 points, at the text's size.
  @ScaledMetric(relativeTo: .footnote) private var controlSize: CGFloat = 28

  #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var isCompact: Bool { sizeClass == .compact }
  #else
    private var isCompact: Bool { false }
  #endif

  var body: some View {
    Group {
      if isCompact {
        // A phone has no room for the web's two rows, so the buttons get a
        // row of their own.
        VStack(alignment: .leading, spacing: 6) {
          HStack(spacing: 4) {
            searchField
            countLabel
          }
          replaceField
          if dynamicTypeSize.isAccessibilitySize {
            // Replace and All get a row of their own too.
            HStack(spacing: 4) {
              toggles
              separator
              navigation
              Spacer(minLength: 4)
              closeButton
            }
            actions
          } else {
            HStack(spacing: 4) {
              toggles
              separator
              navigation
              Spacer(minLength: 4)
              actions
              separator
              closeButton
            }
          }
        }
      } else {
        Grid(alignment: .leading, horizontalSpacing: 4, verticalSpacing: 6) {
          GridRow {
            searchField
            countLabel
            toggles
            separator
            navigation
            separator
            closeButton
          }
          GridRow {
            replaceField
            actions.gridCellColumns(8)
          }
        }
      }
    }
    .padding(.vertical, 8)
    .padding(.horizontal, 10)
    .background(
      RoundedRectangle(cornerRadius: 8).fill(Color.panelBackground)
        .shadow(color: .black.opacity(0.2), radius: 14, y: 12)
        .shadow(color: .black.opacity(0.1), radius: 2, y: 2)
    )
    .scaledFont(size: 13, relativeTo: .footnote)
    .foregroundStyle(Color.webText(0.02))
    #if os(macOS)
      .onExitCommand { model.close() }
    #endif
    .onAppear { isSearchFocused = true }
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Find and Replace")
    .accessibilityIdentifier("find-replace-panel")
  }

  private var searchField: some View {
    field("Find...", "Search text", $model.searchTerm, model.setSearchTerm)
      .focused($isSearchFocused)
      .onSubmit { model.dispatch(FIND_NEXT_COMMAND) }
  }

  private var replaceField: some View {
    field("Replace...", "Replace text", $model.replaceTerm, model.setReplaceTerm)
      .onSubmit { model.dispatch(FIND_NEXT_COMMAND) }
  }

  private var countLabel: some View {
    Text(model.countText)
      .scaledFont(size: 11, relativeTo: .caption2)
      .foregroundStyle(Color.webText(0.47))
      .frame(minWidth: 70)
      .accessibilityIdentifier("find-replace-count")
  }

  @ViewBuilder private var toggles: some View {
    toggle("Aa", "Match case", model.caseSensitive) {
      model.dispatch(TOGGLE_CASE_SENSITIVE_COMMAND)
    }
    toggle(".*", "Use regular expression", model.isRegex) {
      model.dispatch(TOGGLE_REGEX_COMMAND)
    }
  }

  @ViewBuilder private var navigation: some View {
    icon("chevron.up", "Previous match", "Previous match (⇧⌘G)") {
      model.dispatch(FIND_PREV_COMMAND)
    }
    .keyboardShortcut("g", modifiers: [.command, .shift])
    .disabled(model.matchCount == 0)
    icon("chevron.down", "Next match", "Next match (⌘G)") {
      model.dispatch(FIND_NEXT_COMMAND)
    }
    .keyboardShortcut("g", modifiers: .command)
    .disabled(model.matchCount == 0)
  }

  private var closeButton: some View {
    icon("xmark", "Close", "Close (Escape)") { model.close() }
  }

  private var actions: some View {
    HStack(spacing: 4) {
      action("Replace", "Replace current match") { model.dispatch(REPLACE_CURRENT_COMMAND) }
      action("All", "Replace all matches") { model.dispatch(REPLACE_ALL_COMMAND) }
    }
    .disabled(model.matchCount == 0)
  }

  /// `.find-replace-separator`.
  private var separator: some View {
    Rectangle().fill(Color.webLine(0.87)).frame(width: 1, height: controlSize * 18 / 28)
      .padding(.horizontal, 2)
  }

  private func field(
    _ placeholder: String, _ label: String, _ text: Binding<String>,
    _ onChange: @escaping (String) -> Void
  ) -> some View {
    TextField(placeholder, text: text)
      .textFieldStyle(.plain)
      .autocorrectionDisabled()
      #if os(iOS)
        .textInputAutocapitalization(.never)
      #endif
      .onChange(of: text.wrappedValue) { _, value in onChange(value) }
      .padding(.vertical, 5)
      .padding(.horizontal, 8)
      .frame(minWidth: 180)
      .background(RoundedRectangle(cornerRadius: 6).fill(Color.webFill(0.98)))
      .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.webLine(0.87)))
      .accessibilityLabel(label)
      .accessibilityIdentifier(label)
  }

  private func toggle(
    _ title: String, _ label: String, _ isOn: Bool, _ action: @escaping () -> Void
  ) -> some View {
    Button(action: action) {
      Text(title)
        .scaledFont(size: 12, weight: .medium, relativeTo: .caption)
        .fixedSize()
        .foregroundStyle(isOn ? Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255) : .gray)
        .padding(.horizontal, 6)
        .frame(height: controlSize)
        .background(
          RoundedRectangle(cornerRadius: 6)
            .fill(
              isOn ? Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255).opacity(0.1) : .clear)
        )
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .help(label)
    .accessibilityLabel(label)
    .accessibilityAddTraits(isOn ? .isSelected : [])
  }

  private func icon(
    _ symbol: String, _ label: String, _ help: String, _ action: @escaping () -> Void
  ) -> some View {
    Button(action: action) {
      Image(systemName: symbol)
        .scaledFont(size: 12, weight: .semibold, relativeTo: .caption)
        .frame(width: controlSize, height: controlSize)
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .foregroundStyle(Color.webText(0.33))
    .help(help)
    .accessibilityLabel(label)
  }

  private func action(
    _ title: String, _ label: String, _ action: @escaping () -> Void
  )
    -> some View
  {
    // `.find-replace-btn`: text on the panel's background.
    Button(action: action) {
      Text(title).padding(.horizontal, 8).frame(height: controlSize).contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .foregroundStyle(Color.webText(0.2))
    .accessibilityLabel(label)
  }
}
