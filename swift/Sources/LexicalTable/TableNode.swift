import Lexical
import LexicalExtension
import LexicalUtils
import Synchronization

// Port of packages/lexical-table/src/LexicalTableNode.ts. The generated
// JSON implementations (LexicalTableGeneratedJSON.ts) are not ported; the
// schema below drives both.
//
// Not ported here: the sticky scrollbar's listeners and layout reads
// (`attachStickyScrollbarListeners`, `syncStickyScrollbar`,
// `findStickyScrollbarElements`), which belong to the views. The methods
// that take a `TableDOMTable` (`getCordsFromCellNode`, `getDOMCellFromCords`,
// `getCellNodeFromCords` and their `OrThrow` forms) are in
// TableObserver.swift, next to `TableDOMTable`, and read its grid of cell
// keys rather than DOM elements. `setDOMUnmanaged` marks a DOM node for the DOM
// reconciler, which the views replace, so the `<colgroup>` and the sticky
// scrollbar are not marked.

private func isHTMLDivElement(_ element: DOMNode?) -> Bool {
  (element as? DOMElement)?.nodeName == "DIV"
}

/// The first element child of `dom` named `localName` (`:scope > name`).
package func childElement(_ dom: DOMElement, _ localName: String) -> DOMElement? {
  dom.children.first { $0.localName == localName }
}

/// A JS number's truthiness: neither 0 nor `NaN`.
private func isTruthy(_ value: Double) -> Bool { value != 0 && !value.isNaN }

/// A JS string's truthiness.
private func themeClasses(_ value: JSONValue?) -> String? {
  guard let classes = value?.stringValue, !classes.isEmpty else { return nil }
  return classes
}

