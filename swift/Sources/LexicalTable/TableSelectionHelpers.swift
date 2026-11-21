import Lexical
import LexicalClipboard
import LexicalSelection

// Port of packages/lexical-table/src/LexicalTableSelectionHelpers.ts: the
// model half (DESIGN.md, LexicalTable).
//
// Ported: the command handlers `applyTableHandlers` registers (the arrow
// keys, Escape, deletion, cut, `FORMAT_TEXT`, `FORMAT_ELEMENT`,
// `CONTROLLED_TEXT_INSERTION`, Tab, focus and `INSERT_PARAGRAPH`),
// `$handleTableSelectionChangeCommand` and the selection normalization it
// runs, `$syncTableSelectionObservers`, `$selectAdjacentCell` and
// `$handleArrowKey` with its helpers.
//
// Left to the views (see DESIGN.md, The table view):
// - the pointer handlers (`registerTableWindowHandlers`,
//   `$handleTableClick`, auto-scroll), the triple-click `mousedown`
//   listener, and the document-level `paste` and `copy` listeners, which
//   listen to DOM events;
// - the observer's attachment to its table element
//   (`attachTableObserverToTableElement`, `getTableObserverFromTableElement`,
//   `getDOMCellFromTarget`, `getDOMCellInTableFromTarget`,
//   `doesTargetContainText`);
// - `$fixTableSelectionForSelectedTable`, which compares the DOM selection
//   with the model's: the views turn a drag that leaves the table into a
//   range selection.
// - `$getTableEdgeCursorPosition` reads where the DOM selection's anchor is
//   (beside the table element, in its scrollable wrapper); a native view
//   never puts the caret there, so it finds no edge.
//
// `$handleArrowKey` reads the rects of the caret and of a cell's first or
// last block with `getBoundingClientRect`; here a `TableLayout` the editor
// is given (`TableConfig.layout`) answers. Without one, up and down inside
// a cell are left to the view, as JS leaves them to the browser when the
// cell has no DOM.

/// The layout queries `$handleArrowKey` makes, which JS answers with
/// `getBoundingClientRect` and the root element's `aria-controls`. The
/// views supply one (DESIGN.md, The table view); rects are in any one
/// coordinate space with y growing downward.
public protocol TableLayout: AnyObject {
  /// The rect of `selection`'s caret (JS: the DOM selection range's, or the
  /// element's for an element point).
  func selectionRect(_ selection: RangeSelection) -> SelectionRect?
  /// The rect of the rendered block `key` (JS: its element's).
  func nodeRect(_ key: NodeKey) -> SelectionRect?
  /// Whether a typeahead menu is open (JS: the root's `aria-controls` is
  /// `typeahead-menu`).
  var isTypeaheadMenuInView: Bool { get }
}

extension TableLayout {
  public var isTypeaheadMenuInView: Bool { false }
}

/// `isHTMLTableElement(el)`.
package func isHTMLTableElement(_ el: DOMNode?) -> Bool {
  (el as? DOMElement)?.nodeName == "TABLE"
}

/// `getTableElement(tableNode, dom)`: `dom` when it is the `<table>`,
/// otherwise the first `<table>` inside it (the scrollable wrapper's).
public func getTableElement(_ tableNode: TableNode, _ dom: DOMElement) throws -> DOMElement {
  let element = isHTMLTableElement(dom) ? dom : dom.getElementsByTagName("table").first
  guard let element, isHTMLTableElement(element) else {
    throw LexicalError(
      "getTableElement: Expecting table in DOM node for \(type(of: tableNode)) of type \(tableNode.getType()) with key \(tableNode.getKey()), not \(dom.nodeName)"
    )
  }
  return element
}

private func getTableNodeByKeyOrThrow(_ key: NodeKey) throws -> TableNode {
  guard let tableNode = try getNodeByKeyOrThrow(key) as? TableNode else {
    throw LexicalError("Expected TableNode for key \(key)")
  }
  return tableNode
}

/// `$findParentTableCellNodeInTable(tableNode, node)`: the outermost cell
/// of `tableNode` that holds `node`.
public func findParentTableCellNodeInTable(
  _ tableNode: LexicalNode, _ node: LexicalNode?
)
  throws -> TableCellNode?
{
  var currentNode = node
  var lastTableCellNode: TableCellNode? = nil
  while let current = currentNode {
    if tableNode.is(current) {
      return lastTableCellNode
    } else if let cell = current as? TableCellNode {
      lastTableCellNode = cell
    }
    currentNode = try current.getParent()
  }
  return nil
}

/// `Direction`.
private enum Direction { case backward, forward, up, down }

private let ARROW_KEY_COMMANDS_WITH_DIRECTION: [(LexicalCommand<KeyboardEvent>, Direction)] = [
  (KEY_ARROW_DOWN_COMMAND, .down),
  (KEY_ARROW_UP_COMMAND, .up),
  (KEY_ARROW_LEFT_COMMAND, .backward),
  (KEY_ARROW_RIGHT_COMMAND, .forward),
]
private let DELETE_TEXT_COMMANDS = [
  DELETE_WORD_COMMAND, DELETE_LINE_COMMAND, DELETE_CHARACTER_COMMAND,
]
private let DELETE_KEY_COMMANDS = [KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND]

