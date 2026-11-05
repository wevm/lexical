import Foundation
import Lexical
import LexicalA11y
import LexicalCodeCore
import LexicalDevtoolsCore
import LexicalExtension
import LexicalHTML
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalPlainText
import LexicalRichText
import LexicalSelection
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
  var showsTreeView = true {
    didSet { refreshTree() }
  }
  var treeViewShowsHTML = false {
    didSet { refreshTree() }
  }

  // Toolbar state.
  private(set) var blockType = BlockType.paragraph
  private(set) var formats: Set<TextFormatType> = []
  private(set) var alignment: ElementFormatType = "left"
  private(set) var canUndo = false
  private(set) var canRedo = false
  /// The selection is in a link.
  private(set) var isLink = false

  // The floating link editor (`FloatingLinkEditorPlugin`).
  /// The link the panel shows, while the editor is editable.
  private(set) var linkEditorKey: NodeKey?
  private(set) var linkEditorURL = ""
  var isLinkEditMode = false
  var editedLinkURL = "https://"
  @ObservationIgnored private lazy var linkEditorHost = FloatingLinkEditorHost(model: self)

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
    return try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift",
        dependencies: richText
          ? [
            RichTextExtension, HistoryExtension, NormalizeTripleClickSelectionExtension,
            HistoryAnnounceExtension, EditorModeAnnounceExtension,
            configExtension(ListExtension) { $0.shouldPreserveNumbering = false },
            CheckListExtension, CodeExtension, PlaygroundMarkdownShortcutsExtension,
            configExtension(TabIndentationExtension) { $0.maxIndent = 7 },
          ] + links
          : [
            PlainTextExtension, HistoryExtension, HistoryAnnounceExtension,
            EditorModeAnnounceExtension,
          ] + links))
  }

  /// Clickable links are active only while the editor is read-only, as the
  /// playground's settings synchronization drives `ClickableLinkExtension`;
  /// while editing, a click places the caret and the toolbar edits the link.
  private func syncClickableLinks() {
    try? getExtensionDependencyFromEditor(editor, ClickableLinkExtension).output.disabled.set(
      isEditable)
  }

  /// Builds the other kind of editor, keeping the text: a plain text editor
  /// can't hold headings or quotes.
  private func rebuild() {
    // Each block's text, its line breaks as "\n".
    let blocks =
      (try? editor.read { try getRoot().getChildren().map { try $0.getTextContent() } }) ?? []
    for remove in unregister { remove() }
    unregister = []
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
    unregister.append(
      editor.registerRootListener { [weak self] root, _ in
        MainActor.assumeIsolated { self?.linkEditorHost.attach(to: root) }
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
      guard let selection = try getSelection() as? RangeSelection else { return }
      formats = Set(toolbarFormats.map(\.format).filter { selection.hasFormat($0) })
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
      let format = try (block as? ElementNode)?.getFormatType() ?? ""
      alignment = format.isEmpty || format == "start" ? "left" : format
      // `$isLinkNode(parent) || $isLinkNode(node)` in the toolbar, for the
      // focus's node.
      let focus = try selection.focus.getNode()
      let link = try (focus as? LinkNode) ?? (focus.getParent() as? LinkNode)
      isLink = link != nil
    }
    refreshTree()
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

  func indent() { _ = try? editor.dispatchCommand(INDENT_CONTENT_COMMAND) }
  func outdent() { _ = try? editor.dispatchCommand(OUTDENT_CONTENT_COMMAND) }
  func undo() { _ = try? editor.dispatchCommand(UNDO_COMMAND) }
  func redo() { _ = try? editor.dispatchCommand(REDO_COMMAND) }

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
