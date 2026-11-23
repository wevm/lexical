#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import UIKit

  // A table in the UIKit view (DESIGN.md, The table view): the outer text
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
      image = UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1)).image { _ in }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    /// What VoiceOver says for the table's character, as it reads the
    /// root's text: the table's size. The cells are the grid's elements.
    override var accessibilityLabel: String? {
      get { MainActor.assumeIsolated { grid.accessibilitySummary } }
      set {}
    }

    override func viewProvider(
      for parentView: UIView?, location: any NSTextLocation, textContainer: NSTextContainer?
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
  final class TableGridView: UIView {
    let key: NodeKey
    private weak var host: LexicalUITextView?
    let scrollView = UIScrollView()
    private let content = TableGridContentView()
    private(set) var model = TableGridModel()
    /// The cells' views, by cell key.
    private(set) var cellViews: [NodeKey: LexicalUITextView] = [:]
    /// Each cell's text height at its width, until it renders again.
    private var measured: [NodeKey: (width: CGFloat, height: CGFloat)] = [:]
    /// The nodes the model was read from, which keep it while unchanged.
    private var readNodes: [ObjectIdentifier] = []
    private var needsLayoutCells = true
    /// Called when the grid's height changes, which the line must take.
    var onHeightChange: ((CGFloat) -> Void)?
    private(set) var gridHeight: CGFloat = 1
    /// The table selection's handles, at its top left and bottom right
    /// corners, as Notes puts them: UIKit's own stay inside one text view.
    private let startHandle = TableSelectionHandle(isStart: true)
    private let endHandle = TableSelectionHandle(isStart: false)
    /// Notices a drag of a cell's own selection handle, and never begins:
    /// UIKit's recognizer that moves the handle keeps the touch, so the
    /// drag follows that one (`followed`), as AppKit's view follows
    /// `NSTextView`'s tracking.
    private let textHandleDrag = UIPanGestureRecognizer()
    /// UIKit's recognizers of the cells' text tracking the touch as
    /// `textHandleDrag` is about to begin.
    private var trackingRecognizers: [UIGestureRecognizer] = []
    /// Those the drag follows, until one ends.
    private var followed: [UIGestureRecognizer] = []
    /// The cell whose selection handle the drag moves, and the end of its
    /// range that stays.
    private var textHandleSource: (view: LexicalUITextView, anchor: Int)?
    /// The cell a handle drag has left for another, which shows no
    /// selection of its own until the drag ends: the model ignores that
    /// view's own selection changes meanwhile.
    private(set) weak var dragSource: LexicalUITextView?
    /// The cell and location the drag selected to last.
    private var dragTarget: (ObjectIdentifier, Int)?
    /// The edit menu of a table selection, over its cells.
    private lazy var editMenu = UIEditMenuInteraction(delegate: self)
    /// The cell a table selection handle's drag keeps as the anchor.
    private weak var handleAnchor: LexicalUITextView?

    init(key: NodeKey, host: LexicalUITextView) {
      self.key = key
      self.host = host
      super.init(frame: .zero)
      backgroundColor = .clear
      scrollView.showsVerticalScrollIndicator = false
      scrollView.alwaysBounceVertical = false
      scrollView.alwaysBounceHorizontal = false
      scrollView.contentInsetAdjustmentBehavior = .never
      scrollView.addSubview(content)
      addSubview(scrollView)
      content.grid = self
      accessibilityContainerType = .dataTable
      scrollView.delegate = self
      for handle in [startHandle, endHandle] {
        handle.isHidden = true
        handle.pan.addTarget(self, action: #selector(dragTableHandle(_:)))
        addSubview(handle)
      }
      addInteraction(editMenu)
      textHandleDrag.delegate = self
      textHandleDrag.cancelsTouchesInView = false
      addGestureRecognizer(textHandleDrag)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

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
      layoutHandles()
    }

    private func syncCellViews(_ host: LexicalUITextView) {
      let keys = Set(model.cells.map(\.key))
      for (key, view) in cellViews where !keys.contains(key) {
        view.removeFromSuperview()
        cellViews[key] = nil
        measured[key] = nil
      }
      for cell in model.cells where cellViews[cell.key] == nil {
        let view = LexicalUITextView(cell: cell.key, root: host)
        view.tableGrid = self
        content.insertSubview(view, belowSubview: content.overlay)
        cellViews[cell.key] = view
        needsLayoutCells = true
      }
      accessibilityElements = model.cells.compactMap { cellViews[$0.key] }
    }

    /// Whether the grid shows a table selection.
    var hasTableSelection: Bool { !content.selectedCells.isEmpty }

    /// The table's size.
    var accessibilitySummary: String {
      "Table, \(model.rowCount) rows, \(model.columnCount) columns"
    }

    /// The border box of the cell that `view` shows, in `view`'s
    /// coordinates.
    func cellFrame(of view: LexicalUITextView) -> CGRect? {
      guard let key = cellViews.first(where: { $0.value === view })?.key,
        let frame = content.gridLayout.cellFrames[key]
      else { return nil }
      return view.convert(frame, from: content)
    }

    /// The place of the cell that `view` shows.
    func gridCell(of view: LexicalUITextView) -> TableGridCell? {
      model.cells.first { cellViews[$0.key] === view }
    }

    /// A cell's text changed: it is measured again.
    func cellDidRender(_ view: LexicalUITextView) {
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
      content.frame = CGRect(origin: .zero, size: layout.size)
      content.setNeedsDisplay()
      scrollView.contentSize = layout.size
      let height = layout.size.height + CGFloat(theme.marginTop + theme.marginBottom)
      if height != gridHeight {
        gridHeight = height
        onHeightChange?(height)
      }
      setNeedsLayout()
    }

    override func layoutSubviews() {
      super.layoutSubviews()
      scrollView.frame = CGRect(
        x: 0, y: CGFloat(theme.marginTop), width: bounds.width,
        height: max(1, bounds.height - CGFloat(theme.marginTop + theme.marginBottom)))
      layoutHandles()
    }

    /// The cell view under `point` in `view`'s coordinates.
    func cellView(at point: CGPoint, in view: UIView) -> LexicalUITextView? {
      let local = content.convert(point, from: view)
      guard let key = content.gridLayout.cellFrames.first(where: { $0.value.contains(local) })?.key
      else { return nil }
      return cellViews[key]
    }

    /// The cell view under `point` in the grid's coordinates, or the
    /// nearest one at the grid's edge, so a drag past it selects to there.
    private func nearestCellView(at point: CGPoint) -> LexicalUITextView? {
      let local = content.convert(point, from: self)
      let size = content.gridLayout.size
      guard size.width > 2, size.height > 2 else { return nil }
      let clamped = CGPoint(
        x: min(max(local.x, 1), size.width - 2), y: min(max(local.y, 1), size.height - 2))
      return cellView(at: clamped, in: content)
    }

    // MARK: Table selection handles

    /// The selected cells' corners, in the grid's coordinates, and the
    /// cells there.
    private var selectionCorners:
      (start: CGPoint, end: CGPoint, topLeft: LexicalUITextView, bottomRight: LexicalUITextView)?
    {
      guard hasTableSelection else { return nil }
      let selected = model.cells.filter { content.selectedCells.contains($0.key) }
      let top = selected.map(\.row).min()!
      let left = selected.map(\.column).min()!
      let bottom = selected.map { $0.row + $0.rowSpan - 1 }.max()!
      let right = selected.map { $0.column + $0.columnSpan - 1 }.max()!
      guard let topLeft = model.cell(row: top, column: left).flatMap({ cellViews[$0.key] }),
        let bottomRight = model.cell(row: bottom, column: right).flatMap({ cellViews[$0.key] })
      else { return nil }
      let frame = selected.compactMap { content.gridLayout.cellFrames[$0.key] }
        .reduce(CGRect.null) { $0.union($1) }
      let half = CGFloat(theme.borderWidth) / 2
      let box = convert(frame.offsetBy(dx: half, dy: half), from: content)
      return (
        CGPoint(x: box.minX, y: box.minY), CGPoint(x: box.maxX, y: box.maxY), topLeft, bottomRight
      )
    }

    /// Shows the handles at the table selection's corners, or hides them.
    private func layoutHandles() {
      guard let corners = selectionCorners else {
        if !startHandle.isHidden {
          startHandle.isHidden = true
          endHandle.isHidden = true
          editMenu.dismissMenu()
        }
        return
      }
      for (handle, corner) in [(startHandle, corners.start), (endHandle, corners.end)] {
        handle.isHidden = false
        handle.tintColor = host?.tintColor
        handle.place(at: corner, in: bounds)
      }
    }

    /// Shows the edit menu over the table selection, as UIKit shows a text
    /// selection's once a drag of its handle ends.
    private func showEditMenu() {
      guard let corners = selectionCorners else { return }
      // After the touch's end has dismissed the menu it found open.
      DispatchQueue.main.async { [editMenu] in
        editMenu.presentEditMenu(
          with: UIEditMenuConfiguration(
            identifier: nil,
            sourcePoint: CGPoint(x: (corners.start.x + corners.end.x) / 2, y: corners.start.y)))
      }
    }

    /// The cell's view that keeps the keyboard during a table selection,
    /// whose `copy` and `cut` take the selected cells from the model.
    private var editingCellView: LexicalUITextView? {
      hasTableSelection ? cellViews.values.first(where: \.isFirstResponder) : nil
    }

    /// The menu's commands go to that cell's view.
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
      editingCellView?.canPerformAction(action, withSender: sender)
        ?? super.canPerformAction(action, withSender: sender)
    }

    override func target(forAction action: Selector, withSender sender: Any?) -> Any? {
      if let view = editingCellView, view.canPerformAction(action, withSender: sender) {
        return view
      }
      return super.target(forAction: action, withSender: sender)
    }

    /// A drag of a table selection's handle: the cell under the touch is the
    /// selection's focus, and the far corner's cell its anchor.
    @objc private func dragTableHandle(_ pan: UIPanGestureRecognizer) {
      switch pan.state {
      case .began:
        editMenu.dismissMenu()
        guard let corners = selectionCorners else { return }
        handleAnchor = pan.view === startHandle ? corners.bottomRight : corners.topLeft
        dragTarget = nil
      case .changed:
        guard let anchor = handleAnchor,
          let target = nearestCellView(at: pan.location(in: self))
        else { return }
        let location = pan.view === startHandle ? 0 : (target.text as NSString).length
        if let dragTarget, dragTarget == (ObjectIdentifier(target), location) { return }
        dragTarget = (ObjectIdentifier(target), location)
        let anchorLocation = pan.view === startHandle ? (anchor.text as NSString).length : 0
        target.select(from: anchorLocation, of: anchor, to: location)
      default:
        handleAnchor = nil
        dragTarget = nil
        showEditMenu()
      }
    }

    /// A drag of a cell's own selection handle: past the cell's edge, the
    /// selection runs from the cell to the one under the touch, a table
    /// selection, as `TableObserver` makes one on `pointermove`; back in
    /// the cell, it is the cell's range again.
    @objc private func dragTextHandle(_ recognizer: UIGestureRecognizer) {
      switch recognizer.state {
      case .began, .changed:
        guard let source = textHandleSource,
          let target = nearestCellView(at: recognizer.location(in: self))
        else { return }
        let location = target.location(at: recognizer.location(in: target))
        if let dragTarget, dragTarget == (ObjectIdentifier(target), location) { return }
        if target === source.view, dragSource == nil { return }
        dragTarget = (ObjectIdentifier(target), location)
        dragSource = target === source.view ? nil : source.view
        target.select(from: source.anchor, of: source.view, to: location)
      default:
        // The touch ended (a recognizer that failed before reports none).
        for recognizer in followed {
          recognizer.removeTarget(self, action: #selector(dragTextHandle(_:)))
        }
        followed = []
        if dragSource != nil { showEditMenu() }
        textHandleSource = nil
        dragSource = nil
        dragTarget = nil
      }
    }

    /// Scrolls the grid sideways so `rect`, in `view`'s coordinates, shows.
    func scrollToVisible(_ rect: CGRect, in view: UIView) {
      scrollView.scrollRectToVisible(scrollView.convert(rect, from: view), animated: false)
    }
  }

  extension TableGridView: UIGestureRecognizerDelegate, UIScrollViewDelegate {
    /// A touch at a knob of the first responder's selection, while it has a
    /// range, drags UIKit's handle: the drag follows the recognizer of the
    /// cell's that tracks it, and this one gives way.
    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
      guard gestureRecognizer === textHandleDrag else { return true }
      defer { trackingRecognizers = [] }
      let point = gestureRecognizer.location(in: self)
      guard let view = cellViews.values.first(where: \.isFirstResponder),
        view.selectedRange.length > 0, let range = view.selectedTextRange,
        !trackingRecognizers.isEmpty
      else { return false }
      // UIKit's knob is above the start and below the end (one line's rect
      // holds both); the nearest is the one dragged, and the far end stays.
      let selected = view.selectedRange
      var knobs: [(point: CGPoint, anchor: Int)] = []
      for rect in view.selectionRects(for: range) {
        let frame = convert(rect.rect, from: view)
        if rect.containsStart {
          knobs.append((CGPoint(x: frame.minX, y: frame.minY), NSMaxRange(selected)))
        }
        if rect.containsEnd {
          knobs.append((CGPoint(x: frame.maxX, y: frame.maxY), selected.location))
        }
      }
      func distance(_ knob: CGPoint) -> CGFloat { hypot(point.x - knob.x, point.y - knob.y) }
      guard let knob = knobs.min(by: { distance($0.point) < distance($1.point) }),
        distance(knob.point) <= 30
      else { return false }
      textHandleSource = (view, knob.anchor)
      // Which of them keeps reporting the touch is UIKit's to decide.
      for recognizer in followed {
        recognizer.removeTarget(self, action: #selector(dragTextHandle(_:)))
      }
      followed = trackingRecognizers
      for recognizer in followed {
        recognizer.addTarget(self, action: #selector(dragTextHandle(_:)))
      }
      return false
    }

    /// A new touch: the recognizers noted for the last one are done.
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch
    ) -> Bool {
      if gestureRecognizer === textHandleDrag { trackingRecognizers = [] }
      return true
    }

    /// Notes UIKit's recognizers of the cells' text that track the touch.
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer,
      shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
    ) -> Bool {
      if gestureRecognizer === textHandleDrag, otherGestureRecognizer.view is LexicalUITextView,
        [.began, .changed].contains(otherGestureRecognizer.state)
      {
        trackingRecognizers.append(otherGestureRecognizer)
      }
      return true
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) { layoutHandles() }
  }

  extension TableGridView: @MainActor UIEditMenuInteractionDelegate {
    /// Cut, Copy and Paste, which take the selected cells; not the text's
    /// commands (Select, Select All, Replace, Look Up).
    func editMenuInteraction(
      _ interaction: UIEditMenuInteraction, menuFor configuration: UIEditMenuConfiguration,
      suggestedActions: [UIMenuElement]
    ) -> UIMenu? {
      let actions: Set<Selector> = [
        #selector(UIResponderStandardEditActions.cut(_:)),
        #selector(UIResponderStandardEditActions.copy(_:)),
        #selector(UIResponderStandardEditActions.paste(_:)),
      ]
      func keep(_ element: UIMenuElement) -> UIMenuElement? {
        if let command = element as? UICommand {
          return actions.contains(command.action) ? command : nil
        }
        guard let menu = element as? UIMenu else { return nil }
        let children = menu.children.compactMap(keep)
        return children.isEmpty ? nil : menu.replacingChildren(children)
      }
      return UIMenu(children: suggestedActions.compactMap(keep))
    }

    /// The menu points at the selected cells.
    func editMenuInteraction(
      _ interaction: UIEditMenuInteraction, targetRectFor configuration: UIEditMenuConfiguration
    ) -> CGRect {
      guard let corners = selectionCorners else { return .null }
      return CGRect(
        x: corners.start.x, y: corners.start.y, width: corners.end.x - corners.start.x,
        height: corners.end.y - corners.start.y)
    }
  }

  /// A table selection's handle: UIKit's knob, at a corner of the selected
  /// cells, in a 44-point target, as the HIG asks of a control.
  @MainActor
  final class TableSelectionHandle: UIView, UIGestureRecognizerDelegate {
    let isStart: Bool
    let pan = UIPanGestureRecognizer()
    /// The corner, in the handle's coordinates.
    private var corner = CGPoint.zero
    private static let knobDiameter: CGFloat = 12
    private static let targetSize: CGFloat = 44

    init(isStart: Bool) {
      self.isStart = isStart
      super.init(frame: CGRect(x: 0, y: 0, width: Self.targetSize, height: Self.targetSize))
      isOpaque = false
      backgroundColor = .clear
      contentMode = .redraw
      pan.delegate = self
      addGestureRecognizer(pan)
      isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    /// Puts the knob just inside the selection's `corner`, so the grid's
    /// edge never cuts it, and centers the target on it, kept inside
    /// `bounds` so the touch reaches it.
    func place(at corner: CGPoint, in bounds: CGRect) {
      let inset = Self.knobDiameter / 2 + 1
      let corner = CGPoint(
        x: corner.x + (isStart ? inset : -inset), y: corner.y + (isStart ? inset : -inset))
      let size = Self.targetSize
      let origin = CGPoint(
        x: min(max(corner.x - size / 2, bounds.minX), max(bounds.minX, bounds.maxX - size)),
        y: min(max(corner.y - size / 2, bounds.minY), max(bounds.minY, bounds.maxY - size)))
      frame = CGRect(origin: origin, size: CGSize(width: size, height: size))
      self.corner = CGPoint(x: corner.x - origin.x, y: corner.y - origin.y)
      superview?.bringSubviewToFront(self)
      setNeedsDisplay()
    }

    override func tintColorDidChange() {
      super.tintColorDidChange()
      setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
      let diameter = Self.knobDiameter
      let knob = CGRect(
        x: corner.x - diameter / 2, y: corner.y - diameter / 2, width: diameter, height: diameter)
      let ring = UIBezierPath(ovalIn: knob.insetBy(dx: -1.5, dy: -1.5))
      UIColor.systemBackground.setFill()
      ring.fill()
      tintColor.setFill()
      UIBezierPath(ovalIn: knob).fill()
    }

    /// Scroll views and the cells' text gestures wait for the handle's
    /// drag to fail.
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer,
      shouldBeRequiredToFailBy otherGestureRecognizer: UIGestureRecognizer
    ) -> Bool { true }
  }

  /// VoiceOver reads the grid as a data table, `<table>`'s role on the web:
  /// each cell's text view is its element at its row and column, and a
  /// header row's or column's cells head the cells of their column or row.
  extension TableGridView: @MainActor UIAccessibilityContainerDataTable {
    func accessibilityRowCount() -> Int { model.rowCount }

    func accessibilityColumnCount() -> Int { model.columnCount }

    func accessibilityDataTableCellElement(
      forRow row: Int, column: Int
    )
      -> (any UIAccessibilityContainerDataTableCell)?
    {
      model.cell(row: row, column: column).flatMap { cellViews[$0.key] }
    }

    func accessibilityHeaderElements(
      forRow row: Int
    ) -> [any UIAccessibilityContainerDataTableCell]? {
      model.cells.filter { $0.headsRow && $0.covers(row: row, column: $0.column) }
        .compactMap { cellViews[$0.key] }
    }

    func accessibilityHeaderElements(
      forColumn column: Int
    )
      -> [any UIAccessibilityContainerDataTableCell]?
    {
      model.cells.filter { $0.headsColumn && $0.covers(row: $0.row, column: column) }
        .compactMap { cellViews[$0.key] }
    }
  }

  /// A table cell's text view is its grid's element at the cell's rows and
  /// columns; another view of the editor is in no table.
  extension LexicalUITextView: @MainActor UIAccessibilityContainerDataTableCell {
    public func accessibilityRowRange() -> NSRange {
      guard let cell = tableGrid?.gridCell(of: self) else {
        return NSRange(location: NSNotFound, length: 0)
      }
      return NSRange(location: cell.row, length: cell.rowSpan)
    }

    public func accessibilityColumnRange() -> NSRange {
      guard let cell = tableGrid?.gridCell(of: self) else {
        return NSRange(location: NSNotFound, length: 0)
      }
      return NSRange(location: cell.column, length: cell.columnSpan)
    }
  }

  /// The grid's cells: backgrounds and borders behind the cell views, and
  /// the table selection over them.
  @MainActor
  final class TableGridContentView: UIView {
    weak var grid: TableGridView?
    var model = TableGridModel()
    var gridLayout = TableGridLayout()
    var selectedCells: Set<NodeKey> = [] {
      didSet { if selectedCells != oldValue { overlay.setNeedsDisplay() } }
    }
    var isTableSelected = false {
      didSet { if isTableSelected != oldValue { overlay.setNeedsDisplay() } }
    }
    let overlay = TableSelectionOverlay()

    override init(frame: CGRect) {
      super.init(frame: frame)
      backgroundColor = .clear
      contentMode = .redraw
      overlay.content = self
      overlay.autoresizingMask = [.flexibleWidth, .flexibleHeight]
      addSubview(overlay)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func layoutSubviews() {
      super.layoutSubviews()
      overlay.frame = bounds
    }

    override func draw(_ rect: CGRect) {
      guard let theme = grid?.theme else { return }
      let half = CGFloat(theme.borderWidth) / 2
      for cell in model.cells {
        guard let frame = gridLayout.cellFrames[cell.key]?.offsetBy(dx: half, dy: half),
          let background = model.background(of: cell, theme: theme)
        else { continue }
        background.uiColor.setFill()
        UIRectFill(frame)
      }
      theme.borderColor.uiColor.setStroke()
      for frame in gridLayout.cellFrames.values {
        let path = UIBezierPath(rect: frame.offsetBy(dx: half, dy: half))
        path.lineWidth = CGFloat(theme.borderWidth)
        path.stroke()
      }
    }

    fileprivate func drawSelection() {
      guard let theme = grid?.theme else { return }
      let half = CGFloat(theme.borderWidth) / 2
      theme.selectedCellFill.uiColor.setFill()
      for key in selectedCells {
        if let frame = gridLayout.cellFrames[key] {
          UIBezierPath(rect: frame.offsetBy(dx: half, dy: half)).fill()
        }
      }
      if isTableSelected {
        let width = CGFloat(theme.selectedOutlineWidth)
        let path = UIBezierPath(rect: bounds.insetBy(dx: width / 2, dy: width / 2))
        path.lineWidth = width
        theme.selectedOutlineColor.uiColor.setStroke()
        path.stroke()
      }
    }
  }

  /// Draws the table selection over the cells; touches pass through.
  @MainActor
  final class TableSelectionOverlay: UIView {
    weak var content: TableGridContentView?

    override init(frame: CGRect) {
      super.init(frame: frame)
      isOpaque = false
      backgroundColor = .clear
      isUserInteractionEnabled = false
      contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func draw(_ rect: CGRect) { content?.drawSelection() }
  }
#endif
