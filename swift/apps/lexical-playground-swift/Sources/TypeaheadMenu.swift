import Lexical
import LexicalMenu
import LexicalTextKit
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias PlatformView = NSView
  private typealias TextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias PlatformView = UIView
  private typealias TextView = LexicalUITextView
#endif

// The menu `LexicalMenu`'s default renderer draws (`.typeahead-popover`, a
// `ul` of `li.item`s with an icon and a `.text`), and `useMenuAnchorRef`,
// which positions it under the query: `LexicalMenu` in the library keeps
// the state and the keyboard commands; this draws it.
//
// JS portals the menu into `document.body` at the range's viewport rect.
// Here it is a child panel of the window on macOS, at the query's screen
// rect, and a subview of the window on iOS, at the query's rect there,
// which the text view measures (`rect(from:to:)`), moved as the text
// scrolls. A click or tap on an item
// chooses it, as the `li`'s `onClick` does; the pointer highlights an item
// on macOS (`onMouseEnter`). Neither takes the focus from the text view, so
// the software keyboard stays up on iOS.

/// One row of a menu: what `MenuItem` draws for an option.
struct TypeaheadMenuRow: Identifiable {
  let id: String
  let title: String
  /// An SF Symbol standing in for the playground's `i.icon` mask image.
  let symbol: String?
  /// A leading glyph drawn as text (an emoji option's emoji).
  let glyph: String?
}

@MainActor
@Observable
final class TypeaheadMenuModel {
  var rows: [TypeaheadMenuRow] = []
  var selectedIndex: Int?
  /// The option at `scrollTarget` is scrolled into view
  /// (`SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`).
  var scrollTarget: Int?
  /// `.mentions-menu { width: 250px }`, which the default renderer gives
  /// every menu.
  var width: CGFloat = 250
  @ObservationIgnored var choose: (Int) -> Void = { _ in }
  @ObservationIgnored var highlight: (Int) -> Void = { _ in }
}

struct TypeaheadMenuView: View {
  let model: TypeaheadMenuModel
  static let shadowMargin: CGFloat = 16

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        VStack(spacing: 0) {
          ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
            item(row, index)
          }
        }
      }
      .scrollIndicators(.hidden)
      .onChange(of: model.scrollTarget) { _, target in
        if let target, model.rows.indices.contains(target) {
          proxy.scrollTo(model.rows[target].id)
        }
      }
    }
    // `ul { max-height: 200px }`.
    .frame(width: model.width, height: min(200, CGFloat(model.rows.count) * 36))
    .clipShape(RoundedRectangle(cornerRadius: 8))
    // `.typeahead-popover`: white, radius 8, `0 5px 10px rgba(0,0,0,.3)`.
    .background(
      RoundedRectangle(cornerRadius: 8).fill(.background)
        .shadow(color: .black.opacity(0.3), radius: 5, y: 5)
    )
    .padding(Self.shadowMargin)
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Typeahead menu")
    .accessibilityIdentifier("typeahead-menu")
  }

  /// `li.item`: padding 8 (`ul li { margin: 0 }` wins over `li`'s side
  /// margins), a 20-point icon 8 from the text, 14-point text on a
  /// 20-point line, `#eee` while selected, clipped by the menu's corners.
  private func item(_ row: TypeaheadMenuRow, _ index: Int) -> some View {
    let isSelected = model.selectedIndex == index
    return HStack(spacing: 8) {
      if let glyph = row.glyph {
        Text(glyph).font(.system(size: 14)).frame(width: 20, height: 20)
      } else if let symbol = row.symbol {
        Image(systemName: symbol).font(.system(size: 14)).frame(width: 20, height: 20)
      }
      Text(row.title)
        .font(.system(size: 14))
        .lineLimit(1)
        .frame(minWidth: 150, maxWidth: .infinity, minHeight: 20, alignment: .leading)
    }
    .foregroundStyle(Color.primary)
    .padding(8)
    .background(isSelected ? Color.gray.opacity(0.15) : .clear)
    .contentShape(Rectangle())
    .id(row.id)
    .onTapGesture { model.choose(index) }
    #if os(macOS)
      .onHover { inside in if inside { model.highlight(index) } }
    #endif
    .accessibilityElement(children: .combine)
    .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
    .accessibilityIdentifier("typeahead-item-\(index)")
  }
}

