import Lexical

// Port of packages/lexical-table/src/LexicalTableObserver.ts: the model
// state of `TableObserver` and `TableObservers`.
//
// JS keeps the grid of the table's DOM (`TableDOMTable`), whose cells carry
// their `<td>`/`<th>` element, and rebuilds it from a `MutationObserver` on
// the table element. The native views have no table element, so a
// `TableDOMCell` carries the key of its cell node where JS carries `elem`
// (JS reads the node back with `$getNearestNodeFromDOMNode(elem)`), and
// `getTable` builds the same grid (a row's cells by index, as JS walks the
// `<tr>`s and their cells) from the table node's children. The observer
// rebuilds it when its table is reconciled, where JS's `MutationObserver`
// fires. The DOM-only parts are left out: the theme classes `$addHighlightToDOM`
// and `$enableHighlightStyle` toggle on the cells and the table (the views
// draw the selection from the model, see DESIGN.md), and the native
// selection `updateDOMSelection` clears. `$getTableAndElementByKey` returns
// the table element, so it belongs to the views and is not ported.

/// `TableDOMCell`: a cell of the grid, at column `x` of row `y`.
public final class TableDOMCell {
  /// The key of the cell's `TableCellNode`, where JS holds its element.
  public let key: NodeKey
  public var highlighted: Bool
  public var hasBackgroundColor: Bool
  public let x: Int
  public let y: Int

  public init(key: NodeKey, highlighted: Bool, hasBackgroundColor: Bool, x: Int, y: Int) {
    self.key = key
    self.highlighted = highlighted
    self.hasBackgroundColor = hasBackgroundColor
    self.x = x
    self.y = y
  }
}

/// `TableDOMRows`.
public typealias TableDOMRows = [[TableDOMCell?]?]

/// `TableDOMTable`.
public struct TableDOMTable {
  public var domRows: TableDOMRows
  public var columns: Int
  public var rows: Int

  public init(domRows: TableDOMRows = [], columns: Int = 0, rows: Int = 0) {
    self.domRows = domRows
    self.columns = columns
    self.rows = rows
  }
}

/// `TableNextFocus`.
package struct TableNextFocus {
  var tableKey: NodeKey
  var focusCell: TableDOMCell
  var override: Bool
}

/// `TableObservers`: tracks table selection state that sits across all
/// tables.
public final class TableObservers {
  package let observers = JSMap<NodeKey, TableObserver>()
  package var nextFocus: TableNextFocus?
  package var shouldCheckSelectionForTable: NodeKey?
  /// The last touch-tapped cell, retained after pointerup for native word
  /// selection.
  package var touchTapCellKey: NodeKey?

  public init() {}

  /// `setNextFocus(nextFocus)` (`@internal`).
  package func setNextFocus(_ nextFocus: TableNextFocus?) {
    self.nextFocus = nextFocus
  }

  /// `getAndClearNextFocus()` (`@internal`).
  package func getAndClearNextFocus() -> TableNextFocus? {
    let nextFocus = self.nextFocus
    if nextFocus != nil {
      self.nextFocus = nil
    }
    return nextFocus
  }

  /// `setShouldCheckSelectionForTable(tableKey)` (`@internal`).
  package func setShouldCheckSelectionForTable(_ tableKey: NodeKey) {
    shouldCheckSelectionForTable = tableKey
  }

  /// `getAndClearShouldCheckSelectionForTable()` (`@internal`).
  package func getAndClearShouldCheckSelectionForTable() -> NodeKey? {
    if let shouldCheckSelectionForTable {
      self.shouldCheckSelectionForTable = nil
      return shouldCheckSelectionForTable
    }
    return nil
  }

  /// `removeObserver(tableKey)` (`@internal`): unregisters the observer of
  /// `tableKey`. Returns whether there was one.
  @discardableResult
  package func removeObserver(_ tableKey: NodeKey) -> Bool {
    guard let observer = observers.get(tableKey) else {
      return false
    }
    observer.removeListeners()
    _ = observers.delete(tableKey)
    return true
  }

  /// `removeAllObservers()` (`@internal`).
  package func removeAllObservers() {
    for tableKey in observers.keysArray {
      removeObserver(tableKey)
    }
  }

  /// `$getTableNodesAndObservers()` (`@internal`): the observers whose
  /// table still exists, removing the others (a destroyed mutation can be
  /// missed while the root element is detached).
  package func getTableNodesAndObservers() throws -> [(TableNode, TableObserver)] {
    var tableNodesAndObservers: [(TableNode, TableObserver)] = []
    for (tableKey, tableObserver) in Array(observers) {
      if let tableNode = try getNodeByKey(tableKey) as? TableNode {
        tableNodesAndObservers.append((tableNode, tableObserver))
      } else {
        removeObserver(tableKey)
      }
    }
    return tableNodesAndObservers
  }
}