/// `applyTableHandlers(tableNode, element, editor, hasTabHandler,
/// tableObservers)`: the observer of `tableNode` and its command handlers.
/// JS also takes the table's element, which the observer listens to.
public func applyTableHandlers(
  _ tableNode: TableNode, _ editor: Editor, _ hasTabHandler: Bool,
  _ tableObservers: TableObservers, _ layout: (any TableLayout)? = nil
) throws -> TableObserver {
  guard editor.getRootElement() != nil else {
    throw LexicalError("applyTableHandlers: editor has no root element set")
  }

  let tableObserver = try TableObserver(editor, tableNode.getKey())

  for (command, direction) in ARROW_KEY_COMMANDS_WITH_DIRECTION {
    tableObserver.listenersToRemove.append(
      editor.registerCommand(
        command,
        { event, _ in
          try handleArrowKey(
            editor, event, direction, tableNode, tableObserver, tableObservers, layout)
        }, COMMAND_PRIORITY_HIGH))
  }

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      KEY_ESCAPE_COMMAND,
      { event, _ in
        if let selection = try getSelection() as? TableSelection,
          let focusCellNode = try findParentTableCellNodeInTable(
            tableNode, selection.focus.getNode())
        {
          stopEvent(event)
          try focusCellNode.selectEnd()
          return true
        }
        return false
      }, COMMAND_PRIORITY_HIGH))

  let deleteTextHandler: (Bool, Editor) throws -> Bool = { _, _ in
    let selection = try getSelection()

    if try !isSelectionInTable(selection, tableNode) {
      return false
    }

    if selection is TableSelection {
      try tableObserver.clearText()
      return true
    }

    return false
  }

  for command in DELETE_TEXT_COMMANDS {
    tableObserver.listenersToRemove.append(
      editor.registerCommand(command, deleteTextHandler, COMMAND_PRIORITY_HIGH))
  }

  let deleteCellHandler: (DOMEvent?) throws -> Bool = { event in
    let selection = try getSelection()
    guard let selection, selection is TableSelection || selection is RangeSelection,
      let (anchor, focus) = selection.getStartEndPoints()
    else {
      return false
    }

    // If the selection is inside the table but should remove the whole
    // table we expand the selection so that both the anchor and focus are
    // outside the table and the editor's command listener will handle the
    // delete
    let isAnchorInside = try tableNode.isParentOf(anchor.getNode())
    let isFocusInside = try tableNode.isParentOf(focus.getNode())
    if isAnchorInside != isFocusInside {
      let tablePoint = isAnchorInside ? anchor : focus
      let outerPoint = isAnchorInside ? focus : anchor
      // Preserve the outer point
      let (key, offset, type) = (outerPoint.key, outerPoint.offset, outerPoint.type)
      // Expand the selection around the table
      let newSelection =
        try tablePoint.isBefore(outerPoint)
        ? tableNode.selectPrevious() : tableNode.selectNext()
      // Restore the outer point of the selection
      try (isAnchorInside ? newSelection.focus : newSelection.anchor).set(key, offset, type)
      // Let the base implementation handle the rest
      return false
    }

    if try !isSelectionInTable(selection, tableNode) {
      return false
    }

    if selection is TableSelection {
      if let event {
        event.preventDefault()
        event.stopPropagation()
      }
      try tableObserver.clearText()
      return true
    }

    return false
  }

  for command in DELETE_KEY_COMMANDS {
    tableObserver.listenersToRemove.append(
      editor.registerCommand(
        command, { event, _ in try deleteCellHandler(event) }, COMMAND_PRIORITY_HIGH))
  }

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      CUT_COMMAND,
      { event, _ in
        guard let selection = try getSelection() else {
          return false
        }
        guard selection is TableSelection || selection is RangeSelection else {
          return false
        }
        // Copying to the clipboard is async in JS, so the data is captured
        // before it is deleted
        var clipboardEvent: ClipboardEvent? = nil
        if case .clipboard(let e) = event {
          clipboardEvent = e
        }
        try copyToClipboard(editor, clipboardEvent, getClipboardDataFromSelection(selection))
        let intercepted = try deleteCellHandler(event?.event)
        if let selection = selection as? RangeSelection {
          try selection.removeText()
          return true
        }
        return intercepted
      }, COMMAND_PRIORITY_HIGH))

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      FORMAT_TEXT_COMMAND,
      { payload, _ in
        let selection = try getSelection()

        if try !isSelectionInTable(selection, tableNode) {
          return false
        }

        if selection is TableSelection {
          try tableObserver.formatCells(payload)
          return true
        }

        return false
      }, COMMAND_PRIORITY_HIGH))

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      FORMAT_ELEMENT_COMMAND,
      { formatType, _ in
        guard let selection = try getSelection() as? TableSelection,
          try isSelectionInTable(selection, tableNode)
        else {
          return false
        }

        guard let anchorNode = try selection.anchor.getNode() as? TableCellNode,
          let focusNode = try selection.focus.getNode() as? TableCellNode
        else {
          return false
        }

        let (tableMap, anchorCell, focusCell) = try computeTableMap(
          tableNode, anchorNode, focusNode)
        // The same rect TableSelection.getNodes() walks: a merged cell that
        // straddles the boundary pulls the rect outwards.
        let rect = try computeTableCellRectBoundary(tableMap, anchorCell, focusCell)

        if rect.minRow == 0 && rect.minColumn == 0 && rect.maxRow == tableMap.count - 1
          && rect.maxColumn == tableMap[0].count - 1
        {
          try tableNode.setFormat(formatType)
          return true
        }

        var visited = Set<ObjectIdentifier>()
        for i in rect.minRow...rect.maxRow {
          for j in rect.minColumn...rect.maxColumn {
            let cell = try mapValue(tableMap, i, j).cell
            if !visited.insert(ObjectIdentifier(cell)).inserted {
              continue
            }
            try cell.setFormat(formatType)

            for child in try cell.getChildren() {
              if let child = child as? ElementNode, try !child.isInline() {
                try child.setFormat(formatType)
              }
            }
          }
        }
        return true
      }, COMMAND_PRIORITY_HIGH))

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { payload, _ in
        let selection = try getSelection()

        if try !isSelectionInTable(selection, tableNode) {
          return false
        }

        if selection is TableSelection {
          try tableObserver.clearHighlight()
          return false
        } else if let selection = selection as? RangeSelection {
          guard
            try findMatchingParent(selection.anchor.getNode(), { $0 is TableCellNode })
              is TableCellNode
          else {
            return false
          }

          if let edgePosition = try getTableEdgeCursorPosition(editor, selection, tableNode) {
            try insertParagraphAtTableEdge(edgePosition, tableNode, [createTextNode(payload)])
            return true
          }
        }

        return false
      }, COMMAND_PRIORITY_HIGH))

  if hasTabHandler {
    tableObserver.listenersToRemove.append(
      editor.registerCommand(
        KEY_TAB_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
            try isSelectionInTable(selection, tableNode)
          else {
            return false
          }

          guard let tableCellNode = try findCellNode(selection.anchor.getNode()),
            try tableNode.is(findTableNode(tableCellNode))
          else {
            return false
          }

          stopEvent(event)
          try selectAdjacentCell(tableCellNode, event.shiftKey ? .previous : .next)

          return true
        }, COMMAND_PRIORITY_HIGH))
  }

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      FOCUS_COMMAND, { _, _ in try tableNode.isSelected() }, COMMAND_PRIORITY_HIGH))

  tableObserver.listenersToRemove.append(
    editor.registerCommand(
      INSERT_PARAGRAPH_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
          try isSelectionInTable(selection, tableNode)
        else {
          return false
        }
        if let edgePosition = try getTableEdgeCursorPosition(editor, selection, tableNode) {
          try insertParagraphAtTableEdge(edgePosition, tableNode)
          return true
        }
        return false
      }, COMMAND_PRIORITY_HIGH))

  return tableObserver
}

