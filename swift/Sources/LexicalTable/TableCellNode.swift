import Lexical

// Port of packages/lexical-table/src/LexicalTableCellNode.ts. The generated
// JSON implementations (LexicalTableGeneratedJSON.ts) are a JS speed-up of
// the schema walk and are not ported; the schema below drives both.

/// `TableCellHeaderState`: a mask of `TableCellHeaderStates`.
public typealias TableCellHeaderState = Int

/// `TableCellHeaderStates`.
public enum TableCellHeaderStates {
  public static let NO_STATUS: TableCellHeaderState = 0
  public static let ROW: TableCellHeaderState = 1
  public static let COLUMN: TableCellHeaderState = 2
  public static let BOTH: TableCellHeaderState = 3
}

/// `TableCellNode`: a `<td>`, or a `<th>` when it has a header state.
open class TableCellNode: ElementNode {
  package var __colSpan: Int = 1
  package var __rowSpan: Int = 1
  package var __headerState: TableCellHeaderState = TableCellHeaderStates.NO_STATUS
  package var __width: Double?
  package var __backgroundColor: String?
  package var __verticalAlign: String?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TableCellNode.self, type: "tablecell", extends: ElementNode.self,
      json: [
        // defaultAsNull preserves the legacy `backgroundColor || null`
        // semantics: an empty string means "no background", which exportDOM
        // checks for.
        SchemaField(
          "backgroundColor", nullable(stringValue(), defaultAsNull: true),
          field: \TableCellNode.__backgroundColor),
        // A span is a positive integer; 0 (the historical `|| 1` case), a
        // negative, or a fractional span is out of domain and falls back to 1.
        SchemaField(
          "colSpan", numberValue(1, integer: true, min: 1), field: \TableCellNode.__colSpan),
        // Read through getHeaderStyles and applied through setHeaderStyles
        // (with its default BOTH mask), whose bitwise operators read the
        // number as JS's ToInt32 does.
        SchemaField(
          "headerState", numberValue(Double(TableCellHeaderStates.NO_STATUS)),
          of: TableCellNode.self, get: { .int(try $0.getHeaderStyles()) },
          set: { try $0.setHeaderStyles(jsToInt32($1?.numberValue ?? 0)) }),
        SchemaField(
          "rowSpan", numberValue(1, integer: true, min: 1), field: \TableCellNode.__rowSpan),
        // The domain exportJSON already enforces via isValidVerticalAlign;
        // anything else (including the historical falsy `|| undefined` case)
        // is absent. `undefined` leads the list, so it is the default.
        SchemaField(
          "verticalAlign", enumValue([nil, .string("middle"), .string("bottom")]),
          of: TableCellNode.self,
          get: { try $0.getSerializedVerticalAlign().map(JSONValue.string) },
          set: { try $0.setVerticalAlign($1?.stringValue) }),
        // A width of 0 is not a real width, matching the historical
        // `serializedNode.width || undefined`.
        SchemaField(
          "width", optional(numberValue(), omitDefault: true), field: \TableCellNode.__width),
      ],
      children: .block)
  }

  open override class func importDOM() -> DOMConversionMap? {
    [
      "td": { _ in DOMConversion(priority: 0, convertTableCellNodeElement) },
      "th": { _ in DOMConversion(priority: 0, convertTableCellNodeElement) },
    ]
  }

  /// `new TableCellNode(headerState, colSpan, width, key)`.
  public convenience init(
    _ headerState: TableCellHeaderState = TableCellHeaderStates.NO_STATUS, colSpan: Int = 1,
    width: Double? = nil, key: NodeKey? = nil
  ) throws {
    try self.init(key: key)
    __colSpan = colSpan
    __rowSpan = 1
    __headerState = headerState
    __width = width
    __backgroundColor = nil
    __verticalAlign = nil
  }

  /// `headerState` and `verticalAlign` are accessor properties, which the
  /// schema does not carry across a clone.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prev = prevNode as! TableCellNode
    __colSpan = prev.__colSpan
    __rowSpan = prev.__rowSpan
    __headerState = prev.__headerState
    __width = prev.__width
    __backgroundColor = prev.__backgroundColor
    __verticalAlign = prev.__verticalAlign
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement(try getTag())

    if let width = __width, isTruthy(width) {
      element.style.setProperty("width", "\(JSONValue.number(width).stringify())px")
    }
    if __colSpan > 1 {
      setDOMColSpan(element, __colSpan)
    }
    if __rowSpan > 1 {
      setDOMRowSpan(element, __rowSpan)
    }
    if let backgroundColor = __backgroundColor {
      element.style.setProperty("background-color", backgroundColor)
    }
    if isValidVerticalAlign(__verticalAlign) {
      element.style.setProperty("vertical-align", __verticalAlign!)
    }

    addClassNamesToElement(
      element, config.theme["tableCell"]?.stringValue,
      try hasHeader() ? config.theme["tableCellHeader"]?.stringValue : nil)

    return element
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let output = try super.exportDOM(editor)

    if let element = output.element as? DOMElement {
      element.setAttribute("data-temporary-table-cell-lexical-key", getKey())
      element.style.setProperty("border", "1px solid black")
      if __colSpan > 1 {
        setDOMColSpan(element, __colSpan)
      }
      if __rowSpan > 1 {
        setDOMRowSpan(element, __rowSpan)
      }
      let width = try getWidth().flatMap { isTruthy($0) ? $0 : nil } ?? COLUMN_WIDTH
      element.style.setProperty("width", "\(JSONValue.number(width).stringify())px")

      let verticalAlign = try getVerticalAlign()
      element.style.setProperty(
        "vertical-align", verticalAlign.flatMap { $0.isEmpty ? nil : $0 } ?? "top")
      element.style.setProperty("text-align", "start")
      if __backgroundColor == nil, try hasHeader() {
        element.style.setProperty("background-color", "#f2f3f5")
      }
    }

    return output
  }

  open func getColSpan() throws -> Int {
    try getLatest().__colSpan
  }

  @discardableResult
  open func setColSpan(_ colSpan: Int) throws -> Self {
    let writable = try getWritable()
    writable.__colSpan = colSpan
    return writable
  }

  open func getRowSpan() throws -> Int {
    try getLatest().__rowSpan
  }

  @discardableResult
  open func setRowSpan(_ rowSpan: Int) throws -> Self {
    let writable = try getWritable()
    writable.__rowSpan = rowSpan
    return writable
  }

  /// `"th"` or `"td"`.
  open func getTag() throws -> String {
    try hasHeader() ? "th" : "td"
  }

  @discardableResult
  open func setHeaderStyles(
    _ headerState: TableCellHeaderState, _ mask: TableCellHeaderState = TableCellHeaderStates.BOTH
  ) throws -> Self {
    let writable = try getWritable()
    writable.__headerState = (headerState & mask) | (writable.__headerState & ~mask)
    return writable
  }

  open func getHeaderStyles() throws -> TableCellHeaderState {
    try getLatest().__headerState
  }

  @discardableResult
  open func setWidth(_ width: Double?) throws -> Self {
    let writable = try getWritable()
    writable.__width = width
    return writable
  }

  open func getWidth() throws -> Double? {
    try getLatest().__width
  }

  /// `getSerializedVerticalAlign()` (internal): the serialized
  /// `verticalAlign`, or `nil` to omit it.
  package func getSerializedVerticalAlign() throws -> String? {
    let verticalAlign = try getLatest().__verticalAlign
    return isValidVerticalAlign(verticalAlign) ? verticalAlign : nil
  }

  open func getBackgroundColor() throws -> String? {
    try getLatest().__backgroundColor
  }

  @discardableResult
  open func setBackgroundColor(_ newBackgroundColor: String?) throws -> Self {
    let writable = try getWritable()
    writable.__backgroundColor = newBackgroundColor
    return writable
  }

  open func getVerticalAlign() throws -> String? {
    try getLatest().__verticalAlign
  }

  @discardableResult
  open func setVerticalAlign(_ newVerticalAlign: String?) throws -> Self {
    let writable = try getWritable()
    writable.__verticalAlign = newVerticalAlign.flatMap { $0.isEmpty ? nil : $0 }
    return writable
  }

  @discardableResult
  open func toggleHeaderStyle(_ headerStateToToggle: TableCellHeaderState) throws -> Self {
    let writable = try getWritable()

    // The test is bitwise ("does it already have all of these bits") so the
    // mutation has to be too.
    if (writable.__headerState & headerStateToToggle) == headerStateToToggle {
      writable.__headerState &= ~headerStateToToggle
    } else {
      writable.__headerState |= headerStateToToggle
    }

    return writable
  }

  open func hasHeaderState(_ headerState: TableCellHeaderState) throws -> Bool {
    try (getHeaderStyles() & headerState) == headerState
  }

  open func hasHeader() throws -> Bool {
    try getLatest().__headerState != TableCellHeaderStates.NO_STATUS
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevNode = prevNode as! TableCellNode
    return prevNode.__headerState != __headerState || prevNode.__width != __width
      || prevNode.__colSpan != __colSpan || prevNode.__rowSpan != __rowSpan
      || prevNode.__backgroundColor != __backgroundColor
      || prevNode.__verticalAlign != __verticalAlign
  }

  open override func isShadowRoot() -> Bool { true }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool { true }

  open override func canBeEmpty() throws -> Bool { false }

  open override func canIndent() -> Bool { false }
}

