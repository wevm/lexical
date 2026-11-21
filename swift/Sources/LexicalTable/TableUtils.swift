import Lexical

// Port of packages/lexical-table/src/LexicalTableUtils.ts.
//
// Not ported: the deprecated `$insertTableRow`, `$insertTableColumn` and
// `$getTableCellSiblingsFromTableCellNode`, which find cells through the
// reconciled DOM (`TableDOMTable`, LexicalTableObserver.ts) that the native
// views do not have.

/// The value at `row[i]`, or `nil` where JS reads `undefined`.
private func at(_ row: [TableMapValueType?]?, _ i: Int) -> TableMapValueType? {
  guard let row, i >= 0, i < row.count else { return nil }
  return row[i]
}

/// The row at `map[i]`, or `nil` where JS reads `undefined`.
private func rowAt(_ map: TableMapType, _ i: Int) -> [TableMapValueType?]? {
  i >= 0 && i < map.count ? map[i] : nil
}

/// `row[i]` destructured, which throws where JS reads a property of
/// `undefined`.
private func value(_ row: [TableMapValueType?]?, _ i: Int) throws -> TableMapValueType {
  guard let value = at(row, i) else {
    throw LexicalError("Cannot read properties of undefined (reading 'cell')")
  }
  return value
}

/// `$createTableNodeWithDimensions(rowCount, columnCount, includeHeaders)`.
public func createTableNodeWithDimensions(
  _ rowCount: Int, _ columnCount: Int, _ includeHeaders: InsertTableCommandPayloadHeaders = true
) throws -> TableNode {
  let tableNode = try createTableNode()

  for iRow in 0..<max(rowCount, 0) {
    let tableRowNode = try createTableRowNode()

    for iColumn in 0..<max(columnCount, 0) {
      var headerState = TableCellHeaderStates.NO_STATUS

      switch includeHeaders {
      case .rowsAndColumns(let rows, let columns):
        if iRow == 0 && rows {
          headerState |= TableCellHeaderStates.ROW
        }
        if iColumn == 0 && columns {
          headerState |= TableCellHeaderStates.COLUMN
        }
      case .boolean(let include):
        if include {
          if iRow == 0 {
            headerState |= TableCellHeaderStates.ROW
          }
          if iColumn == 0 {
            headerState |= TableCellHeaderStates.COLUMN
          }
        }
      }

      let tableCellNode = try createTableCellNode(headerState)
      let paragraphNode = try createParagraphNode()
      try paragraphNode.append(createTextNode())
      try tableCellNode.append(paragraphNode)
      try tableRowNode.append(tableCellNode)
    }

    try tableNode.append(tableRowNode)
  }

  return tableNode
}

/// `$getTableCellNodeFromLexicalNode(startingNode)`.
public func getTableCellNodeFromLexicalNode(_ startingNode: LexicalNode) throws -> TableCellNode? {
  try findMatchingParent(startingNode, { $0 is TableCellNode }) as? TableCellNode
}

/// `$getTableRowNodeFromTableCellNodeOrThrow(startingNode)`.
public func getTableRowNodeFromTableCellNodeOrThrow(
  _ startingNode: LexicalNode
) throws -> TableRowNode {
  guard let node = try findMatchingParent(startingNode, { $0 is TableRowNode }) as? TableRowNode
  else {
    throw LexicalError("Expected table cell to be inside of table row.")
  }
  return node
}

/// `$getTableNodeFromLexicalNodeOrThrow(startingNode)`.
public func getTableNodeFromLexicalNodeOrThrow(_ startingNode: LexicalNode) throws -> TableNode {
  guard let node = try findMatchingParent(startingNode, { $0 is TableNode }) as? TableNode else {
    throw LexicalError("Expected table cell to be inside of table.")
  }
  return node
}

/// `$getTableRowIndexFromTableCellNode(tableCellNode)`.
public func getTableRowIndexFromTableCellNode(_ tableCellNode: TableCellNode) throws -> Int {
  let tableRowNode = try getTableRowNodeFromTableCellNodeOrThrow(tableCellNode)
  let tableNode = try getTableNodeFromLexicalNodeOrThrow(tableRowNode)
  return try tableNode.getChildren().firstIndex { $0.is(tableRowNode) } ?? -1
}

/// `$getTableColumnIndexFromTableCellNode(tableCellNode)`.
public func getTableColumnIndexFromTableCellNode(_ tableCellNode: TableCellNode) throws -> Int {
  let tableRowNode = try getTableRowNodeFromTableCellNodeOrThrow(tableCellNode)
  return try tableRowNode.getChildren().firstIndex { $0.is(tableCellNode) } ?? -1
}

/// `$removeTableRowAtIndex(tableNode, indexToDelete)`.
@discardableResult
public func removeTableRowAtIndex(_ tableNode: TableNode, _ indexToDelete: Int) throws -> TableNode
{
  let tableRows = try tableNode.getChildren()

  if indexToDelete >= tableRows.count || indexToDelete < 0 {
    throw LexicalError("Expected table cell to be inside of table row.")
  }

  let targetRowNode = tableRows[indexToDelete]
  try targetRowNode.remove()
  return tableNode
}

private func getHeaderState(
  _ currentState: TableCellHeaderState, _ possibleState: TableCellHeaderState
) -> TableCellHeaderState {
  if currentState == TableCellHeaderStates.BOTH || currentState == possibleState {
    return possibleState
  }
  return TableCellHeaderStates.NO_STATUS
}

/// The anchor and focus of the current selection, which must be a
/// `RangeSelection` or a `TableSelection`.
private func rangeOrTableSelection() throws -> (any BaseSelection, Point, Point) {
  let selection = try getSelection()
  guard let selection, selection is RangeSelection || selection is TableSelection,
    let (anchor, focus) = selection.getStartEndPoints()
  else {
    throw LexicalError("Expected a RangeSelection or TableSelection")
  }
  return (selection, anchor, focus)
}

/// `$insertTableRowAtSelection(insertAfter)`: inserts a table row before
/// or after the current focus cell node, taking into account any spans.
/// Returns the inserted table row node.
@discardableResult
public func insertTableRowAtSelection(_ insertAfter: Bool = true) throws -> TableRowNode? {
  let (_, anchorPoint, focusPoint) = try rangeOrTableSelection()
  let anchor = try anchorPoint.getNode()
  let focus = try focusPoint.getNode()
  let (anchorCell, _, _) = try getNodeTriplet(anchor)
  let (focusCell, _, grid) = try getNodeTriplet(focus)
  let (_, focusCellMap, anchorCellMap) = try computeTableMap(grid, focusCell, anchorCell)
  let anchorStartRow = anchorCellMap.startRow
  let focusStartRow = focusCellMap.startRow
  if insertAfter {
    return try insertTableRowAtNode(
      anchorStartRow + anchorCell.__rowSpan > focusStartRow + focusCell.__rowSpan
        ? anchorCell : focusCell,
      true)
  } else {
    return try insertTableRowAtNode(
      focusStartRow < anchorStartRow ? focusCell : anchorCell, false)
  }
}