/// `$handleTableSelectionChangeCommand(tableObservers, editor)`
/// (`@internal`).
package func handleTableSelectionChangeCommand(
  _ tableObservers: TableObservers, _ editor: Editor
) throws -> Bool {
  let selection = try getSelection()
  let prevSelection = try getPreviousSelection()

  if let nextFocus = tableObservers.getAndClearNextFocus(),
    try getNodeByKey(nextFocus.tableKey) is TableNode
  {
    let tableKey = nextFocus.tableKey
    let focusCell = nextFocus.focusCell
    guard let tableObserver = tableObservers.observers.get(tableKey) else {
      throw LexicalError("tableObserver not found for tableKey: \(tableKey)")
    }
    if let selection = selection as? TableSelection,
      selection.tableKey == tableObserver.tableNodeKey
    {
      if focusCell.x == tableObserver.focusX && focusCell.y == tableObserver.focusY {
        // The selection is already the correct table selection
        return false
      } else {
        try tableObserver.setFocusCellForSelection(focusCell)
        return true
      }
    } else if let anchorCell = tableObserver.anchorCell, tableObserver.anchorCellNodeKey != nil,
      focusCell.key != anchorCell.key, tableObserver.tableSelection != nil
    {
      // The selection has crossed cells
      try tableObserver.setFocusCellForSelection(focusCell, true)
      return true
    }
  }
  let shouldCheckSelectionForTable = tableObservers.getAndClearShouldCheckSelectionForTable()
  // If they pressed the down arrow with the selection outside of the
  // table, and then the selection ends up in the table but not in the
  // first cell, then move the selection to the first cell.
  if let shouldCheckSelectionForTable, prevSelection is RangeSelection,
    let selection = selection as? RangeSelection, selection.isCollapsed()
  {
    // The table may have been removed before this selection change
    // was dispatched, in which case there is nothing to check
    if let tableNode = try getNodeByKey(shouldCheckSelectionForTable) as? TableNode {
      let anchor = try selection.anchor.getNode()
      let firstRow = try tableNode.getFirstChild()
      if let anchorCell = try findCellNode(anchor), let firstRow = firstRow as? TableRowNode,
        let firstCell = try firstRow.getFirstChild() as? TableCellNode,
        try tableNode.is(
          findMatchingParent(anchorCell, { $0.is(tableNode) || $0.is(firstCell) }))
      {
        // The selection moved to the table, but not in the first cell
        try firstCell.selectStart()
        return true
      }
    }
  }

  if let selection = selection as? TableSelection {
    try fixTableSelectionForSelectedTable(editor, selection)
  }

  if let selection = selection as? RangeSelection {
    try fixRangeSelectionForSelectedTable(selection, tableObservers)
  }

  return false
}

/// `$syncTableSelectionObservers(tableObservers, editor)` (`@internal`):
/// synchronizes the observers' selection state once the tables are
/// reconciled.
package func syncTableSelectionObservers(
  _ tableObservers: TableObservers, _ editor: Editor
)
  throws
{
  for (tableNode, tableObserver) in try tableObservers.getTableNodesAndObservers() {
    try syncTableSelectionState(editor, tableNode, tableObserver)
  }
}

