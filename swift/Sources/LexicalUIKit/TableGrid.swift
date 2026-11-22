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

    /// What VoiceOver says for the table's character: a `UITextView` is one
    /// accessibility element, so it does not reach the cells' views, and
    /// reads the table's size and text instead.
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

    /// The table's size and its cells' text, row by row.
    var accessibilitySummary: String {
      var rows: [[String]] = Array(repeating: [], count: model.rowCount)
      for cell in model.cells { rows[cell.row].append(cellViews[cell.key]?.text ?? "") }
      let size = "Table, \(model.rowCount) rows, \(model.columnCount) columns"
      return ([size] + rows.map { $0.joined(separator: ", ") }).joined(separator: ". ")
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
    }

    /// The cell view under `point` in `view`'s coordinates.
    func cellView(at point: CGPoint, in view: UIView) -> LexicalUITextView? {
      let local = content.convert(point, from: view)
      guard let key = content.gridLayout.cellFrames.first(where: { $0.value.contains(local) })?.key
      else { return nil }
      return cellViews[key]
    }

    /// Scrolls the grid sideways so `rect`, in `view`'s coordinates, shows.
    func scrollToVisible(_ rect: CGRect, in view: UIView) {
      scrollView.scrollRectToVisible(scrollView.convert(rect, from: view), animated: false)
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