/// `$insertTableRow__EXPERIMENTAL`. Deprecated: renamed to
/// `insertTableRowAtSelection`.
@discardableResult
public func insertTableRow__EXPERIMENTAL(_ insertAfter: Bool = true) throws -> TableRowNode? {
  try insertTableRowAtSelection(insertAfter)
}

/// `$insertTableRowAtNode(cellNode, insertAfter)`: inserts a table row
/// before or after `cellNode`, taking into account any spans. Returns the
/// inserted table row node.
@discardableResult
public func insertTableRowAtNode(
  _ cellNode: TableCellNode, _ insertAfter: Bool = true
) throws -> TableRowNode? {
  let (_, _, grid) = try getNodeTriplet(cellNode)
  // Commands below assume a rectangular grid; repair a ragged one first.
  try padTableRows(grid)
  let (gridMap, cellMap, _) = try computeTableMap(grid, cellNode, cellNode)
  let columnCount = rowAt(gridMap, 0)?.count ?? 0
  let cellStartRow = cellMap.startRow
  var insertedRow: TableRowNode? = nil
  if insertAfter {
    let insertAfterEndRow = cellStartRow + cellNode.__rowSpan - 1
    let insertAfterEndRowMap = rowAt(gridMap, insertAfterEndRow)
    let newRow = try createTableRowNode()
    for i in 0..<columnCount {
      let mapValue = try value(insertAfterEndRowMap, i)
      let cell = mapValue.cell
      if mapValue.startRow + cell.__rowSpan - 1 <= insertAfterEndRow {
        let currentCellHeaderState = mapValue.cell.__headerState
        let headerState = getHeaderState(currentCellHeaderState, TableCellHeaderStates.COLUMN)
        try newRow.append(createTableCellNode(headerState).append(createParagraphNode()))
      } else {
        try cell.setRowSpan(cell.__rowSpan + 1)
      }
    }
    guard let insertAfterEndRowNode = try grid.getChildAtIndex(insertAfterEndRow) as? TableRowNode
    else {
      throw LexicalError("insertAfterEndRow is not a TableRowNode")
    }
    try insertAfterEndRowNode.insertAfter(newRow)
    insertedRow = newRow
  } else {
    let insertBeforeStartRow = cellStartRow
    let insertBeforeStartRowMap = rowAt(gridMap, insertBeforeStartRow)
    let newRow = try createTableRowNode()
    for i in 0..<columnCount {
      let mapValue = try value(insertBeforeStartRowMap, i)
      let cell = mapValue.cell
      if mapValue.startRow == insertBeforeStartRow {
        let currentCellHeaderState = mapValue.cell.__headerState
        let headerState = getHeaderState(currentCellHeaderState, TableCellHeaderStates.COLUMN)
        try newRow.append(createTableCellNode(headerState).append(createParagraphNode()))
      } else {
        try cell.setRowSpan(cell.__rowSpan + 1)
      }
    }
    guard
      let insertBeforeStartRowNode = try grid.getChildAtIndex(insertBeforeStartRow)
        as? TableRowNode
    else {
      throw LexicalError("insertBeforeStartRow is not a TableRowNode")
    }
    try insertBeforeStartRowNode.insertBefore(newRow)
    insertedRow = newRow
  }
  return insertedRow
}

/// `$insertTableColumnAtSelection(insertAfter)`: inserts a column before
/// or after the current focus cell node, taking into account any spans.
/// Returns the first inserted cell node.
@discardableResult
public func insertTableColumnAtSelection(_ insertAfter: Bool = true) throws -> TableCellNode? {
  let (_, anchorPoint, focusPoint) = try rangeOrTableSelection()
  let anchor = try anchorPoint.getNode()
  let focus = try focusPoint.getNode()
  let (anchorCell, _, _) = try getNodeTriplet(anchor)
  let (focusCell, _, grid) = try getNodeTriplet(focus)
  let (_, focusCellMap, anchorCellMap) = try computeTableMap(grid, focusCell, anchorCell)
  let anchorStartColumn = anchorCellMap.startColumn
  let focusStartColumn = focusCellMap.startColumn
  if insertAfter {
    return try insertTableColumnAtNode(
      anchorStartColumn + anchorCell.__colSpan > focusStartColumn + focusCell.__colSpan
        ? anchorCell : focusCell,
      true)
  } else {
    return try insertTableColumnAtNode(
      focusStartColumn < anchorStartColumn ? focusCell : anchorCell, false)
  }
}

/// `$insertTableColumn__EXPERIMENTAL`. Deprecated: renamed to
/// `insertTableColumnAtSelection`.
@discardableResult
public func insertTableColumn__EXPERIMENTAL(_ insertAfter: Bool = true) throws -> TableCellNode? {
  try insertTableColumnAtSelection(insertAfter)
}

/// `$insertTableColumnAtNode(cellNode, insertAfter, shouldSetSelection)`:
/// inserts a column before or after `cellNode`, taking into account any
/// spans. Returns the first inserted cell node.
@discardableResult
public func insertTableColumnAtNode(
  _ cellNode: TableCellNode, _ insertAfter: Bool = true, _ shouldSetSelection: Bool = true
) throws -> TableCellNode? {
  let (_, _, grid) = try getNodeTriplet(cellNode)
  // Commands below assume a rectangular grid; repair a ragged one first.
  try padTableRows(grid)
  let (gridMap, cellMap, _) = try computeTableMap(grid, cellNode, cellNode)
  let rowCount = gridMap.count
  let startColumn = cellMap.startColumn
  let insertAfterColumn = insertAfter ? startColumn + cellNode.__colSpan - 1 : startColumn - 1
  guard let gridFirstChild = try grid.getFirstChild() as? TableRowNode else {
    throw LexicalError("Expected firstTable child to be a row")
  }
  var firstInsertedCell: TableCellNode? = nil
  func createTableCellNodeForInsertTableColumn(
    _ headerState: TableCellHeaderState = TableCellHeaderStates.NO_STATUS
  ) throws -> TableCellNode {
    let cell = try createTableCellNode(headerState).append(createParagraphNode())
    if firstInsertedCell == nil {
      firstInsertedCell = cell
    }
    return cell
  }
  var loopRow: TableRowNode = gridFirstChild
  for i in 0..<rowCount {
    if i != 0 {
      guard let currentRow = try loopRow.getNextSibling() as? TableRowNode else {
        throw LexicalError("Expected row nextSibling to be a row")
      }
      loopRow = currentRow
    }
    let rowMap = gridMap[i]

    let currentCellHeaderState =
      try value(rowMap, insertAfterColumn < 0 ? 0 : insertAfterColumn).cell.__headerState

    let headerState = getHeaderState(currentCellHeaderState, TableCellHeaderStates.ROW)

    if insertAfterColumn < 0 {
      try insertFirst(loopRow, createTableCellNodeForInsertTableColumn(headerState))
      continue
    }
    let current = try value(rowMap, insertAfterColumn)
    let currentCell = current.cell
    if current.startColumn + currentCell.__colSpan - 1 <= insertAfterColumn {
      // Find the last cell this row actually owns at or before the insertion
      // column. Grid positions covered by a rowSpan from an earlier row are
      // not children of this row, so they can not be inserted after.
      var insertAfterCell: TableCellNode? = nil
      var column = 0
      while column <= insertAfterColumn {
        let currentCellMap = try value(rowMap, column)
        if currentCellMap.startRow == i {
          insertAfterCell = currentCellMap.cell
        }
        if currentCellMap.cell.__colSpan > 1 {
          column += currentCellMap.cell.__colSpan - 1
        }
        column += 1
      }
      if let insertAfterCell {
        try insertAfterCell.insertAfter(createTableCellNodeForInsertTableColumn(headerState))
      } else {
        // Every grid column to the left is covered by a rowSpan from an
        // earlier row, so the new cell is this row's first child.
        try insertFirst(loopRow, createTableCellNodeForInsertTableColumn(headerState))
      }
    } else {
      try currentCell.setColSpan(currentCell.__colSpan + 1)
    }
  }
  if let firstInsertedCell, shouldSetSelection {
    try moveSelectionToCell(firstInsertedCell)
  }
  if let colWidths = try grid.getColWidths() {
    var newColWidths = colWidths
    let columnIndex = insertAfterColumn < 0 ? 0 : insertAfterColumn
    // JS splices in `newColWidths[columnIndex]`, `undefined` past the end,
    // which JSON writes as `null`; a width of 0 is no width either.
    let newWidth = columnIndex < newColWidths.count ? newColWidths[columnIndex] : 0
    newColWidths.insert(newWidth, at: min(columnIndex, newColWidths.count))
    try grid.setColWidths(newColWidths)
  }
  return firstInsertedCell
}

