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
  /// The editor's visible width, which the menu stays within at the
  /// larger text sizes.
  var maxWidth: CGFloat = .infinity
  /// The room below or above the query, whichever is taller, which the
  /// menu stays within, scrolling, at the larger text sizes.
  var maxHeight: CGFloat = .infinity
  @ObservationIgnored var choose: (Int) -> Void = { _ in }
  @ObservationIgnored var highlight: (Int) -> Void = { _ in }
}

struct TypeaheadMenuView: View {
  let model: TypeaheadMenuModel
  static let shadowMargin: CGFloat = 16
  /// How much the reader's text size scales the web's sizes: a row is 36
  /// points tall (`li`'s 20-point line and 8 points of padding), and the
  /// menu at most 200, at the default size.
  @ScaledMetric(relativeTo: .subheadline) private var scale: CGFloat = 1
  @Environment(\.dynamicTypeSize) private var dynamicTypeSize

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
    .frame(
      width: menuWidth,
      height: min(200 * scale, model.maxHeight, model.rows.reduce(0) { $0 + rowHeight($1) })
    )
    .clipShape(RoundedRectangle(cornerRadius: 8))
    // `.typeahead-popover`: white, radius 8, `0 5px 10px rgba(0,0,0,.3)`.
    .floatingSurface()
    .padding(Self.shadowMargin)
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Typeahead menu")
    .accessibilityIdentifier("typeahead-menu")
  }

  private var menuWidth: CGFloat {
    min(model.width * scale, model.maxWidth - 2 * Self.shadowMargin)
  }

  /// A row's height: 36 points, scaled, on one line, and at the
  /// accessibility sizes, where a title wraps, its lines' height and the
  /// padding.
  private func rowHeight(_ row: TypeaheadMenuRow) -> CGFloat {
    let height = 36 * scale
    #if canImport(UIKit)
      guard dynamicTypeSize.isAccessibilitySize else { return height }
      let icon = row.glyph != nil || row.symbol != nil ? 20 * scale + 8 : 0
      let title = (row.title as NSString).boundingRect(
        with: CGSize(width: menuWidth - 16 * scale - icon, height: .greatestFiniteMagnitude),
        options: .usesLineFragmentOrigin,
        attributes: [.font: UIFont.systemFont(ofSize: 14 * scale)], context: nil)
      return max(height, ceil(title.height) + 16 * scale)
    #else
      return height
    #endif
  }

  /// `li.item`: padding 8 (`ul li { margin: 0 }` wins over `li`'s side
  /// margins), a 20-point icon 8 from the text, 14-point text on a
  /// 20-point line, `#eee` while selected, clipped by the menu's corners.
  /// `#eee`, and its dark equivalent.
  private var selectedBackground: Color {
    Color(light: Color(white: 0.933), dark: Color.primary.opacity(0.12))
  }

  private func item(_ row: TypeaheadMenuRow, _ index: Int) -> some View {
    let isSelected = model.selectedIndex == index
    return HStack(spacing: 8) {
      if let glyph = row.glyph {
        Text(glyph).font(.system(size: 14 * scale)).frame(width: 20 * scale, height: 20 * scale)
      } else if let symbol = row.symbol {
        Image(systemName: symbol).font(.system(size: 14 * scale))
          .frame(width: 20 * scale, height: 20 * scale)
      }
      Text(row.title)
        .font(.system(size: 14 * scale))
        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
        .frame(maxWidth: .infinity, minHeight: 20 * scale, alignment: .leading)
    }
    .foregroundStyle(Color.primary)
    .padding(8 * scale)
    .background(isSelected ? selectedBackground : .clear)
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

/// Calls its action as the text of the editor's text view scrolls, for
/// what is over the text and follows it: as the bounds of the scroll
/// view's clip view change on macOS, and after the text view's content
/// offset changes on iOS, once it has laid out the scrolled text.
@MainActor
final class TextScrollObserver {
  private let action: () -> Void
  private weak var textView: TextView?
  #if os(macOS)
    private var observer: (any NSObjectProtocol)?
  #else
    private var observation: NSKeyValueObservation?
  #endif

  init(_ action: @escaping () -> Void) {
    self.action = action
  }

  #if os(macOS)
    isolated deinit {
      if let observer { NotificationCenter.default.removeObserver(observer) }
    }
  #endif

  /// Follows `root`, the editor's new root element.
  func observe(_ root: AnyObject?) {
    textView = root as? TextView
    #if os(macOS)
      // The text view attaches as it is made, before SwiftUI puts it in
      // its scroll view.
      DispatchQueue.main.async { [weak self] in self?.observeClipView() }
    #else
      observation = textView?.observe(\.contentOffset) { [weak self] _, _ in
        DispatchQueue.main.async { self?.action() }
      }
    #endif
  }

  #if os(macOS)
    private func observeClipView() {
      if let observer { NotificationCenter.default.removeObserver(observer) }
      observer = nil
      guard let clipView = textView?.enclosingScrollView?.contentView else { return }
      clipView.postsBoundsChangedNotifications = true
      observer = NotificationCenter.default.addObserver(
        forName: NSView.boundsDidChangeNotification, object: clipView, queue: .main
      ) { [weak self] _ in MainActor.assumeIsolated { self?.action() } }
    }
  #endif
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
    /// The keyboard's frame in the screen's space, from its notifications:
    /// the window's `keyboardLayoutGuide`, once made, keeps sheets from
    /// rising above the keyboard.
    private var keyboardFrame: CGRect = .null
    private var keyboardObserver: (any NSObjectProtocol)?
  #endif
  private weak var textView: TextView?
  private var hostingView: PlatformView { hostingController.view }
  private var removeScroll: (() -> Void)?
  private lazy var scrolling = TextScrollObserver { [weak self] in self?.position(isScroll: true) }

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
    #if !os(macOS)
      keyboardObserver = NotificationCenter.default.addObserver(
        forName: UIResponder.keyboardWillChangeFrameNotification, object: nil, queue: .main
      ) { [weak self] notification in
        let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect
        MainActor.assumeIsolated { self?.keyboardFrame = frame ?? .null }
      }
    #endif
    removeScroll = menu.editor.registerCommand(
      SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
      { [weak self] payload, _ in
        MainActor.assumeIsolated { self?.model.scrollTarget = payload.index }
        return true
      }, menu.commandPriority)
  }

  isolated deinit {
    removeScroll?()
    #if !os(macOS)
      if let keyboardObserver { NotificationCenter.default.removeObserver(keyboardObserver) }
    #endif
    hide()
  }

  private var isShown: Bool {
    #if os(macOS)
      panel.isVisible
    #else
      !hostingView.isHidden
    #endif
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
    textView = root as? TextView
    scrolling.observe(root)
    refresh()
  }

  private func refresh() {
    model.rows = menu.options.map(row)
    model.selectedIndex = menu.selectedIndex
    // After the view has laid out the update that moved the query.
    DispatchQueue.main.async { [weak self] in self?.position() }
  }

  /// `isScroll`: the text scrolled, rather than the menu or its query
  /// changing.
  private func position(isScroll: Bool = false) {
    // A node menu's anchor is its node (`LexicalNodeMenuPlugin` anchors at
    // the node's element): an element spans its children.
    let elementSize =
      menu.resolution.flatMap { resolution in
        try? menu.editor.read(.latest) {
          try (getNodeByKey(resolution.anchorKey) as? ElementNode)?.getChildrenSize()
        }
      } ?? nil
    guard let textView, let resolution = menu.resolution, !menu.options.isEmpty,
      let range = textView.rect(
        from: createPoint(
          resolution.anchorKey, elementSize == nil ? resolution.startOffset : 0,
          elementSize == nil ? .text : .element),
        to: createPoint(
          resolution.anchorKey, elementSize ?? resolution.endOffset,
          elementSize == nil ? .text : .element))
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
    // the editor (with a 6-point margin) closes the menu. A query typed
    // below the editor's bottom is out of it until the text view scrolls
    // its caret into view, after this: the menu waits, hidden, as the
    // web's check waits for a scroll event.
    if range.minY < rootVisible.minY - 6 || range.minY > rootVisible.maxY + 6 {
      if isScroll, isShown { menu.close() } else { hide() }
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
      let keyboard = container.convert(keyboardFrame, from: container.screen.coordinateSpace)
      let bottom =
        keyboardFrame.isNull ? container.bounds.maxY : min(container.bounds.maxY, keyboard.minY)
      model.maxWidth = root.width
      model.maxHeight = max(
        44, bottom - rect.maxY - 3 - 2 * TypeaheadMenuView.shadowMargin,
        rect.minY - root.minY - rect.height - 2 * TypeaheadMenuView.shadowMargin)
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
