import Lexical

// Port of packages/lexical-table/src/LexicalTableSelection.ts.

/// `TableSelectionShape`.
public struct TableSelectionShape: Equatable {
  public var fromX: Int
  public var fromY: Int
  public var toX: Int
  public var toY: Int
}

/// `TableMapValueType`: a cell and where it starts in the grid.
public struct TableMapValueType {
  public var cell: TableCellNode
  public var startRow: Int
  public var startColumn: Int
}

/// `TableMapType`: each grid position's cell, row by row. A `nil` is a
/// hole of the JS sparse array, where no cell covers the position.
public typealias TableMapType = [[TableMapValueType?]]

/// The `TableMapValueType` at `[row][column]`: what JS reads as
/// `map[row][column]`, which throws where JS reads a property of
/// `undefined`.
package func mapValue(_ map: TableMapType, _ row: Int, _ column: Int) throws -> TableMapValueType {
  guard row >= 0, row < map.count, column >= 0, column < map[row].count,
    let value = map[row][column]
  else {
    throw LexicalError("Cannot read properties of undefined (reading 'cell')")
  }
  return value
}

/// What `$getCellNodes` finds for the anchor and the focus.
private struct CellNodes {
  var anchorCell: TableCellNode
  var anchorNode: LexicalNode
  var anchorRow: TableRowNode
  var anchorTable: TableNode
  var focusCell: TableCellNode
  var focusNode: LexicalNode
  var focusRow: TableRowNode
  var focusTable: TableNode
}

private func getCellNodes(_ tableSelection: TableSelection) throws -> CellNodes {
  func find(
    _ k: String, _ point: Point
  ) throws -> (
    LexicalNode, TableCellNode, TableRowNode, TableNode
  ) {
    let node = try point.getNode()
    guard let cellNode = try findMatchingParent(node, { $0 is TableCellNode }) as? TableCellNode
    else {
      throw LexicalError(
        "Expected TableSelection \(k) to be (or a child of) TableCellNode, got key \(node.getKey()) of type \(node.getType())"
      )
    }
    guard let rowNode = try cellNode.getParent() as? TableRowNode else {
      throw LexicalError("Expected TableSelection \(k) cell parent to be a TableRowNode")
    }
    guard let tableNode = try rowNode.getParent() as? TableNode else {
      throw LexicalError("Expected TableSelection \(k) row parent to be a TableNode")
    }
    return (node, cellNode, rowNode, tableNode)
  }
  let (anchorNode, anchorCell, anchorRow, anchorTable) = try find(
    "anchor", tableSelection.anchor)
  let (focusNode, focusCell, focusRow, focusTable) = try find("focus", tableSelection.focus)
  // TODO: nested tables may violate this
  guard anchorTable.is(focusTable) else {
    throw LexicalError("Expected TableSelection anchor and focus to be in the same table")
  }
  return CellNodes(
    anchorCell: anchorCell, anchorNode: anchorNode, anchorRow: anchorRow,
    anchorTable: anchorTable, focusCell: focusCell, focusNode: focusNode, focusRow: focusRow,
    focusTable: focusTable)
}

/// `TableSelection`: a rectangle of cells, from the anchor cell to the
/// focus cell of one table.
public final class TableSelection: BaseSelection {
  public var tableKey: NodeKey
  public let anchor: Point
  public let focus: Point
  package var _cachedNodes: [LexicalNode]?
  public var dirty: Bool

  public init(_ tableKey: NodeKey, _ anchor: Point, _ focus: Point) {
    self.anchor = anchor
    self.focus = focus
    self._cachedNodes = nil
    self.dirty = false
    self.tableKey = tableKey
    anchor._selection = self
    focus._selection = self
  }

  public func getStartEndPoints() -> (Point, Point)? {
    (anchor, focus)
  }

  /// `createTableSelection` unfortunately makes it very easy to create
  /// nonsense selections, so this checks whether the selection probably
  /// makes sense.
  public func isValid() throws -> Bool {
    if tableKey == "root" || anchor.key == "root" || anchor.type != .element
      || focus.key == "root" || focus.type != .element
    {
      return false
    }

    // Check if the referenced nodes still exist in the editor
    let tableNode = try getNodeByKey(tableKey)
    let anchorNode = try getNodeByKey(anchor.key)
    let focusNode = try getNodeByKey(focus.key)

    return tableNode != nil && anchorNode != nil && focusNode != nil
  }

  /// Whether the focus logically precedes the anchor in the EditorState.
  public func isBackward() throws -> Bool {
    try focus.isBefore(anchor)
  }

  public func getCachedNodes() -> [LexicalNode]? { _cachedNodes }

  public func setCachedNodes(_ nodes: [LexicalNode]?) { _cachedNodes = nodes }

