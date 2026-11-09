import Foundation
import Lexical
import LexicalMark
import LexicalText
import LexicalTextKit
import LexicalUtils
import Observation
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias PlatformView = NSView
  private typealias PlatformTextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias PlatformView = UIView
  private typealias PlatformTextView = LexicalUITextView
#endif

// Port of packages/lexical-playground/src/plugins/CommentPlugin and its
// store, src/commenting: comments on a selection, kept in marks
// (`@lexical/mark`) whose IDs are the threads'. The store is in memory, as
// the playground's is without collaboration.
//
// The web's comment editors (a nested plain text Lexical editor in the
// input box and each reply composer) are text fields here, which give the
// same text; the marks of the threads at the caret get `.selected`'s look
// as highlights (`TextKitInputController.highlights`), where the web adds
// the class to their elements.

/// `INSERT_INLINE_COMMAND`: opens the input box for a comment on the
/// selection.
let INSERT_INLINE_COMMAND = LexicalCommand<Void>("INSERT_INLINE_COMMAND")

/// `Comment` in commenting/index.ts.
struct PlaygroundComment: Identifiable, Equatable {
  var author: String
  var content: String
  var deleted = false
  var id = createUID()
  var timeStamp = Date()
}

/// `Thread`: the comments on one quote.
struct CommentThread: Identifiable, Equatable {
  var comments: [PlaygroundComment]
  var id = createUID()
  var quote: String
}

/// An entry of `Comments`: a thread, or a comment on its own.
enum CommentOrThread: Identifiable, Equatable {
  case comment(PlaygroundComment)
  case thread(CommentThread)

  var id: String {
    switch self {
    case .comment(let comment): comment.id
    case .thread(let thread): thread.id
    }
  }
}

/// `createUID`: up to five random lowercase letters.
private func createUID() -> String {
  String((0..<5).map { _ in "abcdefghijklmnopqrstuvwxyz".randomElement()! })
}

/// `CommentStore`, without collaboration.
@MainActor
@Observable
final class CommentStore {
  private(set) var comments: [CommentOrThread] = []

  /// Adds a thread (or a comment) at `offset`, by default the end, or with
  /// `thread`, a comment to it.
  func addComment(
    _ commentOrThread: CommentOrThread, thread: CommentThread? = nil, offset: Int? = nil
  ) {
    if let thread, case .comment(let comment) = commentOrThread {
      guard let i = comments.firstIndex(where: { $0.id == thread.id }),
        case .thread(var next) = comments[i]
      else { return }
      next.comments.insert(comment, at: offset ?? next.comments.count)
      comments[i] = .thread(next)
    } else {
      comments.insert(commentOrThread, at: offset ?? comments.count)
    }
  }

  /// Removes a comment of `thread`, returning where it was and its deleted
  /// stand-in (`markDeleted`), or removes a thread.
  @discardableResult
  func deleteCommentOrThread(
    _ commentOrThread: CommentOrThread, thread: CommentThread? = nil
  ) -> (
    index: Int, markedComment: PlaygroundComment
  )? {
    var commentIndex: Int?
    if let thread {
      if let i = comments.firstIndex(where: { $0.id == thread.id }),
        case .thread(var next) = comments[i]
      {
        commentIndex = next.comments.firstIndex { $0.id == commentOrThread.id }
        if let commentIndex { next.comments.remove(at: commentIndex) }
        comments[i] = .thread(next)
      }
    } else if let i = comments.firstIndex(where: { $0.id == commentOrThread.id }) {
      commentIndex = i
      comments.remove(at: i)
    }
    guard case .comment(var comment) = commentOrThread, let commentIndex else { return nil }
    comment.content = "[Deleted Comment]"
    comment.deleted = true
    return (commentIndex, comment)
  }
}

/// `CommentPlugin`'s state: the store, the threads' marks, the threads at
/// the caret and what is shown.
@MainActor
@Observable
final class CommentsModel {
  let store = CommentStore()
  /// `markNodeMap`: each thread's mark nodes.
  private(set) var markNodeMap: [String: Set<NodeKey>] = [:]
  /// The threads whose marks the caret is in.
  private(set) var activeIDs: [String] = []
  /// The text node a non-collapsed selection starts in, which the add
  /// comment button is beside.
  private(set) var activeAnchorKey: NodeKey?
  var showCommentInput = false {
    didSet {
      guard showCommentInput != oldValue else { return }
      refreshHosts()
      onInputChange?()
    }
  }
  /// Called when the input box opens or closes: the format popup hides
  /// while it is open, as the web's does without a DOM selection.
  @ObservationIgnored var onInputChange: (() -> Void)?
  var showComments = false
  /// `useCollabAuthorName` without collaboration.
  let author = "Playground User"