/// A JS number's truthiness: neither 0 nor `NaN`.
private func isTruthy(_ value: Double) -> Bool { value != 0 && !value.isNaN }

package func isValidVerticalAlign(_ verticalAlign: String?) -> Bool {
  verticalAlign == "middle" || verticalAlign == "bottom"
}

/// The HTML "rules for parsing non-negative integers", as jsdom applies
/// them to a reflected attribute.
private func parseNonNegativeInteger(_ input: String) -> Int? {
  let trimmed = input.drop { " \t\n\u{0C}\r".contains($0) }
  // Only ASCII whitespace may lead; parseInt would skip any.
  if let first = trimmed.first, first.isWhitespace { return nil }
  guard let value = jsParseInt(String(trimmed)), value >= 0 else { return nil }
  return value
}

/// `HTMLTableCellElement.colSpan`: clamped to 1...1000, 1 by default.
package func domColSpan(_ element: DOMElement) -> Int {
  guard let value = element.getAttribute("colspan").flatMap(parseNonNegativeInteger) else {
    return 1
  }
  return min(max(value, 1), 1000)
}

/// `HTMLTableCellElement.rowSpan`: clamped to 0...65534, 1 by default.
package func domRowSpan(_ element: DOMElement) -> Int {
  guard let value = element.getAttribute("rowspan").flatMap(parseNonNegativeInteger) else {
    return 1
  }
  return min(value, 65534)
}

