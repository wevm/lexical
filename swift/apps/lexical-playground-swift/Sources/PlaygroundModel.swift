import Foundation
import Lexical
import LexicalA11y
import LexicalAutoEmbed
import LexicalCodeCore
import LexicalCodeTreeSitter
import LexicalCodeTreeSitterBash
import LexicalCodeTreeSitterC
import LexicalCodeTreeSitterCPP
import LexicalCodeTreeSitterCSS
import LexicalCodeTreeSitterGo
import LexicalCodeTreeSitterHTML
import LexicalCodeTreeSitterJSON
import LexicalCodeTreeSitterJava
import LexicalCodeTreeSitterJavaScript
import LexicalCodeTreeSitterMarkdown
import LexicalCodeTreeSitterObjC
import LexicalCodeTreeSitterPowerShell
import LexicalCodeTreeSitterPython
import LexicalCodeTreeSitterRust
import LexicalCodeTreeSitterSQL
import LexicalCodeTreeSitterSwift
import LexicalCodeTreeSitterTypeScript
import LexicalCodeTreeSitterXML
import LexicalDevtoolsCore
import LexicalExtension
import LexicalHTML
import LexicalHashtag
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMark
import LexicalMarkdown
import LexicalOverflow
import LexicalPlainText
import LexicalRichText
import LexicalSelection
import LexicalTextKit
import LexicalUtils
import Observation

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

/// The block types the toolbar offers (the playground's `blockTypeToBlockName`
/// for the nodes the Swift port has).
enum BlockType: String, CaseIterable, Identifiable {
  case paragraph, h1, h2, h3, h4, h5, h6, bullet, number, check, quote, code
  var id: String { rawValue }

  var name: String {
    switch self {
    case .paragraph: "Normal"
    case .h1: "Heading 1"
    case .h2: "Heading 2"
    case .h3: "Heading 3"
    case .h4: "Heading 4"
    case .h5: "Heading 5"
    case .h6: "Heading 6"
    case .bullet: "Bulleted List"
    case .number: "Numbered List"
    case .check: "Check List"
    case .quote: "Quote"
    case .code: "Code Block"
    }
  }

  /// The list types, which the toolbar also has buttons for.
  static let lists: [(type: BlockType, symbol: String)] = [
    (.bullet, "list.bullet"), (.number, "list.number"), (.check, "checklist"),
  ]
}

/// `validateUrl` of the playground's utils/url.ts: one token (trailing
/// whitespace allowed), `https://` as the toolbar's placeholder, and a
/// supported protocol. Foundation's URL parser stands in for `new URL()`.
func validateUrl(_ url: String) -> Bool {
  if url == "https://" { return true }
  let trimmed = url.replacing(/\s+$/, with: "")
  guard !trimmed.isEmpty, !trimmed.contains(where: \.isWhitespace) else { return false }
  var parsed = URL(string: trimmed).flatMap { $0.scheme == nil ? nil : $0 }
  if parsed == nil, trimmed.hasPrefix("www."), trimmed.count > 4 {
    parsed = URL(string: "https://" + trimmed)
  }
  guard let scheme = parsed?.scheme?.lowercased() else { return false }
  return ["http", "https", "mailto", "sms", "tel"].contains(scheme)
}

/// Opens links for `ClickableLinkExtension`: the system's handler for the
/// URL (a browser for a web link).
final class PlaygroundLinkWindow: LinkWindow {
  func open(_ url: String, _ target: String, _ features: String?) {
    guard let url = URL(string: url) else { return }
    DispatchQueue.main.async {
      #if os(macOS)
        NSWorkspace.shared.open(url)
      #else
        UIApplication.shared.open(url)
      #endif
    }
  }
}

/// The playground's transformers (`PLAYGROUND_TRANSFORMERS`) for the nodes
/// the Swift port has: `CHECK_LIST`, which `TRANSFORMERS` leaves out, ahead
/// of the built-ins.
nonisolated(unsafe) let playgroundTransformers: [any Transformer] = [CHECK_LIST] + TRANSFORMERS

/// `PlaygroundMarkdownShortcutsExtension`: Markdown typed at the start of a
/// block (`# `, `- `, `[ ] `, `> `, ` ``` `) or around text (`**bold**`)
/// becomes its node or format.
let PlaygroundMarkdownShortcutsExtension = defineExtension(
  name: "@lexical/playground-swift/MarkdownShortcuts",
  dependencies: [ListExtension, CheckListExtension, LinkExtension, CodeExtension],
  register: { editor, _, _ in try registerMarkdownShortcuts(editor, playgroundTransformers) })

/// Every grammar the playground links, one for each language of the code
/// language picker but C-like and Plain Text, which stay plain.
/// A block with no language stays plain, as the web playground's
/// `NULL_LANG_PRISM_TOKENIZER` (`defaultLanguage: null`) does.
func makePlaygroundCodeTokenizer() -> TreeSitterTokenizer {
  TreeSitterTokenizer(
    [
      .bash, .c, .cpp, .css, .go, .html, .java, .javaScript, .json, .markdown, .objC,
      .powerShell, .python, .rust, .sql, .swift, .typeScript, .xml,
    ],
    defaultLanguage: nil)
}