/// `$deleteTableColumn(tableNode, targetIndex)`. Deprecated: it does not
/// support merged cells; use `deleteTableColumnAtSelection`.
@discardableResult
public func deleteTableColumn(_ tableNode: TableNode, _ targetIndex: Int) throws -> TableNode {
  let tableRows = try tableNode.getChildren()

  for currentTableRowNode in tableRows {
    if let currentTableRowNode = currentTableRowNode as? TableRowNode {
      let tableRowChildren = try currentTableRowNode.getChildren()

      if targetIndex >= tableRowChildren.count || targetIndex < 0 {
        throw LexicalError("Table column target index out of range")
      }

      try tableRowChildren[targetIndex].remove()
    }
  }

  return tableNode
}

/// `$deleteTableRowAtSelection()`.
public func deleteTableRowAtSelection() throws {
  let (selection, anchorPoint, focusPoint) = try rangeOrTableSelection()
  let (anchor, focus) =
    try selection.isBackward()
    ? (focusPoint.getNode(), anchorPoint.getNode())
    : (anchorPoint.getNode(), focusPoint.getNode())
  let (anchorCell, _, grid) = try getNodeTriplet(anchor)
  let (focusCell, _, _) = try getNodeTriplet(focus)
  // Commands below assume a rectangular grid; repair a ragged one first.
  try padTableRows(grid)
  let (gridMap, anchorCellMap, focusCellMap) = try computeTableMap(grid, anchorCell, focusCell)
  let anchorStartRow = anchorCellMap.startRow
  let focusStartRow = focusCellMap.startRow
  let focusEndRow = focusStartRow + focusCell.__rowSpan - 1
  if gridMap.count == focusEndRow - anchorStartRow + 1 {
    // Empty grid. Move the selection out of the table before removing it,
    // otherwise a TableSelection is left pointing at cells that no longer
    // exist.
    try grid.selectPrevious()
    try grid.remove()
    return
  }
  let columnCount = rowAt(gridMap, 0)?.count ?? 0
  let nextRow = rowAt(gridMap, focusEndRow + 1)
  let nextRowNode = try grid.getChildAtIndex(focusEndRow + 1)
  var row = focusEndRow
  while row >= anchorStartRow {
    var column = columnCount - 1
    while column >= 0 {
      defer { column -= 1 }
      let mapValue = try value(rowAt(gridMap, row), column)
      let cell = mapValue.cell
      let cellStartRow = mapValue.startRow
      if mapValue.startColumn != column {
        // Don't repeat work for the same Cell
        continue
      }
      // Rows overflowing top or bottom have to be trimmed
      if cellStartRow < anchorStartRow || cellStartRow + cell.__rowSpan - 1 > focusEndRow {
        let intersectionStart = max(cellStartRow, anchorStartRow)
        let intersectionEnd = min(cell.__rowSpan + cellStartRow - 1, focusEndRow)

        let overflowRowsCount =
          intersectionStart <= intersectionEnd ? intersectionEnd - intersectionStart + 1 : 0
        try cell.setRowSpan(cell.__rowSpan - overflowRowsCount)
      }
      // Rows overflowing bottom have to be moved to the next row
      if cellStartRow >= anchorStartRow && cellStartRow + cell.__rowSpan - 1 > focusEndRow
        // Handle overflow only once
        && row == focusEndRow
      {
        guard let nextRowNode = nextRowNode as? TableRowNode else {
          throw LexicalError("Expected a TableRowNode")
        }
        var insertAfterCell: TableCellNode? = nil
        var columnIndex = 0
        while columnIndex < column {
          let currentCellMap = try value(nextRow, columnIndex)
          let currentCell = currentCellMap.cell
          // Checking the cell having startRow as same as nextRow
          if currentCellMap.startRow == row + 1 {
            insertAfterCell = currentCell
          }
          if currentCell.__colSpan > 1 {
            columnIndex += currentCell.__colSpan - 1
          }
          columnIndex += 1
        }
        if let insertAfterCell {
          try insertAfterCell.insertAfter(cell)
        } else {
          try insertFirst(nextRowNode, cell)
        }
      }
    }
    guard let rowNode = try grid.getChildAtIndex(row) as? TableRowNode else {
      throw LexicalError("Expected TableNode childAtIndex(\(row)) to be RowNode")
    }
    try rowNode.remove()
    row -= 1
  }
  if let nextRow {
    try moveSelectionToCell(try value(nextRow, 0).cell)
  } else {
    let previousRow = rowAt(gridMap, anchorStartRow - 1)
    try moveSelectionToCell(try value(previousRow, 0).cell)
  }
}

/// `$deleteTableRow__EXPERIMENTAL`. Deprecated: renamed to
/// `deleteTableRowAtSelection`.
public func deleteTableRow__EXPERIMENTAL() throws {
  try deleteTableRowAtSelection()
}