/// `element.colSpan = value`.
private func setDOMColSpan(_ element: DOMElement, _ value: Int) {
  element.setAttribute("colspan", String(value <= 2_147_483_647 && value >= 0 ? value : 1))
}

/// `element.rowSpan = value`.
private func setDOMRowSpan(_ element: DOMElement, _ value: Int) {
  element.setAttribute("rowspan", String(value <= 2_147_483_647 && value >= 0 ? value : 1))
}

/// `HTMLTableCellElement.cellIndex`: the index among the `<td>` and `<th>`
/// children of the nearest `<tr>`, or -1.
package func cellIndex(_ element: DOMElement) -> Int {
  var ancestor = element.parentElement
  while let tr = ancestor, tr.localName != "tr" { ancestor = tr.parentElement }
  guard let tr = ancestor else { return -1 }
  let cells = tr.children.filter { $0.localName == "td" || $0.localName == "th" }
  return cells.firstIndex { $0 === element } ?? -1
}

/// `HTMLTableRowElement.rowIndex`: the index in its table's `rows` (the
/// `<thead>` rows, then the `<tbody>` and direct ones, then the `<tfoot>`
/// ones), or -1.
package func rowIndex(_ row: DOMElement) -> Int {
  guard let parent = row.parentElement else { return -1 }
  var table: DOMElement? = parent
  if ["thead", "tbody", "tfoot"].contains(parent.localName) {
    table = parent.parentElement
  }
  guard let table, table.localName == "table" else { return -1 }
  var header: [DOMElement] = []
  var body: [DOMElement] = []
  var footer: [DOMElement] = []
  for child in table.children {
    let rows = child.children.filter { $0.localName == "tr" }
    switch child.localName {
    case "thead": header += rows
    case "tbody": body += rows
    case "tfoot": footer += rows
    case "tr": body.append(child)
    default: break
    }
  }
  return (header + body + footer).firstIndex { $0 === row } ?? -1
}