/// `$fixRangeSelectionForSelectedTable(selection, tableObservers)`: handles
/// range selections that cross into, out of, or within tables.
private func fixRangeSelectionForSelectedTable(
  _ selection: RangeSelection, _ tableObservers: TableObservers
) throws {
  let prevSelection = try getPreviousSelection()
  let anchor = selection.anchor
  let focus = selection.focus
  let anchorNode = try anchor.getNode()
  let focusNode = try focus.getNode()
  // Using explicit comparison with table node to ensure it's not a nested
  // table as in that case we'll leave selection resolving to that table
  let anchorCellNode = try findCellNode(anchorNode)
  let focusCellNode = try findCellNode(focusNode)
  let anchorCellTable = try anchorCellNode.flatMap(findTableNode)
  let focusCellTable = try focusCellNode.flatMap(findTableNode)

  // iOS can double-tap an empty cell by issuing only selectionchange after
  // the first tap's pointerup. Only repair a native range from the caret in
  // the last touch-tapped empty cell.
  if tableObservers.touchTapCellKey != nil, !selection.dirty, !selection.isCollapsed(),
    let prevSelection = prevSelection as? RangeSelection, prevSelection.isCollapsed(),
    !(anchorCellNode?.is(focusCellNode) ?? false)
  {
    let paragraph = try getNodeByKey(prevSelection.anchor.key)
    let cell = try paragraph?.getParent()
    if let paragraph = paragraph as? ElementNode, try paragraph.isEmpty(),
      let cell = cell as? TableCellNode, cell.getKey() == tableObservers.touchTapCellKey,
      try cell.getChildren().allSatisfy({ try ($0 as? ElementNode)?.isEmpty() ?? false }),
      anchor.is(prevSelection.anchor) || focus.is(prevSelection.anchor)
    {
      try setSelection(prevSelection.clone())
      return
    }
  }
  let isBackward = try selection.isBackward()

  // The focus should be moved (to cover the whole focus table) if it is
  // moved outside of the anchor's table.
  let shouldMoveFocus =
    try focusCellTable != nil
    && (anchorCellTable == nil || anchorCellTable!.isParentOf(focusCellTable!))
  // The anchor should be moved (to cover the whole anchor table) if the
  // focus is moved outside of the anchor table.
  let shouldMoveAnchor =
    try anchorCellTable != nil
    && (focusCellTable == nil || focusCellTable!.isParentOf(anchorCellTable!))

  if shouldMoveFocus, let focusCellTable, let focusCellNode {
    // Select the whole focus table.
    let newSelection = selection.cloneRange()
    let (tableMap, _, _) = try computeTableMap(focusCellTable, focusCellNode, focusCellNode)
    let firstCell = try mapValue(tableMap, 0, 0).cell
    let lastCell = try mapValue(
      tableMap, tableMap.count - 1, tableMap[tableMap.count - 1].count - 1
    )
    .cell
    try newSelection.focus.set(
      isBackward ? firstCell.getKey() : lastCell.getKey(),
      isBackward ? 0 : lastCell.getChildrenSize(), .element)
    try setSelection(newSelection)
  } else if shouldMoveAnchor, let anchorCellTable, let anchorCellNode {
    // Select the whole anchor table.
    let newSelection = selection.cloneRange()
    let (tableMap, _, _) = try computeTableMap(anchorCellTable, anchorCellNode, anchorCellNode)
    let firstCell = try mapValue(tableMap, 0, 0).cell
    let lastCell = try mapValue(
      tableMap, tableMap.count - 1, tableMap[tableMap.count - 1].count - 1
    )
    .cell
    try newSelection.anchor.set(
      isBackward ? lastCell.getKey() : firstCell.getKey(),
      isBackward ? lastCell.getChildrenSize() : 0, .element)
    try setSelection(newSelection)
  } else if let anchorCellNode, let focusCellNode, let anchorCellTable, let focusCellTable,
    anchorCellTable.is(focusCellTable)
  {
    // A range selection across cells of one table becomes a table
    // selection (a drag up from the first cell, or a shift-click).
    if !anchorCellNode.is(focusCellNode) {
      // Selection normalization is model work: the table or cells may not
      // have DOM yet. Its observer will synchronize after reconciliation.
      try setSelection(createTableSelectionFrom(anchorCellTable, anchorCellNode, focusCellNode))
      return
    }
    guard let tableObserver = tableObservers.observers.get(anchorCellTable.getKey()) else {
      return
    }

    // A touch drag across cells (not a tap) turns the collapsed selections
    // into a table selection (#8538).
    if tableObserver.pointerType == "touch", tableObserver.isSelecting,
      tableObserver.isPointerDrag, selection.isCollapsed(),
      let prevSelection = prevSelection as? RangeSelection, prevSelection.isCollapsed()
    {
      let prevAnchorNode = try getNodeByKey(prevSelection.anchor.key)
      let prevAnchorCellNode = try prevAnchorNode.flatMap(findCellNode)
      let pointerStartNode = try tableObserver.pointerStartCell.flatMap {
        try getNodeByKey($0.key)
      }
      let hasLeftStartCell = pointerStartNode != nil && !pointerStartNode!.is(focusCellNode)
      if hasLeftStartCell, let prevAnchorCellNode,
        try anchorCellTable.is(findTableNode(prevAnchorCellNode)),
        !prevAnchorCellNode.is(focusCellNode)
      {
        try setSelection(
          createTableSelectionFrom(anchorCellTable, prevAnchorCellNode, focusCellNode))
        tableObserver.pointerType = nil
      }
    }
  }
}

/// `$fixTableSelectionForSelectedTable(editor, selection)`: JS turns a
/// `TableSelection` whose DOM selection has left the table into a
/// `RangeSelection`; the views do that from their own selection, so this
/// only checks what JS checks before reading the DOM.
private func fixTableSelectionForSelectedTable(
  _ editor: Editor, _ selection: TableSelection
)
  throws
{
  let prevSelection = try getPreviousSelection()
  if !selection.is(prevSelection) {
    return
  }
  _ = try getTableNodeByKeyOrThrow(selection.tableKey)
}

/// `$syncTableSelectionState(editor, tableNode, tableObserver)`: keeps the
/// observer in sync with the selection.
private func syncTableSelectionState(
  _ editor: Editor, _ tableNode: TableNode, _ tableObserver: TableObserver
) throws {
  let selection = try getSelection()
  let isSelected = try tableNode.isSelected()
  let tableSelection = (selection as? TableSelection).flatMap {
    $0.tableKey == tableNode.getKey() ? $0 : nil
  }
  if tableSelection == nil && !tableObserver.isHighlightingCells
    && !tableObserver.hasHijackedSelectionStyles && !isSelected
  {
    return
  }
  // Refresh the grid now so newly inserted/replaced cells can be selected.
  tableObserver.table = try getTable(tableNode)
  if let tableSelection {
    if let anchor = try getNodeByKey(tableSelection.anchor.key) as? TableCellNode,
      let focus = try getNodeByKey(tableSelection.focus.key) as? TableCellNode
    {
      let anchorCell = try getObserverCellFromCellNodeOrThrow(tableObserver, anchor)
      let focusCell = try getObserverCellFromCellNodeOrThrow(tableObserver, focus)
      tableObserver.anchorCell = anchorCell
      tableObserver.anchorCellNodeKey = anchor.getKey()
      tableObserver.anchorX = anchorCell.x
      tableObserver.anchorY = anchorCell.y
      tableObserver.focusCell = focusCell
      tableObserver.focusCellNodeKey = focus.getKey()
      tableObserver.focusX = focusCell.x
      tableObserver.focusY = focusCell.y
    }
    try tableObserver.updateTableTableSelection(tableSelection)
  } else if tableObserver.isHighlightingCells {
    // Only clear an active table selection's highlighting, preserving the
    // new model selection.
    try tableObserver.clearHighlight(false)
  }

  if tableObserver.hasHijackedSelectionStyles && !isSelected {
    try removeHighlightStyleToTable(editor, tableObserver)
  } else if !tableObserver.hasHijackedSelectionStyles && isSelected {
    try addHighlightStyleToTable(editor, tableObserver)
  }
}