  @ObservationIgnored private weak var editor: Editor?
  @ObservationIgnored private weak var controller: TextKitInputController?
  /// The selection the input box comments on (`selectionRef`).
  @ObservationIgnored private var inputSelection: RangeSelection?
  @ObservationIgnored private lazy var addCommentHost = TextViewOverlayHost(
    rootView: AddCommentBox(comments: self))
  @ObservationIgnored private lazy var inputHost = TextViewOverlayHost(
    rootView: CommentInputBox(comments: self))
  @ObservationIgnored private lazy var scrolling = TextScrollObserver { [weak self] in
    self?.placeAddComment()
  }
  /// The add comment button shows (the web hides it below 600 pixels).
  var isWide = true {
    didSet { refreshHosts() }
  }

  /// Registers the plugin's listeners on `editor`.
  func register(_ editor: Editor) throws -> () -> Void {
    self.editor = editor
    markNodeMap = [:]
    activeIDs = []
    activeAnchorKey = nil
    showCommentInput = false
    var markNodeKeysToIDs: [NodeKey: [String]] = [:]
    return mergeRegister(
      try registerNestedElementResolver(
        editor, MarkNode.self, { from in try createMarkNode(from.getIDs()) },
        { from, to in for id in try from.getIDs() { _ = try to.addID(id) } }),
      try editor.registerMutationListener(
        MarkNode.self,
        { [weak self] mutations, _ in
          try editor.read(.latest) {
            var map = MainActor.assumeIsolated { self?.markNodeMap } ?? [:]
            for (key, mutation) in mutations {
              let ids: [String]
              if mutation == "destroyed" {
                ids = markNodeKeysToIDs[key] ?? []
              } else if let node = try getNodeByKey(key) as? MarkNode {
                ids = try node.getIDs()
              } else {
                ids = []
              }
              for id in ids {
                markNodeKeysToIDs[key] = ids
                if mutation == "destroyed" {
                  map[id]?.remove(key)
                  if map[id]?.isEmpty == true { map[id] = nil }
                } else {
                  map[id, default: []].insert(key)
                }
              }
            }
            MainActor.assumeIsolated { self?.markNodeMap = map }
          }
        }, skipInitialization: false),
      editor.registerUpdateListener { [weak self] payload in
        try payload.editorState.read {
          var ids: [String]?
          var anchorKey: NodeKey?
          let selection = try getSelection() as? RangeSelection
          if let selection, let anchorNode = try selection.anchor.getNode() as? TextNode {
            ids = try getMarkIDs(anchorNode, selection.anchor.offset)
            if !selection.isCollapsed() { anchorKey = anchorNode.getKey() }
          }
          let hidesInput = !payload.tags.has(COLLABORATION_TAG) && selection != nil
          MainActor.assumeIsolated {
            guard let self else { return }
            if self.activeIDs != ids ?? [] { self.activeIDs = ids ?? [] }
            self.activeAnchorKey = anchorKey
            if hidesInput { self.showCommentInput = false }
          }
        }
        MainActor.assumeIsolated { self?.refreshHosts() }
      },
      editor.registerCommand(
        INSERT_INLINE_COMMAND,
        { [weak self] _, _ in
          // The web drops the DOM selection; the input box keeps the
          // editor's to comment on.
          let selection = try getSelection() as? RangeSelection
          MainActor.assumeIsolated {
            self?.inputSelection = selection?.cloneRange()
            self?.showCommentInput = true
          }
          return true
        }, COMMAND_PRIORITY_EDITOR))
  }

  /// Moves the button and the input box into `root`, the editor's text
  /// view, and moves the button as the text scrolls under the Show Comments
  /// button.
  func attach(to root: AnyObject?) {
    controller = textController(root)
    addCommentHost.attach(to: root)
    inputHost.attach(to: root)
    scrolling.observe(root)
    refreshHighlights()
    refreshHosts()
  }

  // MARK: Actions

  /// The add comment button (`onAddComment`).
  func addComment() {
    _ = try? editor?.dispatchCommand(INSERT_INLINE_COMMAND)
  }

