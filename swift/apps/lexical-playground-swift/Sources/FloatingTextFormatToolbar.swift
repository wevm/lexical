import Lexical
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
#else
  import UIKit
  import LexicalUIKit
#endif

// Port of packages/lexical-playground/src/plugins/FloatingTextFormatToolbarPlugin:
// while text is selected (not in a code block or a link, and not while an
// IME composes), the formats the selection has and buttons to toggle them.
//
// On macOS it is the web's popup, a row of buttons above the selection
// (`setFloatingElemPosition`: 10 points above, 5 to the left, below the
// selection when there is no room above, kept inside the scroller), in a
// child panel of the window that never takes the focus from the text view.
//
// On iOS the same actions join the system edit menu, which UIKit shows
// above a selection where the popup would go; the web playground leaves
// the popup out of narrow viewports as well (`isSmallWidthViewport`). See
// `native/ios-format-toolbar-in-edit-menu` in parity/deviations.json.

/// The popup's format buttons, in its order: the format, the button's
/// title, its `aria-label`, and an SF Symbol for the `i.format` icon
/// (`glyph:` and letters where the icon is letters).
let floatingToolbarFormats:
  [(format: TextFormatType, title: String, label: String, symbol: String)] = [
    ("bold", "Bold", "Format text as bold", "bold"),
    ("italic", "Italic", "Format text as italics", "italic"),
    ("underline", "Underline", "Format text to underlined", "underline"),
    ("strikethrough", "Strikethrough", "Format text with a strikethrough", "strikethrough"),
    ("subscript", "Subscript", "Format Subscript", "textformat.subscript"),
    ("superscript", "Superscript", "Format Superscript", "textformat.superscript"),
    (
      "uppercase", "Uppercase", "Format text to uppercase",
      "glyph:AB"
    ),
    ("lowercase", "Lowercase", "Format text to lowercase", "glyph:ab"),
    ("capitalize", "Capitalize", "Format text to capitalize", "glyph:Ab"),
    ("code", "Insert code block", "Insert code block", "chevron.left.forwardslash.chevron.right"),
  ]

/// `.floating-text-format-popup`.
struct FloatingTextFormatToolbarView: View {
  let model: PlaygroundModel
  /// The room around the popup for its shadow.
  static let shadowMargin: CGFloat = 12

  var body: some View {
    HStack(spacing: 2) {
      ForEach(floatingToolbarFormats, id: \.format) { item in
        button(
          item.title, item.label, item.symbol, active: model.floatingFormats.contains(item.format)
        ) { model.toggleFormat(item.format) }
      }
      button("Insert link", "Insert link", "link", active: model.isLink) { model.insertLink() }
      button("Insert comment", "Insert comment", "text.bubble", active: false) {
        model.insertComment()
      }
    }
    .padding(4)
    .frame(height: 43)
    // White, `box-shadow: 0px 5px 10px rgba(0, 0, 0, 0.3)`.
    .floatingSurface()
    .padding(Self.shadowMargin)
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Floating text format toolbar")
    .accessibilityIdentifier("floating-text-format-toolbar")
  }

  /// An SF Symbol, or `glyph:` and text for the case buttons, whose web
  /// icons are letters.
  @ViewBuilder private func icon(_ symbol: String) -> some View {
    if symbol.hasPrefix("glyph:") {
      Text(symbol.dropFirst(6)).scaledFont(size: 13, weight: .semibold, relativeTo: .footnote)
    } else {
      Image(systemName: symbol).scaledFont(size: 15, relativeTo: .subheadline)
    }
  }

  /// `button.popup-item.active`: `rgba(223, 232, 250, 0.3)`; the accent in
  /// a dark appearance.
  private var activeBackground: Color {
    Color(
      light: Color(red: 223 / 255, green: 232 / 255, blue: 250 / 255).opacity(0.3),
      dark: Color.accentColor.opacity(0.35))
  }

  private func button(
    _ title: String, _ label: String, _ symbol: String, active: Bool,
    action: @escaping () -> Void
  ) -> some View {
    Button(action: action) {
      icon(symbol)
        .frame(width: 18, height: 18)
        // Black, at 0.6 while inactive.
        .foregroundStyle(Color.primary.opacity(active ? 1 : 0.6))
        .padding(8)
        .background(
          RoundedRectangle(cornerRadius: 10)
            .fill(active ? activeBackground : .clear)
        )
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .help(title)
    .accessibilityLabel(label)
  }
}

/// Keeps the popup above the selection on macOS, and the edit menu's
/// format actions on iOS.
@MainActor
final class FloatingTextFormatToolbarHost: TypeaheadMenuAttaching {
  private weak var model: PlaygroundModel?
  #if os(macOS)
    private weak var textView: LexicalNSTextView?
    private let panel = MenuPanel()
    private let hostingController: NSHostingController<FloatingTextFormatToolbarView>
    private lazy var scrolling = TextScrollObserver { [weak self] in self?.position() }
  #endif