/// `TableObserver`: the selection state of one table.
public final class TableObserver {
  public var focusX: Int
  public var focusY: Int
  package var listenersToRemove: [() -> Void]
  public var table: TableDOMTable
  public var isHighlightingCells: Bool
  public var anchorX: Int
  public var anchorY: Int
  public let tableNodeKey: NodeKey
  public var anchorCell: TableDOMCell?
  public var focusCell: TableDOMCell?
  public var anchorCellNodeKey: NodeKey?
  public var focusCellNodeKey: NodeKey?
  public let editor: Editor
  public var tableSelection: TableSelection?
  public var hasHijackedSelectionStyles: Bool
  public var isSelecting: Bool
  /// Whether the pointer gesture in progress is a drag rather than a tap.
  public var isPointerDrag: Bool
  /// The cell the pointer gesture in progress started on.
  public var pointerStartCell: TableDOMCell?
  public var pointerType: String?

  public init(_ editor: Editor, _ tableNodeKey: NodeKey) throws {
    isHighlightingCells = false
    anchorX = -1
    anchorY = -1
    focusX = -1
    focusY = -1
    listenersToRemove = []
    self.tableNodeKey = tableNodeKey
    self.editor = editor
    table = TableDOMTable()
    tableSelection = nil
    anchorCellNodeKey = nil
    focusCellNodeKey = nil
    anchorCell = nil
    focusCell = nil
    hasHijackedSelectionStyles = false
    isSelecting = false
    isPointerDrag = false
    pointerStartCell = nil
    pointerType = nil
    try trackTable()
  }

  public func getTable() -> TableDOMTable { table }

  public func removeListeners() {
    let listeners = listenersToRemove
    listenersToRemove = []
    for removeListener in listeners {
      removeListener()
    }
  }

  /// `$lookup()`: the table node (JS also returns its element).
  public func lookup() throws -> TableNode {
    guard let tableNode = try getNodeByKey(tableNodeKey) as? TableNode else {
      throw LexicalError("TableObserver: Expected tableNodeKey \(tableNodeKey) to be a TableNode")
    }
    return tableNode
  }

  /// `trackTable()`: the grid of the table, which `registerTableSelectionObserver`
  /// refreshes when the table is reconciled, as JS's `MutationObserver` does.
  package func trackTable() throws {
    try editor.read(.latest) { () throws -> Void in
      table = try LexicalTable.getTable(lookup())
    }
  }

  /// `$clearHighlight(setEmptySelection)`.
  public func clearHighlight(_ setEmptySelection: Bool = true) throws {
    isHighlightingCells = false
    anchorX = -1
    anchorY = -1
    focusX = -1
    focusY = -1
    tableSelection = nil
    anchorCellNodeKey = nil
    focusCellNodeKey = nil
    anchorCell = nil
    focusCell = nil
    hasHijackedSelectionStyles = false

    try enableHighlightStyle()

    let grid = try LexicalTable.getTable(lookup())
    try updateDOMForSelection(editor, grid, nil)
    if setEmptySelection, try getSelection() != nil {
      try setSelection(nil)
      _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
    }
  }

  /// `$enableHighlightStyle()`: JS also removes the `tableSelection` theme
  /// class from the table element.
  public func enableHighlightStyle() throws {
    _ = try lookup()
    hasHijackedSelectionStyles = false
  }

  /// `$disableHighlightStyle()`: JS also adds the `tableSelection` theme
  /// class to the table element.
  public func disableHighlightStyle() throws {
    _ = try lookup()
    hasHijackedSelectionStyles = true
  }

  /// `$updateTableTableSelection(selection)`.
  public func updateTableTableSelection(_ selection: TableSelection?) throws {
    if let selection {
      guard selection.tableKey == tableNodeKey else {
        throw LexicalError(
          "TableObserver.$updateTableTableSelection: selection.tableKey !== this.tableNodeKey ('\(selection.tableKey)' !== '\(tableNodeKey)')"
        )
      }
      tableSelection = selection
      isHighlightingCells = true
      try disableHighlightStyle()
      updateDOMSelection()
      try updateDOMForSelection(editor, table, tableSelection)
    } else {
      try clearHighlight()
    }
  }