/// The text formats the toolbar toggles, in its order.
let toolbarFormats: [(format: TextFormatType, title: String, symbol: String)] = [
  ("bold", "Bold", "bold"), ("italic", "Italic", "italic"), ("underline", "Underline", "underline"),
  ("strikethrough", "Strikethrough", "strikethrough"),
  ("code", "Code", "chevron.left.forwardslash.chevron.right"),
  ("subscript", "Subscript", "textformat.subscript"),
  ("superscript", "Superscript", "textformat.superscript"),
  ("highlight", "Highlight", "highlighter"),
]

/// The alignments the toolbar offers (`FORMAT_ELEMENT_COMMAND` payloads).
let toolbarAlignments: [(format: ElementFormatType, title: String, symbol: String)] = [
  ("left", "Left Align", "text.alignleft"), ("center", "Center Align", "text.aligncenter"),
  ("right", "Right Align", "text.alignright"), ("justify", "Justify Align", "text.justify"),
]

/// The playground's state: the editor, what the toolbar shows for the
/// selection, the tree view and the debug settings.
@MainActor
@Observable
final class PlaygroundModel {
  /// A new editor is built when the rich/plain setting changes; the view
  /// is keyed by `generation` so it attaches to the new one.
  private(set) var editor: Editor
  private(set) var generation = 0

  // Debug settings (the playground's Settings panel).
  var isRichText = true {
    didSet { if isRichText != oldValue { rebuild() } }
  }
  var isEditable = true {
    didSet {
      try? editor.setEditable(isEditable)
      syncClickableLinks()
      refreshLinkEditor()
    }
  }
  /// The Autocomplete setting, which `useSynchronizeSettings` passes to
  /// `AutocompleteExtension`; off by default, as on the web.
  var isAutocomplete = false {
    didSet { syncAutocomplete() }
  }
  var showsTreeView = true {
    didSet { refreshTree() }
  }
  /// "Char Limit" and "Char Limit (UTF-8)": `CharacterLimitPlugin` with a
  /// limit of 5, counting UTF-16 code units when both are on.
  var isCharLimit = false {
    didSet { syncCharacterLimit() }
  }
  var isCharLimitUtf8 = false {
    didSet { syncCharacterLimit() }
  }
  var isMaxLength = false {
    didSet { syncMaxLength() }
  }
  /// "Use Brackets for Highlighting" (`shouldAllowHighlightingWithBrackets`).
  var isSpecialText = false {
    didSet { syncSpecialText() }
  }
  /// The characters the character limit leaves, while it is on.
  private(set) var remainingCharacters: Int?
  @ObservationIgnored private var unregisterCharacterLimit: (() -> Void)?
  var treeViewShowsHTML = false {
    didSet { refreshTree() }
  }

  // Toolbar state.
  private(set) var blockType = BlockType.paragraph
  private(set) var formats: Set<TextFormatType> = []
  private(set) var alignment: ElementFormatType = "left"
  /// The selection's inline styles, as the web toolbar reads them with
  /// `$getSelectionStyleValueForProperty` (`""` where the text disagrees).
  private(set) var fontFamily = "Arial"
  private(set) var fontSize = "\(defaultFontSize)px"
  private(set) var fontColor = "#000"
  private(set) var backgroundColor = "#fff"
  private(set) var canUndo = false
  private(set) var canRedo = false
  /// The selection is in a link.
  private(set) var isLink = false

  // The floating text format toolbar (`FloatingTextFormatToolbarPlugin`).
  /// Text is selected where the popup formats it (`isText`).
  private(set) var isFloatingText = false
  /// The formats of the popup's buttons the selection has.
  private(set) var floatingFormats: Set<TextFormatType> = []
  /// The selection's anchor and focus, which the popup is placed above.
  @ObservationIgnored private(set) var selectionPoints: (anchor: Point, focus: Point)?
  @ObservationIgnored private lazy var formatToolbarHost = FloatingTextFormatToolbarHost(
    model: self)

  // The floating link editor (`FloatingLinkEditorPlugin`).
  /// The link the panel shows, while the editor is editable.
  private(set) var linkEditorKey: NodeKey?
  private(set) var linkEditorURL = ""
  var isLinkEditMode = false
  /// The Keyboard Shortcuts panel is shown, where it is a sheet (iOS).
  var showsShortcuts = false
  var editedLinkURL = "https://"
  @ObservationIgnored private lazy var linkEditorHost = FloatingLinkEditorHost(model: self)
  @ObservationIgnored private let codeActionMenuHost = CodeActionMenuHost()
  #if os(macOS)
    @ObservationIgnored private let draggableBlockHost = DraggableBlockHost()
  #endif
  @ObservationIgnored private let stickyNotesHost = StickyNotesHost()
  /// The selection's code block and its language (`codeLanguage` and
  /// `selectedElementKey` of the toolbar's state).
  private(set) var codeLanguage = ""
  @ObservationIgnored private var selectedCodeKey: NodeKey?
  /// The typeahead menus' hosts (`ComponentPickerPlugin`, …), kept while
  /// their editor is.
  @ObservationIgnored private var menuHosts: [AnyObject & TypeaheadMenuAttaching] = []
  /// `CommentPlugin`'s threads and panel.
  let comments = CommentsModel()
  /// `FindReplaceExtension`'s panel.
  let findReplace = FindReplaceModel()
  /// `TableOfContentsPlugin`'s headings.
  let tableOfContents = TableOfContentsModel()
  /// `ActionsPlugin`'s Import and Export of `.lexical` files.
  let files = LexicalFileModel()

