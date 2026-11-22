import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalSelection
import LexicalUtils
import Synchronization

// Port of packages/lexical-table/src/LexicalTablePluginHelpers.ts.
//
// `registerTableSelectionObserver` attaches an observer to each table when
// the reconciler renders it, as JS does when the table has DOM; JS also
// listens to pointer events on the editor's window there
// (`registerTableWindowHandlers`), which belong to the views. A triple
// click (`CLICK_COMMAND` with `detail >= 3`) resolves its block from the
// event's `targetKey`, where JS reads `event.target`.

/// `$insertTable(payload, hasNestedTables)`.
private func insertTable(
  _ payload: InsertTableCommandPayload, _ hasNestedTables: Bool
) throws
  -> Bool
{
  guard let selection = try getSelection() ?? getPreviousSelection(),
    let selection = selection as? RangeSelection
  else {
    return false
  }

  // Prevent nested tables by checking if we're already inside a table
  if !hasNestedTables, try findTableNode(selection.anchor.getNode()) != nil {
    return false
  }

  let tableNode = try createTableNodeWithDimensions(
    jsNumber(payload.rows), jsNumber(payload.columns), payload.includeHeaders ?? true)
  try insertNodeToNearestRoot(tableNode)

  if let firstDescendant = try tableNode.getFirstDescendant() as? TextNode {
    try firstDescendant.select()
  }

  return true
}

/// `Number(value)` truncated to the count `$createTableNodeWithDimensions`
/// loops over (`NaN` loops zero times).
private func jsNumber(_ value: String) -> Int {
  let trimmed = String(
    value.drop(while: \.isWhitespace).reversed().drop(while: \.isWhitespace).reversed())
  guard let number = Double(trimmed), number.isFinite, number > 0 else { return 0 }
  return Int(number.rounded(.up))
}

/// One entry of `rowsLost`: by table, the rows that lost cells in the
/// update whose pending state `pending` is.
private struct RowsLostEntry: @unchecked Sendable {
  weak var pending: EditorState?
  var tables: [NodeKey: Set<NodeKey>]
}

/// `rowsLost`: per update (its pending editor state), by table, the rows
/// that lost cells in the update (see `tableRowTransform`), which the
/// table's transform pads when the update did not change the table's rows.
/// JS's `WeakMap<EditorState, ...>`: entries whose state is gone are
/// dropped.
private let rowsLost = Mutex<[ObjectIdentifier: RowsLostEntry]>([:])

/// The rows of `table` that lost cells in the running update, if any.
private func rowsLostIn(_ table: TableNode) throws -> Set<NodeKey>? {
  guard let pending = try getEditor()._pendingEditorState else {
    return nil
  }
  let key = table.getKey()
  return rowsLost.withLock { map in
    guard let entry = map[ObjectIdentifier(pending)], entry.pending === pending else {
      return nil
    }
    return entry.tables[key]
  }
}

/// Records that `row` of `table` lost cells in the update of `pending`.
private func recordRowLost(_ pending: EditorState, _ table: NodeKey, _ row: NodeKey) {
  rowsLost.withLock { map in
    map = map.filter { $0.value.pending != nil }
    let id = ObjectIdentifier(pending)
    var entry = map[id]
    if entry?.pending !== pending {
      entry = RowsLostEntry(pending: pending, tables: [:])
    }
    entry!.tables[table, default: []].insert(row)
    map[id] = entry
  }
}

private func tableCellTransform(_ node: TableCellNode) throws {
  guard let row = try node.getParent() as? TableRowNode else {
    // TableCellNode must be a child of TableRowNode.
    try node.remove()
    return
  }
  if try node.isEmpty() {
    // TableCellNode should never be empty
    try node.append(createParagraphNode())
  }
  // A cell changed (its span raised) after rows of its table lost cells in
  // this update: the table's width may have changed, so the table's
  // transform pads those rows again.
  if let table = try row.getParent() as? TableNode, try rowsLostIn(table) != nil {
    try table.markDirty()
  }
}

private func tableRowTransform(_ node: TableRowNode) throws {
  guard let table = try node.getParent() as? TableNode else {
    // TableRowNode must be a child of TableNode.
    // TODO: Future support of tbody/thead/tfoot may change this
    try node.remove()
    return
  }
  try unwrapAndFilterDescendants(node, { $0 is TableCellNode })
  // A row that lost cells (a delete across cells removes some) can be
  // shorter than the table. The table's transform pads it, and Lexical runs
  // that only on a table marked dirty itself. A row that gained cells is
  // left as its author made it. A row that changed after rows lost cells in
  // this update (it grew) marks the table again, so those rows are padded
  // to the width the table has now.
  let editor = try getEditor()
  let previous = editor._editorState._nodeMap.get(node.getKey())
  if let pending = editor._pendingEditorState, let previous = previous as? TableRowNode,
    try node.getChildrenSize() < previous.__size
  {
    recordRowLost(pending, table.getKey(), node.getKey())
    try table.markDirty()
  } else if try rowsLostIn(table) != nil {
    try table.markDirty()
  }
}