  /// `updateDOMSelection()` (`@internal`): JS removes the native selection's
  /// ranges, which the views do from the model selection.
  package func updateDOMSelection() {}

  /// `$setFocusCellForSelection(cell, ignoreStart)`.
  @discardableResult
  public func setFocusCellForSelection(
    _ cell: TableDOMCell, _ ignoreStart: Bool = false
  ) throws
    -> Bool
  {
    let tableNode = try lookup()

    let cellX = cell.x
    let cellY = cell.y
    focusCell = cell

    // Enable highlighting if: ignoreStart is true, or anchor differs from
    // focus, or we have valid tableSelection with anchor (for first drag
    // after column switch)
    if !isHighlightingCells {
      let shouldEnable =
        ignoreStart || anchorX != cellX || anchorY != cellY
        || (tableSelection != nil && anchorCellNodeKey != nil)
      if shouldEnable {
        isHighlightingCells = true
        try disableHighlightStyle()
      }
    }

    // Skip if we're trying to select the same cell we already have selected
    // But only if focusX/focusY are valid (not -1, which means not reset)
    if focusX != -1 && focusY != -1 && cellX == focusX && cellY == focusY {
      return false
    }

    focusX = cellX
    focusY = cellY

    if isHighlightingCells {
      let focusTableCellNode = try getNearestTableCellInTableFromDOMNode(tableNode, cell)

      if tableSelection != nil && anchorCellNodeKey != nil {
        var targetCellNode = focusTableCellNode

        // Fallback: use coordinates if DOM lookup failed (handles timing
        // issues on first drag)
        if targetCellNode == nil && ignoreStart {
          targetCellNode = try tableNode.getCellNodeFromCords(cellX, cellY, table)
        }

        if let targetCellNode {
          let anchorTableCell = try getAnchorTableCellOrThrow()
          focusCellNodeKey = targetCellNode.getKey()
          let tableSelection = try createTableSelectionFrom(
            tableNode, anchorTableCell, targetCellNode)
          self.tableSelection = tableSelection

          try setSelection(tableSelection)
          _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
          try updateDOMForSelection(editor, table, tableSelection)
          return true
        }
      }
    }
    return false
  }

  /// `$getAnchorTableCell()`.
  public func getAnchorTableCell() throws -> TableCellNode? {
    guard let anchorCellNodeKey else { return nil }
    return try getNodeByKey(anchorCellNodeKey) as? TableCellNode
  }

  /// `$getAnchorTableCellOrThrow()`.
  public func getAnchorTableCellOrThrow() throws -> TableCellNode {
    guard let anchorTableCell = try getAnchorTableCell() else {
      throw LexicalError("TableObserver anchorTableCell is null")
    }
    return anchorTableCell
  }

  /// `$getFocusTableCell()`.
  public func getFocusTableCell() throws -> TableCellNode? {
    guard let focusCellNodeKey else { return nil }
    return try getNodeByKey(focusCellNodeKey) as? TableCellNode
  }

  /// `$getFocusTableCellOrThrow()`.
  public func getFocusTableCellOrThrow() throws -> TableCellNode {
    guard let focusTableCell = try getFocusTableCell() else {
      throw LexicalError("TableObserver focusTableCell is null")
    }
    return focusTableCell
  }

  /// `$setAnchorCellForSelection(cell)`.
  public func setAnchorCellForSelection(_ cell: TableDOMCell) throws {
    isHighlightingCells = false
    anchorCell = cell
    anchorX = cell.x
    anchorY = cell.y
    // Reset focus state to prevent stale values from previous selections
    focusX = -1
    focusY = -1
    focusCell = nil
    focusCellNodeKey = nil

    let tableNode = try lookup()
    let anchorTableCellNode = try getNearestTableCellInTableFromDOMNode(tableNode, cell)

    if let anchorTableCellNode {
      let anchorNodeKey = anchorTableCellNode.getKey()
      if let tableSelection {
        let clone = tableSelection.cloneTable()
        clone.set(tableNode.getKey(), anchorNodeKey, anchorNodeKey)
        self.tableSelection = clone
      } else {
        tableSelection = try createTableSelectionFrom(
          tableNode, anchorTableCellNode, anchorTableCellNode)
      }
      anchorCellNodeKey = anchorNodeKey
    }
  }