  /// `cancelAddComment`: closes the input box and gives the selection back.
  func cancelAddComment() {
    try? editor?.update {
      if let selection = try getSelection() { selection.dirty = true }
    }
    showCommentInput = false
    inputSelection = nil
    focusEditor()
  }

  /// The input box's Comment button: a thread on the selection's text
  /// (`submitComment` and `submitAddComment`).
  func submitInlineComment(_ content: String) {
    guard let editor, let selection = inputSelection,
      !content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    else { return }
    var quote = (try? editor.read(.latest) { try selection.getTextContent() }) ?? ""
    if quote.utf16.count > 100 {
      quote = String(utf16CodeUnits: Array(quote.utf16.prefix(99)), count: 99) + "…"
    }
    let thread = CommentThread(
      comments: [PlaygroundComment(author: author, content: content)], quote: quote)
    store.addComment(.thread(thread))
    try? editor.update {
      try wrapSelectionInMarkNode(selection, selection.isBackward(), thread.id)
    }
    showCommentInput = false
    inputSelection = nil
  }

  /// A thread's reply composer.
  func submitReply(_ content: String, to thread: CommentThread) {
    guard !content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
    store.addComment(.comment(PlaygroundComment(author: author, content: content)), thread: thread)
  }

  /// `deleteCommentOrThread`: a comment becomes "[Deleted Comment]"; a
  /// thread goes, and its ID leaves its marks, which unwrap once they have
  /// none.
  func delete(_ commentOrThread: CommentOrThread, in thread: CommentThread? = nil) {
    if case .comment = commentOrThread {
      guard let info = store.deleteCommentOrThread(commentOrThread, thread: thread) else { return }
      store.addComment(.comment(info.markedComment), thread: thread, offset: info.index)
      return
    }
    store.deleteCommentOrThread(commentOrThread)
    let id = thread?.id ?? commentOrThread.id
    guard let keys = markNodeMap[id] else { return }
    DispatchQueue.main.async { [weak self] in
      try? self?.editor?.update {
        for key in keys {
          if let node = try getNodeByKey(key) as? MarkNode {
            _ = try node.deleteID(id)
            if try node.getIDs().isEmpty { try unwrapMarkNode(node) }
          }
        }
      }
    }
  }

  /// `handleClickThread`: puts the caret at the start of the thread's
  /// first mark, unless it is in one already.
  func selectThread(_ id: String) {
    guard let keys = markNodeMap[id], !activeIDs.contains(id), let key = keys.first else { return }
    try? editor?.update {
      _ = try (getNodeByKey(key) as? MarkNode)?.selectStart()
    }
    focusEditor()
  }

  // MARK: Rendering

  /// The selected look of the active threads' marks (`.selected`), which
  /// also opens the panel, and while the input box is open, the selection
  /// it comments on.
  private func refreshHighlights() {
    guard let controller, let editor else { return }
    var selected: [TextKitHighlightRange] = []
    var overlapping: [TextKitHighlightRange] = []
    try? editor.read(.latest) {
      for id in activeIDs {
        for key in markNodeMap[id] ?? [] {
          guard let mark = try getNodeByKey(key) as? MarkNode else { continue }
          let range = TextKitHighlightRange.node(
            key, isText: false, length: try mark.getChildrenSize())
          if try mark.getIDs().count > 1 {
            overlapping.append(range)
          } else {
            selected.append(range)
          }
          showComments = true
        }
      }
    }
    controller.highlights["comment-selected"] =
      selected.isEmpty
      ? nil : TextKitHighlight(ranges: selected, backgroundColor: markYellow(0.5), priority: 1)
    controller.highlights["comment-selected-overlap"] =
      overlapping.isEmpty
      ? nil : TextKitHighlight(ranges: overlapping, backgroundColor: markYellow(0.7), priority: 2)
    // `CommentInputBox`'s spans over the selection's rects.
    if showCommentInput, let selection = inputSelection {
      controller.highlights["comment-input"] = TextKitHighlight(
        ranges: [
          TextKitHighlightRange(
            anchorKey: selection.anchor.key, anchorOffset: selection.anchor.offset,
            anchorType: selection.anchor.type, focusKey: selection.focus.key,
            focusOffset: selection.focus.offset, focusType: selection.focus.type)
        ], backgroundColor: markYellow(0.3), priority: 3)
    } else {
      controller.highlights["comment-input"] = nil
    }
  }

