import Lexical
import LexicalLink
import LexicalSelection
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

// Port of packages/lexical-playground/src/plugins/FloatingLinkEditorPlugin:
// while the editor is editable and the selection is in a link, a panel
// below the link shows its URL, which opens it, with buttons to edit and
// remove it. The toolbar's link button links the selection to `https://`
// and opens the panel's URL field. A Command (or Control) click on a link
// opens it, as the plugin's `CLICK_COMMAND` handler does.
//
// JS positions the panel with Floating UI, `bottom-start` with a 10-pixel
// offset, flipped above the link and shifted to stay 10 pixels inside the
// scroller. Here it is a subview of the text view at the link's first rect
// in the view's coordinates, so it scrolls with the text as the absolutely
// positioned panel does.

/// `sanitizeUrl` in the playground's utils/url.ts: a URL of an unsupported
/// protocol becomes `about:blank`.
func sanitizeUrl(_ url: String) -> String {
  guard let scheme = URL(string: url)?.scheme?.lowercased() else { return url }
  return ["http", "https", "mailto", "sms", "tel"].contains(scheme) ? url : "about:blank"
}

/// `getSelectedNode` in the playground's utils.
func getSelectedNode(_ selection: RangeSelection) throws -> LexicalNode {
  let anchorNode = try selection.anchor.getNode()
  let focusNode = try selection.focus.getNode()
  if anchorNode === focusNode { return anchorNode }
  if try selection.isBackward() {
    return try isAtNodeEnd(selection.focus) ? anchorNode : focusNode
  }
  return try isAtNodeEnd(selection.anchor) ? anchorNode : focusNode
}

/// `$getSelectedLinkNode`: the link the selection's node is or is in, or
/// for a caret at the end of a text node, the link right after it.
private func getSelectedLinkNode(_ selection: RangeSelection) throws -> LinkNode? {
  let node = try getSelectedNode(selection)
  if let link = node as? LinkNode { return link }
  if let link = try findMatchingParent(node, { $0 is LinkNode }) as? LinkNode { return link }
  if selection.isCollapsed(), selection.anchor.type == .text {
    let anchorNode = try selection.anchor.getNode()
    if selection.anchor.offset == (try anchorNode.getTextContentSize()) {
      return try anchorNode.getNextSibling() as? LinkNode
    }
  }
  return nil
}

/// The link the panel is for: `isLink` in `useFloatingLinkEditorToolbar`
/// (a selection inside one link and nothing else), with the link
/// `$updateLinkEditor` positions the panel at.
func floatingLink(_ selection: BaseSelection?) throws -> LinkNode? {
  if let selection = selection as? RangeSelection {
    let focusLinkNode = try getSelectedLinkNode(selection)
    let focusAutoLinkNode =
      try findMatchingParent(getSelectedNode(selection), { $0 is AutoLinkNode }) as? AutoLinkNode
    guard let link = focusLinkNode ?? focusAutoLinkNode else { return nil }
    for node in try selection.getNodes() where !(node is LineBreakNode) {
      let linkNode = try findMatchingParent(node, { $0 is LinkNode }) as? LinkNode
      let autoLinkNode = try findMatchingParent(node, { $0 is AutoLinkNode }) as? AutoLinkNode
      if let focusLinkNode, linkNode !== focusLinkNode { return nil }
      if let linkNode, linkNode !== focusLinkNode { return nil }
      if let focusAutoLinkNode, autoLinkNode !== focusAutoLinkNode { return nil }
      if let autoLinkNode, try autoLinkNode !== focusAutoLinkNode || autoLinkNode.getIsUnlinked() {
        return nil
      }
    }
    return link
  }
  if let selection = selection as? NodeSelection, let node = try selection.getNodes().first {
    return try (node as? LinkNode) ?? (node.getParent() as? LinkNode)
  }
  return nil
}

/// The panel (`FloatingLinkEditor`): the URL as a link, edit and remove, or
/// in edit mode the URL field with cancel and confirm.
struct FloatingLinkEditorView: View {
  let model: PlaygroundModel
  @FocusState private var isInputFocused: Bool
  @Environment(\.colorScheme) private var colorScheme

  /// The transparent margin around the panel, which its shadow draws in.
  static let shadowMargin: CGFloat = 16