/// `TableNode`: a `<table>`, holding table rows, or the scrollable wrapper
/// around it when scrollable tables are active.
open class TableNode: ElementNode {
  package var __rowStriping: Bool = false
  package var __frozenColumnCount: Int = 0
  package var __frozenRowCount: Int = 0
  package var __colWidths: [Double]? = nil

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TableNode.self, type: "table", extends: ElementNode.self,
      json: [
        // Read straight off the field; applied through setColWidths.
        SchemaField(
          "colWidths", optional(arrayValue(numberValue())), of: TableNode.self,
          get: { node in node.__colWidths.map { .array($0.map(JSONValue.number)) } },
          set: { node, value in
            try node.setColWidths(value?.arrayValue.map { $0.map { $0.numberValue ?? 0 } })
          }),
        // The getters stay methods: each normalizes a falsy value to
        // `undefined`, so a table with no striping and no frozen rows or
        // columns writes none of them. The setters are the methods that
        // write the fields, so that a subclass that overrides one decides.
        SchemaField(
          "frozenColumnCount", numberValue(), of: TableNode.self,
          get: { try $0.getSerializedFrozenColumnCount().map(JSONValue.int) },
          set: { try $0.setFrozenColumns(clampedInt($1?.numberValue ?? 0)) }),
        SchemaField(
          "frozenRowCount", numberValue(), of: TableNode.self,
          get: { try $0.getSerializedFrozenRowCount().map(JSONValue.int) },
          set: { try $0.setFrozenRows(clampedInt($1?.numberValue ?? 0)) }),
        SchemaField(
          "rowStriping", booleanValue(), of: TableNode.self,
          get: { try $0.getSerializedRowStriping().map(JSONValue.bool) },
          set: { try $0.setRowStriping($1?.boolValue ?? false) }),
      ])
  }

  open override class func importDOM() -> DOMConversionMap? {
    ["table": { _ in DOMConversion(priority: 1, convertTableElement) }]
  }

  /// The schema's properties are all accessors, which it does not carry
  /// across a clone.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prev = prevNode as! TableNode
    __rowStriping = prev.__rowStriping
    __frozenColumnCount = prev.__frozenColumnCount
    __frozenRowCount = prev.__frozenRowCount
    __colWidths = prev.__colWidths
  }

  /// `getSerializedFrozenColumnCount()` (internal): these three are
  /// omitted from the JSON when falsy.
  package func getSerializedFrozenColumnCount() throws -> Int? {
    let count = try getLatest().__frozenColumnCount
    return count != 0 ? count : nil
  }

  /// `getSerializedFrozenRowCount()` (internal).
  package func getSerializedFrozenRowCount() throws -> Int? {
    let count = try getLatest().__frozenRowCount
    return count != 0 ? count : nil
  }

  /// `getSerializedRowStriping()` (internal).
  package func getSerializedRowStriping() throws -> Bool? {
    try getLatest().__rowStriping ? true : nil
  }

  open func getColWidths() throws -> [Double]? {
    try getLatest().__colWidths
  }

  /// Values are immutable, so there is nothing to freeze, as JS does in DEV.
  @discardableResult
  open func setColWidths(_ colWidths: [Double]?) throws -> Self {
    let writable = try getWritable()
    writable.__colWidths = colWidths
    return writable
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    destination == "html"
  }

  /// `getDOMSlot(element)`: the `<table>` (inside the wrapper, when there
  /// is one), after its `<colgroup>`. JS throws the invariant when
  /// `createDOM` did not make a table; this method can't throw, so it stops.
  open override func getDOMSlot(_ element: DOMElement) -> DOMSlot {
    let tableElement =
      !isHTMLTableElement(element) ? element.getElementsByTagName("table").first : element
    guard let tableElement, isHTMLTableElement(tableElement) else {
      preconditionFailure("TableNode.getDOMSlot: createDOM() did not return a table")
    }
    return super.getDOMSlot(element).withElement(tableElement)
      .withAfter(childElement(tableElement, "colgroup"))
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let tableElement = getDocument().createElement("table")
    if !__style.isEmpty {
      setDOMStyleFromCSS(tableElement.style, __style)
    }
    let colWidths = try getColWidths()
    if colWidths != nil {
      let colGroup = getDocument().createElement("colgroup")
      tableElement.appendChild(colGroup)
    }
    addClassNamesToElement(tableElement, config.theme["table"]?.stringValue)
    try updateTableElement(nil, tableElement, config)
    if try isScrollableTablesActive(editor) {
      let hasStickyScrollbar = try isStickyScrollbarActive(editor)
      let scrollableWrapper = createScrollableWrapper(tableElement, config, hasStickyScrollbar)
      updateTableWrapper(nil, scrollableWrapper, tableElement, config)
      if hasStickyScrollbar {
        let stickyScrollbar = createStickyScrollbar(config)
        let outerWrapper = getDocument().createElement("div")
        outerWrapper.setAttribute("data-lexical-sticky-scrollbar", "true")
        outerWrapper.appendChild(scrollableWrapper)
        outerWrapper.appendChild(stickyScrollbar)
        return outerWrapper
      }
      return scrollableWrapper
    }
    return tableElement
  }

  open func updateTableWrapper(
    _ prevNode: TableNode?, _ tableWrapper: DOMElement, _ tableElement: DOMElement,
    _ config: EditorConfig
  ) {
    if __frozenColumnCount != (prevNode?.__frozenColumnCount ?? 0) {
      LexicalTable.setFrozenColumns(tableWrapper, tableElement, config, __frozenColumnCount)
    }
    if __frozenRowCount != (prevNode?.__frozenRowCount ?? 0) {
      LexicalTable.setFrozenRows(tableWrapper, tableElement, config, __frozenRowCount)
    }
  }

  open func updateTableElement(
    _ prevNode: TableNode?, _ tableElement: DOMElement, _ config: EditorConfig
  ) throws {
    if __style != (prevNode?.__style ?? "") {
      setDOMStyleFromCSS(tableElement.style, __style, prevNode?.__style ?? "")
    }
    if __rowStriping != (prevNode?.__rowStriping ?? false) {
      LexicalTable.setRowStriping(tableElement, config, __rowStriping)
    }
    let prevColCount = try prevNode?.getColumnCount() ?? 0
    let prevColWidths = prevNode?.__colWidths
    // JS compares the frozen arrays by identity; values compare by content.
    if try getColumnCount() != prevColCount || getColWidths() != prevColWidths {
      try updateColgroup(tableElement, getColumnCount(), getColWidths())
    }
    try alignTableElement(tableElement, config, getFormatType())
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevNode = prevNode as! TableNode
    let tableElement = try getTableElement(self, dom)
    if (dom === tableElement) == (try isScrollableTablesActive()) {
      return true
    }
    if isHTMLDivElement(dom) {
      let hasStickyDom = dom.hasAttribute("data-lexical-sticky-scrollbar")
      if try hasStickyDom != isStickyScrollbarActive() {
        return true
      }
      // The scrollable wrapper is the table's immediate parent in both
      // layouts (the inner div in sticky mode, dom itself otherwise).
      if let scrollable = tableElement.parentElement, isHTMLDivElement(scrollable) {
        updateTableWrapper(prevNode, scrollable, tableElement, config)
      }
    }
    try updateTableElement(prevNode, tableElement, config)
    return false
  }

  open func scaleDOMColWidths(_ dom: DOMElement, _ scale: Double) throws {
    guard let colWidths = try getColWidths() else {
      return
    }
    let tableElement = try getTableElement(self, dom)
    try updateColgroup(tableElement, getColumnCount(), colWidths.map { $0 * scale })
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let superExport = try super.exportDOM(editor)
    let element = superExport.element
    let exportedElement: DOMNode? =
      !isHTMLTableElement(element) && element is DOMElement
      ? (element as! DOMElement).getElementsByTagName("table").first : element
    // ElementNode.exportDOM writes the direction and the indent onto
    // whatever createDOM returned, which is the scroll wrapper (or the
    // sticky scrollbar wrapper around it) when scrollable tables are active.
    // Those wrappers are not part of the export, so move what they were
    // given onto the <table> itself.
    if let element = element as? DOMElement, let exportedElement = exportedElement as? DOMElement,
      isHTMLTableElement(exportedElement), exportedElement !== element
    {
      if !element.dir.isEmpty {
        exportedElement.dir = element.dir
      }
      let paddingInlineStart = element.style["padding-inline-start"]
      if !paddingInlineStart.isEmpty {
        exportedElement.style.setProperty("padding-inline-start", paddingInlineStart)
      }
      if let indent = element.getAttribute("data-lexical-indent") {
        exportedElement.setAttribute("data-lexical-indent", indent)
      }
    }
    return DOMExportOutput(
      element: exportedElement,
      after: { [self] tableElement in
        var tableElement = tableElement
        if let after = superExport.after {
          tableElement = try after(tableElement)
        }
        if !isHTMLTableElement(tableElement), let element = tableElement as? DOMElement {
          tableElement = element.getElementsByTagName("table").first
        }
        guard let tableElement = tableElement as? DOMElement, isHTMLTableElement(tableElement)
        else {
          return nil
        }
        try alignTableElement(tableElement, editor._config, getFormatType())

        // Scan the table map to build a map of table cell key to the
        // columns it needs
        let (tableMap, _, _) = try computeTableMapSkipCellCheck(self, nil, nil)
        var cellValues: [NodeKey: (startColumn: Int, colSpan: Int)] = [:]
        for mapRow in tableMap {
          for mapValue in mapRow {
            guard let mapValue else {
              // JS reads `.cell` of the sparse array's hole.
              throw LexicalError("Cannot read properties of undefined (reading 'cell')")
            }
            let key = mapValue.cell.getKey()
            if cellValues[key] == nil {
              cellValues[key] = (
                startColumn: mapValue.startColumn, colSpan: try mapValue.cell.getColSpan()
              )
            }
          }
        }

        // scan the DOM to find the table cell keys that were used and mark
        // those columns
        var knownColumns = Set<Int>()
        for row in tableElement.children where row.localName == "tr" {
          for cellDOM in row.children
          where cellDOM.hasAttribute("data-temporary-table-cell-lexical-key") {
            if let key = cellDOM.getAttribute("data-temporary-table-cell-lexical-key"),
              !key.isEmpty
            {
              let cellSpan = cellValues[key]
              cellDOM.removeAttribute("data-temporary-table-cell-lexical-key")
              if let cellSpan {
                cellValues[key] = nil
                for i in 0..<max(cellSpan.colSpan, 0) {
                  knownColumns.insert(i + cellSpan.startColumn)
                }
              }
            }
          }
        }

        // Compute the colgroup and columns in the export
        if let colGroup = childElement(tableElement, "colgroup") {
          // Only include the <col /> for rows that are in the output
          let cols = colGroup.children.filter { $0.localName == "col" }.enumerated()
            .filter { knownColumns.contains($0.offset) }.map { $0.element as DOMNode }
          colGroup.replaceChildren(cols)
        }

        // Wrap direct descendant rows in a tbody for export
        let rows = tableElement.children.filter { $0.localName == "tr" }
        if !rows.isEmpty {
          let tBody = getDocument().createElement("tbody")
          for row in rows {
            tBody.appendChild(row)
          }
          tableElement.append(tBody)
        }
        return tableElement
      })
  }

  open override func canBeEmpty() throws -> Bool { false }

  open override func isShadowRoot() -> Bool { true }

  open func getRowStriping() throws -> Bool {
    try getLatest().__rowStriping
  }

  @discardableResult
  open func setRowStriping(_ newRowStriping: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__rowStriping = newRowStriping
    return writable
  }

  @discardableResult
  open func setFrozenColumns(_ columnCount: Int) throws -> Self {
    let writable = try getWritable()
    writable.__frozenColumnCount = columnCount
    return writable
  }

  open func getFrozenColumns() throws -> Int {
    try getLatest().__frozenColumnCount
  }

  @discardableResult
  open func setFrozenRows(_ rowCount: Int) throws -> Self {
    let writable = try getWritable()
    writable.__frozenRowCount = rowCount
    return writable
  }

  open func getFrozenRows() throws -> Int {
    try getLatest().__frozenRowCount
  }

  open func canSelectBefore() -> Bool { true }

  open override func canIndent() -> Bool { false }

  open func getColumnCount() throws -> Int {
    guard let firstRow = try getFirstChild() as? TableRowNode else {
      return 0
    }

    var columnCount = 0
    for cell in try firstRow.getChildren() {
      if let cell = cell as? TableCellNode {
        columnCount += try cell.getColSpan()
      }
    }

    return columnCount
  }
}

