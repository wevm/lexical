import Foundation
import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import LexicalText
import LexicalTextKit
import Observation

/// The Markdown the composer turns into formatting as it is typed: lists,
/// quotes, code blocks, inline code, bold, italic, strikethrough and links.
/// No headings, which a chat message has no use for.
nonisolated(unsafe) let chatTransformers: [any Transformer] = [
  QUOTE, UNORDERED_LIST, ORDERED_LIST, CODE, INLINE_CODE, BOLD_ITALIC_STAR, BOLD_STAR,
  BOLD_UNDERSCORE, ITALIC_STAR, ITALIC_UNDERSCORE, STRIKETHROUGH, LINK,
]

let ChatMarkdownShortcutsExtension = defineExtension(
  name: "chat-composer/MarkdownShortcuts",
  dependencies: [ListExtension, LinkExtension, CodeExtension],
  register: { editor, _, _ in try registerMarkdownShortcuts(editor, chatTransformers) })

/// The nodes and behavior a message has, in the composer and in the list.
/// (A function: an array of `any AnyLexicalExtensionArgument` is not
/// `Sendable`, so it cannot be a global constant.)
private func messageExtensions() -> [any AnyLexicalExtensionArgument] {
  [
    RichTextExtension, ListExtension, CodeExtension, LinkExtension, MentionExtension,
    configExtension(AutoLinkExtension) { $0.matchers = [autoLinkUrlMatcher] },
  ]
}

/// The editors' look: the library's default, and the mentions.
let chatTheme: TextKitTheme = {
  var theme = TextKitTheme.default
  theme["mention"] = TextKitStyle(
    css: "background-color: rgba(24, 119, 232, 0.2); color: rgb(24, 119, 232)")
  return theme
}()

/// The editor of the composer, with history, Markdown shortcuts and
/// clearing.
@MainActor
func makeComposerEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "chat-composer/Composer",
      dependencies: messageExtensions() + [
        HistoryExtension, ClearEditorExtension, ChatMarkdownShortcutsExtension,
      ]))
}

/// A read-only editor showing a sent message.
@MainActor
func makeMessageEditor(_ state: JSONObject?) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "chat-composer/Message", dependencies: messageExtensions(),
      editorConfig: InitialEditorConfig(editable: false)))
  if let state {
    try editor.setEditorState(editor.parseEditorState(state))
  }
  return editor
}

/// A file the user attached: app state shown as a chip above the
/// composer, not a node in the message.
struct Attachment: Identifiable, Equatable {
  let id = UUID()
  let name: String
  let size: Int

  var symbol: String {
    switch (name as NSString).pathExtension.lowercased() {
    case "png", "jpg", "jpeg", "gif", "heic", "webp": "photo"
    case "pdf": "doc.richtext"
    case "zip": "doc.zipper"
    default: "doc"
    }
  }
}

@MainActor
@Observable
final class ChatMessage: Identifiable {
  enum Author { case me, assistant }
  let id = UUID()
  let author: Author
  let editor: Editor
  let attachments: [Attachment]

  init(author: Author, editor: Editor, attachments: [Attachment] = []) {
    self.author = author
    self.editor = editor
    self.attachments = attachments
  }
}

@MainActor
@Observable
final class ChatModel {
  let composer: Editor
  private(set) var messages: [ChatMessage] = []
  var attachments: [Attachment] = []
  /// Whether the composer has nothing to send: no text and no attachments.
  private(set) var isComposerEmpty = true
  /// While a reply plays, the composer is read-only.
  private(set) var isStreaming = false
  /// The formats at the composer's selection, for the format buttons.
  private(set) var activeFormats: Set<String> = []
  private(set) var isLink = false
  let mentions = SuggestionsModel()
  let commands = SuggestionsModel()
  @ObservationIgnored private var unregister: [() -> Void] = []

  var canSend: Bool { !isStreaming && (!isComposerEmpty || !attachments.isEmpty) }

  init() {
    // An example that cannot build its editor has nothing to show.
    composer = try! makeComposerEditor()
    unregister = [
      composer.registerUpdateListener { [weak self] payload in
        let state = payload.editorState
        MainActor.assumeIsolated { self?.composerChanged(state) }
      },
      // Enter sends; Shift+Enter falls through to rich text, which
      // inserts a line break. Enter in a code block or a list goes on
      // editing it, and a menu open on the query takes Enter first.
      composer.registerCommand(
        KEY_ENTER_COMMAND,
        { [weak self] event, editor in
          guard let self, event?.shiftKey != true, !editor.isComposing(),
            try !Self.isInCodeOrList()
          else { return false }
          event?.preventDefault()
          MainActor.assumeIsolated { self.send() }
          return true
        }, COMMAND_PRIORITY_LOW),
      // Files dropped on or pasted into the composer become attachments.
      composer.registerCommand(
        DRAG_DROP_PASTE,
        { [weak self] files, _ in
          MainActor.assumeIsolated {
            self?.attachments += files.map { Attachment(name: $0.name, size: $0.data.count) }
          }
          return true
        }, COMMAND_PRIORITY_LOW),
      registerMentionsMenu(composer, mentions),
      registerSlashMenu(composer, commands),
    ]
    messages = welcomeMessages()
  }

  isolated deinit {
    unregister.forEach { $0() }
  }

