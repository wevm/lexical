import Foundation
import Lexical

#if canImport(CoreGraphics)
  import CoreGraphics
#endif

// A table's grid as both text views lay it out (DESIGN.md, The table view):
// where each cell goes, read from the nodes' JSON as TextKit reads list
// items, so the views need not depend on `LexicalTable`, and the frames the
// playground's CSS gives the cells. The views only measure the cells' text
// and place their own views at these frames.

/// One cell's place in the grid.
package struct TableGridCell: Equatable {
  package var key: NodeKey
  package var row: Int
  package var column: Int
  package var rowSpan: Int
  package var columnSpan: Int
  /// `TableCellHeaderStates`: 1 (`ROW`) in a header row, so it heads its
  /// column; 2 (`COLUMN`) in a header column, so it heads its row.
  package var headerState: Int
  package var background: TextKitColor?

  /// Whether the cell is a header cell. Setting it to `true` on a cell that
  /// heads nothing makes it head its column (`ROW`).
  package var isHeader: Bool {
    get { headerState != 0 }
    set { headerState = newValue ? (headerState == 0 ? 1 : headerState) : 0 }
  }
  /// Whether the cell heads its column (it is in a header row).
  package var headsColumn: Bool { headerState & 1 != 0 }
  /// Whether the cell heads its row (it is in a header column).
  package var headsRow: Bool { headerState & 2 != 0 }

  /// Whether the cell covers `row` and `column`.
  package func covers(row: Int, column: Int) -> Bool {
    (self.row..<self.row + rowSpan).contains(row)
      && (self.column..<self.column + columnSpan).contains(column)
  }
}

/// The shape of a table, as its nodes' JSON describes it.
package struct TableGridModel: Equatable {
  package var cells: [TableGridCell] = []
  package var rowCount = 0
  package var columnCount = 0
  package var columnWidths: [Double] = []
  package var rowStriping = false

  package init() {}

  /// Reads the table at `key` (call it in a read). Cells are placed as
  /// `$computeTableMap` places them: each at the next column its row
  /// leaves free, spans taking the columns and rows they cover.
  package init?(table key: NodeKey) throws {
    guard let table = try getNodeByKey(key) as? ElementNode else { return nil }
    let json = try table.exportJSON()
    rowStriping = json["rowStriping"]?.boolValue ?? false
    var taken: [[Bool]] = []
    for (r, row) in try table.getChildren().enumerated() {
      guard let row = row as? ElementNode else { continue }
      if taken.count <= r { taken.append([]) }
      var column = 0
      for cell in try row.getChildren() {
        guard let cell = cell as? ElementNode else { continue }
        while column < taken[r].count, taken[r][column] { column += 1 }
        let cellJSON = try cell.exportJSON()
        let rowSpan = max(1, Int(cellJSON["rowSpan"]?.numberValue ?? 1))
        let columnSpan = max(1, Int(cellJSON["colSpan"]?.numberValue ?? 1))
        for dr in 0..<rowSpan {
          while taken.count <= r + dr { taken.append([]) }
          for dc in 0..<columnSpan {
            while taken[r + dr].count <= column + dc { taken[r + dr].append(false) }
            taken[r + dr][column + dc] = true
          }
        }
        cells.append(
          TableGridCell(
            key: cell.getKey(), row: r, column: column, rowSpan: rowSpan,
            columnSpan: columnSpan,
            headerState: Int(cellJSON["headerState"]?.numberValue ?? 0),
            background: cellJSON["backgroundColor"]?.stringValue.flatMap {
              TextKitColor(css: $0)
            }))
        column += columnSpan
      }
    }
    rowCount = taken.count
    columnCount = taken.map(\.count).max() ?? 0
    columnWidths = json["colWidths"]?.arrayValue?.compactMap(\.numberValue) ?? []
  }

  /// The table's, its rows' and its cells' node objects in `state`, which
  /// are the same while none of them changed, so the model is read again
  /// only when one did.
  package static func nodes(of key: NodeKey, in state: EditorState) -> [ObjectIdentifier] {
    let nodeMap = state._nodeMap
    guard let table = nodeMap.get(key) as? ElementNode else { return [] }
    var nodes = [ObjectIdentifier(table)]
    var row = table.__first
    while let rowKey = row, let rowNode = nodeMap.get(rowKey) as? ElementNode {
      nodes.append(ObjectIdentifier(rowNode))
      var cell = rowNode.__first
      while let cellKey = cell, let cellNode = nodeMap.get(cellKey) {
        nodes.append(ObjectIdentifier(cellNode))
        cell = cellNode.__next
      }
      row = rowNode.__next
    }
    return nodes
  }

  /// The cell that covers `row` and `column`, if any.
  package func cell(row: Int, column: Int) -> TableGridCell? {
    cells.first { $0.covers(row: row, column: column) }
  }

  /// A cell's background: its own color, else a header's, else a stripe on
  /// every other row's first cell, as the playground's CSS paints them.
  package func background(of cell: TableGridCell, theme: TextKitTable) -> TextKitColor? {
    cell.background
      ?? (cell.isHeader
        ? theme.headerBackground
        : rowStriping && cell.row % 2 == 1 && cell.column == 0 ? theme.stripeBackground : nil)
  }
}