/// `$updateColgroup(dom, colCount, colWidths)`.
private func updateColgroup(_ dom: DOMElement, _ colCount: Int, _ colWidths: [Double]?) {
  var colGroup = childElement(dom, "colgroup")
  guard let colWidths else {
    colGroup?.remove()
    return
  }
  if colGroup == nil {
    let created = getDocument().createElement("colgroup")
    dom.insertBefore(created, dom.firstChild)
    colGroup = created
  }
  var cols: [DOMNode] = []
  for i in 0..<max(colCount, 0) {
    let col = getDocument().createElement("col")
    if i < colWidths.count, isTruthy(colWidths[i]) {
      col.style.setProperty("width", "\(JSONValue.number(colWidths[i]).stringify())px")
    }
    cols.append(col)
  }
  colGroup!.replaceChildren(cols)
}

private func setRowStriping(_ dom: DOMElement, _ config: EditorConfig, _ rowStriping: Bool) {
  if rowStriping {
    addClassNamesToElement(dom, config.theme["tableRowStriping"]?.stringValue)
    dom.setAttribute("data-lexical-row-striping", "true")
  } else {
    removeClassNamesFromElement(dom, config.theme["tableRowStriping"]?.stringValue)
    dom.removeAttribute("data-lexical-row-striping")
  }
}