/// `$selectAdjacentCell(tableCellNode, direction)`.
@discardableResult
private func selectAdjacentCell(
  _ tableCellNode: TableCellNode, _ direction: CaretDirection
)
  throws -> RangeSelection
{
  let isNext = direction == .next
  func sibling(_ node: LexicalNode) throws -> LexicalNode? {
    try isNext ? node.getNextSibling() : node.getPreviousSibling()
  }
  if let sibling = try sibling(tableCellNode) as? ElementNode {
    return try sibling.selectEnd()
  }
  guard let parentRow = try findMatchingParent(tableCellNode, { $0 is TableRowNode }) else {
    throw LexicalError("selectAdjacentCell: Cell not in table row")
  }
  var nextRow = try sibling(parentRow)
  while let row = nextRow as? TableRowNode {
    if let child = try (isNext ? row.getFirstChild() : row.getLastChild()) as? ElementNode {
      return try child.selectEnd()
    }
    nextRow = try sibling(row)
  }
  guard let parentTable = try findMatchingParent(parentRow, { $0 is TableNode }) else {
    throw LexicalError("selectAdjacentCell: Row not in table")
  }
  return try isNext ? parentTable.selectNext() : parentTable.selectPrevious()
}

private func selectTableNodeInDirection(
  _ tableObserver: TableObserver, _ tableNode: TableNode, _ x: Int, _ y: Int,
  _ direction: Direction
) throws -> Bool {
  let isForward = direction == .forward
  let table = tableObserver.table

  switch direction {
  case .backward, .forward:
    if x != (isForward ? table.columns - 1 : 0) {
      try selectTableCellNode(
        tableNode.getCellNodeFromCordsOrThrow(x + (isForward ? 1 : -1), y, table), isForward)
    } else {
      if y != (isForward ? table.rows - 1 : 0) {
        try selectTableCellNode(
          tableNode.getCellNodeFromCordsOrThrow(
            isForward ? 0 : table.columns - 1, y + (isForward ? 1 : -1), table), isForward)
      } else if !isForward {
        try tableNode.selectPrevious()
      } else {
        try tableNode.selectNext()
      }
    }
    return true

  case .up:
    if y != 0 {
      try selectTableCellNode(tableNode.getCellNodeFromCordsOrThrow(x, y - 1, table), false)
    } else {
      try tableNode.selectPrevious()
    }
    return true

  case .down:
    if y != table.rows - 1 {
      try selectTableCellNode(tableNode.getCellNodeFromCordsOrThrow(x, y + 1, table), true)
    } else {
      try tableNode.selectNext()
    }
    return true
  }
}

private enum ColumnCorner { case minColumn, maxColumn }
private enum RowCorner { case minRow, maxRow }
private typealias Corner = (ColumnCorner, RowCorner)

private func rectValue(_ rect: TableCellRectBoundary, _ column: ColumnCorner) -> Int {
  column == .minColumn ? rect.minColumn : rect.maxColumn
}

private func rectValue(_ rect: TableCellRectBoundary, _ row: RowCorner) -> Int {
  row == .minRow ? rect.minRow : rect.maxRow
}

private func getCorner(_ rect: TableCellRectBoundary, _ cellValue: TableMapValueType) -> Corner? {
  let colName: ColumnCorner
  let rowName: RowCorner
  if cellValue.startColumn == rect.minColumn {
    colName = .minColumn
  } else if cellValue.startColumn + cellValue.cell.__colSpan - 1 == rect.maxColumn {
    colName = .maxColumn
  } else {
    return nil
  }
  if cellValue.startRow == rect.minRow {
    rowName = .minRow
  } else if cellValue.startRow + cellValue.cell.__rowSpan - 1 == rect.maxRow {
    rowName = .maxRow
  } else {
    return nil
  }
  return (colName, rowName)
}

/// The corner of `rect` the anchor sits on, or the one opposite the focus,
/// or the top-left.
private func getAnchorCorner(
  _ rect: TableCellRectBoundary, _ anchorCellValue: TableMapValueType,
  _ focusCellValue: TableMapValueType
) -> Corner {
  if let anchorCorner = getCorner(rect, anchorCellValue) {
    return anchorCorner
  }
  if let focusCorner = getCorner(rect, focusCellValue) {
    return oppositeCorner(focusCorner)
  }
  return (.minColumn, .minRow)
}

private func oppositeCorner(_ corner: Corner) -> Corner {
  (
    corner.0 == .minColumn ? .maxColumn : .minColumn,
    corner.1 == .minRow ? .maxRow : .minRow
  )
}

private func cellAtCornerOrThrow(
  _ tableMap: TableMapType, _ rect: TableCellRectBoundary, _ corner: Corner
) throws -> TableMapValueType {
  let rowNum = rectValue(rect, corner.1)
  guard rowNum >= 0, rowNum < tableMap.count else {
    throw LexicalError(
      "cellAtCornerOrThrow: \(corner.1) = \(rowNum) missing in tableMap")
  }
  let colNum = rectValue(rect, corner.0)
  guard colNum >= 0, colNum < tableMap[rowNum].count, let cell = tableMap[rowNum][colNum] else {
    throw LexicalError(
      "cellAtCornerOrThrow: \(corner.0) = \(colNum) missing in tableMap")
  }
  return cell
}

private func extractRectCorners(
  _ tableMap: TableMapType, _ anchorCellValue: TableMapValueType,
  _ newFocusCellValue: TableMapValueType
) throws -> (TableMapValueType, TableMapValueType) {
  // Both the anchor and focus might be moved to ensure a rectangle given a
  // potentially ragged merge shape
  let rect = try computeTableCellRectBoundary(tableMap, anchorCellValue, newFocusCellValue)
  let anchorCorner = getAnchorCorner(rect, anchorCellValue, newFocusCellValue)
  return (
    try cellAtCornerOrThrow(tableMap, rect, anchorCorner),
    try cellAtCornerOrThrow(tableMap, rect, oppositeCorner(anchorCorner))
  )
}

