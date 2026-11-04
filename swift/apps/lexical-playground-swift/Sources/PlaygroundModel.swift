import Lexical
import LexicalDevtoolsCore
import LexicalExtension
import LexicalHTML
import LexicalHistory
import LexicalPlainText
import LexicalRichText
import LexicalSelection
import Observation

/// The block types the toolbar offers (the playground's `blockTypeToBlockName`
/// for the v0.1.0 nodes).
enum BlockType: String, CaseIterable, Identifiable {
  case paragraph, h1, h2, h3, h4, h5, h6, quote
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
    case .quote: "Quote"
    }
  }
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
    didSet { try? editor.setEditable(isEditable) }
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
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift",
        dependencies: richText
          ? [RichTextExtension, HistoryExtension, NormalizeTripleClickSelectionExtension]
          : [PlainTextExtension, HistoryExtension]))
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
    try? editor.read {
      guard let selection = try getSelection() as? RangeSelection else { return }
      formats = Set(toolbarFormats.map(\.format).filter { selection.hasFormat($0) })
      let anchor = try selection.anchor.getNode()
      let block = try anchor.getKey() == "root" ? anchor : anchor.getTopLevelElementOrThrow()
      switch block {
      case let heading as HeadingNode: blockType = BlockType(rawValue: try heading.getTag()) ?? .h1
      case is QuoteNode: blockType = .quote
      default: blockType = .paragraph
      }
      let format = try (block as? ElementNode)?.getFormatType() ?? ""
      alignment = format.isEmpty || format == "start" ? "left" : format
    }
    refreshTree()
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

  /// `formatParagraph` / `formatHeading` / `formatQuote` in the playground's
  /// block format menu.
  func setBlockType(_ type: BlockType) {
    try? editor.update {
      let selection = try getSelection()
      switch type {
      case .paragraph: try setBlocksType(selection) { try createParagraphNode() }
      case .quote: try setBlocksType(selection) { try createQuoteNode() }
      default: try setBlocksType(selection) { try createHeadingNode(type.rawValue) }
      }
    }
  }

  /// Removes the text formats and styles of the selection (the
  /// playground's Clear Formatting, for the v0.1.0 nodes).
  func clearFormatting() {
    try? editor.update {
      guard let selection = try getSelection() as? RangeSelection else { return }
      for node in try selection.extract() {
        if let text = node as? TextNode {
          _ = try text.setFormat(0).setStyle("")
        }
      }
      selection.format = 0
      selection.style = ""
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