/// Whether the running update left the rows of `table` as they were (the
/// same rows in the same order, in a table that was there before it).
private func sameRows(_ table: TableNode) throws -> Bool {
  let before = try getEditor()._editorState._nodeMap
  guard let previous = before.get(table.getKey()) as? TableNode else {
    return false
  }
  var key = previous.__first
  for current in try table.getChildrenKeys() {
    if key != current {
      return false
    }
    key = key.flatMap { before.get($0) }?.__next
  }
  return key == nil
}

private func tableTransform(_ node: TableNode) throws {
  // TableRowNode is the only valid child for TableNode
  // TODO: Future support of tbody/thead/tfoot/caption may change this
  try unwrapAndFilterDescendants(node, { $0 is TableRowNode })

  // A new table, or one whose rows the update changed, gets all its rows
  // padded. One whose rows are the ones it had gets the rows that lost
  // cells padded, and no other: the other short rows (a table ragged by
  // collaborators' concurrent inserts) are their authors' to pad.
  let rows = try rowsLostIn(node)
  if try !sameRows(node) {
    try padTableRows(node)
  } else if let rows {
    try padTableRows(node) { rows.contains($0.getKey()) }
  }
  let colWidths = try node.getColWidths()
  let columnCount = try node.getColumnCount()
  if let colWidths, colWidths.count != columnCount {
    var newColWidths: [Double]? = nil
    if columnCount < colWidths.count {
      newColWidths = Array(colWidths.prefix(columnCount))
    } else if let fillWidth = colWidths.last {
      // Repeat the last column width.
      newColWidths = colWidths + Array(repeating: fillWidth, count: columnCount - colWidths.count)
    }
    try node.setColWidths(newColWidths)
  }
}

private func tableClickCommand(_ event: MouseEvent) throws -> Bool {
  guard event.detail >= 3, let targetKey = event.targetKey,
    let startNode = try getNodeByKey(targetKey)
  else {
    return false
  }
  guard
    let blockNode = try findMatchingParent(
      startNode, { node in try (node as? ElementNode).map { try !$0.isInline() } ?? false })
      as? ElementNode
  else {
    return false
  }
  guard try blockNode.getParent() is TableCellNode else {
    return false
  }
  // Select the block with the text points a native selection of it
  // resolves to.
  _ = try normalizeSelection(blockNode.select(0))
  return true
}

private func tableSelectAllCommand() throws -> Bool {
  guard let selection = try getSelection() as? RangeSelection else {
    return false
  }

  // Check if the selection is inside a table
  let anchorNode = try selection.anchor.getNode()
  guard let tableNode = try findTableNode(anchorNode) else {
    return false
  }

  // Only intercept if the table is the only child of root (#8074)
  let root = try getRoot()
  if try !root.is(tableNode.getParent()) || root.getChildrenSize() != 1 {
    return false
  }

  // Get the table map to find first and last cells (handles merged cells
  // correctly)
  let (tableMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)
  guard let firstRow = tableMap.first, !firstRow.isEmpty, let firstCellMap = firstRow[0],
    let lastRow = tableMap.last, let lastCellMap = lastRow.last ?? nil
  else {
    return false
  }

  // Create a TableSelection that selects all cells
  try setSelection(createTableSelectionFrom(tableNode, firstCellMap.cell, lastCellMap.cell))
  return true
}