private func adjustFocusInDirection(
  _ tableObserver: TableObserver, _ tableMap: TableMapType,
  _ anchorCellValue: TableMapValueType, _ focusCellValue: TableMapValueType,
  _ direction: Direction
) throws -> Bool {
  let rect = try computeTableCellRectBoundary(tableMap, anchorCellValue, focusCellValue)
  let spans = try computeTableCellRectSpans(tableMap, rect)
  let anchorCorner = getAnchorCorner(rect, anchorCellValue, focusCellValue)
  let (focusColumn, focusRow) = oppositeCorner(anchorCorner)
  var fCol = rectValue(rect, focusColumn)
  var fRow = rectValue(rect, focusRow)
  switch direction {
  case .forward: fCol += focusColumn == .maxColumn ? 1 : spans.leftSpan
  case .backward: fCol -= focusColumn == .minColumn ? 1 : spans.rightSpan
  case .down: fRow += focusRow == .maxRow ? 1 : spans.topSpan
  case .up: fRow -= focusRow == .minRow ? 1 : spans.bottomSpan
  }
  guard fRow >= 0, fRow < tableMap.count else {
    return false
  }
  guard fCol >= 0, fCol < tableMap[fRow].count, let newFocusCellValue = tableMap[fRow][fCol]
  else {
    return false
  }
  // The anchor and new focus are in the selection, but it may have to grow
  // to stay rectangular
  let (finalAnchorCell, finalFocusCell) = try extractRectCorners(
    tableMap, anchorCellValue, newFocusCellValue)
  let anchorDOM = try getObserverCellFromCellNodeOrThrow(tableObserver, finalAnchorCell.cell)
  let focusDOM = try getObserverCellFromCellNodeOrThrow(tableObserver, finalFocusCell.cell)
  try tableObserver.setAnchorCellForSelection(anchorDOM)
  try tableObserver.setFocusCellForSelection(focusDOM, true)
  return true
}

private func isSelectionInTable(
  _ selection: (any BaseSelection)?, _ tableNode: TableNode
)
  throws -> Bool
{
  if selection is RangeSelection || selection is TableSelection,
    let (anchor, focus) = selection?.getStartEndPoints()
  {
    // TODO this should probably return false if there's an unrelated
    //      shadow root between the node and the table (e.g. another table,
    //      collapsible, etc.)
    let isAnchorInside = try tableNode.isParentOf(anchor.getNode())
    let isFocusInside = try tableNode.isParentOf(focus.getNode())

    return isAnchorInside && isFocusInside
  }

  return false
}

private func selectTableCellNode(_ tableCell: TableCellNode, _ fromStart: Bool) throws {
  if fromStart {
    try tableCell.selectStart()
  } else {
    try tableCell.selectEnd()
  }
}

/// `$findCellNode(node)`: the cell that is `node` or holds it.
public func findCellNode(_ node: LexicalNode) throws -> TableCellNode? {
  try findMatchingParent(node, { $0 is TableCellNode }) as? TableCellNode
}

/// `$findTableNode(node)`: the table that is `node` or holds it.
public func findTableNode(_ node: LexicalNode) throws -> TableNode? {
  try findMatchingParent(node, { $0 is TableNode }) as? TableNode
}

private func getBlockParentIfFirstNode(_ node: LexicalNode) throws -> ElementNode? {
  var prevNode = node
  var currentNode: LexicalNode? = node
  while let current = currentNode {
    if let element = current as? ElementNode {
      if current !== prevNode, try element.getFirstChild() !== prevNode {
        // Not the first child or the initial node
        return nil
      } else if try !element.isInline() {
        return element
      }
    }
    prevNode = current
    currentNode = try current.getParent()
  }
  return nil
}

private func handleHorizontalArrowKeyRangeSelection(
  _ editor: Editor, _ event: KeyboardEvent, _ selection: RangeSelection, extend: Bool,
  _ isBackward: Bool, _ tableNode: TableNode, _ tableObserver: TableObserver
) throws -> Bool {
  let initialFocus = try caretFromPoint(selection.focus, isBackward ? .previous : .next)
  if try isExtendableTextPointCaret(initialFocus) {
    return false
  }
  var lastCaret = initialFocus
  // TableCellNode is the only shadow root we are interested in piercing so
  // we find the last internal caret and then check its parent
  let iterator = try extendCaretToRange(initialFocus).iterNodeCarets(.shadowRoot)
  while let nextCaret = try iterator.next() {
    if !(nextCaret.isSibling && nextCaret.origin is ElementNode) {
      return false
    }
    lastCaret = nextCaret
  }
  guard let anchorCell = try lastCaret.getParentAtCaret() as? TableCellNode else {
    return false
  }
  let focusCaret = try findNextTableCell(getSiblingCaret(anchorCell, lastCaret.direction))
  guard let anchorCellTable = try findMatchingParent(anchorCell, { $0 is TableNode }) as? TableNode,
    anchorCellTable.is(tableNode)
  else {
    return false
  }
  // JS reads the cell's DOM cell, from a rendered table.
  guard editor._keyToDOMMap.get(anchorCell.getKey()) != nil else {
    return false
  }

  tableObserver.table = try getElementForTableNode(editor, anchorCellTable)
  let anchorDOMCell = try getObserverCellFromCellNodeOrThrow(tableObserver, anchorCell)
  if let focusCaret {
    if extend {
      let focusCell = try getNodeByKey(focusCaret.origin.getKey()) as? TableCellNode
      guard let focusCell, editor._keyToDOMMap.get(focusCell.getKey()) != nil,
        let focusDOMCell = try? getObserverCellFromCellNodeOrThrow(tableObserver, focusCell)
      else {
        return false
      }
      try tableObserver.setAnchorCellForSelection(anchorDOMCell)
      try tableObserver.setFocusCellForSelection(focusDOMCell, true)
    } else {
      let innerFocusCaret = try normalizeCaret(focusCaret)
      try setPointFromCaret(selection.anchor, innerFocusCaret)
      try setPointFromCaret(selection.focus, innerFocusCaret)
    }
  } else if extend {
    // extend the selection from a range inside the cell to a table
    // selection of the cell
    try tableObserver.setAnchorCellForSelection(anchorDOMCell)
    try tableObserver.setFocusCellForSelection(anchorDOMCell, true)
  } else {
    // exit the table
    let outerFocusCaret = try getTableExitCaret(
      getSiblingCaret(anchorCellTable, initialFocus.direction))
    try setPointFromCaret(selection.anchor, outerFocusCaret)
    try setPointFromCaret(selection.focus, outerFocusCaret)
  }
  stopEvent(event)
  return true
}