  var body: some View {
    @Bindable var model = model
    HStack(spacing: 0) {
      if model.isLinkEditMode {
        // `.link-input`: margin 12, padding 8 × 12, radius 15, #eee.
        TextField("", text: $model.editedLinkURL)
          .textFieldStyle(.plain)
          .autocorrectionDisabled()
          #if os(iOS)
            .textInputAutocapitalization(.never)
            .keyboardType(.URL)
          #endif
          .focused($isInputFocused)
          .onSubmit { model.submitLink() }
          #if os(macOS)
            .onExitCommand { model.isLinkEditMode = false }
          #endif
          .padding(.vertical, 8)
          .padding(.horizontal, 12)
          .background(RoundedRectangle(cornerRadius: 15).fill(Color(white: 0.5, opacity: 0.15)))
          .padding(12)
          .accessibilityIdentifier("link-input")
          .onAppear { isInputFocused = true }
        iconButton("Cancel", "xmark") { model.isLinkEditMode = false }
        iconButton("Confirm", "checkmark") { model.submitLink() }
          .padding(.trailing, 6)
      } else {
        // `.link-view`: margin 8 × 12, padding 8 × 12; the `<a>` opens the
        // URL in a new tab.
        Button {
          PlaygroundLinkWindow().open(sanitizeUrl(model.linkEditorURL), "_blank", "noopener")
        } label: {
          // A browser's default link: underlined, #0000ee.
          Text(model.linkEditorURL)
            .underline()
            .foregroundStyle(
              colorScheme == .dark
                ? Color(red: 88 / 255, green: 166 / 255, blue: 1)
                : Color(red: 0, green: 0, blue: 238 / 255)
            )
            .multilineTextAlignment(.leading)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.plain)
        #if os(macOS)
          .pointerStyle(.link)
        #endif
        .accessibilityIdentifier("link-view")
        .padding(.vertical, 16)
        .padding(.leading, 24)
        iconButton("Edit link", "pencil") { model.editLink() }
        iconButton("Remove link", "trash") { model.removeLink() }
          .padding(.trailing, 6)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    // `.link-editor`: white, radius 8, `box-shadow: 0 5px 10px rgba(0, 0,
    // 0, 0.3)`.
    .background(
      RoundedRectangle(cornerRadius: 8).fill(.background)
        .shadow(color: .black.opacity(0.3), radius: 5, y: 5)
    )
    .padding(Self.shadowMargin)
  }

  /// A 35-point-wide icon button, as `.link-edit` and the others.
  private func iconButton(
    _ title: String, _ symbol: String, action: @escaping () -> Void
  ) -> some View {
    Button(title, systemImage: symbol, action: action)
      .labelStyle(.iconOnly)
      .buttonStyle(.borderless)
      .foregroundStyle(.primary)
      .frame(width: 35, height: 35)
      .contentShape(Rectangle())
      .help(title)
  }
}

/// Keeps the panel in the editor's text view, at its link.
@MainActor
final class FloatingLinkEditorHost {
  #if os(macOS)
    private let hostingController: NSHostingController<FloatingLinkEditorView>
    private weak var textView: LexicalNSTextView?
  #else
    private let hostingController: UIHostingController<FloatingLinkEditorView>
    private weak var textView: LexicalUITextView?
  #endif
  private var hostingView: PlatformView { hostingController.view }

  init(model: PlaygroundModel) {
    hostingController = .init(rootView: FloatingLinkEditorView(model: model))
    hostingController.sizingOptions = []
    #if os(iOS)
      hostingController.view.backgroundColor = .clear
    #endif
    hostingView.isHidden = true
  }

  /// Moves the panel into `root`, the editor's new root element.
  func attach(to root: AnyObject?) {
    hostingView.removeFromSuperview()
    #if os(macOS)
      textView = root as? LexicalNSTextView
    #else
      textView = root as? LexicalUITextView
    #endif
    textView?.addSubview(hostingView)
  }

  /// Shows the panel at the rendered node `key`, or hides it for `nil`.
  func position(at key: NodeKey?) {
    guard let textView, let key, let span = textView.controller.document.span(key),
      let rect = firstRect(of: NSRange(location: span.location, length: span.length))
    else {
      hostingView.isHidden = true
      return
    }
    let margin = FloatingLinkEditorView.shadowMargin
    #if os(macOS)
      let visible = textView.visibleRect
    #else
      let visible = textView.bounds.inset(by: textView.adjustedContentInset)
    #endif
    // `.link-editor { max-width: 400px; width: 100% }` of the scroller.
    let width = min(400, visible.width - 20)
    let height =
      hostingController.sizeThatFits(
        in: CGSize(width: width + 2 * margin, height: .greatestFiniteMagnitude)
      ).height - 2 * margin
    // `offset(10)` below the link, `flip()` above it when it does not fit
    // and `shift()` 10 points inside the scroller.
    var y = rect.maxY + 10
    if y + height > visible.maxY - 10, rect.minY - 10 - height >= visible.minY + 10 {
      y = rect.minY - 10 - height
    }
    let x = max(visible.minX + 10, min(rect.minX, visible.maxX - 10 - width))
    hostingView.frame = CGRect(
      x: x - margin, y: y - margin, width: width + 2 * margin, height: height + 2 * margin)
    hostingView.isHidden = false
  }

  /// The first line's rect of `range` in the text view.
  private func firstRect(of range: NSRange) -> CGRect? {
    guard let textView else { return nil }
    #if os(macOS)
      guard let window = textView.window else { return nil }
      let screen = textView.firstRect(forCharacterRange: range, actualRange: nil)
      guard !screen.isEmpty || screen.origin != .zero else { return nil }
      return textView.convert(window.convertFromScreen(screen), from: nil)
    #else
      guard
        let start = textView.position(from: textView.beginningOfDocument, offset: range.location),
        let end = textView.position(from: start, offset: range.length),
        let textRange = textView.textRange(from: start, to: end)
      else { return nil }
      let rect = textView.firstRect(for: textRange)
      return rect.isNull || rect.isInfinite ? nil : rect
    #endif
  }
}