/// `registerTableCellUnmergeTransform(editor)`: a transform that makes every
/// `TableCellNode` 1 × 1, for editors that do not support merged cells.
@discardableResult
public func registerTableCellUnmergeTransform(_ editor: Editor) throws -> () -> Void {
  try editor.registerNodeTransform(TableCellNode.self) { node in
    if try node.getColSpan() > 1 || node.getRowSpan() > 1 {
      // When we have rowSpan we have to map the entire Table to understand
      // where the new Cells fit best; let's analyze all Cells at once
      let (_, _, gridNode) = try getNodeTriplet(node)
      let (gridMap, _, _) = try computeTableMap(gridNode, node, node)
      // TODO this function expects Tables to be normalized. Look into this
      // once it exists
      let rowsCount = gridMap.count
      let columnsCount = gridMap[0].count
      guard var row = try gridNode.getFirstChild() as? TableRowNode else {
        throw LexicalError("Expected TableNode first child to be a RowNode")
      }
      var unmerged: [TableCellNode] = []
      for i in 0..<rowsCount {
        if i != 0 {
          guard let next = try row.getNextSibling() as? TableRowNode else {
            throw LexicalError("Expected TableNode first child to be a RowNode")
          }
          row = next
        }
        var lastRowCell: TableCellNode? = nil
        for j in 0..<columnsCount {
          let cellMap = try mapValue(gridMap, i, j)
          let cell = cellMap.cell
          if cellMap.startRow == i && cellMap.startColumn == j {
            lastRowCell = cell
            unmerged.append(cell)
          } else if try cell.getColSpan() > 1 || cell.getRowSpan() > 1 {
            let newCell = try createTableCellNode(cell.__headerState)
            if let lastRowCell {
              try lastRowCell.insertAfter(newCell)
            } else {
              try insertFirst(row, newCell)
            }
          }
        }
      }
      for cell in unmerged {
        try cell.setColSpan(1)
        try cell.setRowSpan(1)
      }
    }
  }
}

/// `registerTableSelectionObserver(editor, hasTabHandler)`: an observer
/// with its command handlers for each rendered table, and the selection
/// normalization of `SELECTION_CHANGE_COMMAND`. `layout` answers the rect
/// queries of the arrow keys (see `TableLayout`).
public func registerTableSelectionObserver(
  _ editor: Editor, _ hasTabHandler: Bool = true, layout: (any TableLayout)? = nil
) throws -> () -> Void {
  let tableObservers = TableObservers()
  var selectionNeedsSync = false
  // Without a layout of its own, the root element answers, as JS measures
  // whatever DOM the editor renders into.
  let layout = layout ?? RootElementTableLayout(editor)

  let initializeTableNode: (TableNode, NodeKey) throws -> Void = { tableNode, nodeKey in
    let tableSelection = try applyTableHandlers(
      tableNode, editor, hasTabHandler, tableObservers, layout)
    _ = tableObservers.observers.set(nodeKey, tableSelection)
  }

  return mergeRegister(
    // Register once so attaching/replacing a root cannot reorder HIGH
    // listeners.
    editor.registerCommand(
      SELECTION_CHANGE_COMMAND,
      { _, _ in
        if editor.getRootElement() == nil {
          return false
        }
        selectionNeedsSync = true
        return try handleTableSelectionChangeCommand(tableObservers, editor)
      }, COMMAND_PRIORITY_HIGH),
    editor.registerUpdateListener { payload in
      if !selectionNeedsSync {
        // Cell/row changes dirty their table ancestor without necessarily
        // mutating the TableNode itself or changing the selection.
        for key in payload.dirtyElements.keysArray where tableObservers.observers.has(key) {
          selectionNeedsSync = true
          break
        }
      }
      if selectionNeedsSync {
        selectionNeedsSync = false
        if editor.getRootElement() != nil {
          try editor.read(.latest) { () throws -> Void in
            try syncTableSelectionObservers(tableObservers, editor)
          }
        }
      }
    },
    try editor.registerMutationListener(
      TableNode.self,
      { nodeMutations, _ in
        selectionNeedsSync = true
        try editor.read(.latest) { () throws -> Void in
          for (nodeKey, mutation) in nodeMutations {
            let tableSelection = tableObservers.observers.get(nodeKey)
            if mutation == "created" || mutation == "updated" {
              if editor._keyToDOMMap.get(nodeKey) == nil {
                // The table is not rendered, as when the editor has no root
                // element yet. The reconciler reports it as created when it
                // renders it, and the observer attaches then.
                tableObservers.removeObserver(nodeKey)
                continue
              }
              guard let tableNode = try getNodeByKey(nodeKey) as? TableNode else {
                throw LexicalError(
                  "TableObserver: Expected tableNodeKey \(nodeKey) to be a TableNode")
              }
              if let tableSelection {
                // JS makes a new observer when the update created a new
                // table element; here the grid is refreshed, as its
                // MutationObserver would.
                try tableSelection.trackTable()
              } else {
                try initializeTableNode(tableNode, nodeKey)
              }
            } else if mutation == "destroyed" {
              tableObservers.removeObserver(nodeKey)
            }
          }
        }
      }, skipInitialization: false),
    {
      // Hook might be called multiple times so cleaning up tables listeners
      // as well, as it'll be reinitialized during recurring call
      tableObservers.removeAllObservers()
    })
}