  private(set) var tree = ""
  private(set) var commandsLog: [LexicalCommandLogEntry] = []
  @ObservationIgnored private var commandIndex = 0
  @ObservationIgnored private var unregister: [() -> Void] = []

  init() {
    // A playground that can't build its editor has nothing to show.
    editor = try! Self.makeEditor(richText: true)
    try? loadWelcomeDocument(editor)
    attach()
  }

  static func makeEditor(richText: Bool) throws -> Editor {
    // Links in both modes, as the playground's AppExtension has them; lists,
    // code blocks and Markdown shortcuts in rich text only.
    let links: [any AnyLexicalExtensionArgument] = [
      MentionsExtension,
      configExtension(LinkExtension) { $0.validateUrl = validateUrl },
      configExtension(AutoLinkExtension) {
        $0.excludeParents = [isCodeNode]
        $0.matchers = [autoLinkUrlMatcher, autoLinkEmailMatcher]
      },
      configExtension(ClickableLinkExtension) {
        $0.newTab = true
        $0.getWindow = { _ in PlaygroundLinkWindow() }
      },
    ]
    // The rest of the playground's AppExtension, in both modes.
    let always: [any AnyLexicalExtensionArgument] = [
      HashtagExtension, OverflowExtension, MaxLengthExtension, KeywordsExtension,
      DateTimeExtension, SpecialTextExtension, MarkExtension, FindReplaceExtension,
      EmojisExtension,
    ]
    return try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift",
        dependencies: richText
          ? [
            RichTextExtension, HistoryExtension, NormalizeTripleClickSelectionExtension,
            HistoryAnnounceExtension, EditorModeAnnounceExtension,
            configExtension(ListExtension) { $0.shouldPreserveNumbering = false },
            CheckListExtension, CodeExtension, HorizontalRuleExtension, PageBreakExtension,
            ImagesExtension, DecoratorDragExtension, EmbedsExtension, PollExtension,
            EquationsExtension,
            DragDropPasteExtension, PlaygroundDOMRenderExtension,
            // The web playground's `CodeHighlightExtension` brings both:
            // Tab and Shift+Tab indent and outdent code lines.
            configExtension(CodeHighlighterExtension) {
              $0.tokenizer = makePlaygroundCodeTokenizer()
            },
            configExtension(CodeIndentExtension) {
              $0.escapeWithArrows = true
              $0.tabSize = 2
            },
            PlaygroundMarkdownShortcutsExtension,
            PlaygroundShortcutsExtension,
            configExtension(TabIndentationExtension) { $0.maxIndent = 7 },
            configExtension(AutocompleteExtension) { $0.disabled = true },
          ] + links + always
          : [
            PlainTextExtension, HistoryExtension, HistoryAnnounceExtension,
            EditorModeAnnounceExtension,
          ] + links + always,
        // `PlaygroundNodes`' nodes that no extension brings.
        nodes: { richText ? [.klass(StickyNode.self)] : [] }))
  }

  /// Clickable links are active only while the editor is read-only, as the
  /// playground's settings synchronization drives `ClickableLinkExtension`;
  /// while editing, a click places the caret and the toolbar edits the link.
  private func syncClickableLinks() {
    try? getExtensionDependencyFromEditor(editor, ClickableLinkExtension).output.disabled.set(
      isEditable)
  }

  private func syncSpecialText() {
    try? getExtensionDependencyFromEditor(editor, SpecialTextExtension).output.disabled.set(
      !isSpecialText)
  }

  private func syncMaxLength() {
    try? getExtensionDependencyFromEditor(editor, MaxLengthExtension).output.disabled.set(
      !isMaxLength)
  }

  private func syncCharacterLimit() {
    unregisterCharacterLimit?()
    unregisterCharacterLimit = nil
    remainingCharacters = nil
    guard isCharLimit || isCharLimitUtf8 else { return }
    let utf16 = isCharLimit
    unregisterCharacterLimit = try? registerCharacterLimit(
      editor, maxCharacters: 5, strlen: { utf16 ? $0.utf16.count : $0.utf8.count },
      remainingCharacters: { [weak self] remaining in
        MainActor.assumeIsolated { self?.remainingCharacters = remaining }
      })
  }

  private func syncAutocomplete() {
    try? getExtensionDependencyFromEditor(editor, AutocompleteExtension).output.disabled.set(
      !isAutocomplete)
  }

  /// Builds the other kind of editor, keeping the text: a plain text editor
  /// can't hold headings or quotes.
  private func rebuild() {
    // Each block's text, its line breaks as "\n".
    let blocks =
      (try? editor.read { try getRoot().getChildren().map { try $0.getTextContent() } }) ?? []
    for remove in unregister { remove() }
    unregister = []
    unregisterCharacterLimit?()
    unregisterCharacterLimit = nil
    editor = try! Self.makeEditor(richText: isRichText)
    try? editor.update(discrete: true) {
      let root = try getRoot().clear()
      for block in blocks {
        let paragraph = try createParagraphNode()
        for (i, line) in block.split(separator: "\n", omittingEmptySubsequences: false).enumerated()
        {
          if i > 0 { try paragraph.append(createLineBreakNode()) }
          if !line.isEmpty { try paragraph.append(createTextNode(String(line))) }
        }
        try root.append(paragraph)
      }
      _ = try root.selectEnd()
    }
    try? editor.setEditable(isEditable)
    generation += 1
    commandsLog = []
    attach()
  }

  private func attach() {
    syncClickableLinks()
    let picker = registerComponentPicker(
      editor, showImageDialog: { [weak self] in self?.isInsertingImage = true },
      showPollDialog: { [weak self] in self?.isInsertingPoll = true },
      showEquationDialog: { [weak self] in self?.isInsertingEquation = true })
    let emoji = registerEmojiPicker(editor)
    let mentions = registerMentions(editor)
    menuHosts = [picker.host, emoji.host, mentions.host]
    // Rich text only, as the web registers its embed nodes there.
    if isRichText,
      let embeds = try? registerAutoEmbedPlugin(
        editor, showModal: { [weak self] in self?.embedConfig = $0 })
    {
      menuHosts.append(embeds.host)
      unregister.append(embeds.unregister)
    }
    if isRichText {
      syncAutocomplete()
      menuHosts.append(AutocompleteGhostHost(editor: editor))
    }
    unregister += [picker.unregister, emoji.unregister, mentions.unregister]
    comments.onInputChange = { [weak self] in self?.formatToolbarHost.refresh() }
    #if os(macOS)
      comments.formatToolbarFrame = { [weak self] in self?.formatToolbarHost.panelFrame() }
    #endif
    if let unregisterComments = try? comments.register(editor) {
      unregister.append(unregisterComments)
    }
    if let unregisterFindReplace = try? findReplace.register(editor) {
      unregister.append(unregisterFindReplace)
    }
    unregister.append(tableOfContents.register(editor))
    syncMaxLength()
    syncSpecialText()
    syncCharacterLimit()
    unregister.append(
      editor.registerRootListener { [weak self] root, _ in
        MainActor.assumeIsolated {
          // The view attaches before its first render, so the theme is
          // in place for it.
          if let controller = textController(root) {
            controller.theme["mention"] = TextKitStyle(
              css: "background-color: \(mentionBackgroundColor)")
            controller.theme["keyword"] = TextKitStyle(css: keywordThemeStyle)
            controller.theme["specialText"] = TextKitStyle(css: specialTextThemeStyle)
          }
          self?.linkEditorHost.attach(to: root)
          self?.formatToolbarHost.attach(to: root)
          if let self {
            self.codeActionMenuHost.attach(to: root, editor: self.editor) {
              [weak self] key, language in self?.setCodeLanguage(language, in: key)
            }
            self.stickyNotesHost.attach(to: root, editor: self.editor)
          }
          #if os(macOS)
            if let self { attachContextMenu(to: root, editor: self.editor) }
            if let self {
              self.draggableBlockHost.attach(
                to: self.isRichText ? root : nil, editor: self.editor)
            }
          #endif
          self?.menuHosts.forEach { $0.attach(to: root) }
          self?.comments.attach(to: root)
        }
        return nil
      })
    unregister.append(
      editor.registerCommand(
        KEY_ESCAPE_COMMAND,
        { [weak self] _, _ in
          MainActor.assumeIsolated {
            guard let self, self.linkEditorKey != nil else { return false }
            self.hideLinkEditor()
            return true
          }
        }, COMMAND_PRIORITY_HIGH))
    // A Command or Control click on a link opens it in a new tab, as the
    // plugin's `CLICK_COMMAND` handler does while the editor is editable.
    unregister.append(
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard event.metaKey || event.ctrlKey,
            let selection = try getSelection() as? RangeSelection,
            let link = try findMatchingParent(getSelectedNode(selection), { $0 is LinkNode })
              as? LinkNode
          else { return false }
          PlaygroundLinkWindow().open(try link.getURL(), "_blank", nil)
          return true
        }, COMMAND_PRIORITY_LOW))
    unregister.append(
      editor.registerUpdateListener { [weak self] _ in
        MainActor.assumeIsolated { self?.refresh() }
      })
    unregister.append(
      editor.registerCommand(
        CAN_UNDO_COMMAND,
        { [weak self] payload, _ in
          MainActor.assumeIsolated { self?.canUndo = payload }
          return false
        }, COMMAND_PRIORITY_CRITICAL))
    unregister.append(
      editor.registerCommand(
        CAN_REDO_COMMAND,
        { [weak self] payload, _ in
          MainActor.assumeIsolated { self?.canRedo = payload }
          return false
        }, COMMAND_PRIORITY_CRITICAL))
    // `ShortcutsExtension`'s command listeners.
    for shortcut in PlaygroundShortcut.allCases where !shortcut.isBuiltin {
      unregister.append(
        editor.registerCommand(
          shortcut.command,
          { [weak self] event, _ in
            event.preventDefault()
            MainActor.assumeIsolated { self?.perform(shortcut) }
            return true
          }, COMMAND_PRIORITY_EDITOR))
    }
    logCommands()
    refresh()
  }

  /// The commands log of the tree view (`registerLexicalCommandLogger`):
  /// the last 10 commands, for the commands the playground's UI and the
  /// views dispatch.
  private func logCommands() {
    func log<P>(_ command: LexicalCommand<P>, _ payload: @escaping (P) -> String = { "\($0)" }) {
      unregister.append(
        editor.registerCommand(
          command,
          { [weak self] value, _ in
            MainActor.assumeIsolated { self?.record(command.type ?? "UNKNOWN", payload(value)) }
            return false
          }, COMMAND_PRIORITY_CRITICAL))
    }
    log(FORMAT_TEXT_COMMAND)
    log(FORMAT_ELEMENT_COMMAND)
    log(INSERT_UNORDERED_LIST_COMMAND) { _ in "undefined" }
    log(INSERT_ORDERED_LIST_COMMAND) { _ in "undefined" }
    log(INSERT_CHECK_LIST_COMMAND) { _ in "undefined" }
    log(REMOVE_LIST_COMMAND) { _ in "undefined" }
    log(TOGGLE_LINK_COMMAND) { payload in payload.map { "\($0)" } ?? "null" }
    log(INDENT_CONTENT_COMMAND) { _ in "undefined" }
    log(OUTDENT_CONTENT_COMMAND) { _ in "undefined" }
    log(UNDO_COMMAND) { _ in "undefined" }
    log(REDO_COMMAND) { _ in "undefined" }
    log(CAN_UNDO_COMMAND)
    log(CAN_REDO_COMMAND)
    log(CLEAR_HISTORY_COMMAND) { _ in "undefined" }
    log(SELECTION_CHANGE_COMMAND) { _ in "undefined" }
    log(KEY_DOWN_COMMAND) { _ in "KeyboardEvent" }
    log(BEFORE_INPUT_COMMAND) { _ in "InputEvent" }
    log(CLICK_COMMAND) { _ in "MouseEvent" }
    log(PASTE_COMMAND) { _ in "ClipboardEvent" }
    log(COPY_COMMAND) { _ in "ClipboardEvent" }
    log(CUT_COMMAND) { _ in "ClipboardEvent" }
  }

  private func record(_ type: String, _ payload: String) {
    commandIndex += 1
    commandsLog = Array(
      (commandsLog + [LexicalCommandLogEntry(index: commandIndex, type: type, payload: payload)])
        .suffix(10))
    refreshTree()
  }

  private func refresh() {
    refreshLinkEditor()
    try? editor.read {
      guard let selection = try getSelection() as? RangeSelection else {
        isFloatingText = false
        return
      }
      formats = Set(toolbarFormats.map(\.format).filter { selection.hasFormat($0) })
      fontColor = try getSelectionStyleValueForProperty(selection, "color", "#000")
      backgroundColor = try getSelectionStyleValueForProperty(selection, "background-color", "#fff")
      fontFamily = try getSelectionStyleValueForProperty(selection, "font-family", "Arial")
      fontSize = try getSelectionStyleValueForProperty(
        selection, "font-size", "\(defaultFontSize)px")
      let anchor = try selection.anchor.getNode()
      let block = try anchor.getKey() == "root" ? anchor : anchor.getTopLevelElementOrThrow()
      // A list's type is the nearest list's, as the playground's toolbar
      // reads it for a nested list.
      if let list = try getNearestNodeOfType(anchor, ListNode.self) {
        blockType = BlockType(rawValue: try list.getListType()) ?? .bullet
      } else {
        switch block {
        case let heading as HeadingNode:
          blockType = BlockType(rawValue: try heading.getTag()) ?? .h1
        case is QuoteNode: blockType = .quote
        case is CodeNode: blockType = .code
        default: blockType = .paragraph
        }
      }
      let code = blockType == .code ? block as? CodeNode : nil
      selectedCodeKey = code?.getKey()
      codeLanguage = try code?.getLanguage() ?? ""
      let format = try (block as? ElementNode)?.getFormatType() ?? ""
      alignment = format.isEmpty || format == "start" ? "left" : format
      // `$isLinkNode(parent) || $isLinkNode(node)` in the toolbar, for the
      // focus's node.
      let focus = try selection.focus.getNode()
      let link = try (focus as? LinkNode) ?? (focus.getParent() as? LinkNode)
      isLink = link != nil
      try refreshFloatingToolbar(selection)
    }
    formatToolbarHost.refresh()
    codeActionMenuHost.refresh(selectedCodeKey: selectedCodeKey)
    refreshTree()
  }

  /// The toolbar's `onCodeLanguageSelect`: sets the language of code block
  /// `key`, by default the selection's, `nil` for none. One update, so one
  /// undo step, which re-highlights the block and keeps the selection.
  func setCodeLanguage(_ language: String?, in key: NodeKey? = nil) {
    guard let key = key ?? selectedCodeKey else { return }
    try? editor.update {
      try addUpdateTag(SKIP_SELECTION_FOCUS_TAG)
      _ = try (getNodeByKey(key) as? CodeNode)?.setLanguage(language)
    }
  }

  /// `updatePopup` of the floating text format toolbar.
  private func refreshFloatingToolbar(_ selection: RangeSelection) throws {
    // No popup while an IME composes.
    if editor.isComposing() { return }
    selectionPoints = (selection.anchor, selection.focus)
    floatingFormats = Set(floatingToolbarFormats.map(\.format).filter { selection.hasFormat($0) })
    let node = try getSelectedNode(selection)
    let text = try selection.getTextContent()
    if try !(selection.anchor.getNode().getParent() is CodeNode), !text.isEmpty {
      isFloatingText = node is TextNode || node is ParagraphNode
    } else {
      isFloatingText = false
    }
    if !selection.isCollapsed(), text.replacingOccurrences(of: "\n", with: "").isEmpty {
      isFloatingText = false
    }
  }

  /// `$updateToolbar` and `$updateLinkEditor` of the floating link editor:
  /// shows the panel at the selection's link, after the view has laid out
  /// this update.
  private func refreshLinkEditor() {
    let link =
      isEditable
      ? try? editor.read { try floatingLink(getSelection()).map { ($0.getKey(), try $0.getURL()) } }
      : nil
    if link?.0 != linkEditorKey { isLinkEditMode = isLinkEditMode && link != nil }
    linkEditorKey = link?.0
    linkEditorURL = link?.1 ?? ""
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.linkEditorHost.position(at: self.linkEditorKey)
    }
  }

  private func hideLinkEditor() {
    linkEditorKey = nil
    isLinkEditMode = false
    linkEditorHost.position(at: nil)
  }

  /// The panel's edit button.
  func editLink() {
    editedLinkURL = linkEditorURL
    isLinkEditMode = true
    refreshLinkEditor()
  }

  /// The panel's trash button.
  func removeLink() {
    _ = try? editor.dispatchCommand(TOGGLE_LINK_COMMAND, nil)
  }

  /// `handleLinkSubmission`: links the selection to the URL typed, and
  /// makes an autolink it is in a plain link.
  func submitLink() {
    if linkEditorKey != nil {
      let url = sanitizeUrl(editedLinkURL)
      try? editor.update { [editor] in
        _ = try editor.dispatchCommand(TOGGLE_LINK_COMMAND, .url(url))
        if let selection = try getSelection() as? RangeSelection,
          let parent = try getSelectedNode(selection).getParent() as? AutoLinkNode
        {
          let link = try createLinkNode(
            parent.getURL(),
            LinkAttributes(rel: parent.__rel, target: parent.__target, title: parent.__title))
          _ = try parent.replace(link, includeChildren: true)
        }
      }
    }
    editedLinkURL = "https://"
    isLinkEditMode = false
    refreshLinkEditor()
  }

  /// The toolbar's `insertLink`: links the selection to `https://` and
  /// edits the URL in the panel, or unlinks a link.
  func insertLink() {
    if !isLink {
      isLinkEditMode = true
      editedLinkURL = "https://"
      _ = try? editor.dispatchCommand(TOGGLE_LINK_COMMAND, .url(sanitizeUrl("https://")))
    } else {
      isLinkEditMode = false
      _ = try? editor.dispatchCommand(TOGGLE_LINK_COMMAND, nil)
    }
  }

  private func refreshTree() {
    guard showsTreeView else { return }
    tree = (try? generateContent(editor, commandsLog, treeViewShowsHTML)) ?? ""
  }

  // MARK: Toolbar actions

  func toggleFormat(_ format: TextFormatType) {
    _ = try? editor.dispatchCommand(FORMAT_TEXT_COMMAND, format)
  }

  func align(_ format: ElementFormatType) {
    _ = try? editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, format)
  }

  /// `applyStyleText` and `FontDropDown`'s `handleClick`: patches the
  /// selected text's inline style (`$patchStyleText`).
  func applyStyleText(_ property: String, _ value: String) {
    try? editor.update {
      if let selection = try getSelection() {
        try patchStyleText(selection, [property: .value(value)])
      }
    }
  }

  /// The font size stepper's buttons (`updateFontSize`): the next size up
  /// or down from the one shown, or from each text's own size when the
  /// selection's sizes differ.
  func stepFontSize(_ step: FontSizeStep, from inputValue: String) {
    try? editor.update {
      guard let selection = try getSelection() else { return }
      if let current = Double(inputValue) {
        let next = calculateNextFontSize(current, step)
        try patchStyleText(selection, ["font-size": .value("\(formatFontSize(next))px")])
      } else {
        try patchStyleText(
          selection,
          [
            "font-size": .compute { prev, _ in
              let size = prev.flatMap { Double($0.dropLast(2)) } ?? Double(defaultFontSize)
              return "\(formatFontSize(calculateNextFontSize(size, step)))px"
            }
          ])
      }
    }
  }

  /// The font size field (`updateFontSizeByInputValue`): the size typed,
  /// clamped to the allowed range.
  func setFontSize(_ size: Double) {
    let clamped = min(max(size, Double(minAllowedFontSize)), Double(maxAllowedFontSize))
    applyStyleText("font-size", "\(formatFontSize(clamped))px")
  }

  /// The Insert menu's Horizontal Rule.
  /// The Insert Image dialog is open (`showModal('Insert Image', ...)`).
  var isInsertingImage = false

  func insertImage(_ payload: InsertImagePayload) {
    _ = try? editor.dispatchCommand(INSERT_IMAGE_COMMAND, payload)
  }

  func insertHorizontalRule() {
    _ = try? editor.dispatchCommand(INSERT_HORIZONTAL_RULE_COMMAND)
  }

  func insertPageBreak() {
    _ = try? editor.dispatchCommand(INSERT_PAGE_BREAK)
  }

  /// The Insert Poll dialog is open (`showModal('Insert Poll', ...)`).
  var isInsertingPoll = false

  /// The Insert Equation dialog is open (`showModal('Insert Equation', ...)`).
  var isInsertingEquation = false

  /// The Insert menu's Sticky Note: a note at the editor's top left corner,
  /// appended to the root.
  func insertStickyNote() {
    try? editor.update { try getRoot().append(createStickyNode(0, 0)) }
  }

  /// The embed dialog that is open (`AutoEmbedDialog`), for its config.
  var embedConfig: PlaygroundEmbedConfig?

  /// The Insert menu's embeds, which open the dialog for their config.
  func insertEmbed(_ type: String) {
    _ = try? editor.dispatchCommand(INSERT_EMBED_COMMAND, type)
  }

  /// The toolbar's Date: today, at midnight.
  func insertDate() {
    _ = try? editor.dispatchCommand(INSERT_DATETIME_COMMAND, midnight())
  }

  /// The popup's and the shortcut's Insert comment.
  func insertComment() {
    _ = try? editor.dispatchCommand(INSERT_INLINE_COMMAND)
  }

  func indent() { _ = try? editor.dispatchCommand(INDENT_CONTENT_COMMAND) }
  func outdent() { _ = try? editor.dispatchCommand(OUTDENT_CONTENT_COMMAND) }
  func undo() { _ = try? editor.dispatchCommand(UNDO_COMMAND) }
  func redo() { _ = try? editor.dispatchCommand(REDO_COMMAND) }

  /// A keyboard shortcut's action, from its key press or its menu item:
  /// what `ShortcutsExtension` registers for each (and the editor's own
  /// shortcuts, for the menu).
  func perform(_ shortcut: PlaygroundShortcut) {
    switch shortcut {
    case .normal: setBlockType(.paragraph)
    case .heading1: if blockType != .h1 { setBlockType(.h1) }
    case .heading2: if blockType != .h2 { setBlockType(.h2) }
    case .heading3: if blockType != .h3 { setBlockType(.h3) }
    case .numberedList: setBlockType(.number)
    case .bulletList: setBlockType(.bullet)
    case .checkList: setBlockType(.check)
    case .codeBlock: setBlockType(.code)
    case .quote: if blockType != .quote { setBlockType(.quote) }
    case .addComment: insertComment()
    case .increaseFontSize: stepFontSize(.increment, from: toolbarFontSize(fontSize))
    case .decreaseFontSize: stepFontSize(.decrement, from: toolbarFontSize(fontSize))
    case .insertCodeBlock: toggleFormat("code")
    case .strikethrough: toggleFormat("strikethrough")
    case .lowercase: toggleFormat("lowercase")
    case .uppercase: toggleFormat("uppercase")
    case .capitalize: toggleFormat("capitalize")
    case .centerAlign: align("center")
    case .justifyAlign: align("justify")
    case .leftAlign: align("left")
    case .rightAlign: align("right")
    case .subscript: toggleFormat("subscript")
    case .superscript: toggleFormat("superscript")
    case .indent: indent()
    case .outdent: outdent()
    case .clearFormatting: clearFormatting()
    case .insertLink: insertLink()
    case .bold: toggleFormat("bold")
    case .italic: toggleFormat("italic")
    case .underline: toggleFormat("underline")
    case .undo: undo()
    case .redo: redo()
    }
  }

  /// `formatParagraph`, `formatHeading`, `formatBulletList`,
  /// `formatNumberedList`, `formatCheckList`, `formatQuote` and
  /// `formatCode` in the playground's block format menu. A list command for
  /// the list type the selection is already in removes the list, as there.
  func setBlockType(_ type: BlockType) {
    switch type {
    case .bullet, .number, .check:
      if blockType == type {
        _ = try? editor.dispatchCommand(REMOVE_LIST_COMMAND)
      } else if type == .bullet {
        _ = try? editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
      } else if type == .number {
        _ = try? editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND)
      } else {
        _ = try? editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND)
      }
      return
    default: break
    }
    let isCode = blockType == .code
    try? editor.update {
      let selection = try getSelection()
      switch type {
      case .paragraph: try setBlocksType(selection) { try createParagraphNode() }
      case .quote: try setBlocksType(selection) { try createQuoteNode() }
      case .code:
        // `formatCode`: a collapsed selection turns its block into a code
        // block; a range becomes one code block holding its text.
        guard !isCode, let selection else { return }
        if selection.isCollapsed() {
          try setBlocksType(selection) { try createCodeNode() }
        } else {
          let text = try selection.getTextContent()
          let code = try createCodeNode()
          try selection.insertNodes([code])
          try code.append(createTextNode(text))
          _ = try code.selectEnd()
        }
      default: try setBlocksType(selection) { try createHeadingNode(type.rawValue) }
      }
    }
  }

  /// The playground's Clear Formatting (`$clearFormatting` in
  /// ToolbarPlugin/utils.ts): the selected text loses its formats and
  /// styles, fully selected blocks their alignment and indent, and selected
  /// headings and quotes become paragraphs. A caret clears its block's
  /// alignment and indent.
  func clearFormatting() {
    try? editor.update {
      guard let selection = try getSelection() as? RangeSelection else { return }
      let anchor = selection.anchor
      let focus = selection.focus
      let extractedNodes = try selection.extract()
      if anchor.key == focus.key, anchor.offset == focus.offset {
        try clearBlockFormat(getNearestBlockElementAncestorOrThrow(anchor.getNode()))
        return
      }
      // Which blocks are fully selected, before the replacements below
      // detach nodes the selection's carets refer to.
      var fullySelectedBlocks: Set<NodeKey>?
      if let postExtract = try getSelection() as? RangeSelection {
        var blocks = Set<NodeKey>()
        for node in extractedNodes where node is TextNode {
          let block = try getNearestBlockElementAncestorOrThrow(node)
          if !blocks.contains(block.getKey()), try isBlockFullySelected(block, postExtract) {
            blocks.insert(block.getKey())
          }
        }
        fullySelectedBlocks = blocks
      }
      for node in extractedNodes {
        if let text = node as? TextNode {
          if try text.getStyle() != "" { _ = try text.setStyle("") }
          if try text.getFormat() != 0 { _ = try text.setFormat(0) }
          let block = try getNearestBlockElementAncestorOrThrow(text)
          if fullySelectedBlocks?.contains(block.getKey()) ?? true {
            try clearBlockFormat(block)
          }
        } else if node is HeadingNode || node is QuoteNode {
          _ = try node.replace(createParagraphNode(), includeChildren: true)
        }
      }
      // The toolbar reads the selection's cached format (#8881).
      if let cleared = try getSelection() as? RangeSelection {
        cleared.setFormat(0)
        cleared.setStyle("")
      }
    }
  }

  // MARK: Import and export

  /// The editor state's JSON, indented two spaces, as the playground's
  /// Export button writes it.
  func exportJSON() -> String {
    (try? prettyJSON(.object(editor.getEditorState().toJSON()))) ?? ""
  }

  func importJSON(_ json: String) throws {
    try editor.setEditorState(editor.parseEditorState(json))
    _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
  }

  func exportHTML() -> String {
    let editor = editor
    return (try? editor.read { try generateHtmlFromNodes(editor) }) ?? ""
  }

  /// The document as Markdown (`$convertToMarkdownString`, as the
  /// playground's Markdown toggle writes it).
  func exportMarkdown() -> String {
    (try? editor.read { try convertToMarkdownString(playgroundTransformers) }) ?? ""
  }

  /// Replaces the document with `markdown`'s nodes.
  func importMarkdown(_ markdown: String) throws {
    try editor.update(discrete: true) {
      try convertFromMarkdownString(markdown, playgroundTransformers)
    }
    _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
  }

  /// Replaces the document with the nodes `html` imports to.
  func importHTML(_ html: String) throws {
    let editor = editor
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOM(editor, DOMParser().parseFromString(html))
      let root = try getRoot().clear()
      _ = try root.select()
      try getSelection()?.insertNodes(nodes)
    }
  }

  func clearEditor() {
    try? editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode())
      _ = try getRoot().selectStart()
    }
    _ = try? editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
  }

  func loadWelcome() {
    try? loadWelcomeDocument(editor)
    _ = try? editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
  }
}