/// `$convertTableCellNodeElement(domNode)`.
package func convertTableCellNodeElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let domNode_ = domNode as? DOMElement else { return nil }
  let nodeName = domNode.nodeName.lowercased()

  var width: Double? = nil

  if let match = try? PIXEL_VALUE_REG_EXP.wholeMatch(in: domNode_.style["width"]) {
    width = Double(match.1)
  }

  // Determine header state based on the 'scope' attribute
  var headerState = TableCellHeaderStates.NO_STATUS

  if nodeName == "th" {
    let scope = domNode_.getAttribute("scope")
    if scope == "col" {
      headerState = TableCellHeaderStates.COLUMN
    } else if scope == "row" {
      headerState = TableCellHeaderStates.ROW
    } else {
      let parentRow = domNode_.parentElement
      var isInHeaderRow = false
      if let parentRow, parentRow.nodeName.lowercased() == "tr",
        let section = parentRow.parentElement
      {
        isInHeaderRow = section.nodeName.lowercased() == "thead" || rowIndex(parentRow) == 0
      }
      let isFirstColumn = cellIndex(domNode_) == 0

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

  let tableCellNode = try createTableCellNode(
    headerState, colSpan: domColSpan(domNode_), width: width)

  tableCellNode.__rowSpan = domRowSpan(domNode_)
  let backgroundColor = domNode_.style["background-color"]
  if backgroundColor != "" {
    tableCellNode.__backgroundColor = backgroundColor
  }
  let verticalAlign = domNode_.style["vertical-align"]
  if isValidVerticalAlign(verticalAlign) {
    tableCellNode.__verticalAlign = verticalAlign
  }
  try setDirectionFromDOM(tableCellNode, domNode_)

  let style = domNode_.style
  let textDecoration = style["text-decoration"].split(
    separator: " ", omittingEmptySubsequences: false)
  let hasBoldFontWeight = style["font-weight"] == "700" || style["font-weight"] == "bold"
  let hasLinethroughTextDecoration = textDecoration.contains("line-through")
  let hasItalicFontStyle = style["font-style"] == "italic"
  let hasUnderlineTextDecoration = textDecoration.contains("underline")
  let color = style["color"]
  return DOMConversionOutput(
    node: tableCellNode,
    after: { childLexicalNodes in
      var result: [LexicalNode] = []
      var paragraphNode: ParagraphNode? = nil

      func removeSingleLineBreakNode() throws {
        if let paragraphNode {
          let firstChild = try paragraphNode.getFirstChild()
          if let firstChild = firstChild as? LineBreakNode,
            try paragraphNode.getChildrenSize() == 1
          {
            try firstChild.remove()
          }
        }
      }

      for child in childLexicalNodes {
        if try isInlineElementOrDecoratorNode(child) || child is TextNode
          || child is LineBreakNode
        {
          if let child = child as? TextNode {
            if hasBoldFontWeight {
              try child.toggleFormat("bold")
            }
            if hasLinethroughTextDecoration {
              try child.toggleFormat("strikethrough")
            }
            if hasItalicFontStyle {
              try child.toggleFormat("italic")
            }
            if hasUnderlineTextDecoration {
              try child.toggleFormat("underline")
            }
            if !color.isEmpty {
              let existingStyle = try child.getStyle()
              if !existingStyle.contains("color:") {
                try child.setStyle(existingStyle + "color: \(color);")
              }
            }
          }

          if let paragraphNode {
            try paragraphNode.append(child)
          } else {
            let paragraph = try createParagraphNode().append(child)
            paragraphNode = paragraph
            result.append(paragraph)
          }
        } else {
          result.append(child)
          try removeSingleLineBreakNode()
          paragraphNode = nil
        }
      }

      try removeSingleLineBreakNode()

      if result.isEmpty {
        result.append(try createParagraphNode())
      }
      return result
    })
}

/// `$createTableCellNode(headerState, colSpan, width)`.
public func createTableCellNode(
  _ headerState: TableCellHeaderState = TableCellHeaderStates.NO_STATUS, colSpan: Int = 1,
  width: Double? = nil
) throws -> TableCellNode {
  try applyNodeReplacement(TableCellNode(headerState, colSpan: colSpan, width: width))
}

/// `$isTableCellNode(node)`.
public func isTableCellNode(_ node: LexicalNode?) -> Bool { node is TableCellNode }