private func setFrozenColumns(
  _ dom: DOMElement, _ tableElement: DOMElement, _ config: EditorConfig, _ frozenColumnCount: Int
) {
  if frozenColumnCount > 0 {
    addClassNamesToElement(dom, config.theme["tableFrozenColumn"]?.stringValue)
    tableElement.setAttribute("data-lexical-frozen-column", "true")
  } else {
    removeClassNamesFromElement(dom, config.theme["tableFrozenColumn"]?.stringValue)
    tableElement.removeAttribute("data-lexical-frozen-column")
  }
}

private func setFrozenRows(
  _ dom: DOMElement, _ tableElement: DOMElement, _ config: EditorConfig, _ frozenRowCount: Int
) {
  if frozenRowCount > 0 {
    addClassNamesToElement(dom, config.theme["tableFrozenRow"]?.stringValue)
    tableElement.setAttribute("data-lexical-frozen-row", "true")
  } else {
    removeClassNamesFromElement(dom, config.theme["tableFrozenRow"]?.stringValue)
    tableElement.removeAttribute("data-lexical-frozen-row")
  }
}

private func alignTableElement(
  _ dom: DOMElement, _ config: EditorConfig, _ formatType: ElementFormatType
) {
  guard let tableAlignment = config.theme["tableAlignment"], tableAlignment != .null,
    tableAlignment != .bool(false), tableAlignment != .string("")
  else {
    return
  }
  var removeClasses: [String?] = []
  var addClasses: [String?] = []
  for format in ["center", "right"] {
    guard let classes = themeClasses(tableAlignment.objectValue?[format] ?? nil) else {
      continue
    }
    if format == formatType {
      addClasses.append(classes)
    } else {
      removeClasses.append(classes)
    }
  }
  let toRemove = normalizeClassNames(contentsOf: removeClasses)
  if !toRemove.isEmpty { dom.classList.remove(contentsOf: toRemove) }
  let toAdd = normalizeClassNames(contentsOf: addClasses)
  if !toAdd.isEmpty { dom.classList.add(contentsOf: toAdd) }
}

