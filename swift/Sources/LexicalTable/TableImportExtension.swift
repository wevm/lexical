import Lexical
import LexicalHTML
import LexicalUtils

// Port of packages/lexical-table/src/TableImportExtension.ts: the
// `DOMImportExtension` rules for tables (`@experimental` upstream), which
// `TableExtension` contributes.

/// Bitmask of TextNode format bits implied by a `<th>` / `<td>`'s inline
/// styles (`font-weight: bold`, `font-style: italic`, and `underline` /
/// `line-through` in `text-decoration`).
private func cellTextFormatMask(_ style: CSSStyleDeclaration) -> Int {
  var mask = 0
  let fontWeight = style["font-weight"]
  if fontWeight == "700" || fontWeight == "bold" {
    mask |= IS_BOLD
  }
  if style["font-style"] == "italic" {
    mask |= IS_ITALIC
  }
  let decoration = style["text-decoration"].split(separator: " ", omittingEmptySubsequences: false)
  if decoration.contains("underline") {
    mask |= IS_UNDERLINE
  }
  if decoration.contains("line-through") {
    mask |= IS_STRIKETHROUGH
  }
  return mask
}

/// Coalesces inline and line-break runs inside a `<td>`/`<th>` into their
/// own `ParagraphNode`s, leaving block children (paragraphs, nested tables,
/// block decorators) in place as their own siblings, and drops a sole
/// leading `<br>` as the legacy `removeSingleLineBreakNode` did.
private func packageCellChildren(_ children: [LexicalNode]) throws -> [LexicalNode] {
  var result: [LexicalNode] = []
  var paragraph: ParagraphNode? = nil

  func flushSingleLineBreak() throws {
    if let paragraph {
      let first = try paragraph.getFirstChild()
      if first is LineBreakNode, try paragraph.getChildrenSize() == 1 {
        try first!.remove()
      }
    }
  }

  for child in children {
    if try isInlineElementOrDecoratorNode(child) || child is TextNode || child is LineBreakNode {
      if let paragraph {
        try paragraph.append(child)
      } else {
        let newParagraph = try createParagraphNode().append(child)
        paragraph = newParagraph
        result.append(newParagraph)
      }
    } else {
      // Block children start their own sibling; the inline run being
      // accumulated is closed off here.
      try flushSingleLineBreak()
      paragraph = nil
      result.append(child)
    }
  }
  try flushSingleLineBreak()
  if result.isEmpty {
    result.append(try createParagraphNode())
  }
  return result
}

nonisolated(unsafe) private let TableRule = defineImportRule(
  name: "@lexical/table/table", match: sel.tag("table")
) { ctx, el, _ in
  let node = try createTableNode()
  if el.hasAttribute("data-lexical-row-striping") {
    try node.setRowStriping(true)
  }
  if el.hasAttribute("data-lexical-frozen-column") {
    try node.setFrozenColumns(1)
  }
  if el.hasAttribute("data-lexical-frozen-row") {
    try node.setFrozenRows(1)
  }
  if let colGroup = childElement(el, "colgroup") {
    var columns: [Double]? = []
    for col in colGroup.children where col.localName == "col" {
      var width = col.style["width"]
      if !isPixelValue(width) {
        width = col.getAttribute("width") ?? ""
        if width.isEmpty || !width.allSatisfy({ $0.isASCII && $0.isNumber }) {
          columns = nil
          break
        }
      }
      columns!.append(jsParseFloat(width))
    }
    if let columns {
      try node.setColWidths(columns)
    }
  }
  return [
    try node.splice(
      0, 0, descendantsMatching(ctx.importChildren(el)) { $0 is TableRowNode })
  ]
}

nonisolated(unsafe) private let TableRowRule = defineImportRule(
  name: "@lexical/table/tr", match: sel.tag("tr")
) { ctx, el, _ in
  let height = isPixelValue(el.style["height"]) ? jsParseFloat(el.style["height"]) : nil
  return [
    try createTableRowNode(height).splice(
      0, 0, descendantsMatching(ctx.importChildren(el)) { $0 is TableCellNode })
  ]
}