private func getTableExitCaret(_ initialCaret: SiblingCaret) throws -> PointCaret {
  if let adjacent = try getAdjacentChildCaret(initialCaret), adjacent.isChild {
    return try normalizeCaret(adjacent)
  }
  return initialCaret
}

private func findNextTableCell(_ initialCaret: SiblingCaret) throws -> ChildCaret? {
  let iterator = try extendCaretToRange(initialCaret).iterNodeCarets(.root)
  while let nextCaret = try iterator.next() {
    let origin = nextCaret.origin
    if let origin = origin as? TableCellNode {
      if nextCaret.isChild {
        let caret: ChildCaret = getChildCaret(origin as ElementNode, initialCaret.direction);
        return caret
      }
    } else if !(origin is TableRowNode) {
      break
    }
  }
  return nil
}

/// Whether the selection focus sits before `tableNode` in document order,
/// the only side an ArrowDown can move the caret into the table from.
private func isSelectionBeforeTable(
  _ selection: (any BaseSelection)?, _ tableNode: TableNode
)
  throws -> Bool
{
  guard let selection = selection as? RangeSelection else {
    return false
  }
  let focusCaret = try caretFromPoint(selection.focus, .next)
  // A ChildCaret is ordered at the table's 'enter' (pre-order) position, so
  // any caret strictly before it is outside of and above the table.
  let tableCaret = getChildCaret(tableNode as ElementNode, .next)
  return try getCommonAncestor(focusCaret.origin, tableCaret.origin) != nil
    && comparePointCaretNext(focusCaret, tableCaret) < 0
}

