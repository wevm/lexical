import Lexical
import LexicalCodeCore
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias PlatformView = NSView
#else
  import UIKit
  import LexicalUIKit
  private typealias PlatformView = UIView
#endif

// Port of packages/lexical-playground/src/plugins/CodeActionMenuPlugin: at
// the top right of a code block, 8 points in, the block's language
// ("(No language)" without one) and a copy button, which shows a check for
// a second after copying. Prettier's button is not ported.
//
// JS shows it while the mouse is over a code block, listening to
// `mousemove` while the document has one. macOS does the same with a
// tracking area on the text view. iOS has no hover, so there the menu is
// shown while the selection is in a code block (parity/deviations.json).
// The menu is a subview of the text view, so it scrolls with the block.

/// What the menu shows.
@MainActor @Observable
final class CodeActionMenuState {
  var language = ""
  var isCopyCompleted = false
  var copy: () -> Void = {}
}

/// `CodeActionMenuContainer`: `.code-action-menu-container`, 10-point text
/// at 50% of the text color, with `.menu-item` buttons.
struct CodeActionMenuView: View {
  let state: CodeActionMenuState

  var body: some View {
    HStack(spacing: 4) {
      Text(state.language.isEmpty ? "(No language)" : getLanguageFriendlyName(state.language))
        .accessibilityIdentifier("code-highlight-language")
      Button {
        state.copy()
      } label: {
        Image(systemName: state.isCopyCompleted ? "checkmark" : "doc.on.doc")
          .frame(width: 16, height: 16)
          .opacity(0.6)
      }
      .buttonStyle(CodeActionButtonStyle())
      .accessibilityLabel("copy")
      .accessibilityIdentifier("code-copy")
    }
    .font(.system(size: 10))
    .foregroundStyle(.primary.opacity(0.5))
    .frame(height: 35.8)
    .fixedSize()
  }
}

/// `button.menu-item`: 4-point padding, radius 4, a border on hover and
/// `rgb(223, 232, 250)` while pressed.
private struct CodeActionButtonStyle: ButtonStyle {
  @State private var isHovered = false

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .padding(4)
      .background(
        RoundedRectangle(cornerRadius: 4).fill(
          configuration.isPressed ? Color.accentColor.opacity(0.2) : .clear)
      )
      .overlay(
        RoundedRectangle(cornerRadius: 4).strokeBorder(
          .primary.opacity(configuration.isPressed ? 0.45 : isHovered ? 0.3 : 0), lineWidth: 1)
      )
      .contentShape(Rectangle())
      .onHover { isHovered = $0 }
  }
}

/// The block the menu is for is gone.
private struct CodeActionMenuGone: Error {}

/// Keeps the menu in the editor's text view, at the code block it is for.
@MainActor
final class CodeActionMenuHost {
  private let state = CodeActionMenuState()
  private weak var editor: Editor?
  /// The block the menu is at.
  private var codeKey: NodeKey?
  private var copyTimer: Timer?
  #if os(macOS)
    private let hostingController: NSHostingController<CodeActionMenuView>
    private weak var textView: LexicalNSTextView?
    private lazy var tracker = MouseTracker { [weak self] in self?.mouseMoved($0) }
    private var trackingArea: NSTrackingArea?
    /// The code blocks the editor has (`codeSetRef`).
    private var codeKeys: Set<NodeKey> = []
    private var unregisterMutations: (() -> Void)?
  #else
    private let hostingController: UIHostingController<CodeActionMenuView>
    private weak var textView: LexicalUITextView?
  #endif
  private var hostingView: PlatformView { hostingController.view }

  init() {
    hostingController = .init(rootView: CodeActionMenuView(state: state))
    hostingController.sizingOptions = []
    #if os(iOS)
      hostingView.backgroundColor = .clear
    #endif
    hostingView.isHidden = true
    state.copy = { [weak self] in self?.copy() }
  }