/// `$deleteTableColumnAtSelection()`.
public func deleteTableColumnAtSelection() throws {
  let (_, anchorPoint, focusPoint) = try rangeOrTableSelection()
  let anchor = try anchorPoint.getNode()
  let focus = try focusPoint.getNode()
  let (anchorCell, _, grid) = try getNodeTriplet(anchor)
  let (focusCell, _, _) = try getNodeTriplet(focus)
  // Commands below assume a rectangular grid; repair a ragged one first.
  try padTableRows(grid)
  let (gridMap, anchorCellMap, focusCellMap) = try computeTableMap(grid, anchorCell, focusCell)
  let anchorStartColumn = anchorCellMap.startColumn
  let focusStartRow = focusCellMap.startRow
  let focusStartColumn = focusCellMap.startColumn
  let startColumn = min(anchorStartColumn, focusStartColumn)
  let endColumn = max(
    anchorStartColumn + anchorCell.__colSpan - 1, focusStartColumn + focusCell.__colSpan - 1)
  let selectedColumnCount = endColumn - startColumn + 1
  let columnCount = rowAt(gridMap, 0)?.count ?? 0
  if columnCount == endColumn - startColumn + 1 {
    // Empty grid
    try grid.selectPrevious()
    try grid.remove()
    return
  }
  let rowCount = gridMap.count
  for row in 0..<rowCount {
    for column in startColumn...endColumn {
      let mapValue = try value(gridMap[row], column)
      let cell = mapValue.cell
      let cellStartColumn = mapValue.startColumn
      if cellStartColumn < startColumn {
        if column == startColumn {
          let overflowLeft = startColumn - cellStartColumn
          // Overflowing left
          try cell.setColSpan(
            cell.__colSpan
              // Possible overflow right too
              - min(selectedColumnCount, cell.__colSpan - overflowLeft))
        }
      } else if cellStartColumn + cell.__colSpan - 1 > endColumn {
        if column == endColumn {
          // Overflowing right
          let inSelectedArea = endColumn - cellStartColumn + 1
          try cell.setColSpan(cell.__colSpan - inSelectedArea)
        }
      } else {
        try cell.remove()
      }
    }
  }
  let focusRowMap = rowAt(gridMap, focusStartRow)
  let nextColumn =
    anchorStartColumn > focusStartColumn
    ? at(focusRowMap, anchorStartColumn + anchorCell.__colSpan)
    : at(focusRowMap, focusStartColumn + focusCell.__colSpan)
  if let nextColumn {
    try moveSelectionToCell(nextColumn.cell)
  } else {
    let previousRow =
      focusStartColumn < anchorStartColumn
      ? try value(focusRowMap, focusStartColumn - 1)
      : try value(focusRowMap, anchorStartColumn - 1)
    try moveSelectionToCell(previousRow.cell)
  }
  if let colWidths = try grid.getColWidths() {
    var newColWidths = colWidths
    let start = min(startColumn, newColWidths.count)
    newColWidths.removeSubrange(start..<min(start + selectedColumnCount, newColWidths.count))
    try grid.setColWidths(newColWidths)
  }
}

/// `$deleteTableColumn__EXPERIMENTAL`. Deprecated: renamed to
/// `deleteTableColumnAtSelection`.
public func deleteTableColumn__EXPERIMENTAL() throws {
  try deleteTableColumnAtSelection()
}

private func moveSelectionToCell(_ cell: TableCellNode) throws {
  if let firstDescendant = try cell.getFirstDescendant() {
    try firstDescendant.getParentOrThrow().selectStart()
  } else {
    try cell.selectStart()
  }
}

private func insertFirst(_ parent: ElementNode, _ node: LexicalNode) throws {
  if let firstChild = try parent.getFirstChild() {
    try firstChild.insertBefore(node)
  } else {
    try parent.append(node)
  }
}

/// `$computeTableMap(tableNode, cellA, cellB)`: the grid of `tableNode`,
/// spans expanded, and the map values of `cellA` and `cellB`, which must be
/// in it.
public func computeTableMap(
  _ tableNode: TableNode, _ cellA: TableCellNode, _ cellB: TableCellNode
) throws -> (TableMapType, TableMapValueType, TableMapValueType) {
  let (tableMap, cellAValue, cellBValue) = try computeTableMapSkipCellCheck(
    tableNode, cellA, cellB)
  guard let cellAValue else { throw LexicalError("Anchor not found in Table") }
  guard let cellBValue else { throw LexicalError("Focus not found in Table") }
  return (tableMap, cellAValue, cellBValue)
}

/// `$computeTableMapSkipCellCheck(tableNode, cellA, cellB)`: the grid of
/// `tableNode`, spans expanded, and the map values of `cellA` and `cellB`.
public func computeTableMapSkipCellCheck(
  _ tableNode: TableNode, _ cellA: TableCellNode?, _ cellB: TableCellNode?
) throws -> (tableMap: TableMapType, cellAValue: TableMapValueType?, cellBValue: TableMapValueType?)
{
  var tableMap: TableMapType = []
  var cellAValue: TableMapValueType? = nil
  var cellBValue: TableMapValueType? = nil
  func ensureMapRow(_ i: Int) {
    while tableMap.count <= i { tableMap.append([]) }
  }
  func setMapValue(_ row: Int, _ column: Int, _ value: TableMapValueType) {
    while tableMap[row].count <= column { tableMap[row].append(nil) }
    tableMap[row][column] = value
  }
  let gridChildren = try tableNode.getChildren()
  for rowIdx in 0..<gridChildren.count {
    guard let row = gridChildren[rowIdx] as? TableRowNode else {
      throw LexicalError("Expected TableNode children to be TableRowNode")
    }
    ensureMapRow(rowIdx)
    var colIdx = 0
    var cell = try row.getFirstChild()
    while let current = cell {
      guard let current = current as? TableCellNode else {
        throw LexicalError("Expected TableRowNode children to be TableCellNode")
      }
      // Skip past any columns that were merged from a higher row
      while colIdx < tableMap[rowIdx].count, tableMap[rowIdx][colIdx] != nil {
        colIdx += 1
      }
      let value = TableMapValueType(cell: current, startRow: rowIdx, startColumn: colIdx)
      let rowSpan = current.__rowSpan
      let colSpan = current.__colSpan
      for j in 0..<max(rowSpan, 0) {
        if rowIdx + j >= gridChildren.count {
          // The table is non-rectangular with a rowSpan below the last
          // <tr> in the table.
          break
        }
        ensureMapRow(rowIdx + j)
        for i in 0..<max(colSpan, 0) {
          setMapValue(rowIdx + j, colIdx + i, value)
        }
      }
      if let cellA, cellAValue == nil, cellA.is(current) {
        cellAValue = value
      }
      if let cellB, cellBValue == nil, cellB.is(current) {
        cellBValue = value
      }
      cell = try current.getNextSibling()
    }
  }
  return (tableMap, cellAValue, cellBValue)
}