/// `$clearBlockFormat`: a block's alignment and indent, reset.
private func clearBlockFormat(_ block: ElementNode) throws {
  if try block.getFormat() != 0 { _ = try block.setFormat("") }
  if try block.getIndent() != 0 { _ = try block.setIndent(0) }
}

/// `JSON.stringify(value, null, 2)`.
func prettyJSON(_ value: JSONValue, _ indent: String = "") -> String {
  let inner = indent + "  "
  switch value {
  case .array(let items):
    if items.isEmpty { return "[]" }
    return "[\n" + items.map { inner + prettyJSON($0, inner) }.joined(separator: ",\n")
      + "\n\(indent)]"
  case .object(let object):
    let entries = Array(object)
    if entries.isEmpty { return "{}" }
    return "{\n"
      + entries.map {
        inner + JSONValue.string($0.key).stringify() + ": " + prettyJSON($0.value, inner)
      }
      .joined(separator: ",\n") + "\n\(indent)}"
  default:
    return value.stringify()
  }
}

// MARK: Font size (ToolbarContext.tsx and ToolbarPlugin/utils.ts)

let minAllowedFontSize = 8
let maxAllowedFontSize = 72
let defaultFontSize = 16

enum FontSizeStep {
  case increment, decrement
}

/// `calculateNextFontSize`: the stepper's next size, in larger steps for
/// larger sizes.
func calculateNextFontSize(_ current: Double, _ step: FontSizeStep) -> Double {
  let min = Double(minAllowedFontSize)
  let max = Double(maxAllowedFontSize)
  switch step {
  case .decrement:
    switch current {
    case let size where size > max: return max
    case let size where size >= 48: return current - 12
    case let size where size >= 24: return current - 4
    case let size where size >= 14: return current - 2
    case let size where size >= 9: return current - 1
    default: return min
    }
  case .increment:
    switch current {
    case let size where size < min: return min
    case let size where size < 12: return current + 1
    case let size where size < 20: return current + 2
    case let size where size < 36: return current + 4
    case let size where size <= 60: return current + 12
    default: return max
    }
  }
}

/// A size as JS's `String(number)` writes it: no `.0` for a whole number.
func formatFontSize(_ size: Double) -> String {
  size == size.rounded() ? String(Int(size)) : String(size)
}

/// `parseFontSizeForToolbar` without its unit: the size in pixels of a
/// `px` or `pt` value, `""` for anything else.
func toolbarFontSize(_ value: String) -> String {
  guard let match = value.wholeMatch(of: /(\d+(?:\.\d+)?)(px|pt)/),
    let size = Double(match.1)
  else { return "" }
  return formatFontSize(match.2 == "pt" ? (size * 4 / 3).rounded() : size)
}