/// `$createScrollableWrapper(tableElement, config, hideNativeScrollbar)`.
private func createScrollableWrapper(
  _ tableElement: DOMElement, _ config: EditorConfig, _ hideNativeScrollbar: Bool
) -> DOMElement {
  let wrapper = getDocument().createElement("div")
  if let classes = themeClasses(config.theme["tableScrollableWrapper"]) {
    addClassNamesToElement(wrapper, classes)
  } else {
    wrapper.style.setProperty("overflow-x", "auto")
  }
  if hideNativeScrollbar {
    wrapper.style.setProperty("scrollbar-width", "none")
  }
  wrapper.appendChild(tableElement)
  return wrapper
}

/// `$createStickyScrollbar(config)`.
private func createStickyScrollbar(_ config: EditorConfig) -> DOMElement {
  let doc = getDocument()
  let scrollbar = doc.createElement("div")
  if let classes = themeClasses(config.theme["tableStickyScrollbar"]) {
    addClassNamesToElement(scrollbar, classes)
  } else {
    scrollbar.style.setProperty("position", "sticky")
    scrollbar.style.setProperty("bottom", "0")
    scrollbar.style.setProperty("overflow-x", "scroll")
    scrollbar.style.setProperty("overflow-y", "hidden")
  }
  scrollbar.style.setProperty("display", "none")
  scrollbar.setAttribute("aria-hidden", "true")
  // `scrollbar.tabIndex = -1`.
  scrollbar.setAttribute("tabindex", "-1")
  let spacer = doc.createElement("div")
  spacer.style.setProperty("height", "1px")
  spacer.style.setProperty("width", "0px")
  scrollbar.appendChild(spacer)
  return scrollbar
}