  private static func isInCodeOrList() throws -> Bool {
    guard let selection = try getSelection() as? RangeSelection else { return false }
    let block = try selection.anchor.getNode().getTopLevelElement()
    return isCodeNode(block) || block is ListNode
  }

  private func composerChanged(_ state: EditorState) {
    let (isEmpty, formats, isLink) =
      (try? state.read { () -> (Bool, Set<String>, Bool) in
        // `$isRootTextContentEmpty`, as the send button of a web composer
        // would check it.
        let isEmpty = try isRootTextContentEmpty(false)
        guard let selection = try getSelection() as? RangeSelection else {
          return (isEmpty, [], false)
        }
        let formats = Set(
          (["bold", "italic", "code"] as [TextFormatType]).filter { selection.hasFormat($0) })
        let node = try selection.anchor.getNode()
        let isLink = try isLinkNode(node) || isLinkNode(node.getParent())
        return (isEmpty, formats, isLink)
      }) ?? (true, [], false)
    if isComposerEmpty != isEmpty { isComposerEmpty = isEmpty }
    if activeFormats != formats { activeFormats = formats }
    if self.isLink != isLink { self.isLink = isLink }
  }

  /// `-draft <text>` and `-attach <name>` start the composer with a draft
  /// and attachments.
  func applyLaunchArguments() {
    let arguments = ProcessInfo.processInfo.arguments
    func values(_ flag: String) -> [String] {
      arguments.indices.dropLast().filter { arguments[$0] == flag }.map { arguments[$0 + 1] }
    }
    if let draft = values("-draft").first {
      try? composer.update(discrete: true) {
        try convertFromMarkdownString(draft, chatTransformers)
        _ = try getRoot().selectEnd()
      }
    }
    attachments += values("-attach").map { Attachment(name: $0, size: 48_213) }
    composer.focusView()
  }

  // MARK: Actions

  func toggleFormat(_ format: TextFormatType) {
    _ = try? composer.dispatchCommand(FORMAT_TEXT_COMMAND, format)
    composer.focusView()
  }

  /// Links the selected text to `url`, or removes the link when `url` is
  /// empty.
  func setLink(_ url: String) {
    let url = url.trimmingCharacters(in: .whitespaces)
    _ = try? composer.dispatchCommand(TOGGLE_LINK_COMMAND, url.isEmpty ? nil : .url(url))
    composer.focusView()
  }

  func addFiles(_ urls: [URL]) {
    for url in urls {
      let didAccess = url.startAccessingSecurityScopedResource()
      defer { if didAccess { url.stopAccessingSecurityScopedResource() } }
      let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
      attachments.append(Attachment(name: url.lastPathComponent, size: size))
    }
  }

  func remove(_ attachment: Attachment) {
    attachments.removeAll { $0.id == attachment.id }
  }

  /// Sends the composer's message with the attachments, clears both, and
  /// plays a reply.
  func send() {
    guard canSend, let state = try? composer.getEditorState().toJSON(),
      let editor = try? makeMessageEditor(isComposerEmpty ? nil : state)
    else { return }
    messages.append(ChatMessage(author: .me, editor: editor, attachments: attachments))
    attachments = []
    _ = try? composer.dispatchCommand(CLEAR_EDITOR_COMMAND, ())
    _ = try? composer.dispatchCommand(CLEAR_HISTORY_COMMAND, ())
    streamReply()
  }

  /// A fake reply that arrives a few words at a time, with the composer
  /// read-only meanwhile.
  private func streamReply() {
    guard let reply = try? makeMessageEditor(nil) else { return }
    messages.append(ChatMessage(author: .assistant, editor: reply))
    isStreaming = true
    try? composer.setEditable(false)
    let words = replyText.split(separator: " ").map(String.init)
    Task { [weak self] in
      for (index, word) in words.enumerated() {
        try? await Task.sleep(for: .milliseconds(60))
        try? reply.update {
          let paragraph = try getRoot().getFirstChild() as? ElementNode
          let text = try paragraph?.getLastChild() as? TextNode
          if let text {
            _ = try text.setTextContent(text.getTextContent() + " " + word)
          } else if index == 0 {
            try paragraph?.append(createTextNode(word))
          }
        }
      }
      guard let self else { return }
      try? reply.update {
        // The last words are bold, to show the reply is rich text too.
        let paragraph = try getRoot().getFirstChild() as? ElementNode
        try paragraph?.append(createTextNode(" Anything else?").toggleFormat("bold"))
      }
      self.isStreaming = false
      try? self.composer.setEditable(true)
      self.composer.focusView()
    }
  }

  private let replyText =
    "Thanks! This reply streams in word by word while the composer is read-only, as an assistant's answer would."

  private func welcomeMessages() -> [ChatMessage] {
    let markdown = [
      "Welcome! Type a message and press **Enter** to send it, or **Shift+Enter** for a new line.",
      "Try `@` to mention someone, `/` for commands, **bold**, *italic*, `code` and [links](https://lexical.dev), or attach a file with the paperclip.",
    ]
    return markdown.compactMap { text in
      guard let editor = try? makeMessageEditor(nil) else { return nil }
      try? editor.update(discrete: true) {
        try convertFromMarkdownString(text, chatTransformers)
      }
      return ChatMessage(author: .assistant, editor: editor)
    }
  }
}

extension Editor {
  /// Gives the editor's view the keyboard focus.
  @MainActor
  func focusView() {
    try? focus()
  }
}