  init(model: PlaygroundModel) {
    self.model = model
    #if os(macOS)
      hostingController = NSHostingController(rootView: FloatingTextFormatToolbarView(model: model))
      hostingController.sizingOptions = []
      panel.contentView = hostingController.view
    #endif
  }

  func attach(to root: AnyObject?) {
    #if os(macOS)
      hide()
      textView = root as? LexicalNSTextView
      scrolling.observe(root)
      refresh()
    #else
      // The formats, then `ContextMenuPlugin`'s items, before the system's.
      let textView = root as? LexicalUITextView
      textView?.editMenuElements = { [weak self, weak textView] _ in
        MainActor.assumeIsolated {
          (self?.editMenuElements() ?? []) + (textView.map(contextMenuElements) ?? [])
        }
      }
    #endif
  }

  /// Follows the model after an update, once the view has laid it out.
  func refresh() {
    #if os(macOS)
      DispatchQueue.main.async { [weak self] in self?.position() }
    #endif
  }

  #if os(macOS)
    private func hide() {
      panel.parent?.removeChildWindow(panel)
      panel.orderOut(nil)
    }

    private func position() {
      guard let model, model.isRichText, model.isFloatingText, !model.isLink, model.isEditable,
        !model.comments.showCommentInput, let selection = model.selectionPoints, let textView,
        let window = textView.window,
        let screen = window.screen,
        let range = textView.rect(from: selection.anchor, to: selection.focus)
      else { return hide() }
      // Screen space with y growing downward, as the DOM's.
      let screenTop = screen.frame.maxY
      func toDown(_ r: CGRect) -> CGRect {
        let s = window.convertToScreen(textView.convert(r, to: nil))
        return CGRect(x: s.minX, y: screenTop - s.maxY, width: s.width, height: s.height)
      }
      let target = toDown(range)
      let scroller = toDown(textView.visibleRect)
      let margin = FloatingTextFormatToolbarView.shadowMargin
      let size = hostingController.sizeThatFits(
        in: CGSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude))
      let width = size.width - 2 * margin
      let height = size.height - 2 * margin
      // `setFloatingElemPosition`.
      let verticalGap: CGFloat = 10
      let horizontalOffset: CGFloat = 5
      var top = target.minY - height - verticalGap
      var left = target.minX - horizontalOffset
      if model.alignment == "right" || model.alignment == "end" {
        left = target.maxX - width + horizontalOffset
      }
      if top < scroller.minY {
        top += height + target.height + verticalGap * 2
      }
      if left + width > scroller.maxX {
        left = scroller.maxX - width - horizontalOffset
      }
      if left < scroller.minX {
        left = scroller.minX + horizontalOffset
      }
      // A selection scrolled out of the editor shows no popup.
      if target.maxY < scroller.minY || target.minY > scroller.maxY { return hide() }
      panel.setFrame(
        CGRect(
          x: left - margin, y: screenTop - (top + height + margin), width: size.width,
          height: size.height),
        display: true)
      if panel.parent !== window {
        panel.parent?.removeChildWindow(panel)
        window.addChildWindow(panel, ordered: .above)
      }
      panel.orderFront(nil)
    }
  #else
    private func editMenuElements() -> [UIMenuElement] {
      guard let model, model.isRichText, model.isFloatingText, !model.isLink, model.isEditable
      else { return [] }
      func action(_ title: String, _ symbol: String, _ format: TextFormatType) -> UIAction {
        UIAction(
          title: title, image: UIImage(systemName: symbol),
          state: model.floatingFormats.contains(format) ? .on : .off
        ) { [weak model] _ in model?.toggleFormat(format) }
      }
      let formats = floatingToolbarFormats.map { action($0.title, $0.symbol, $0.format) }
      let link = UIAction(title: "Insert link", image: UIImage(systemName: "link")) {
        [weak model] _ in model?.insertLink()
      }
      let comment = UIAction(title: "Insert comment", image: UIImage(systemName: "text.bubble")) {
        [weak model] _ in model?.insertComment()
      }
      // Bold, italic, underline and strikethrough in the menu's row, as
      // icons; the rest of the popup's buttons in a Format submenu, and
      // Insert comment after it.
      return [
        UIMenu(
          options: .displayInline, preferredElementSize: .small,
          children: Array(formats.prefix(4))),
        UIMenu(
          title: "Format", image: UIImage(systemName: "textformat"),
          children: Array(formats.dropFirst(4)) + [link]),
        comment,
      ]
    }
  #endif
}