/// `$padTableRows(tableNode, $shouldPad)` (`@experimental`): appends empty
/// cells to every row that covers fewer grid columns (spans included) than
/// the widest row, so that the table is rectangular again. `shouldPad`
/// limits the padding to the rows it accepts. Returns whether any cell was
/// added.
@discardableResult
public func padTableRows(
  _ tableNode: TableNode, _ shouldPad: (TableRowNode) throws -> Bool = { _ in true }
) throws -> Bool {
  let (gridMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)
  let maxRowLength = gridMap.reduce(0) { max($0, $1.count) }
  let rowNodes = try tableNode.getChildren()
  var changed = false
  for i in 0..<gridMap.count {
    guard i < rowNodes.count, let rowNode = rowNodes[i] as? TableRowNode, try shouldPad(rowNode)
    else {
      continue
    }
    // Count the grid positions this row covers, including those covered by
    // a rowSpan from an earlier row. Holes are not counted, and an appended
    // cell fills the first hole, so the row ends up maxRowLength wide.
    let rowLength = gridMap[i].reduce(0) { $1 != nil ? $0 + 1 : $0 }
    if rowLength >= maxRowLength {
      continue
    }
    // Padding cells are in a header row whenever the row they extend is
    // one: inherit only that bit from the row's last cell.
    let lastCell = try rowNode.getLastChild()
    let headerState =
      try (lastCell as? TableCellNode).map {
        try $0.getHeaderStyles() & TableCellHeaderStates.ROW
      } ?? TableCellHeaderStates.NO_STATUS
    for _ in rowLength..<maxRowLength {
      try rowNode.append(createTableCellNode(headerState).append(createParagraphNode()))
    }
    changed = true
  }
  return changed
}

/// `$getNodeTriplet(source)`: the cell that is or holds `source`, its row
/// and its table.
public func getNodeTriplet(
  _ source: LexicalNode
) throws -> (TableCellNode, TableRowNode, TableNode) {
  let cell: TableCellNode
  if let source = source as? TableCellNode {
    cell = source
  } else {
    guard let cell_ = try findMatchingParent(source, { $0 is TableCellNode }) as? TableCellNode
    else {
      throw LexicalError("Expected to find a parent TableCellNode")
    }
    cell = cell_
  }
  guard let row = try cell.getParent() as? TableRowNode else {
    throw LexicalError("Expected TableCellNode to have a parent TableRowNode")
  }
  guard let grid = try row.getParent() as? TableNode else {
    throw LexicalError("Expected TableRowNode to have a parent TableNode")
  }
  return (cell, row, grid)
}

/// `$getNodeTriplet(point)`.
public func getNodeTriplet(_ source: Point) throws -> (TableCellNode, TableRowNode, TableNode) {
  guard
    let cell = try findMatchingParent(source.getNode(), { $0 is TableCellNode }) as? TableCellNode
  else {
    throw LexicalError("Expected to find a parent TableCellNode")
  }
  return try getNodeTriplet(cell)
}

/// `TableCellRectBoundary`.
public struct TableCellRectBoundary: Equatable {
  public var minColumn: Int
  public var minRow: Int
  public var maxColumn: Int
  public var maxRow: Int
}

/// `TableCellRectSpans`.
package struct TableCellRectSpans: Equatable {
  package var topSpan: Int
  package var leftSpan: Int
  package var rightSpan: Int
  package var bottomSpan: Int
}

/// `$computeTableCellRectSpans(map, boundary)`.
package func computeTableCellRectSpans(
  _ map: TableMapType, _ boundary: TableCellRectBoundary
) throws -> TableCellRectSpans {
  var topSpan = 1
  var leftSpan = 1
  var rightSpan = 1
  var bottomSpan = 1
  let topRow = rowAt(map, boundary.minRow)
  let bottomRow = rowAt(map, boundary.maxRow)
  var col = boundary.minColumn
  while col <= boundary.maxColumn {
    topSpan = max(topSpan, try value(topRow, col).cell.__rowSpan)
    bottomSpan = max(bottomSpan, try value(bottomRow, col).cell.__rowSpan)
    col += 1
  }
  var row = boundary.minRow
  while row <= boundary.maxRow {
    leftSpan = max(leftSpan, try value(rowAt(map, row), boundary.minColumn).cell.__colSpan)
    rightSpan = max(rightSpan, try value(rowAt(map, row), boundary.maxColumn).cell.__colSpan)
    row += 1
  }
  return TableCellRectSpans(
    topSpan: topSpan, leftSpan: leftSpan, rightSpan: rightSpan, bottomSpan: bottomSpan)
}

/// `$computeTableCellRectBoundary(map, cellAMap, cellBMap)`: the bounding
/// rectangle of two cells, expanded until it holds every merged cell that
/// overlaps it.
public func computeTableCellRectBoundary(
  _ map: TableMapType, _ cellAMap: TableMapValueType, _ cellBMap: TableMapValueType
) throws -> TableCellRectBoundary {
  // Initial boundaries based on the anchor and focus cells
  var minColumn = min(cellAMap.startColumn, cellBMap.startColumn)
  var minRow = min(cellAMap.startRow, cellBMap.startRow)
  var maxColumn = max(
    cellAMap.startColumn + cellAMap.cell.__colSpan - 1,
    cellBMap.startColumn + cellBMap.cell.__colSpan - 1)
  var maxRow = max(
    cellAMap.startRow + cellAMap.cell.__rowSpan - 1,
    cellBMap.startRow + cellBMap.cell.__rowSpan - 1)

  // Keep expanding until we have a complete rectangle
  var hasChanges: Bool
  repeat {
    hasChanges = false

    // Check all cells in the table
    let columnCount = rowAt(map, 0)?.count ?? 0
    for row in 0..<map.count {
      for col in 0..<columnCount {
        guard let cell = at(map[row], col) else {
          continue
        }

        let cellEndCol = cell.startColumn + cell.cell.__colSpan - 1
        let cellEndRow = cell.startRow + cell.cell.__rowSpan - 1

        // Check if this cell intersects with our current selection rectangle
        let intersectsHorizontally = cell.startColumn <= maxColumn && cellEndCol >= minColumn
        let intersectsVertically = cell.startRow <= maxRow && cellEndRow >= minRow

        if intersectsHorizontally && intersectsVertically {
          // Expand boundaries to include this cell completely
          let newMinColumn = min(minColumn, cell.startColumn)
          let newMaxColumn = max(maxColumn, cellEndCol)
          let newMinRow = min(minRow, cell.startRow)
          let newMaxRow = max(maxRow, cellEndRow)

          if newMinColumn != minColumn || newMaxColumn != maxColumn || newMinRow != minRow
            || newMaxRow != maxRow
          {
            minColumn = newMinColumn
            maxColumn = newMaxColumn
            minRow = newMinRow
            maxRow = newMaxRow
            hasChanges = true
          }
        }
      }
    }
  } while hasChanges

  return TableCellRectBoundary(
    minColumn: minColumn, minRow: minRow, maxColumn: maxColumn, maxRow: maxRow)
}

/// `$isSimpleTable(table)`: whether the table has no merged cells.
public func isSimpleTable(_ table: TableNode) throws -> Bool {
  let rows = try table.getChildren()
  var columns: Int? = nil
  for row in rows {
    guard let row = row as? TableRowNode else {
      return false
    }
    if columns == nil {
      columns = try row.getChildrenSize()
    }
    if try row.getChildrenSize() != columns {
      return false
    }
    for cell in try row.getChildren() {
      guard let cell = cell as? TableCellNode, try cell.getRowSpan() == 1,
        try cell.getColSpan() == 1
      else {
        return false
      }
    }
  }
  return (columns ?? 0) > 0
}