/// Where a table's cells go, as the playground's CSS places them
/// (`table-layout: fixed`, `width: fit-content`): each column as wide as
/// its `colWidths` entry or the theme's default, each row as tall as its
/// tallest cell, a cell over several rows growing the last of them to fit.
package struct TableGridLayout: Equatable {
  /// Each cell's border box, from the grid's top left corner.
  package var cellFrames: [NodeKey: CGRect] = [:]
  /// Each cell's text, inside its padding and border.
  package var textFrames: [NodeKey: CGRect] = [:]
  /// The grid's size, its outer border included.
  package var size = CGSize.zero

  package init() {}

  /// Lays `model` out in `theme`, with `textHeight` the height of a cell's
  /// text at a width.
  package init(
    _ model: TableGridModel, theme: TextKitTable,
    textHeight: (TableGridCell, CGFloat) -> CGFloat
  ) {
    var x: [CGFloat] = [0]
    for column in 0..<model.columnCount {
      let width =
        column < model.columnWidths.count ? model.columnWidths[column] : theme.columnWidth
      x.append(x[column] + CGFloat(width))
    }
    let padH = CGFloat(theme.cellPaddingHorizontal)
    let padV = CGFloat(theme.cellPaddingVertical)
    let border = CGFloat(theme.borderWidth)
    func textWidth(_ cell: TableGridCell) -> CGFloat {
      max(1, x[cell.column + cell.columnSpan] - x[cell.column] - 2 * padH - border)
    }
    var heights: [NodeKey: CGFloat] = [:]
    for cell in model.cells {
      heights[cell.key] = textHeight(cell, textWidth(cell)) + 2 * padV + border
    }
    var rows = [CGFloat](repeating: 0, count: model.rowCount)
    for cell in model.cells where cell.rowSpan == 1 {
      rows[cell.row] = max(rows[cell.row], heights[cell.key] ?? 0)
    }
    for cell in model.cells where cell.rowSpan > 1 {
      let need = heights[cell.key] ?? 0
      let have = rows[cell.row..<(cell.row + cell.rowSpan)].reduce(0, +)
      if need > have { rows[cell.row + cell.rowSpan - 1] += need - have }
    }
    var y: [CGFloat] = [0]
    for height in rows { y.append(y.last! + height) }
    for cell in model.cells {
      let frame = CGRect(
        x: x[cell.column], y: y[cell.row],
        width: x[cell.column + cell.columnSpan] - x[cell.column],
        height: y[cell.row + cell.rowSpan] - y[cell.row])
      cellFrames[cell.key] = frame
      textFrames[cell.key] = CGRect(
        x: frame.minX + padH + border / 2, y: frame.minY + padV + border / 2,
        width: textWidth(cell), height: max(1, frame.height - 2 * padV - border))
    }
    size = CGSize(width: x.last! + border, height: y.last! + border)
  }
}