/// A weak reference to an editor, for `scrollableEditors`.
private struct WeakEditor: @unchecked Sendable {
  weak var editor: Editor?
}

/// `scrollableEditors`: JS's `WeakSet<LexicalEditor>`.
private let scrollableEditors = Mutex<[ObjectIdentifier: WeakEditor]>([:])

/// `$isScrollableTablesActive(editor)`: whether tables in `editor` (the
/// active editor by default) render inside their scrollable wrapper.
public func isScrollableTablesActive(_ editor: Editor? = nil) throws -> Bool {
  let editor = try editor ?? getEditor()
  return scrollableEditors.withLock { $0[ObjectIdentifier(editor)]?.editor === editor }
}

/// `$isStickyScrollbarActive(editor)`: whether the editor's
/// `@lexical/table/Table` peer (`TableExtension`) has both
/// `hasStickyScrollbar` and `hasHorizontalScroll`, read with `peek()` so a
/// reconcile inside an effect does not subscribe it.
public func isStickyScrollbarActive(_ editor: Editor? = nil) throws -> Bool {
  let editor = try editor ?? getEditor()
  guard
    let dep = try getPeerDependencyFromEditor(
      editor, "@lexical/table/Table", as: type(of: TableExtension))
  else { return false }
  return dep.output.hasStickyScrollbar.peek() && dep.output.hasHorizontalScroll.peek()
}

/// `setScrollableTablesActive(editor, active)`.
public func setScrollableTablesActive(_ editor: Editor, _ active: Bool) {
  if active {
    if editor._config.theme["tableScrollableWrapper"] == nil {
      consoleWarn(
        "TableNode: hasHorizontalScroll is active but theme.tableScrollableWrapper is not defined."
      )
    }
    scrollableEditors.withLock { editors in
      editors = editors.filter { $0.value.editor != nil }
      editors[ObjectIdentifier(editor)] = WeakEditor(editor: editor)
    }
  } else {
    scrollableEditors.withLock { $0[ObjectIdentifier(editor)] = nil }
  }
}

/// `$convertTableElement(domNode)`.
package func convertTableElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let domNode = domNode as? DOMElement else { return nil }
  let tableNode = try createTableNode()
  if domNode.hasAttribute("data-lexical-row-striping") {
    try tableNode.setRowStriping(true)
  }
  if domNode.hasAttribute("data-lexical-frozen-column") {
    try tableNode.setFrozenColumns(1)
  }
  if domNode.hasAttribute("data-lexical-frozen-row") {
    try tableNode.setFrozenRows(1)
  }
  if let colGroup = childElement(domNode, "colgroup") {
    var columns: [Double]? = []
    for col in colGroup.children where col.localName == "col" {
      var width = col.style["width"]
      if !isPixelValue(width) {
        // Also support deprecated width attribute for google docs
        width = col.getAttribute("width") ?? ""
        if width.isEmpty || !width.allSatisfy({ $0.isASCII && $0.isNumber }) {
          columns = nil
          break
        }
      }
      columns!.append(jsParseFloat(width))
    }
    if let columns {
      try tableNode.setColWidths(columns)
    }
  }
  try setDirectionFromDOM(tableNode, domNode)
  return DOMConversionOutput(
    node: tableNode,
    after: { children in try descendantsMatching(children, TableRowNode.self) })
}

/// `parseFloat` of a `PIXEL_VALUE_REG_EXP` match or of an all-digit string.
package func jsParseFloat(_ value: String) -> Double {
  if let match = try? PIXEL_VALUE_REG_EXP.wholeMatch(in: value) {
    return Double(match.1)!
  }
  return Double(value)!
}

/// `$createTableNode()`.
public func createTableNode() throws -> TableNode {
  try applyNodeReplacement(TableNode())
}

/// `$isTableNode(node)`.
public func isTableNode(_ node: LexicalNode?) -> Bool { node is TableNode }