  private func markYellow(_ alpha: Double) -> TextKitColor {
    TextKitColor(red: 1, green: 212 / 255, blue: 0, alpha: alpha)
  }

  /// Places the add comment button at the right of the editor, level with
  /// the selection's first text node, and the input box below the
  /// selection, once the view has laid out the update. The playground
  /// calls it when the editor resizes too, as the web's boxes follow the
  /// window's `resize`.
  func refreshHosts() {
    refreshHighlights()
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      // In a compact width the playground shows the box as a sheet.
      self.inputHost.place(self.showCommentInput && self.isWide ? self.inputFrame() : nil)
      self.placeAddComment()
    }
  }

  private func placeAddComment() {
    addCommentHost.place(!showCommentInput && isWide ? addCommentFrame() : nil)
  }

  /// `AddCommentBox`'s `updatePosition`: at the editor's right edge, 30
  /// above the anchor's element. The web centers it on the edge, outside
  /// the editor; here it stays inside the text view it scrolls with.
  ///
  /// The web's Show Comments button is in the page's top right corner,
  /// above the editor. Here it is in the editor's, over the text, so where
  /// the box would be under it (a selection in the first line, or in a
  /// line scrolled to the top) the box goes below it.
  private func addCommentFrame() -> CGRect? {
    guard let key = activeAnchorKey, let textView = addCommentHost.textView,
      let anchor = try? editor?.read(
        .latest,
        { () -> Point? in
          guard try getNodeByKey(key) != nil else { return nil }
          return createPoint(key, 0, .text)
        }), let rect = textView.rect(from: anchor, to: anchor)
    else { return nil }
    let visible = Self.visibleRect(textView)
    var frame = CGRect(x: visible.maxX - 50, y: rect.minY - 30, width: 40, height: 60)
    // The Show Comments button's corner, with its margin on each side.
    let editor = Self.editorRect(textView)
    let corner = ShowCommentsButton.side + 2 * ShowCommentsButton.margin
    let showComments = CGRect(
      x: editor.maxX - corner, y: editor.minY, width: corner, height: corner)
    if frame.intersects(showComments) { frame.origin.y = showComments.maxY }
    return frame
  }

  /// `CommentInputBox`'s `updateLocation`: 250 wide, 20 below the
  /// selection, centered under a one-line selection or 125 left of a
  /// longer one's start, and at least 10 from the left.
  private func inputFrame() -> CGRect? {
    guard let selection = inputSelection, let textView = inputHost.textView,
      let rect = textView.rect(from: selection.anchor, to: selection.focus),
      let start = textView.rect(from: selection.anchor, to: selection.anchor),
      let end = textView.rect(from: selection.focus, to: selection.focus)
    else { return nil }
    let isOneLine = abs(start.minY - end.minY) < 1
    let left = max(10, isOneLine ? rect.midX - 125 : min(start.minX, end.minX) - 125)
    let height = inputHost.fittingHeight(width: 250)
    return CGRect(x: left, y: rect.maxY + 20, width: 250, height: height)
  }

  /// The editor's frame, which the Show Comments button is over, in the
  /// text view's coordinates.
  private static func editorRect(_ textView: PlatformTextView) -> CGRect {
    #if os(macOS)
      textView.enclosingScrollView.map { textView.convert($0.bounds, from: $0) }
        ?? textView.visibleRect
    #else
      textView.bounds
    #endif
  }

  private static func visibleRect(_ textView: PlatformTextView) -> CGRect {
    #if os(macOS)
      textView.visibleRect
    #else
      textView.bounds.inset(by: textView.adjustedContentInset)
    #endif
  }

  private func focusEditor() {
    guard let textView = inputHost.textView else { return }
    #if os(macOS)
      textView.window?.makeFirstResponder(textView)
    #else
      textView.becomeFirstResponder()
    #endif
  }
}

/// A SwiftUI view kept in the editor's text view, so it scrolls with the
/// text as the web's absolutely positioned boxes do.
@MainActor
private final class TextViewOverlayHost<Content: View> {
  #if os(macOS)
    private let hostingController: NSHostingController<Content>
  #else
    private let hostingController: UIHostingController<Content>
  #endif
  private(set) weak var textView: PlatformTextView?

  init(rootView: Content) {
    hostingController = .init(rootView: rootView)
    hostingController.sizingOptions = []
    #if os(iOS)
      hostingController.view.backgroundColor = .clear
    #endif
    hostingController.view.isHidden = true
  }