nonisolated(unsafe) private let TableCellRule = defineImportRule(
  name: "@lexical/table/cell", match: sel.tag("td", "th")
) { ctx, el, _ in
  let isHeader = el.nodeName == "TH"
  let width = isPixelValue(el.style["width"]) ? jsParseFloat(el.style["width"]) : nil
  var headerState = TableCellHeaderStates.NO_STATUS
  if isHeader {
    let scope = el.getAttribute("scope")
    if scope == "col" {
      headerState = TableCellHeaderStates.COLUMN
    } else if scope == "row" {
      headerState = TableCellHeaderStates.ROW
    } else {
      var isInHeaderRow = false
      if let parentRow = el.parentElement, parentRow.localName == "tr" {
        isInHeaderRow =
          parentRow.parentElement?.nodeName == "THEAD" || rowIndex(parentRow) == 0
      }
      let isFirstColumn = cellIndex(el) == 0
      if isInHeaderRow {
        headerState |= TableCellHeaderStates.ROW
      }
      if isFirstColumn {
        headerState |= TableCellHeaderStates.COLUMN
      }
      if headerState == TableCellHeaderStates.NO_STATUS {
        headerState = TableCellHeaderStates.ROW
      }
    }
  }
  let cell = try createTableCellNode(headerState, colSpan: domColSpan(el), width: width)
  cell.__rowSpan = domRowSpan(el)
  let backgroundColor = el.style["background-color"]
  if backgroundColor != "" {
    cell.__backgroundColor = backgroundColor
  }
  let verticalAlign = el.style["vertical-align"]
  if isValidVerticalAlign(verticalAlign) {
    cell.__verticalAlign = verticalAlign
  }
  // Propagate the cell's bold/italic/underline/strikethrough as format
  // bits, and the cell's `color` as a parsed-style record, which the core
  // `#text` rule applies to each TextNode.
  let inheritedFormat = try ctx.get(ImportTextFormat)
  let cellFormat = inheritedFormat | cellTextFormatMask(el.style)
  let color = el.style["color"]
  var branchContext: [ContextPairOrUpdater] = []
  if cellFormat != inheritedFormat {
    branchContext.append(contextValue(ImportTextFormat, cellFormat))
  }
  if !color.isEmpty {
    var cellStyle = try ctx.get(ImportTextStyle)
    cellStyle["color"] = color
    branchContext.append(contextValue(ImportTextStyle, cellStyle))
  }
  // Each `ParagraphNode` child (from a real `<p>`, or a lowered `<div>`)
  // stays its own sibling paragraph.
  let packaged = try packageCellChildren(
    ctx.importChildren(el, ImportChildrenOpts(context: branchContext)))
  // Only `<td>` propagates `text-align` onto its block children, as the
  // legacy `wrapContinuousInlines` ran only for block DOM nodes.
  let children = try isHeader ? packaged : propagateTextAlignToBlockChildren(packaged, el)
  return [try cell.splice(0, 0, children)]
}

/// `TableSchema` (`@experimental`): a `ChildSchema` that accepts only
/// `TableRowNode` children; orphan `TableCellNode` runs are wrapped in a
/// synthesized row.
nonisolated(unsafe) public let TableSchema = ChildSchema(
  name: "TableSchema", accepts: { child, _ in child is TableRowNode },
  packageRun: { run, _, _ in
    run.allSatisfy { $0 is TableCellNode } ? [try createTableRowNode().splice(0, 0, run)] : []
  })

/// `TableRowSchema` (`@experimental`): a `ChildSchema` that accepts only
/// `TableCellNode` children; others are dropped.
nonisolated(unsafe) public let TableRowSchema = ChildSchema(
  name: "TableRowSchema", accepts: { child, _ in child is TableCellNode })

/// `TableImportRules` (`@experimental`): the import rules for `TableNode`,
/// `TableRowNode` and `TableCellNode`, which `TableExtension` registers
/// with `CoreImportExtension`.
nonisolated(unsafe) public let TableImportRules = [TableRule, TableRowRule, TableCellRule]