private func handleArrowKey(
  _ editor: Editor, _ event: KeyboardEvent, _ direction: Direction, _ tableNode: TableNode,
  _ tableObserver: TableObserver, _ tableObservers: TableObservers,
  _ layout: (any TableLayout)?
) throws -> Bool {
  if direction == .up || direction == .down, layout?.isTypeaheadMenuInView ?? false {
    return false
  }

  let selection = try getSelection()

  // Handle arrow key into a table (including from a table into a nested
  // table)
  if try !isSelectionInTable(selection, tableNode) {
    if let selection = selection as? RangeSelection {
      if direction == .backward {
        if selection.focus.offset > 0 {
          return false
        }
        guard let parentNode = try getBlockParentIfFirstNode(selection.focus.getNode()),
          let siblingNode = try parentNode.getPreviousSibling() as? TableNode
        else {
          return false
        }
        stopEvent(event)
        if event.shiftKey {
          try selection.focus.set(
            siblingNode.getParentOrThrow().getKey(), siblingNode.getIndexWithinParent(),
            .element)
        } else {
          try siblingNode.selectEnd()
        }
        return true
      } else if event.shiftKey && (direction == .up || direction == .down) {
        let focusNode = try selection.focus.getNode()
        let isTableUnselect =
          try !selection.isCollapsed()
          && ((direction == .up && !selection.isBackward())
            || (direction == .down && selection.isBackward()))
        if isTableUnselect {
          var focusParentNode = try findMatchingParent(focusNode, { $0 is TableNode })
          if focusParentNode is TableCellNode {
            focusParentNode = try findMatchingParent(focusParentNode!, { $0 is TableNode })
          }
          if focusParentNode !== tableNode {
            return false
          }
          guard let focusParentNode else {
            return false
          }
          guard
            let sibling = try direction == .down
              ? focusParentNode.getNextSibling() : focusParentNode.getPreviousSibling()
          else {
            return false
          }
          var newOffset = 0
          if direction == .up, let sibling = sibling as? ElementNode {
            newOffset = try sibling.getChildrenSize()
          }
          var newFocusNode = sibling
          if direction == .up, let sibling = sibling as? ElementNode {
            newFocusNode = try sibling.getLastChild() ?? sibling
            newOffset = try (newFocusNode as? TextNode)?.getTextContentSize() ?? 0
          }
          let newSelection = selection.cloneRange()

          try newSelection.focus.set(
            newFocusNode.getKey(), newOffset, newFocusNode is TextNode ? .text : .element)
          try setSelection(newSelection)
          stopEvent(event)
          return true
        } else if try isRootOrShadowRoot(focusNode) {
          let nodes = try selection.getNodes()
          let selectedNode = direction == .up ? nodes.last : nodes.first
          if let selectedNode,
            try findParentTableCellNodeInTable(tableNode, selectedNode) != nil
          {
            guard let firstDescendant = try tableNode.getFirstDescendant(),
              let lastDescendant = try tableNode.getLastDescendant()
            else {
              return false
            }
            let (firstCellNode, _, _) = try getNodeTriplet(firstDescendant)
            let (lastCellNode, _, _) = try getNodeTriplet(lastDescendant)
            let firstCellCoords = try tableNode.getCordsFromCellNode(
              firstCellNode, tableObserver.table)
            let lastCellCoords = try tableNode.getCordsFromCellNode(
              lastCellNode, tableObserver.table)
            let firstCellDOM = try tableNode.getDOMCellFromCordsOrThrow(
              firstCellCoords.x, firstCellCoords.y, tableObserver.table)
            let lastCellDOM = try tableNode.getDOMCellFromCordsOrThrow(
              lastCellCoords.x, lastCellCoords.y, tableObserver.table)
            try tableObserver.setAnchorCellForSelection(firstCellDOM)
            try tableObserver.setFocusCellForSelection(lastCellDOM, true)
            return true
          }
          return false
        } else {
          var focusParentNode = try findMatchingParent(focusNode) { node in
            try (node as? ElementNode).map { try !$0.isInline() } ?? false
          }
          if focusParentNode is TableCellNode {
            focusParentNode = try findMatchingParent(focusParentNode!, { $0 is TableNode })
          }
          guard let focusParentNode else {
            return false
          }
          let sibling =
            try direction == .down
            ? focusParentNode.getNextSibling() : focusParentNode.getPreviousSibling()
          if let sibling = sibling as? TableNode, tableObserver.tableNodeKey == sibling.getKey() {
            guard let firstDescendant = try sibling.getFirstDescendant(),
              let lastDescendant = try sibling.getLastDescendant()
            else {
              return false
            }
            let (firstCellNode, _, _) = try getNodeTriplet(firstDescendant)
            let (lastCellNode, _, _) = try getNodeTriplet(lastDescendant)
            let newSelection = selection.cloneRange()
            try newSelection.focus.set(
              (direction == .up ? firstCellNode : lastCellNode).getKey(),
              direction == .up ? 0 : lastCellNode.getChildrenSize(), .element)
            stopEvent(event)
            try setSelection(newSelection)
            return true
          }
        }
      }
    }
    if direction == .down, try isScrollableTablesActive(editor),
      // Only arm the workaround when ArrowDown could actually move the caret
      // into the table.
      try isSelectionBeforeTable(selection, tableNode)
    {
      // Enable Firefox workaround
      tableObservers.setShouldCheckSelectionForTable(tableNode.getKey())
    }
    return false
  }

  if let selection = selection as? RangeSelection {
    if direction == .backward || direction == .forward {
      return try handleHorizontalArrowKeyRangeSelection(
        editor, event, selection, extend: event.shiftKey, direction == .backward, tableNode,
        tableObserver)
    }

    if selection.isCollapsed() {
      let anchor = selection.anchor
      let focus = selection.focus
      guard
        let anchorCellNode = try findMatchingParent(anchor.getNode(), { $0 is TableCellNode })
          as? TableCellNode,
        try anchorCellNode.is(findMatchingParent(focus.getNode(), { $0 is TableCellNode }))
      else {
        return false
      }
      if let anchorCellTable = try findTableNode(anchorCellNode), anchorCellTable !== tableNode,
        editor._keyToDOMMap.get(anchorCellTable.getKey()) != nil
      {
        tableObserver.table = try getTable(anchorCellTable)
        return try handleArrowKey(
          editor, event, direction, anchorCellTable, tableObserver, tableObservers, layout)
      }

      // JS reads the rects of the rendered caret and of the cell's edge
      // block; without a layout the view moves the caret.
      guard let layout,
        editor._keyToDOMMap.get(anchorCellNode.__key) != nil,
        editor._keyToDOMMap.get(anchor.key) != nil
      else {
        return false
      }

      let edgeSelectionRect =
        anchor.type == .element ? layout.nodeRect(anchor.key) : layout.selectionRect(selection)
      guard let edgeSelectionRect else {
        return false
      }

      guard
        let edgeChild = try direction == .up
          ? anchorCellNode.getFirstChild() : anchorCellNode.getLastChild()
      else {
        return false
      }

      guard editor._keyToDOMMap.get(edgeChild.__key) != nil,
        let edgeRect = layout.nodeRect(edgeChild.__key)
      else {
        return false
      }

      let isExiting =
        direction == .up
        ? edgeRect.top > edgeSelectionRect.top - edgeSelectionRect.height
        : edgeSelectionRect.bottom + edgeSelectionRect.height > edgeRect.bottom

      if isExiting {
        stopEvent(event)

        let cords = try tableNode.getCordsFromCellNode(anchorCellNode, tableObserver.table)

        if event.shiftKey {
          let cell = try tableNode.getDOMCellFromCordsOrThrow(
            cords.x, cords.y, tableObserver.table)
          try tableObserver.setAnchorCellForSelection(cell)
          try tableObserver.setFocusCellForSelection(cell, true)
        } else {
          return try selectTableNodeInDirection(
            tableObserver, tableNode, cords.x, cords.y, direction)
        }

        return true
      }
    }
  } else if let selection = selection as? TableSelection {
    if selection.tableKey != tableNode.getKey() {
      return false
    }
    let anchorCellNode = try findMatchingParent(selection.anchor.getNode(), { $0 is TableCellNode })
    let focusCellNode = try findMatchingParent(selection.focus.getNode(), { $0 is TableCellNode })

    guard let tableNodeFromSelection = try selection.getNodes().first as? TableNode else {
      throw LexicalError(
        "$handleArrowKey: TableSelection.getNodes()[0] expected to be TableNode")
    }
    guard let anchorCellNode = anchorCellNode as? TableCellNode,
      let focusCellNode = focusCellNode as? TableCellNode,
      editor._keyToDOMMap.get(tableNodeFromSelection.getKey()) != nil
    else {
      return false
    }
    try tableObserver.updateTableTableSelection(selection)

    let grid = try getTable(tableNodeFromSelection)
    let cordsAnchor = try tableNode.getCordsFromCellNode(anchorCellNode, grid)
    let anchorCell = try tableNode.getDOMCellFromCordsOrThrow(cordsAnchor.x, cordsAnchor.y, grid)
    try tableObserver.setAnchorCellForSelection(anchorCell)

    stopEvent(event)

    if event.shiftKey {
      let (tableMap, anchorValue, focusValue) = try computeTableMap(
        tableNode, anchorCellNode, focusCellNode)
      return try adjustFocusInDirection(
        tableObserver, tableMap, anchorValue, focusValue, direction)
    } else {
      try focusCellNode.selectEnd()
    }

    return true
  }

  return false
}

private func stopEvent(_ event: DOMEvent) {
  event.preventDefault()
  event.stopPropagation()
}

private enum TableEdge { case first, last }

private func insertParagraphAtTableEdge(
  _ edgePosition: TableEdge, _ tableNode: TableNode, _ children: [LexicalNode] = []
) throws {
  let paragraphNode = try createParagraphNode()
  if edgePosition == .first {
    try tableNode.insertBefore(paragraphNode)
  } else {
    try tableNode.insertAfter(paragraphNode)
  }
  for child in children {
    try paragraphNode.append(child)
  }
  try paragraphNode.selectEnd()
}

/// `$getTableEdgeCursorPosition(editor, selection, tableNode)`: JS answers
/// only when the DOM selection's anchor is beside the table element (in its
/// wrapper), which a native view never does, so there is no edge.
private func getTableEdgeCursorPosition(
  _ editor: Editor, _ selection: RangeSelection, _ tableNode: TableNode
) throws -> TableEdge? {
  nil
}