  func attach(to root: AnyObject?) {
    hostingController.view.removeFromSuperview()
    textView = root as? PlatformTextView
    textView?.addSubview(hostingController.view)
  }

  func fittingHeight(width: CGFloat) -> CGFloat {
    hostingController.sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude))
      .height
  }

  /// Shows the view at `frame`, or hides it for `nil`.
  func place(_ frame: CGRect?) {
    guard let frame else {
      hostingController.view.isHidden = true
      return
    }
    hostingController.view.frame = frame
    hostingController.view.isHidden = false
  }
}

// MARK: Views

/// `AddCommentBox`: a 40 × 60 white pill with the add comment icon.
private struct AddCommentBox: View {
  let comments: CommentsModel

  var body: some View {
    Button {
      comments.addComment()
    } label: {
      Image(systemName: "text.bubble")
        .scaledFont(size: 18, relativeTo: .title3)
        .foregroundStyle(.primary.opacity(0.7))
        .frame(width: 40, height: 60)
        .contentShape(RoundedRectangle(cornerRadius: 20))
    }
    .buttonStyle(.plain)
    .background(
      RoundedRectangle(cornerRadius: 20).fill(Color.panelBackground)
        .shadow(color: .black.opacity(0.2), radius: 1.5)
    )
    .help("Add comment")
    .accessibilityLabel("Add comment")
    .accessibilityIdentifier("add-comment")
  }
}

/// `CommentInputBox`: the comment's text with Cancel and Comment.
struct CommentInputBox: View {
  let comments: CommentsModel
  @State private var content = ""
  @FocusState private var isFocused: Bool
  @Environment(\.dynamicTypeSize) private var dynamicTypeSize
  /// `.CommentPlugin_CommentInputBox`'s 250 pixels, at the text's size.
  @ScaledMetric(relativeTo: .subheadline) private var width: CGFloat = 250

  private var canSubmit: Bool {
    !content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
  }

  var body: some View {
    VStack(spacing: 0) {
      TextField("Type a comment...", text: $content, axis: .vertical)
        .textFieldStyle(.plain)
        .scaledFont(size: 15, relativeTo: .subheadline)
        .lineLimit(3...)
        .focused($isFocused)
        #if os(macOS)
          .onExitCommand { comments.cancelAddComment() }
        #endif
        .padding(EdgeInsets(top: 9, leading: 9, bottom: 10, trailing: 10))
        .overlay(
          RoundedRectangle(cornerRadius: 5)
            .stroke(
              isFocused
                ? Color(red: 66 / 255, green: 135 / 255, blue: 245 / 255) : .webLine(0.8))
        )
        .padding(10)
        .accessibilityIdentifier("comment-input")
      // The buttons stack at the accessibility sizes, where their labels
      // do not fit side by side.
      let buttons =
        dynamicTypeSize.isAccessibilitySize
        ? AnyLayout(VStackLayout(spacing: 10)) : AnyLayout(HStackLayout(spacing: 10))
      buttons {
        Button("Cancel") {
          content = ""
          comments.cancelAddComment()
        }
        .buttonStyle(CommentButtonStyle(isPrimary: false))
        Button("Comment") {
          comments.submitInlineComment(content)
          content = ""
        }
        .buttonStyle(CommentButtonStyle(isPrimary: canSubmit))
        .disabled(!canSubmit)
        .accessibilityIdentifier("comment-submit")
      }
      .padding([.horizontal, .bottom], 10)
    }
    .frame(maxWidth: width)
    .frame(minHeight: 80)
    .background(
      RoundedRectangle(cornerRadius: 5).fill(Color.panelBackground)
        .shadow(color: .black.opacity(0.1), radius: 2.5)
    )
    .onChange(of: comments.showCommentInput, initial: true) { _, isShown in
      guard isShown else { return }
      content = ""
      // Once the host shows the box (`AutoFocusPlugin`).
      DispatchQueue.main.async { isFocused = true }
    }
  }
}

/// The playground's `Button`, and `.primary` for the input box's Comment.
private struct CommentButtonStyle: ButtonStyle {
  let isPrimary: Bool
  @Environment(\.isEnabled) private var isEnabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .scaledFont(size: 14, weight: isPrimary ? .bold : .regular, relativeTo: .subheadline)
      .foregroundStyle(isPrimary ? .white : .webText(0.27))
      .frame(maxWidth: .infinity)
      .padding(.vertical, 10)
      .background(
        RoundedRectangle(cornerRadius: 5).fill(
          isPrimary
            ? Color(red: 66 / 255, green: 135 / 255, blue: 245 / 255)
            : .webFill(0.93)
        )
      )
      .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.5)
  }
}