/// `$moveTableColumn(tableNode, originColumn, targetColumn)`: moves a
/// column within a simple (non-merged) table.
public func moveTableColumn(
  _ tableNode: TableNode, _ originColumn: Int, _ targetColumn: Int
) throws {
  if originColumn == targetColumn {
    return
  }
  let columnCount = try tableNode.getColumnCount()
  if originColumn < 0 || originColumn >= columnCount || targetColumn < 0
    || targetColumn >= columnCount
  {
    return
  }
  if try !isSimpleTable(tableNode) {
    return
  }
  let rows = try tableNode.getChildren().compactMap { $0 as? TableRowNode }
  for row in rows {
    var cells = try row.getChildren()
    let moved = cells.remove(at: originColumn)
    cells.insert(moved, at: min(targetColumn, cells.count))
    try row.splice(0, cells.count, cells)
  }
  if let colWidths = try tableNode.getColWidths(), colWidths.count == columnCount {
    var newWidths = colWidths
    let movedWidth = newWidths.remove(at: originColumn)
    newWidths.insert(movedWidth, at: targetColumn)
    try tableNode.setColWidths(newWidths)
  }
}

/// `$moveTableRow(tableNode, originRow, targetRow)`: moves a row within a
/// simple (non-merged) table.
public func moveTableRow(_ tableNode: TableNode, _ originRow: Int, _ targetRow: Int) throws {
  if originRow == targetRow {
    return
  }
  let rows = try tableNode.getChildren().compactMap { $0 as? TableRowNode }
  let rowCount = rows.count
  if originRow < 0 || originRow >= rowCount || targetRow < 0 || targetRow >= rowCount {
    return
  }
  if try !isSimpleTable(tableNode) {
    return
  }
  let originRowNode = rows[originRow]
  let targetRowNode = rows[targetRow]
  if targetRow > originRow {
    try targetRowNode.insertAfter(originRowNode)
  } else {
    try targetRowNode.insertBefore(originRowNode)
  }
}

/// What `getTableCellNodeRect` returns.
public struct TableCellNodeRect: Equatable {
  public var rowIndex: Int
  public var columnIndex: Int
  public var rowSpan: Int
  public var colSpan: Int
}

/// `$getTableCellNodeRect(tableCellNode)`: where the cell starts in the
/// grid, and its spans.
public func getTableCellNodeRect(_ tableCellNode: TableCellNode) throws -> TableCellNodeRect? {
  let (cellNode, _, gridNode) = try getNodeTriplet(tableCellNode)
  let rows = try gridNode.getChildren().compactMap { $0 as? TableRowNode }
  let rowCount = rows.count
  guard let firstRow = rows.first else {
    throw LexicalError("Cannot read properties of undefined (reading 'getChildren')")
  }
  let columnCount = try firstRow.getChildren().count

  // A matrix of the same size as the table, to track the position of each
  // cell. JS's arrays grow past `columnCount`; a row past `rowCount` is
  // `undefined`, which a write throws on.
  var cellMatrix: [[TableCellNode?]] = Array(
    repeating: Array(repeating: nil, count: columnCount), count: rowCount)
  func occupied(_ row: Int, _ column: Int) -> Bool {
    column < cellMatrix[row].count && cellMatrix[row][column] != nil
  }

  for rowIndex in 0..<rowCount {
    let row = rows[rowIndex]
    let cells = try row.getChildren().compactMap { $0 as? TableCellNode }
    var columnIndex = 0

    for cell in cells {
      // Find the next available position in the matrix, skip the position
      // of merged cells
      while occupied(rowIndex, columnIndex) {
        columnIndex += 1
      }

      let rowSpan = cell.__rowSpan != 0 ? cell.__rowSpan : 1
      let colSpan = cell.__colSpan != 0 ? cell.__colSpan : 1

      // Put the cell into the corresponding position in the matrix
      for i in 0..<max(rowSpan, 0) {
        guard rowIndex + i < rowCount else {
          throw LexicalError("Cannot set properties of undefined")
        }
        for j in 0..<max(colSpan, 0) {
          let column = columnIndex + j
          while cellMatrix[rowIndex + i].count <= column { cellMatrix[rowIndex + i].append(nil) }
          cellMatrix[rowIndex + i][column] = cell
        }
      }

      // Return to the original index, row span and column span of the cell.
      if cellNode === cell {
        return TableCellNodeRect(
          rowIndex: rowIndex, columnIndex: columnIndex, rowSpan: rowSpan, colSpan: colSpan)
      }

      columnIndex += colSpan
    }
  }

  return nil
}