  /// Moves the menu into `root`, the editor's new root element.
  func attach(to root: AnyObject?, editor: Editor) {
    self.editor = editor
    hostingView.removeFromSuperview()
    show(nil)
    #if os(macOS)
      if let trackingArea { textView?.removeTrackingArea(trackingArea) }
      trackingArea = nil
      unregisterMutations?()
      codeKeys = []
      textView = root as? LexicalNSTextView
      guard let textView else { return }
      textView.addSubview(hostingView)
      let area = NSTrackingArea(
        rect: .zero,
        options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
        owner: tracker)
      textView.addTrackingArea(area)
      trackingArea = area
      unregisterMutations = try? editor.registerMutationListener(
        CodeNode.self,
        { [weak self] mutations, _ in
          MainActor.assumeIsolated {
            guard let self else { return }
            for (key, mutation) in mutations {
              switch mutation {
              case "created": self.codeKeys.insert(key)
              case "destroyed": self.codeKeys.remove(key)
              default: break
              }
            }
            if self.codeKeys.isEmpty { self.show(nil) }
          }
        }, skipInitialization: false)
    #else
      textView = root as? LexicalUITextView
      textView?.addSubview(hostingView)
    #endif
  }

  /// Follows an update: keeps the menu at its block once the view has laid
  /// the update out, and on iOS moves it to `selectedCodeKey`, the code
  /// block the selection is in.
  func refresh(selectedCodeKey: NodeKey?) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      #if os(macOS)
        self.show(self.codeKey)
      #else
        self.show(selectedCodeKey)
      #endif
    }
  }

  /// Shows the menu at code block `key`, or hides it for `nil`.
  private func show(_ key: NodeKey?) {
    if key != codeKey { state.isCopyCompleted = false }
    codeKey = key
    guard let key, let textView, let editor, let rect = textView.blockRect(for: key),
      let language = try? editor.read({ () throws -> String in
        guard let code = try getNodeByKey(key) as? CodeNode else { throw CodeActionMenuGone() }
        return try code.getLanguage() ?? ""
      })
    else {
      codeKey = nil
      hostingView.isHidden = true
      return
    }
    state.language = language
    let size = hostingController.sizeThatFits(
      in: CGSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude))
    // `right: editorRight - codeRight + CODE_PADDING`, `top: codeTop`.
    hostingView.frame = CGRect(
      x: rect.maxX - 8 - size.width, y: rect.minY, width: size.width, height: size.height)
    hostingView.isHidden = false
  }

  /// `CopyButton`'s `handleClick`: copies the block's text.
  private func copy() {
    guard let key = codeKey,
      let content = try? editor?.read({ try getNodeByKey(key)?.getTextContent() })
    else { return }
    #if os(macOS)
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(content, forType: .string)
    #else
      UIPasteboard.general.string = content
    #endif
    state.isCopyCompleted = true
    copyTimer?.invalidate()
    copyTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { [weak self] _ in
      MainActor.assumeIsolated { self?.state.isCopyCompleted = false }
    }
  }

  #if os(macOS)
    /// `getMouseInfo`: the code block under the mouse, keeping the menu
    /// while the mouse is over it.
    private func mouseMoved(_ event: NSEvent?) {
      guard let textView, let event, !codeKeys.isEmpty else { return show(nil) }
      let point = textView.convert(event.locationInWindow, from: nil)
      if !hostingView.isHidden, hostingView.frame.contains(point) { return }
      show(codeKeys.first { textView.blockRect(for: $0)?.contains(point) == true })
    }
  #endif
}

#if os(macOS)
  /// The tracking area's owner, which forwards the mouse's moves (`nil` when
  /// it leaves the view).
  final class MouseTracker: NSResponder {
    let onMove: (NSEvent?) -> Void

    init(_ onMove: @escaping (NSEvent?) -> Void) {
      self.onMove = onMove
      super.init()
    }

    required init?(coder: NSCoder) { fatalError() }

    override func mouseMoved(with event: NSEvent) { onMove(event) }
    override func mouseEntered(with event: NSEvent) { onMove(event) }
    override func mouseExited(with event: NSEvent) { onMove(nil) }
  }
#endif