/// `CommentPlugin_ShowCommentsButton`.
struct ShowCommentsButton: View {
  /// Its side, and its margin from the editor's top right corner
  /// (`top: 10px; right: 10px`), where the add comment button keeps clear
  /// of it.
  static let side: CGFloat = 36
  static let margin: CGFloat = 10

  @Bindable var comments: CommentsModel

  var body: some View {
    Button {
      comments.showComments.toggle()
    } label: {
      Image(systemName: "bubble.left.and.text.bubble.right")
        .scaledFont(size: 16, relativeTo: .callout)
        .foregroundStyle(.primary.opacity(comments.showComments ? 1 : 0.6))
        .frame(width: Self.side, height: Self.side)
        .background(
          RoundedRectangle(cornerRadius: 10)
            .fill(Color.webFill(comments.showComments ? 0.8 : 0.867)))
    }
    .buttonStyle(.plain)
    .help(comments.showComments ? "Hide Comments" : "Show Comments")
    .accessibilityLabel(comments.showComments ? "Hide Comments" : "Show Comments")
    .accessibilityIdentifier("show-comments")
    // It floats over the editor's text, so it grows only as a bar's
    // buttons do.
    .accessibilityShowsLargeContentViewer()
    .barDynamicTypeSize()
  }
}

/// `CommentsPanel`: the threads, each with its quote, comments and a reply
/// composer.
struct CommentsPanel: View {
  let comments: CommentsModel

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text("Comments")
        .scaledFont(size: 20, relativeTo: .title3)
        .foregroundStyle(Color.webText(0.27))
        .padding(.leading, 15)
        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
      Divider().overlay(Color.webLine(0.93))
      if comments.store.comments.isEmpty {
        Text("No Comments")
          .foregroundStyle(Color.webText(0.47))
          .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        ScrollViewReader { proxy in
          ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
              ForEach(comments.store.comments) { item in
                switch item {
                case .thread(let thread):
                  ThreadView(comments: comments, thread: thread).id(thread.id)
                case .comment(let comment): CommentRow(comments: comments, comment: comment)
                }
              }
            }
          }
          .onChange(of: comments.activeIDs) { _, ids in
            if let id = ids.first { withAnimation { proxy.scrollTo(id) } }
          }
        }
      }
    }
    .frame(width: 300)
    .frame(maxHeight: .infinity)
    .background(
      UnevenRoundedRectangle(topLeadingRadius: 10).fill(Color.panelBackground)
        .shadow(color: .black.opacity(0.1), radius: 5)
    )
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("comments-panel")
  }
}

/// `CommentsPanelList`'s thread item.
private struct ThreadView: View {
  let comments: CommentsModel
  let thread: CommentThread
  @State private var reply = ""
  @State private var isConfirmingDelete = false

