#if os(macOS)
  import AppKit
  import Lexical
  import LexicalTextKit

  // A table in the AppKit view (DESIGN.md, The table view): the outer text
  // view shows the table's one character as a block attachment, whose view
  // is a grid of the cells' own text views over the same editor, scrolling
  // sideways when it is wider than the line. `TableGridModel` and
  // `TableGridLayout` say where the cells go.

  /// A table's attachment, whose view is its grid.
  final class TableAttachment: NSTextAttachment {
    let grid: TableGridView
    /// Read by layout, which TextKit runs on the main thread.
    nonisolated(unsafe) var height: CGFloat = 1

    @MainActor
    init(grid: TableGridView) {
      self.grid = grid
      super.init(data: nil, ofType: nil)
      allowsTextAttachmentView = true
      image = NSImage(size: NSSize(width: 1, height: 1))
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewProvider(
      for parentView: NSView?, location: any NSTextLocation, textContainer: NSTextContainer?
    ) -> NSTextAttachmentViewProvider? {
      nonisolated(unsafe) let layoutManager = textContainer?.textLayoutManager
      nonisolated(unsafe) let attachment = self
      let provider = TableViewProvider(
        textAttachment: attachment, parentView: parentView, textLayoutManager: layoutManager,
        location: location)
      provider.tracksTextAttachmentViewBounds = true
      return provider
    }
  }

  final class TableViewProvider: NSTextAttachmentViewProvider {
    /// The line's width, the grid's height: it scrolls sideways inside.
    override func attachmentBounds(
      for attributes: [NSAttributedString.Key: Any], location: any NSTextLocation,
      textContainer: NSTextContainer?, proposedLineFragment: CGRect, position: CGPoint
    ) -> CGRect {
      guard let attachment = textAttachment as? TableAttachment else { return .zero }
      return CGRect(
        x: 0, y: 0, width: max(proposedLineFragment.width, 1), height: attachment.height)
    }

    override func loadView() {
      guard let attachment = textAttachment as? TableAttachment else { return }
      view = DecoratorContainerView(content: attachment.grid)
    }
  }

  /// A table's grid: its cells' text views in a scroll view that scrolls
  /// sideways only, as `tableScrollableWrapper`'s `overflow-x: auto`.
  @MainActor
  final class TableGridView: NSView {
    let key: NodeKey
    private weak var host: LexicalNSTextView?
    let scrollView = NSScrollView()
    private let content = TableGridContentView()
    private(set) var model = TableGridModel()
    /// The cells' views, by cell key.
    private(set) var cellViews: [NodeKey: LexicalNSTextView] = [:]
    /// Each cell's text height at its width, until it renders again.
    private var measured: [NodeKey: (width: CGFloat, height: CGFloat)] = [:]
    /// The nodes the model was read from, which keep it while unchanged.
    private var readNodes: [ObjectIdentifier] = []
    private var needsLayoutCells = true
    /// Called when the grid's height changes, which the line must take.
    var onHeightChange: ((CGFloat) -> Void)?
    private(set) var gridHeight: CGFloat = 1

    init(key: NodeKey, host: LexicalNSTextView) {
      self.key = key
      self.host = host
      super.init(frame: .zero)
      scrollView.hasHorizontalScroller = true
      scrollView.hasVerticalScroller = false
      scrollView.autohidesScrollers = true
      scrollView.drawsBackground = false
      scrollView.verticalScrollElasticity = .none
      scrollView.documentView = content
      scrollView.autoresizingMask = [.width, .height]
      addSubview(scrollView)
      content.grid = self
      setAccessibilityElement(true)
      setAccessibilityRole(.table)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var isFlipped: Bool { true }

    var theme: TextKitTable { host?.theme.table ?? .playground }

    /// Reads the table again when its nodes changed, keeps one view per
    /// cell, and shows `update`'s table selection and node selection.
    func refresh(_ update: TextKitUpdate) {
      guard let host else { return }
      let editor = host.editor
      let nodes = TableGridModel.nodes(of: key, in: editor.getEditorState())
      if nodes != readNodes {
        readNodes = nodes
        if let model = (try? editor.read(.latest) { try TableGridModel(table: key) }) ?? nil {
          if model != self.model {
            self.model = model
            needsLayoutCells = true
          }
        }
        syncCellViews(host)
      }
      content.selectedCells = update.selectedTableCells.intersection(cellViews.keys)
      content.isTableSelected = update.selectedNodes.contains(key)
      layoutCells()
    }

    private func syncCellViews(_ host: LexicalNSTextView) {
      let keys = Set(model.cells.map(\.key))
      for (key, view) in cellViews where !keys.contains(key) {
        view.removeFromSuperview()
        cellViews[key] = nil
        measured[key] = nil
      }
      for cell in model.cells {
        let view =
          cellViews[cell.key]
          ?? {
            let view = LexicalNSTextView(cell: cell.key, root: host)
            view.tableGrid = self
            content.addSubview(view)
            cellViews[cell.key] = view
            needsLayoutCells = true
            return view
          }()
        view.setAccessibilityRowIndexRange(NSRange(location: cell.row, length: cell.rowSpan))
        view.setAccessibilityColumnIndexRange(
          NSRange(location: cell.column, length: cell.columnSpan))
        view.setAccessibilityRoleDescription(cell.isHeader ? "header cell" : "cell")
      }
      setAccessibilityRowCount(model.rowCount)
      setAccessibilityColumnCount(model.columnCount)
      setAccessibilityChildren(model.cells.compactMap { cellViews[$0.key] })
    }

    /// A cell's text changed: it is measured again.
    func cellDidRender(_ view: LexicalNSTextView) {
      guard let key = cellViews.first(where: { $0.value === view })?.key else { return }
      measured[key] = nil
      needsLayoutCells = true
      layoutCells()
    }

    /// Places the cells, measuring those whose text or width changed.
    func layoutCells() {
      guard needsLayoutCells else { return }
      needsLayoutCells = false
      let theme = theme
      let layout = TableGridLayout(model, theme: theme) { cell, width in
        if let cached = measured[cell.key], cached.width == width { return cached.height }
        guard let view = cellViews[cell.key] else { return 0 }
        view.frame.size.width = width
        let height = view.contentHeight
        measured[cell.key] = (width, height)
        return height
      }
      for (key, frame) in layout.textFrames { cellViews[key]?.frame = frame }
      content.gridLayout = layout
      content.model = model
      content.frame = NSRect(origin: .zero, size: layout.size)
      content.needsDisplay = true
      let margins = CGFloat(theme.marginTop + theme.marginBottom)
      let height =
        layout.size.height + margins
        + (layout.size.width > bounds.width && bounds.width > 0
          ? NSScroller.scrollerWidth(for: .regular, scrollerStyle: .overlay) : 0)
      if height != gridHeight {
        gridHeight = height
        onHeightChange?(height)
      }
      placeScrollView()
    }

    override func layout() {
      super.layout()
      placeScrollView()
    }

    private func placeScrollView() {
      scrollView.frame = NSRect(
        x: 0, y: CGFloat(theme.marginTop), width: bounds.width,
        height: max(1, bounds.height - CGFloat(theme.marginTop + theme.marginBottom)))
    }

    /// Scrolls the grid sideways so `rect`, in `view`'s coordinates, shows.
    func scrollToVisible(_ rect: NSRect, in view: NSView) {
      let target = content.convert(rect, from: view)
      let clip = scrollView.contentView
      var x = clip.bounds.minX
      if target.maxX > x + clip.bounds.width { x = target.maxX - clip.bounds.width }
      if target.minX < x { x = target.minX }
      x = max(0, min(x, content.frame.width - clip.bounds.width))
      guard x != clip.bounds.minX else { return }
      clip.scroll(to: NSPoint(x: x, y: clip.bounds.minY))
      scrollView.reflectScrolledClipView(clip)
    }

    /// The cell view under `point` in window coordinates.
    func cellView(atWindowPoint point: NSPoint) -> LexicalNSTextView? {
      let local = content.convert(point, from: nil)
      guard let key = content.gridLayout.cellFrames.first(where: { $0.value.contains(local) })?.key
      else {
        return nil
      }
      return cellViews[key]
    }
  }

  /// The grid's cells: backgrounds and borders behind the cell views, and
  /// the table selection over them.
  @MainActor
  final class TableGridContentView: NSView {
    weak var grid: TableGridView?
    var model = TableGridModel()
    var gridLayout = TableGridLayout()
    var selectedCells: Set<NodeKey> = [] {
      didSet { if selectedCells != oldValue { overlay.needsDisplay = true } }
    }
    var isTableSelected = false {
      didSet { if isTableSelected != oldValue { overlay.needsDisplay = true } }
    }
    private let overlay = TableSelectionOverlay()

    override init(frame: NSRect) {
      super.init(frame: frame)
      overlay.content = self
      overlay.autoresizingMask = [.width, .height]
      addSubview(overlay)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var isFlipped: Bool { true }

    override func didAddSubview(_ subview: NSView) {
      super.didAddSubview(subview)
      // The selection stays over the cells.
      if subview !== overlay, overlay.superview === self {
        addSubview(overlay, positioned: .above, relativeTo: nil)
      }
    }

    override func layout() {
      super.layout()
      overlay.frame = bounds
    }

    override func draw(_ dirtyRect: NSRect) {
      guard let theme = grid?.theme else { return }
      let half = CGFloat(theme.borderWidth) / 2
      for cell in model.cells {
        guard let frame = gridLayout.cellFrames[cell.key]?.offsetBy(dx: half, dy: half) else {
          continue
        }
        if let background = model.background(of: cell, theme: theme) {
          background.nsColor.setFill()
          frame.fill()
        }
      }
      theme.borderColor.nsColor.setStroke()
      for frame in gridLayout.cellFrames.values {
        let path = NSBezierPath(rect: frame.offsetBy(dx: half, dy: half))
        path.lineWidth = CGFloat(theme.borderWidth)
        path.stroke()
      }
    }

    fileprivate func drawSelection() {
      guard let theme = grid?.theme else { return }
      let half = CGFloat(theme.borderWidth) / 2
      theme.selectedCellFill.nsColor.setFill()
      for key in selectedCells {
        gridLayout.cellFrames[key]?.offsetBy(dx: half, dy: half).fill(using: .sourceOver)
      }
      if isTableSelected {
        let width = CGFloat(theme.selectedOutlineWidth)
        let path = NSBezierPath(rect: bounds.insetBy(dx: width / 2, dy: width / 2))
        path.lineWidth = width
        theme.selectedOutlineColor.nsColor.setStroke()
        path.stroke()
      }
    }
  }

  /// Draws the table selection over the cells; the pointer passes through.
  @MainActor
  final class TableSelectionOverlay: NSView {
    weak var content: TableGridContentView?
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func draw(_ dirtyRect: NSRect) { content?.drawSelection() }
  }
#endif
