import Lexical
import LexicalUtils

#if os(macOS)
  import AppKit
  import LexicalAppKit

  // Port of packages/lexical-playground/src/plugins/DraggableBlockPlugin
  // and `DraggableBlockPlugin_EXPERIMENTAL`: on macOS, while the mouse is
  // over a top-level block, a handle at the editor's left edge (`+` and
  // the ⋮⋮ grip, `.draggable-block-menu`) is centered on the block's first
  // line. Dragging the grip drags the block (its key as
  // `application/x-lexical-drag-block`, its image as the drag image); over
  // the editor a 4-point line (`.draggable-block-target-line`) shows where
  // it goes, and the drop puts it after the block under the mouse, or
  // before the block whose top margin the mouse is in. The `+` adds a
  // paragraph below the block (above it with Option or Control) with the
  // component picker open on it.
  //
  // As on the web, it is shown only while the window is wider than 1025
  // points (`isSmallWidthViewport`), when the text has the web's 46-point
  // side padding for it; iOS does not have it (parity/deviations.json).

  private let DRAG_DATA_FORMAT = "application/x-lexical-drag-block"
  private let SPACE: CGFloat = 4
  private let TARGET_LINE_HALF_HEIGHT: CGFloat = 2
  private let TEXT_BOX_HORIZONTAL_PADDING: CGFloat = 28
  /// `.ContentEditable__root { padding: 8px 46px 40px }`, and 8 points
  /// at `max-width: 1025px`.
  private let WIDE_PADDING: CGFloat = 46

  /// Keeps the handle and the target line in the editor's text view.
  @MainActor
  final class DraggableBlockHost {
    private weak var editor: Editor?
    private weak var textView: LexicalNSTextView?
    private let menu = DraggableBlockMenu()
    private let targetLine = NSView()
    private lazy var tracker = MouseTracker { [weak self] in self?.mouseMoved($0) }
    private var trackingArea: NSTrackingArea?
    private var frameObserver: NSObjectProtocol?
    private var unregister: () -> Void = {}
    /// The block the handle is at (`draggableBlockElem`).
    private var blockKey: NodeKey?
    /// `isDraggingBlockRef`.
    private var isDragging = false
    /// `!isSmallWidthViewport`.
    private var isWide = false

    init() {
      targetLine.wantsLayer = true
      // `background: deepskyblue; opacity: .4`.
      targetLine.layer?.backgroundColor =
        NSColor(srgbRed: 0, green: 191 / 255, blue: 1, alpha: 0.4).cgColor
      targetLine.isHidden = true
      menu.isHidden = true
      menu.onPlus = { [weak self] insertBefore in self?.openComponentPicker(insertBefore) }
      menu.onDrag = { [weak self] event in self?.dragBlock(with: event) }
    }

    /// Moves the handle into `root`, the editor's new root element.
    func attach(to root: AnyObject?, editor: Editor) {
      self.editor = editor
      if let trackingArea { textView?.removeTrackingArea(trackingArea) }
      if let frameObserver { NotificationCenter.default.removeObserver(frameObserver) }
      trackingArea = nil
      frameObserver = nil
      unregister()
      unregister = {}
      menu.removeFromSuperview()
      targetLine.removeFromSuperview()
      show(nil)
      textView = root as? LexicalNSTextView
      guard let textView else { return }
      textView.addSubview(menu)
      textView.addSubview(targetLine)
      let area = NSTrackingArea(
        rect: .zero,
        options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
        owner: tracker)
      textView.addTrackingArea(area)
      trackingArea = area
      textView.postsFrameChangedNotifications = true
      frameObserver = NotificationCenter.default.addObserver(
        forName: NSView.frameDidChangeNotification, object: textView, queue: .main
      ) { [weak self] _ in MainActor.assumeIsolated { self?.updateWidth() } }
      updateWidth()
      unregister = mergeRegister([
        editor.registerCommand(
          DRAGOVER_COMMAND,
          { [weak self] event, _ in
            MainActor.assumeIsolated { self?.onDragover(event) ?? false }
          }, COMMAND_PRIORITY_LOW),
        editor.registerCommand(
          DROP_COMMAND,
          { [weak self] event, _ in
            try MainActor.assumeIsolated { try self?.onDrop(event) ?? false }
          }, COMMAND_PRIORITY_HIGH),
        editor.registerCommand(
          DRAGEND_COMMAND,
          { [weak self] _, _ in
            MainActor.assumeIsolated { self?.onDragEnd() }
            return false
          }, COMMAND_PRIORITY_LOW),
        editor.registerEditableListener { [weak self] _ in
          MainActor.assumeIsolated { self?.show(self?.blockKey) }
        },
      ])
    }

    /// Follows the window's width: the web's media query, with the text's
    /// side padding.
    private func updateWidth() {
      guard let textView else { return }
      isWide = (textView.window?.frame.width ?? 0) > 1025
      let padding = (isWide ? WIDE_PADDING : 8) - (textView.textContainer?.lineFragmentPadding ?? 0)
      if textView.textContainerInset.width != padding {
        textView.textContainerInset = NSSize(
          width: padding, height: textView.textContainerInset.height)
      }
      if !isWide { show(nil) }
    }

    // MARK: Blocks

    /// The top-level blocks' rects, in document order. In an update or a
    /// read (a command's handler) it reads that state; otherwise the
    /// editor's.
    private func blockRects() -> [(key: NodeKey, rect: CGRect)] {
      guard let editor, let textView else { return [] }
      let keys =
        (try? getRoot().getChildrenKeys())
        ?? (try? editor.read { try getRoot().getChildrenKeys() }) ?? []
      return keys.compactMap { key in textView.blockRect(for: key).map { (key, $0) } }
    }

    /// `getBlockElement`: the block whose rect, grown by half the gaps to
    /// its neighbors (its collapsed margins) and across the editor, has
    /// `y`; past the first or last block that block with
    /// `useEdgeAsDefault`. With the margins it was found by.
    private func block(
      at y: CGFloat, useEdgeAsDefault: Bool
    ) -> (
      key: NodeKey, rect: CGRect, marginTop: CGFloat, marginBottom: CGFloat
    )? {
      let rects = blockRects()
      for (index, block) in rects.enumerated() {
        let marginTop = index > 0 ? max(0, block.rect.minY - rects[index - 1].rect.maxY) : 0
        let marginBottom =
          index + 1 < rects.count ? max(0, rects[index + 1].rect.minY - block.rect.maxY) : 0
        let top = block.rect.minY - marginTop / 2
        let bottom = block.rect.maxY + marginBottom / 2
        let isFirst = index == 0
        let isLast = index + 1 == rects.count
        if (y >= top || (useEdgeAsDefault && isFirst))
          && (y < bottom || (useEdgeAsDefault && isLast))
        {
          return (block.key, block.rect, marginTop, marginBottom)
        }
      }
      return nil
    }

    // MARK: The handle

    /// `onMouseMove`: the handle goes to the block under the mouse, and
    /// stays while the mouse is on it.
    private func mouseMoved(_ event: NSEvent?) {
      guard let textView, let event, isWide, !isDragging else {
        if !isDragging { show(nil) }
        return
      }
      let point = textView.convert(event.locationInWindow, from: nil)
      if !menu.isHidden, menu.frame.contains(point) { return }
      show(block(at: point.y, useEdgeAsDefault: false)?.key)
    }

    /// `setMenuPosition`: at `SPACE` from the editor's left edge, centered
    /// on the block's first line (on the whole block where it has no line
    /// of text, as JS falls back to its height).
    private func show(_ key: NodeKey?) {
      blockKey = key
      guard let key, let textView, let editor, editor.isEditable(),
        let rect = textView.blockRect(for: key)
      else {
        menu.isHidden = true
        return
      }
      let start = Point(key, 0, .element)
      let lineHeight =
        textView.rect(from: start, to: start).map(\.height).flatMap { $0 > 0 ? $0 : nil }
        ?? rect.height
      let size = menu.intrinsicContentSize
      let frame = CGRect(
        x: SPACE, y: rect.minY + (lineHeight - size.height) / 2, width: size.width,
        height: size.height)
      if menu.isHidden {
        menu.frame = frame
        menu.isHidden = false
      } else if menu.frame != frame {
        // `transition: transform 140ms ease-in-out`.
        NSAnimationContext.runAnimationGroup { context in
          context.duration = 0.14
          context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
          menu.animator().frame = frame
        }
      }
    }

    // MARK: Dragging

    /// `onDragStart`: drags the block's key, with an image of the block.
    private func dragBlock(with event: NSEvent) {
      guard let textView, let key = blockKey, let rect = textView.blockRect(for: key) else {
        return
      }
      let item = NSPasteboardItem()
      item.setString(key, forType: NSPasteboard.PasteboardType("dev.lexical.drag-block"))
      let draggingItem = NSDraggingItem(pasteboardWriter: item)
      var image: NSImage?
      if let rep = textView.bitmapImageRepForCachingDisplay(in: rect) {
        textView.cacheDisplay(in: rect, to: rep)
        image = NSImage(size: rect.size)
        image?.addRepresentation(rep)
      }
      draggingItem.setDraggingFrame(rect, contents: image)
      isDragging = true
      menu.isHidden = true
      // The text view is the source, so it takes the drop as a move of its
      // own (`DROP_COMMAND` below places the block).
      menu.beginDraggingSession(with: [draggingItem], event: event, source: textView)
    }

    /// `onDragover`: shows where the block would go.
    private func onDragover(_ event: DragEvent) -> Bool {
      guard isDragging, let textView,
        let target = block(at: event.clientY, useEdgeAsDefault: true)
      else { return false }
      // `setTargetLine`.
      let lineTop =
        event.clientY >= target.rect.minY
        ? target.rect.maxY + target.marginBottom / 2 : target.rect.minY - target.marginTop / 2
      let left = TEXT_BOX_HORIZONTAL_PADDING - SPACE
      targetLine.frame = CGRect(
        x: left, y: lineTop - TARGET_LINE_HALF_HEIGHT, width: textView.bounds.width - left * 2,
        height: 2 * TARGET_LINE_HALF_HEIGHT)
      targetLine.isHidden = false
      event.preventDefault()
      return true
    }

    /// `$onDrop`: the block goes after the block under the mouse, or before
    /// it above its top.
    private func onDrop(_ event: DragEvent) throws -> Bool {
      guard isDragging else { return false }
      let key = event.dataTransfer?.getData(DRAG_DATA_FORMAT) ?? ""
      guard let dragged = try getNodeByKey(key),
        let target = block(at: event.clientY, useEdgeAsDefault: true),
        let targetNode = try getNodeByKey(target.key)
      else { return false }
      if targetNode.getKey() == dragged.getKey() { return true }
      if event.clientY >= target.rect.minY {
        _ = try targetNode.insertAfter(dragged)
      } else {
        _ = try targetNode.insertBefore(dragged)
      }
      blockKey = nil
      return true
    }

    /// `onDragEnd`.
    private func onDragEnd() {
      isDragging = false
      targetLine.isHidden = true
    }

    // MARK: Adding a block

    /// `openComponentPicker`: a paragraph after the block (before it with
    /// Option or Control) with the "/" menu open in it, which the web
    /// shows as a picker with a filter field beside the handle.
    private func openComponentPicker(_ insertBefore: Bool) {
      guard let editor, let key = blockKey else { return }
      textView?.window?.makeFirstResponder(textView)
      try? editor.update {
        guard let node = try getNodeByKey(key) else { return }
        let paragraph = try createParagraphNode()
        let text = try createTextNode("/")
        try paragraph.append(text)
        if insertBefore {
          _ = try node.insertBefore(paragraph)
        } else {
          _ = try node.insertAfter(paragraph)
        }
        _ = try text.select(1, 1)
      }
    }
  }

  /// `.draggable-block-menu`: the `+` button and the grip, 16-point icons
  /// at 30% opacity, 2 points apart, with `#efefef` under the one the mouse
  /// is on, radius 4.
  final class DraggableBlockMenu: NSView {
    var onPlus: (Bool) -> Void = { _ in }
    var onDrag: (NSEvent) -> Void = { _ in }
    private let plus = DraggableBlockIcon(.plus)
    private let grip = DraggableBlockIcon(.grip)
    private var mouseDownPoint: NSPoint?

    override init(frame: NSRect) {
      super.init(frame: frame)
      for (index, icon) in [plus, grip].enumerated() {
        icon.frame = NSRect(x: 1 + CGFloat(index) * 18, y: 2, width: 16, height: 16)
        addSubview(icon)
      }
      plus.toolTip = "Click to add below"
      setAccessibilityElement(true)
      setAccessibilityRole(.group)
      setAccessibilityIdentifier("draggable-block-menu")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var isFlipped: Bool { true }
    /// `padding: 2px 1px`, two 16-point icons with a 2-point gap.
    override var intrinsicContentSize: NSSize { NSSize(width: 36, height: 20) }

    override func resetCursorRects() {
      addCursorRect(plus.frame, cursor: .pointingHand)
      addCursorRect(grip.frame, cursor: .openHand)
    }

    override func mouseDown(with event: NSEvent) {
      let point = convert(event.locationInWindow, from: nil)
      mouseDownPoint = grip.frame.insetBy(dx: -1, dy: -2).contains(point) ? point : nil
      if mouseDownPoint == nil, plus.frame.insetBy(dx: -1, dy: -2).contains(point) {
        onPlus(event.modifierFlags.contains(.option) || event.modifierFlags.contains(.control))
      }
    }

    override func mouseDragged(with event: NSEvent) {
      guard let start = mouseDownPoint else { return }
      let point = convert(event.locationInWindow, from: nil)
      guard hypot(point.x - start.x, point.y - start.y) >= 3 else { return }
      mouseDownPoint = nil
      NSCursor.closedHand.set()
      onDrag(event)
    }

    override func mouseUp(with event: NSEvent) { mouseDownPoint = nil }
  }

  /// One icon of the handle, highlighted while the mouse is on it: the
  /// web's `plus.svg` or `draggable-block-menu.svg` (six dots of radius 2
  /// in a 24-point box) at 16 points, in the text color at 30%.
  private final class DraggableBlockIcon: NSView {
    enum Kind { case plus, grip }
    private let kind: Kind
    private var trackingArea: NSTrackingArea?
    private var isHovered = false {
      didSet { needsDisplay = true }
    }

    init(_ kind: Kind) {
      self.kind = kind
      super.init(frame: .zero)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func draw(_ dirtyRect: NSRect) {
      if isHovered {
        // `.draggable-block-menu .icon:hover { background-color: #efefef }`.
        let dark = effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
        (dark ? NSColor.white.withAlphaComponent(0.12) : NSColor(white: 0.937, alpha: 1)).setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 4, yRadius: 4).fill()
      }
      NSColor.labelColor.withAlphaComponent(0.3).set()
      let scale = bounds.width / 24
      switch kind {
      case .grip:
        for x in [8.5, 15.5] {
          for y in [5.0, 12, 19] {
            NSBezierPath(
              ovalIn: NSRect(
                x: (x - 2) * scale, y: (y - 2) * scale, width: 4 * scale, height: 4 * scale)
            ).fill()
          }
        }
      case .plus:
        let path = NSBezierPath()
        path.lineWidth = 2 * scale
        path.lineCapStyle = .round
        path.move(to: NSPoint(x: 12 * scale, y: 5 * scale))
        path.line(to: NSPoint(x: 12 * scale, y: 19 * scale))
        path.move(to: NSPoint(x: 5 * scale, y: 12 * scale))
        path.line(to: NSPoint(x: 19 * scale, y: 12 * scale))
        path.stroke()
      }
    }

    override func updateTrackingAreas() {
      super.updateTrackingAreas()
      if let trackingArea { removeTrackingArea(trackingArea) }
      let area = NSTrackingArea(
        rect: .zero, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
        owner: self)
      addTrackingArea(area)
      trackingArea = area
    }

    override func mouseEntered(with event: NSEvent) { isHovered = true }
    override func mouseExited(with event: NSEvent) { isHovered = false }
  }
#endif