/// `$insertTableIntoGrid(tableNode, selection)` (internal): pastes the
/// cells of `tableNode` into the table `selection` is in, from the anchor
/// cell (or into the selected rectangle of a `TableSelection`), growing the
/// table where the template is larger.
///
/// Merged cells in the target area are unmerged, and template cells with
/// spans are merged into the target, as JS does.
@discardableResult
package func insertTableIntoGrid(
  _ tableNode: TableNode, _ selection: any BaseSelection
) throws
  -> Bool
{
  let isTableSel = selection is TableSelection

  guard let (anchor, focus) = selection.getStartEndPoints() else {
    return false
  }

  let (anchorCellNode, _, gridNode) = try getNodeTriplet(anchor)
  guard
    let focusCellNode = try findMatchingParent(focus.getNode(), { $0 is TableCellNode })
      as? TableCellNode
  else {
    return false
  }

  // Commands below assume a rectangular grid; repair a ragged one first.
  try padTableRows(gridNode)
  let (initialGridMap, anchorCellMap, focusCellMap) = try computeTableMap(
    gridNode, anchorCellNode, focusCellNode)
  let (templateGridMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)
  let initialRowCount = initialGridMap.count
  let initialColCount = initialRowCount > 0 ? initialGridMap[0].count : 0

  // If we have a range selection, we'll fit the template grid into the
  // table, growing the table if necessary.
  var startRow = anchorCellMap.startRow
  var startCol = anchorCellMap.startColumn
  var affectedRowCount = templateGridMap.count
  var affectedColCount = affectedRowCount > 0 ? templateGridMap[0].count : 0

  if isTableSel {
    let selectionBoundary = try computeTableCellRectBoundary(
      initialGridMap, anchorCellMap, focusCellMap)
    let selectionRowCount = selectionBoundary.maxRow - selectionBoundary.minRow + 1
    let selectionColCount = selectionBoundary.maxColumn - selectionBoundary.minColumn + 1
    startRow = selectionBoundary.minRow
    startCol = selectionBoundary.minColumn
    affectedRowCount = min(affectedRowCount, selectionRowCount)
    affectedColCount = min(affectedColCount, selectionColCount)
  }

  // Step 1: Unmerge all merged cells within the affected area
  var didPerformMergeOperations = false
  let lastRowForUnmerge = min(initialRowCount, startRow + affectedRowCount) - 1
  let lastColForUnmerge = min(initialColCount, startCol + affectedColCount) - 1
  var unmergedKeys = Set<NodeKey>()
  if startRow <= lastRowForUnmerge && startCol <= lastColForUnmerge {
    for row in startRow...lastRowForUnmerge {
      for col in startCol...lastColForUnmerge {
        let cellMap = try value(initialGridMap[row], col)
        if unmergedKeys.contains(cellMap.cell.getKey()) {
          continue  // cell was a merged cell that was already handled
        }
        if cellMap.cell.__rowSpan == 1 && cellMap.cell.__colSpan == 1 {
          continue  // cell is not a merged cell
        }
        try unmergeCellNode(cellMap.cell)
        unmergedKeys.insert(cellMap.cell.getKey())
        didPerformMergeOperations = true
      }
    }
  }

  var (interimGridMap, _, _) = try computeTableMapSkipCellCheck(
    gridNode.getWritable(), nil, nil)

  // Step 2: Expand current table (if needed)
  let rowsToInsert = affectedRowCount - initialRowCount + startRow
  for _ in 0..<max(rowsToInsert, 0) {
    let cellMap = try value(rowAt(interimGridMap, initialRowCount - 1), 0)
    try insertTableRowAtNode(cellMap.cell)
  }
  let colsToInsert = affectedColCount - initialColCount + startCol
  for _ in 0..<max(colsToInsert, 0) {
    let cellMap = try value(rowAt(interimGridMap, 0), initialColCount - 1)
    try insertTableColumnAtNode(cellMap.cell, true, false)
  }

  (interimGridMap, _, _) = try computeTableMapSkipCellCheck(gridNode.getWritable(), nil, nil)

  // Step 3: Merge cells and set cell content, to match template grid
  for row in startRow..<(startRow + max(affectedRowCount, 0)) {
    for col in startCol..<(startCol + max(affectedColCount, 0)) {
      let templateRow = row - startRow
      let templateCol = col - startCol
      let templateCellMap = try value(rowAt(templateGridMap, templateRow), templateCol)
      if templateCellMap.startRow != templateRow || templateCellMap.startColumn != templateCol {
        continue  // cell is a merged cell that was already handled
      }

      let templateCell = templateCellMap.cell
      if templateCell.__rowSpan != 1 || templateCell.__colSpan != 1 {
        var cellsToMerge: [TableCellNode] = []
        let lastRowForMerge = min(row + templateCell.__rowSpan, startRow + affectedRowCount) - 1
        let lastColForMerge = min(col + templateCell.__colSpan, startCol + affectedColCount) - 1
        for r in row...max(row, lastRowForMerge) where r <= lastRowForMerge {
          for c in col...max(col, lastColForMerge) where c <= lastColForMerge {
            cellsToMerge.append(try value(rowAt(interimGridMap, r), c).cell)
          }
        }
        try mergeCells(cellsToMerge)
        didPerformMergeOperations = true
      }

      let cell = try value(rowAt(interimGridMap, row), col).cell
      if let backgroundColor = try templateCell.getBackgroundColor() {
        try cell.setBackgroundColor(backgroundColor)
      }
      if let verticalAlign = try templateCell.getVerticalAlign() {
        try cell.setVerticalAlign(verticalAlign)
      }
      let originalChildren = try cell.getChildren()
      for child in try templateCell.getChildren() {
        if child is TextNode {
          let paragraphNode = try createParagraphNode()
          try paragraphNode.append(child)
          try cell.append(child)
        } else {
          try cell.append(child)
        }
      }
      for n in originalChildren {
        try n.remove()
      }
    }
  }

  if isTableSel && didPerformMergeOperations {
    // reset the table selection in case the anchor or focus cell was
    // removed via merge operations
    let (finalGridMap, _, _) = try computeTableMapSkipCellCheck(gridNode.getWritable(), nil, nil)
    let newAnchorCellMap = try value(
      rowAt(finalGridMap, anchorCellMap.startRow), anchorCellMap.startColumn)
    try newAnchorCellMap.cell.selectEnd()
  }

  return true
}

/// `$mergeCells(cellNodes)`: merges `cellNodes`, and every cell their
/// bounding rect covers, into its top-left cell, which it returns.
@discardableResult
public func mergeCells(_ cellNodes: [TableCellNode]) throws -> TableCellNode? {
  guard let first = cellNodes.first else {
    return nil
  }

  // Find the table node
  let tableNode = try getTableNodeFromLexicalNodeOrThrow(first)
  // Commands below assume a rectangular grid; repair a ragged one first.
  _ = try padTableRows(tableNode)
  let (gridMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)

  // Find the boundaries of the selection including merged cells
  var minRow = Int.max
  var maxRow = Int.min
  var minCol = Int.max
  var maxCol = Int.min

  // First pass: find the actual boundaries considering merged cells
  var processedCells = Set<NodeKey>()
  for row in gridMap {
    for case let mapCell? in row {
      let cellKey = mapCell.cell.getKey()
      if processedCells.contains(cellKey) {
        continue
      }

      if try cellNodes.contains(where: { try $0.is(mapCell.cell) }) {
        processedCells.insert(cellKey)
        // Get the actual position of this cell in the grid
        let cellStartRow = mapCell.startRow
        let cellStartCol = mapCell.startColumn
        let cellRowSpan = mapCell.cell.__rowSpan != 0 ? mapCell.cell.__rowSpan : 1
        let cellColSpan = mapCell.cell.__colSpan != 0 ? mapCell.cell.__colSpan : 1

        // Update boundaries considering the cell's actual position and span
        minRow = min(minRow, cellStartRow)
        maxRow = max(maxRow, cellStartRow + cellRowSpan - 1)
        minCol = min(minCol, cellStartCol)
        maxCol = max(maxCol, cellStartCol + cellColSpan - 1)
      }
    }
  }

  // Validate boundaries
  if minRow == Int.max || minCol == Int.max {
    return nil
  }

  // The total span of the merged cell
  let totalRowSpan = maxRow - minRow + 1
  let totalColSpan = maxCol - minCol + 1

  // Use the top-left cell as the target cell
  let targetCellMap = try value(rowAt(gridMap, minRow), minCol)
  let targetCell = targetCellMap.cell

  // Set the spans for the target cell
  try targetCell.setColSpan(totalColSpan)
  try targetCell.setRowSpan(totalRowSpan)

  // Move content from other cells to the target cell
  var seenCells: Set<NodeKey> = [targetCell.getKey()]

  // Second pass: merge content and remove other cells
  for row in minRow...maxRow {
    for col in minCol...maxCol {
      let mapCell = try value(rowAt(gridMap, row), col)
      let currentCell = mapCell.cell
      let key = currentCell.getKey()

      if !seenCells.contains(key) {
        seenCells.insert(key)
        let isEmpty = try cellContainsEmptyParagraph(currentCell)
        if !isEmpty {
          // The loop skips the target, so drop its own empty paragraph before
          // real content lands underneath it.
          if try cellContainsEmptyParagraph(targetCell) {
            try targetCell.clear()
          }
          try targetCell.append(contentsOf: currentCell.getChildren())
        }
        try currentCell.remove()
      }
    }
  }

  // Ensure target cell has content
  if try targetCell.getChildrenSize() == 0 {
    try targetCell.append(createParagraphNode())
  }

  return targetCell
}