/// A menu host the playground moves into each new root element.
@MainActor
protocol TypeaheadMenuAttaching {
  func attach(to root: AnyObject?)
}

/// The input controller of `root`, the editor's text view.
@MainActor
func textController(_ root: AnyObject?) -> TextKitInputController? {
  (root as? TextView)?.controller
}

/// Keeps a menu over the editor's window, under its query
/// (`useMenuAnchorRef` and `useDynamicPositioning`), drawing `menu`'s
/// options with `row`.
@MainActor
final class TypeaheadMenuHost<Option: MenuOption>: TypeaheadMenuAttaching {
  let menu: MenuState<Option>
  private let row: (Option) -> TypeaheadMenuRow
  private let model = TypeaheadMenuModel()
  #if os(macOS)
    private let hostingController: NSHostingController<TypeaheadMenuView>
    private let panel = MenuPanel()
  #else
    private let hostingController: UIHostingController<TypeaheadMenuView>
    private var offsetObservation: NSKeyValueObservation?
  #endif
  private weak var textView: TextView?
  private var hostingView: PlatformView { hostingController.view }
  private var removeScroll: (() -> Void)?
  private var scrollObserver: (any NSObjectProtocol)?

  init(menu: MenuState<Option>, row: @escaping (Option) -> TypeaheadMenuRow) {
    self.menu = menu
    self.row = row
    hostingController = .init(rootView: TypeaheadMenuView(model: model))
    hostingController.sizingOptions = []
    #if os(macOS)
      panel.contentView = hostingController.view
    #else
      hostingController.view.backgroundColor = .clear
      hostingView.isHidden = true
    #endif
    model.choose = { [weak self] index in
      guard let self, self.menu.options.indices.contains(index) else { return }
      self.menu.setHighlightedIndex(index)
      self.menu.selectOptionAndCleanUp(self.menu.options[index])
    }
    model.highlight = { [weak self] index in self?.menu.setHighlightedIndex(index) }
    menu.onChange = { [weak self] in self?.refresh() }
    removeScroll = menu.editor.registerCommand(
      SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
      { [weak self] payload, _ in
        MainActor.assumeIsolated { self?.model.scrollTarget = payload.index }
        return true
      }, menu.commandPriority)
  }

  isolated deinit {
    removeScroll?()
    if let scrollObserver { NotificationCenter.default.removeObserver(scrollObserver) }
    hide()
  }

  private func hide() {
    #if os(macOS)
      panel.parent?.removeChildWindow(panel)
      panel.orderOut(nil)
    #else
      hostingView.isHidden = true
    #endif
  }

  /// Follows `root`, the editor's new root element: the menu moves with
  /// its text as it scrolls.
  func attach(to root: AnyObject?) {
    hide()
    if let scrollObserver { NotificationCenter.default.removeObserver(scrollObserver) }
    scrollObserver = nil
    textView = root as? TextView
    #if os(macOS)
      if let clipView = textView?.enclosingScrollView?.contentView {
        clipView.postsBoundsChangedNotifications = true
        scrollObserver = NotificationCenter.default.addObserver(
          forName: NSView.boundsDidChangeNotification, object: clipView, queue: .main
        ) { [weak self] _ in MainActor.assumeIsolated { self?.position() } }
      }
    #else
      offsetObservation = textView?.observe(\.contentOffset) { [weak self] _, _ in
        MainActor.assumeIsolated { self?.position() }
      }
    #endif
    refresh()
  }