  public func `is`(_ selection: (any BaseSelection)?) -> Bool {
    guard let selection = selection as? TableSelection else { return false }
    return tableKey == selection.tableKey && anchor.is(selection.anchor)
      && focus.is(selection.focus)
  }

  public func set(_ tableKey: NodeKey, _ anchorCellKey: NodeKey, _ focusCellKey: NodeKey) {
    dirty =
      dirty || tableKey != self.tableKey || anchorCellKey != anchor.key
      || focusCellKey != focus.key
    self.tableKey = tableKey
    anchor.key = anchorCellKey
    focus.key = focusCellKey
    _cachedNodes = nil
  }

  public func clone() -> any BaseSelection { cloneTable() }

  /// `clone()`, typed.
  public func cloneTable() -> TableSelection {
    TableSelection(
      tableKey, createPoint(anchor.key, anchor.offset, anchor.type),
      createPoint(focus.key, focus.offset, focus.type))
  }

  public func isCollapsed() -> Bool { false }

  public func extract() throws -> [LexicalNode] {
    try getNodes()
  }

  public func insertRawText(_ text: String) throws {
    if text.isEmpty {
      return
    }
    let trimmed = text.hasSuffix("\n") ? String(text.utf16.dropLast())! : text
    let tsvGrid = trimmed.split(separator: "\n", omittingEmptySubsequences: false).map {
      $0.split(separator: "\t", omittingEmptySubsequences: false)
    }
    let tableNode = try createTableNode()
    for row in tsvGrid {
      let rowNode = try createTableRowNode()
      for cellText in row {
        let cellNode = try createTableCellNode(TableCellHeaderStates.NO_STATUS)
        let paragraph = try createParagraphNode()
        if !cellText.isEmpty {
          try paragraph.append(createTextNode(String(cellText)))
        }
        try cellNode.append(paragraph)
        try rowNode.append(cellNode)
      }
      try tableNode.append(rowNode)
    }
    let anchorCell = try getCellNodes(self).anchorCell
    let rangeSelection = try anchorCell.select(0, anchorCell.getChildrenSize())
    _ = try insertTableIntoGrid(tableNode, rangeSelection)
  }

  public func insertText(_ text: String) throws {
    // Do nothing?
  }

  /// Whether `type` is on the selection: whether any paragraph in its
  /// table cells has the format.
  public func hasFormat(_ type: TextFormatType) throws -> Bool {
    var format = 0

    for cellNode in try getNodes() where cellNode is TableCellNode {
      if let paragraph = try (cellNode as! TableCellNode).getFirstChild() as? ParagraphNode {
        format |= try paragraph.getTextFormat()
      }
    }

    let formatFlag = TEXT_TYPE_TO_FORMAT[type] ?? 0
    return (format & formatFlag) != 0
  }

  public func insertNodes(_ nodes: [LexicalNode]) throws {
    guard let focusNode = try focus.getNode() as? ElementNode else {
      throw LexicalError("Expected TableSelection focus to be an ElementNode")
    }
    let selection = try normalizeSelection(focusNode.select(0, focusNode.getChildrenSize()))
    try selection.insertNodes(nodes)
  }

  /// Deprecated: use `computeTableMap` and `computeTableCellRectBoundary`
  /// directly.
  public func getShape() throws -> TableSelectionShape {
    let cells = try getCellNodes(self)
    guard let anchorCellNodeRect = try getTableCellNodeRect(cells.anchorCell) else {
      throw LexicalError("getCellRect: expected to find AnchorNode")
    }
    guard let focusCellNodeRect = try getTableCellNodeRect(cells.focusCell) else {
      throw LexicalError("getCellRect: expected to find focusCellNode")
    }

    let startX = min(anchorCellNodeRect.columnIndex, focusCellNodeRect.columnIndex)
    let stopX = max(
      anchorCellNodeRect.columnIndex + anchorCellNodeRect.colSpan - 1,
      focusCellNodeRect.columnIndex + focusCellNodeRect.colSpan - 1)

    let startY = min(anchorCellNodeRect.rowIndex, focusCellNodeRect.rowIndex)
    let stopY = max(
      anchorCellNodeRect.rowIndex + anchorCellNodeRect.rowSpan - 1,
      focusCellNodeRect.rowIndex + focusCellNodeRect.rowSpan - 1)

    return TableSelectionShape(
      fromX: min(startX, stopX), fromY: min(startY, stopY), toX: max(startX, stopX),
      toY: max(startY, stopY))
  }