  private var isActive: Bool { comments.activeIDs.contains(thread.id) }

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack(alignment: .top) {
        Text("\(Text("> ").foregroundStyle(Color.webText(0.8)))\(Text(Self.quote(thread.quote)))")
          .scaledFont(size: 15, relativeTo: .subheadline)
          .lineSpacing(4)
          .padding(.horizontal, 10)
          .padding(.top, 10)
          .frame(maxWidth: .infinity, alignment: .leading)
        DeleteButton { isConfirmingDelete = true }
      }
      ForEach(thread.comments) { comment in
        CommentRow(comments: comments, comment: comment, thread: thread)
          .padding(.leading, 5)
      }
      // `CommentsComposer`: a reply, sent with the send button.
      HStack(spacing: 0) {
        TextField("Reply to comment...", text: $reply, axis: .vertical)
          .textFieldStyle(.plain)
          .scaledFont(size: 15, relativeTo: .subheadline)
          .onSubmit(send)
          .accessibilityIdentifier("comment-reply")
        Button(action: send) {
          Image(systemName: "paperplane")
            .scaledFont(size: 16, relativeTo: .callout)
            .foregroundStyle(.primary.opacity(canSend ? 0.5 : 0.3))
        }
        .buttonStyle(.plain)
        .disabled(!canSend)
        .help("Send")
        .accessibilityLabel("Send")
      }
      .padding(EdgeInsets(top: 9, leading: 9, bottom: 10, trailing: 10))
      .background(RoundedRectangle(cornerRadius: 5).stroke(Color.webLine(0.8)))
      .padding(10)
    }
    .background(isActive ? Color.webFill(0.98) : .clear)
    .overlay(alignment: .leading) {
      if isActive { Rectangle().fill(Color.webFill(0.93)).frame(width: 15) }
    }
    .padding(.leading, isActive ? 15 : 0)
    .overlay(alignment: .top) { Divider().overlay(Color.webLine(0.93)) }
    .contentShape(Rectangle())
    .onTapGesture { comments.selectThread(thread.id) }
    .alert("Delete Thread", isPresented: $isConfirmingDelete) {
      Button("Delete", role: .destructive) { comments.delete(.thread(thread)) }
      Button("Cancel", role: .cancel) {}
    } message: {
      Text("Are you sure you want to delete this thread?")
    }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("comment-thread")
  }

  /// The quote's text, on `rgba(255, 212, 0, 0.4)`.
  private static func quote(_ quote: String) -> AttributedString {
    var text = AttributedString(quote)
    text.foregroundColor = Color.webText(0.13)
    text.backgroundColor = Color(red: 1, green: 212 / 255, blue: 0, opacity: 0.4)
    return text
  }

  private var canSend: Bool { !reply.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

  private func send() {
    guard canSend else { return }
    comments.submitReply(reply, to: thread)
    reply = ""
  }
}

/// `CommentsPanelListComment`: the author, how long ago, the text and a
/// delete button.
private struct CommentRow: View {
  let comments: CommentsModel
  let comment: PlaygroundComment
  var thread: CommentThread?
  @State private var isConfirmingDelete = false

  var body: some View {
    HStack(alignment: .top) {
      VStack(alignment: .leading, spacing: 5) {
        // Refreshed every 10 seconds, as the list's counter does.
        TimelineView(.periodic(from: .now, by: 10)) { context in
          let author = Text(comment.author).bold().foregroundStyle(Color.webText(0.27))
          let time = Text(" · " + Self.time(comment.timeStamp, context.date))
            .foregroundStyle(Color.webText(0.6))
          Text("\(author)\(time)")
        }
        Text(comment.content)
          .foregroundStyle(Color.webText(0.27))
          .opacity(comment.deleted ? 0.5 : 1)
      }
      .scaledFont(size: 14, relativeTo: .subheadline)
      .frame(maxWidth: .infinity, alignment: .leading)
      if !comment.deleted { DeleteButton { isConfirmingDelete = true } }
    }
    .padding(EdgeInsets(top: 15, leading: 15, bottom: 15, trailing: 0))
    .overlay(alignment: .leading) {
      if thread != nil { Rectangle().fill(Color.webFill(0.93)).frame(width: 5) }
    }
    .alert("Delete Comment", isPresented: $isConfirmingDelete) {
      Button("Delete", role: .destructive) { comments.delete(.comment(comment), in: thread) }
      Button("Cancel", role: .cancel) {}
    } message: {
      Text("Are you sure you want to delete this comment?")
    }
    .accessibilityElement(children: .combine)
    .accessibilityIdentifier("comment")
  }

  /// "Just now" within 10 seconds, then minutes as
  /// `Intl.RelativeTimeFormat('en', {numeric: 'auto', style: 'short'})`
  /// writes them.
  static func time(_ timeStamp: Date, _ now: Date) -> String {
    let seconds = (timeStamp.timeIntervalSince(now)).rounded()
    if seconds > -10 { return "Just now" }
    let formatter = RelativeDateTimeFormatter()
    formatter.locale = Locale(identifier: "en")
    formatter.unitsStyle = .short
    formatter.dateTimeStyle = .named
    return formatter.localizedString(from: DateComponents(minute: Int((seconds / 60).rounded())))
  }
}

/// `CommentPlugin_CommentsPanel_List_DeleteButton`.
private struct DeleteButton: View {
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      Image(systemName: "trash")
        .scaledFont(size: 13, relativeTo: .footnote)
        .foregroundStyle(.primary.opacity(0.5))
        .frame(width: 30, height: 30)
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .padding(.trailing, 10)
    .padding(.top, 5)
    .help("Delete")
    .accessibilityLabel("Delete")
  }
}