/// `registerTablePlugin(editor, options)`: the table commands and the
/// table integrity transforms. Register the table selection observer
/// separately after this with `registerTableSelectionObserver`.
public func registerTablePlugin(
  _ editor: Editor, hasNestedTables: Signal<Bool>? = nil
) throws
  -> () -> Void
{
  if !editor.hasNodes([TableNode.self]) {
    throw LexicalError("TablePlugin: TableNode is not registered on editor")
  }

  let hasNestedTables = hasNestedTables ?? signal(false)

  return mergeRegister(
    editor.registerCommand(
      INSERT_TABLE_COMMAND,
      { payload, _ in try insertTable(payload, hasNestedTables.peek()) },
      COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      SELECTION_INSERT_CLIPBOARD_NODES_COMMAND,
      { payload, dispatchEditor in
        if editor !== dispatchEditor {
          return false
        }
        return try tableSelectionInsertClipboardNodesCommand(payload, hasNestedTables)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      SELECT_ALL_COMMAND, { _, _ in try tableSelectAllCommand() }, COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      CLICK_COMMAND, { event, _ in try tableClickCommand(event) }, COMMAND_PRIORITY_EDITOR),
    try editor.registerNodeTransform(TableNode.self, tableTransform),
    try editor.registerNodeTransform(TableRowNode.self, tableRowTransform),
    try editor.registerNodeTransform(TableCellNode.self, tableCellTransform))
}

private func tableSelectionInsertClipboardNodesCommand(
  _ selectionPayload: InsertClipboardNodesPayload, _ hasNestedTables: Signal<Bool>
) throws -> Bool {
  let nodes = selectionPayload.nodes
  let selection = selectionPayload.selection

  let hasTables = try nodes.contains { n in
    try n is TableNode || dfs(n).contains { $0.node is TableNode }
  }
  if !hasTables {
    if let selection = selection as? TableSelection {
      var text = ""
      var lastWasBlock = false
      for node in nodes {
        let isBlock = try (node as? ElementNode).map { try !$0.isInline() } ?? false
        if !text.isEmpty && (isBlock || lastWasBlock) {
          text += "\n"
        }
        text += try node.getTextContent()
        lastWasBlock = isBlock
      }
      try selection.insertRawText(text)
      return true
    }
    return false
  }

  let isTableSelection = selection is TableSelection
  let rangeSelection = selection as? RangeSelection
  var isSelectionInsideOfGrid = isTableSelection
  if let rangeSelection {
    isSelectionInsideOfGrid =
      try findMatchingParent(rangeSelection.anchor.getNode(), { $0 is TableCellNode }) != nil
      && findMatchingParent(rangeSelection.focus.getNode(), { $0 is TableCellNode }) != nil
  }

  if !isSelectionInsideOfGrid {
    // Not pasting in a grid - no special handling required.
    return false
  }

  // When pasting just a table, flatten the table on the destination table,
  // even when nested tables are allowed.
  if nodes.count == 1, let table = nodes[0] as? TableNode {
    return try insertTableIntoGrid(table, selection)
  }

  // If nested tables are enabled, allow pasting a table into a single cell.
  if rangeSelection != nil, hasNestedTables.peek(), try !isMultiCellTableSelection(selection) {
    return false
  }

  // If we reached this point, there's a table in the selection and nested
  // tables are not allowed - reject the paste.
  return true
}

private func isMultiCellTableSelection(_ selection: any BaseSelection) throws -> Bool {
  if let selection = selection as? TableSelection,
    try !selection.focus.getNode().is(selection.anchor.getNode())
  {
    return true
  }
  if let selection = selection as? RangeSelection,
    try selection.anchor.getNode() is TableCellNode,
    try !selection.anchor.getNode().is(selection.focus.getNode())
  {
    return true
  }
  return false
}

/// The `TableLayout` of the editor's root element, when it measures its
/// layout (`SelectionLayout`, which the native text views adopt); without
/// one, every query is `nil`, which leaves up and down inside a cell to the
/// view.
final class RootElementTableLayout: TableLayout {
  private weak var editor: Editor?

  init(_ editor: Editor) { self.editor = editor }

  private var rootLayout: (any SelectionLayout)? {
    editor?.getRootElement() as? any SelectionLayout
  }

  func selectionRect(_ selection: RangeSelection) -> SelectionRect? {
    rootLayout?.selectionRect(selection)
  }

  func nodeRect(_ key: NodeKey) -> SelectionRect? { rootLayout?.nodeRect(key) }
}