  public func getNodes() throws -> [LexicalNode] {
    if try !isValid() {
      return []
    }
    if let cachedNodes = _cachedNodes {
      return cachedNodes
    }

    let cells = try getCellNodes(self)
    let tableNode = cells.anchorTable
    let anchorCell = cells.anchorCell
    let focusCell = cells.focusCell

    let focusCellGrid = try focusCell.getParents()[1]
    if focusCellGrid !== tableNode {
      if try !tableNode.isParentOf(focusCell) {
        // focus is on higher Grid level than anchor
        guard let gridParent = try tableNode.getParent() else {
          throw LexicalError("Expected gridParent to have a parent")
        }
        set(tableKey, gridParent.getKey(), focusCell.getKey())
      } else {
        // anchor is on higher Grid level than focus
        guard let focusCellParent = try focusCellGrid.getParent() else {
          throw LexicalError("Expected focusCellParent to have a parent")
        }
        set(tableKey, focusCell.getKey(), focusCellParent.getKey())
      }
      return try getNodes()
    }

    let (map, cellAMap, cellBMap) = try computeTableMap(tableNode, anchorCell, focusCell)
    let boundary = try computeTableCellRectBoundary(map, cellAMap, cellBMap)

    // An ordered map, because merged cells in the grid would otherwise show
    // up multiple times in the nodes array
    let nodeMap = JSMap<NodeKey, LexicalNode>([(tableNode.getKey(), tableNode)])
    var lastRow: TableRowNode? = nil
    if boundary.minRow <= boundary.maxRow, boundary.minColumn <= boundary.maxColumn {
      for i in boundary.minRow...boundary.maxRow {
        for j in boundary.minColumn...boundary.maxColumn {
          let cell = try mapValue(map, i, j).cell
          guard let currentRow = try cell.getParent() as? TableRowNode else {
            throw LexicalError("Expected TableCellNode parent to be a TableRowNode")
          }
          if currentRow !== lastRow {
            nodeMap.set(currentRow.getKey(), currentRow)
            lastRow = currentRow
          }
          if !nodeMap.has(cell.getKey()) {
            try visitRecursively(cell) { childNode in
              nodeMap.set(childNode.getKey(), childNode)
              return true
            }
          }
        }
      }
    }
    let nodes = nodeMap.map { $0.1 }

    if !isCurrentlyReadOnlyMode() {
      _cachedNodes = nodes
    }
    return nodes
  }

  public func getTextContent() throws -> String {
    let nodes = try getNodes().filter { $0 is TableCellNode }
    var textContent = ""
    for i in 0..<nodes.count {
      let node = nodes[i]
      let row = node.__parent
      let nextRow = i + 1 < nodes.count ? nodes[i + 1].__parent : nil
      textContent += try node.getTextContent() + (nextRow != row ? "\n" : "\t")
    }
    return textContent
  }
}

/// `$isTableSelection(x)`.
public func isTableSelection(_ x: Any?) -> Bool { x is TableSelection }

/// `$createTableSelection()`. Deprecated: use `createTableSelectionFrom`.
public func createTableSelection() -> TableSelection {
  let anchor = createPoint("root", 0, .element)
  let focus = createPoint("root", 0, .element)
  return TableSelection("root", anchor, focus)
}

/// `$createTableSelectionFrom(tableNode, anchorCell, focusCell)`: a
/// `TableSelection` from `anchorCell` to `focusCell` in `tableNode`, which
/// must be attached and hold both. When the current selection is a
/// `TableSelection`, it is cloned and re-targeted.
public func createTableSelectionFrom(
  _ tableNode: TableNode, _ anchorCell: TableCellNode, _ focusCell: TableCellNode
) throws -> TableSelection {
  let tableNodeKey = tableNode.getKey()
  let anchorCellKey = anchorCell.getKey()
  let focusCellKey = focusCell.getKey()
  guard try tableNode.isAttached() else {
    throw LexicalError("$createTableSelectionFrom: tableNode \(tableNodeKey) is not attached")
  }
  guard try tableNode.is(findTableNode(anchorCell)) else {
    throw LexicalError(
      "$createTableSelectionFrom: anchorCell \(anchorCellKey) is not in table \(tableNodeKey)")
  }
  guard try tableNode.is(findTableNode(focusCell)) else {
    throw LexicalError(
      "$createTableSelectionFrom: focusCell \(focusCellKey) is not in table \(tableNodeKey)")
  }
  // TODO: Check for rectangular grid
  let prevSelection = try getSelection()
  let nextSelection =
    (prevSelection as? TableSelection)?.cloneTable()
    ?? TableSelection(
      "root", createPoint("root", 0, .element), createPoint("root", 0, .element))
  nextSelection.set(tableNode.getKey(), anchorCell.getKey(), focusCell.getKey())
  return nextSelection
}

/// `$visitRecursively(node, $visit)`: depth first. When `visit` returns
/// `false`, the children of that node are not explored.
package func visitRecursively(
  _ node: LexicalNode, _ visit: (LexicalNode) throws -> Bool
) throws {
  var stack: [[LexicalNode]] = [[node]]
  while !stack.isEmpty {
    if let currentNode = stack[stack.count - 1].popLast() {
      if try visit(currentNode), let element = currentNode as? ElementNode {
        stack.append(try element.getChildren())
      }
    } else {
      stack.removeLast()
    }
  }
}