  /// `$formatCells(type)`.
  public func formatCells(_ type: TextFormatType) throws {
    guard let selection = try getSelection() as? TableSelection else {
      throw LexicalError("Expected Table selection")
    }

    let formatSelection = createRangeSelection()

    let anchor = formatSelection.anchor
    let focus = formatSelection.focus

    let cellNodes = try selection.getNodes().compactMap { $0 as? TableCellNode }
    guard !cellNodes.isEmpty else {
      throw LexicalError("No table cells present")
    }
    let paragraph = try cellNodes[0].getFirstChild()
    let alignFormatWith = try (paragraph as? ParagraphNode)?.getFormatFlags(type, nil)

    for cellNode in cellNodes {
      try anchor.set(cellNode.getKey(), 0, .element)
      try focus.set(cellNode.getKey(), cellNode.getChildrenSize(), .element)
      try formatSelection.formatText(type, alignFormatWith)
    }

    try setSelection(selection)

    _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
  }

  /// `$clearText()`.
  public func clearText() throws {
    guard let tableNode = try getNodeByKey(tableNodeKey) as? TableNode else {
      throw LexicalError("Expected TableNode.")
    }

    guard let selection = try getSelection() as? TableSelection else {
      throw LexicalError("Expected TableSelection")
    }

    let selectedNodes = try selection.getNodes().compactMap { $0 as? TableCellNode }

    // Check if the entire table is selected by verifying first and last cells
    let firstRow = try tableNode.getFirstChild()
    let lastRow = try tableNode.getLastChild()

    var isEntireTableSelected = false
    if let first = selectedNodes.first, let last = selectedNodes.last,
      let firstRow = firstRow as? TableRowNode, let lastRow = lastRow as? TableRowNode
    {
      isEntireTableSelected =
        try first === firstRow.getFirstChild() && last === lastRow.getLastChild()
    }

    if isEntireTableSelected {
      try tableNode.selectPrevious()
      let parent = try tableNode.getParent()
      // Delete entire table
      try tableNode.remove()
      // Handle case when table was the only node
      if let parent = parent as? RootNode, try parent.isEmpty() {
        _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
      }
      return
    }

    for cellNode in selectedNodes {
      // Clearing a cell empties its content; it does not reset how that
      // content is laid out, so the cell's paragraph is copied (format,
      // style, direction and indent, without children) when there is one.
      let firstChild = try cellNode.getFirstChild()
      let paragraphNode: ElementNode =
        try (firstChild as? ParagraphNode).map { try copyNode($0) } ?? createParagraphNode()
      let textNode = try createTextNode()
      try paragraphNode.append(textNode)
      try cellNode.append(paragraphNode)
      for child in try cellNode.getChildren() where child !== paragraphNode {
        try child.remove()
      }
    }

    try updateDOMForSelection(editor, table, nil)

    try setSelection(nil)

    _ = try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
  }
}

/// `getTable(tableNode, dom)`: the grid of `tableNode`. JS walks the
/// table element's rows and cells; this walks the table node's, which the
/// reconciler renders one to one.
public func getTable(_ tableNode: TableNode) throws -> TableDOMTable {
  var domRows: TableDOMRows = []
  var x = 0
  var y = 0
  var sawRow = false
  for row in try tableNode.getChildren() {
    guard let row = row as? ElementNode else { continue }
    if sawRow {
      y += 1
    }
    sawRow = true
    x = 0
    var cells: [TableDOMCell?] = []
    for (index, cell) in try row.getChildren().enumerated() {
      x = index
      guard let cell = cell as? TableCellNode else {
        cells.append(nil)
        continue
      }
      cells.append(
        TableDOMCell(
          key: cell.getKey(), highlighted: false,
          hasBackgroundColor: try cell.getBackgroundColor() != nil, x: x, y: y))
    }
    domRows.append(cells)
  }
  return TableDOMTable(domRows: domRows, columns: x + 1, rows: y + 1)
}

/// `$updateDOMForSelection(editor, table, selection)`: marks the cells
/// `selection` holds as highlighted (JS also toggles the
/// `tableCellSelected` theme class on their elements).
public func updateDOMForSelection(
  _ editor: Editor, _ table: TableDOMTable, _ selection: (any BaseSelection)?
) throws {
  let selectedCellKeys = Set(try selection?.getNodes().map { $0.getKey() } ?? [])
  try forEachTableCell(table) { cell, lexicalNode, _ in
    cell.highlighted = selectedCellKeys.contains(lexicalNode.getKey())
  }
}

/// `$forEachTableCell(grid, cb)`.
public func forEachTableCell(
  _ grid: TableDOMTable,
  _ cb: (TableDOMCell, LexicalNode, (x: Int, y: Int)) throws -> Void
) throws {
  for (y, row) in grid.domRows.enumerated() {
    guard let row else { continue }
    for (x, cell) in row.enumerated() {
      guard let cell else { continue }
      if let lexicalNode = try getNodeByKey(cell.key) {
        try cb(cell, lexicalNode, (x, y))
      }
    }
  }
}