private func cellContainsEmptyParagraph(_ cell: TableCellNode) throws -> Bool {
  if try cell.getChildrenSize() != 1 {
    return false
  }
  let firstChild = try cell.getFirstChildOrThrow()
  guard let paragraph = firstChild as? ParagraphNode, try paragraph.isEmpty() else {
    return false
  }
  return true
}

/// `$unmergeCell()`: unmerges the cell that holds the selection's anchor.
public func unmergeCell() throws {
  let selection = try getSelection()
  guard selection is RangeSelection || selection is TableSelection, let selection else {
    throw LexicalError("Expected a RangeSelection or TableSelection")
  }
  let anchor: LexicalNode
  if let range = selection as? RangeSelection {
    anchor = try range.anchor.getNode()
  } else {
    anchor = try (selection as! TableSelection).anchor.getNode()
  }
  guard let cellNode = try findMatchingParent(anchor, { $0 is TableCellNode }) as? TableCellNode
  else {
    throw LexicalError("Expected to find a parent TableCellNode")
  }
  return try unmergeCellNode(cellNode)
}

/// `$unmergeCellNode(cellNode)`: unmerges the given cell, splitting it back
/// into individual cells. Unlike `unmergeCell`, this does not depend on
/// the current selection. No-op if the cell is not merged.
public func unmergeCellNode(_ cellNode: TableCellNode) throws {
  let (cell, row, grid) = try getNodeTriplet(cellNode)
  let colSpan = cell.__colSpan
  let rowSpan = cell.__rowSpan
  if colSpan == 1 && rowSpan == 1 {
    return
  }
  // Commands below assume a rectangular grid; repair a ragged one first.
  _ = try padTableRows(grid)
  let (map, cellMap, _) = try computeTableMap(grid, cell, cell)
  let startColumn = cellMap.startColumn
  let startRow = cellMap.startRow
  // Create a heuristic for what the style of the unmerged cells should be
  // based on whether every row or column already had that state before the
  // unmerge.
  let baseColStyle = cell.__headerState & TableCellHeaderStates.COLUMN
  let colStyles = try (0..<colSpan).map { i in
    var colStyle = baseColStyle
    var rowIdx = 0
    while colStyle != 0 && rowIdx < map.count {
      colStyle &= try value(map[rowIdx], i + startColumn).cell.__headerState
      rowIdx += 1
    }
    return colStyle
  }
  let baseRowStyle = cell.__headerState & TableCellHeaderStates.ROW
  let rowStyles = try (0..<rowSpan).map { i in
    var rowStyle = baseRowStyle
    var colIdx = 0
    while rowStyle != 0 && colIdx < map[0].count {
      rowStyle &= try value(rowAt(map, i + startRow), colIdx).cell.__headerState
      colIdx += 1
    }
    return rowStyle
  }

  // The cells the merged cell splits into cover the region the merged cell
  // covered, so they keep its presentation: the background and vertical
  // align it carries as a TableCellNode, and the format, style, direction
  // and indent it carries as an ElementNode. `copyNode` carries all of it,
  // preserves a TableCellNode subclass, and returns the copy childless.
  // Only the state that describes a position rather than the cell is reset
  // — the spans, set by the loops below, the header state, recomputed
  // above, and the width, which measured a cell colSpan grid columns wide
  // and which every other path that creates a cell here leaves to the
  // table's colWidths.
  func createSplitCell(_ headerState: TableCellHeaderState) throws -> TableCellNode {
    try copyNode(cell)
      .setColSpan(1)
      .setRowSpan(1)
      .setHeaderStyles(headerState)
      .setWidth(nil)
      .append(createParagraphNode())
  }

  if colSpan > 1 {
    for i in stride(from: 1, to: colSpan, by: 1) {
      try cell.insertAfter(createSplitCell(colStyles[i] | rowStyles[0]))
    }
    try cell.setColSpan(1)
  }

  if rowSpan > 1 {
    var currentRowNode: LexicalNode? = nil
    for i in stride(from: 1, to: rowSpan, by: 1) {
      let currentRow = startRow + i
      let currentRowMap = rowAt(map, currentRow)
      currentRowNode = try (currentRowNode ?? row).getNextSibling()
      guard let rowNode = currentRowNode as? TableRowNode else {
        throw LexicalError("Expected row next sibling to be a row")
      }
      var insertAfterCell: TableCellNode? = nil
      var column = 0
      while column < startColumn {
        let currentCellMap = try value(currentRowMap, column)
        let currentCell = currentCellMap.cell
        if currentCellMap.startRow == currentRow {
          insertAfterCell = currentCell
        }
        if currentCell.__colSpan > 1 {
          column += currentCell.__colSpan - 1
        }
        column += 1
      }
      if let insertAfterCell {
        for j in stride(from: colSpan - 1, through: 0, by: -1) {
          try insertAfterCell.insertAfter(createSplitCell(colStyles[j] | rowStyles[i]))
        }
      } else {
        for j in stride(from: colSpan - 1, through: 0, by: -1) {
          try insertFirst(rowNode, createSplitCell(colStyles[j] | rowStyles[i]))
        }
      }
    }
    try cell.setRowSpan(1)
  }
}

/// `$setTableRowIsHeader(tableNode, rowIndex, isHeader)`: sets or clears
/// the ROW header bit of every cell in the row, keeping the COLUMN bit.
public func setTableRowIsHeader(_ tableNode: TableNode, _ rowIndex: Int, _ isHeader: Bool) throws {
  let (gridMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)
  guard rowIndex >= 0 && rowIndex < gridMap.count else {
    throw LexicalError("Table row index \(rowIndex) out of range")
  }
  let rowMap = gridMap[rowIndex]
  let headerState = isHeader ? TableCellHeaderStates.ROW : TableCellHeaderStates.NO_STATUS
  var visited = Set<ObjectIdentifier>()
  for mapCell in rowMap {
    guard let mapCell else {
      continue
    }
    if visited.insert(ObjectIdentifier(mapCell.cell)).inserted {
      try mapCell.cell.setHeaderStyles(headerState, TableCellHeaderStates.ROW)
    }
  }
}

/// `$setTableColumnIsHeader(tableNode, columnIndex, isHeader)`: sets or
/// clears the COLUMN header bit of every cell in the column, keeping the
/// ROW bit.
public func setTableColumnIsHeader(
  _ tableNode: TableNode, _ columnIndex: Int, _ isHeader: Bool
) throws {
  let (gridMap, _, _) = try computeTableMapSkipCellCheck(tableNode, nil, nil)
  guard !gridMap.isEmpty && columnIndex >= 0 && columnIndex < gridMap[0].count else {
    throw LexicalError("Table column index \(columnIndex) out of range")
  }
  let headerState = isHeader ? TableCellHeaderStates.COLUMN : TableCellHeaderStates.NO_STATUS
  var visited = Set<ObjectIdentifier>()
  for row in gridMap {
    guard let mapCell = at(row, columnIndex) else {
      continue
    }
    if visited.insert(ObjectIdentifier(mapCell.cell)).inserted {
      try mapCell.cell.setHeaderStyles(headerState, TableCellHeaderStates.COLUMN)
    }
  }
}