  private func refresh() {
    model.rows = menu.options.map(row)
    model.selectedIndex = menu.selectedIndex
    // After the view has laid out the update that moved the query.
    DispatchQueue.main.async { [weak self] in self?.position() }
  }

  private func position() {
    guard let textView, let resolution = menu.resolution, !menu.options.isEmpty,
      let range = textView.rect(
        from: createPoint(resolution.anchorKey, resolution.startOffset, .text),
        to: createPoint(resolution.anchorKey, resolution.endOffset, .text))
    else {
      hide()
      return
    }
    #if os(macOS)
      let rootVisible = textView.visibleRect
    #else
      let rootVisible = textView.bounds.inset(by: textView.adjustedContentInset)
    #endif
    // `isTriggerVisibleInNearestScrollContainer`: a query scrolled out of
    // the editor (with a 6-point margin) closes the menu.
    if range.minY < rootVisible.minY - 6 || range.minY > rootVisible.maxY + 6 {
      menu.close()
      return
    }
    // The query, the root and the bottom of the viewport in one space whose
    // y grows downward, as the DOM's: the window's on iOS, and on macOS the
    // screen's, flipped, since the menu is a panel there.
    #if os(macOS)
      guard let window = textView.window, let screen = window.screen else { return hide() }
      let screenTop = screen.frame.maxY
      func toDown(_ r: CGRect) -> CGRect {
        let s = window.convertToScreen(textView.convert(r, to: nil))
        return CGRect(x: s.minX, y: screenTop - s.maxY, width: s.width, height: s.height)
      }
      let rect = toDown(range)
      let root = toDown(rootVisible)
      // `window.innerHeight`: the bottom of the window.
      let bottom = screenTop - window.frame.minY
      let size = hostingController.sizeThatFits(
        in: CGSize(width: screen.frame.width, height: .greatestFiniteMagnitude))
    #else
      guard let container = textView.window else { return hide() }
      if hostingView.superview !== container {
        hostingView.removeFromSuperview()
        container.addSubview(hostingView)
      }
      let rect = textView.convert(range, to: container)
      let root = textView.convert(rootVisible, to: container)
      // `window.innerHeight`: what the keyboard leaves of the window.
      let bottom = min(container.bounds.maxY, container.keyboardLayoutGuide.layoutFrame.minY)
      let size = hostingController.sizeThatFits(
        in: CGSize(width: container.bounds.width, height: .greatestFiniteMagnitude))
    #endif
    let margin = TypeaheadMenuView.shadowMargin
    let width = size.width - 2 * margin
    let height = size.height - 2 * margin
    // `top + anchorHeight + 3`, the anchor being as tall as the range.
    var y = rect.maxY + 3
    // Above the query when the menu would pass the viewport's or the
    // root's bottom and there is room above it.
    if y + height > bottom || y + height > root.maxY, rect.minY - root.minY > height + rect.height {
      y = rect.minY - height - 3
    }
    // Pulled in at the root's right edge.
    let x = min(rect.minX, root.maxX - width)
    let frame = CGRect(
      x: x - margin, y: y - margin, width: width + 2 * margin, height: height + 2 * margin)
    #if os(macOS)
      panel.setFrame(
        CGRect(
          x: frame.minX, y: screenTop - frame.maxY, width: frame.width, height: frame.height),
        display: true)
      if panel.parent !== window {
        panel.parent?.removeChildWindow(panel)
        window.addChildWindow(panel, ordered: .above)
      }
      panel.orderFront(nil)
    #else
      hostingView.frame = frame
      hostingView.isHidden = false
    #endif
  }
}

#if os(macOS)
  /// The menu's window: borderless and transparent (the menu draws its own
  /// shadow), and never key, so the editor keeps the keyboard, as a DOM
  /// menu leaves the focus in the `contenteditable`.
  final class MenuPanel: NSPanel {
    init() {
      super.init(
        contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered,
        defer: true)
      isOpaque = false
      backgroundColor = .clear
      hasShadow = false
      isReleasedWhenClosed = false
    }
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
  }
#endif