/// `$addHighlightStyleToTable(editor, tableSelection)`.
public func addHighlightStyleToTable(_ editor: Editor, _ tableSelection: TableObserver) throws {
  try tableSelection.disableHighlightStyle()
  try forEachTableCell(tableSelection.table) { cell, _, _ in cell.highlighted = true }
}

/// `$removeHighlightStyleToTable(editor, tableObserver)`.
public func removeHighlightStyleToTable(_ editor: Editor, _ tableObserver: TableObserver) throws {
  try tableObserver.enableHighlightStyle()
  try forEachTableCell(tableObserver.table) { cell, _, _ in cell.highlighted = false }
}

/// `$getNearestTableCellInTableFromDOMNode(tableNode, startingDOM)`: the
/// cell of `tableNode` that holds `cell`'s node.
public func getNearestTableCellInTableFromDOMNode(
  _ tableNode: TableNode, _ cell: TableDOMCell
)
  throws -> TableCellNode?
{
  try findParentTableCellNodeInTable(tableNode, getNodeByKey(cell.key))
}

/// `$getObserverCellFromCellNodeOrThrow(tableObserver, tableCellNode)`.
public func getObserverCellFromCellNodeOrThrow(
  _ tableObserver: TableObserver, _ tableCellNode: TableCellNode
) throws -> TableDOMCell {
  let tableNode = try tableObserver.lookup()
  let currentCords = try tableNode.getCordsFromCellNode(tableCellNode, tableObserver.table)
  return try tableNode.getDOMCellFromCordsOrThrow(
    currentCords.x, currentCords.y, tableObserver.table)
}

/// `$getElementForTableNode(editor, tableNode)`: the grid of a reconciled
/// table.
public func getElementForTableNode(
  _ editor: Editor, _ tableNode: TableNode
) throws
  -> TableDOMTable
{
  guard editor._keyToDOMMap.get(tableNode.getKey()) != nil else {
    throw LexicalError("$getElementForTableNode: Table Element Not Found")
  }
  return try getTable(tableNode)
}

// The `TableDOMTable` methods of LexicalTableNode.ts.
extension TableNode {
  /// `getCordsFromCellNode(tableCellNode, table)`.
  public func getCordsFromCellNode(
    _ tableCellNode: TableCellNode, _ table: TableDOMTable
  )
    throws -> (x: Int, y: Int)
  {
    for y in 0..<max(table.rows, 0) where y < table.domRows.count {
      guard let row = table.domRows[y] else { continue }
      for (x, cell) in row.enumerated() {
        guard let cell else { continue }
        if let cellNode = try getNearestTableCellInTableFromDOMNode(self, cell),
          tableCellNode.is(cellNode)
        {
          return (x, y)
        }
      }
    }
    throw LexicalError("Cell not found in table.")
  }

  /// `getDOMCellFromCords(x, y, table)`.
  public func getDOMCellFromCords(_ x: Int, _ y: Int, _ table: TableDOMTable) -> TableDOMCell? {
    guard y >= 0, y < table.domRows.count, let row = table.domRows[y] else {
      return nil
    }
    let index = x < row.count ? x : row.count - 1
    guard index >= 0 else { return nil }
    return row[index]
  }

  /// `getDOMCellFromCordsOrThrow(x, y, table)`.
  public func getDOMCellFromCordsOrThrow(
    _ x: Int, _ y: Int, _ table: TableDOMTable
  ) throws
    -> TableDOMCell
  {
    guard let cell = getDOMCellFromCords(x, y, table) else {
      throw LexicalError("Cell not found at cords.")
    }
    return cell
  }

  /// `getCellNodeFromCords(x, y, table)`.
  public func getCellNodeFromCords(
    _ x: Int, _ y: Int, _ table: TableDOMTable
  ) throws
    -> TableCellNode?
  {
    guard let cell = getDOMCellFromCords(x, y, table) else {
      return nil
    }
    return try getNodeByKey(cell.key) as? TableCellNode
  }

  /// `getCellNodeFromCordsOrThrow(x, y, table)`.
  public func getCellNodeFromCordsOrThrow(
    _ x: Int, _ y: Int, _ table: TableDOMTable
  ) throws
    -> TableCellNode
  {
    guard let node = try getCellNodeFromCords(x, y, table) else {
      throw LexicalError("Node at cords not TableCellNode.")
    }
    return node
  }
}
