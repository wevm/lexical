import Lexical
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of the parts of
// packages/lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx that
// need only the nodes. Upstream mounts a React editor with
// `<TablePlugin hasHorizontalScroll>`; the nodes read that setting through
// `$isScrollableTablesActive`, so these tests set it with
// `setScrollableTablesActive`, which is what the plugin does. Upstream's
// `TableNode.createDOM() and updateDOM() style` compares the reconciled DOM;
// here it compares each table's `createDOM`, as the list tests do.

private var theme: EditorThemeClasses {
  [
    "table": "test-table-class",
    "tableAlignment": [
      "center": "test-table-alignment-center",
      "right": "test-table-alignment-right",
    ],
    "tableFrozenColumn": "test-table-frozen-column-class",
    "tableFrozenRow": "test-table-frozen-row-class",
    "tableRowStriping": "test-table-row-striping-class",
    "tableScrollableWrapper": "table-scrollable-wrapper",
  ]
}

private var editorConfig: EditorConfig { EditorConfig(namespace: "", theme: theme) }

/// `wrapTableHtml(expected)`.
private func wrapTableHtml(_ expected: String) -> String {
  expected.replacing(/<table( dir="(?:auto|ltr|rtl)")?/) { match in
    "<div\(match.1 ?? "") class=\"table-scrollable-wrapper\"><table"
  }.replacing("</table>", with: "</table></div>")
}

/// `expectTableHtmlToBeEqual(actual, expected)`.
private func expectTableHtml(
  _ actual: String, _ expected: String, _ hasHorizontalScroll: Bool,
  sourceLocation: SourceLocation = #_sourceLocation
) {
  #expect(
    prettifyHTML(actual)
      == prettifyHTML(hasHorizontalScroll ? wrapTableHtml(expected) : expected),
    sourceLocation: sourceLocation)
}

/// The `initializeUnitTest` editor with the `beforeEach` paragraph, and
/// scrollable tables as `TablePlugin` sets them.
private func makeEditor(_ hasHorizontalScroll: Bool) throws -> Editor {
  let editor = try createMountedTestEditor(nodes: tableTestNodes, theme: theme)
  setScrollableTablesActive(editor, hasHorizontalScroll)
  try editor.update(discrete: true) {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    try paragraph.select()
  }
  return editor
}

private func lastTable() throws -> TableNode {
  try #require(getRoot().getLastChild() as? TableNode)
}

/// Sorted keys, for comparing JSON as `toEqual` does.
private func canonical(_ value: JSONValue) -> JSONValue {
  switch value {
  case .array(let items): return .array(items.map(canonical))
  case .object(let object):
    return .object(
      JSONObject(
        object.keys.sorted().compactMap { key in object[key].map { (key, canonical($0)) } }))
  default: return value
  }
}

private let exportedCell =
  "border: 1px solid black; width: 75px; vertical-align: top; text-align: start;"
private let exportedHeaderCell = exportedCell + " background-color: rgb(242, 243, 245);"

@Suite struct LexicalTableNodeTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.constructor"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.constructor"
    ),
    arguments: [false, true])
  func constructor(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      #expect(throws: Never.self) { try createTableNode() }
    }
    #expect(throws: (any Error).self) { try createTableNode() }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.createDOM()"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.createDOM()"
    ),
    arguments: [false, true])
  func createDOM(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      let tableNode = try createTableNode()
      expectTableHtml(
        try tableNode.createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"></table>"#, hasHorizontalScroll)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.createDOM() and updateDOM() style"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.createDOM() and updateDOM() style"
    ),
    arguments: [false, true])
  func createAndUpdateDOMStyle(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    func smallTable() throws -> TableNode {
      try createTableNode().append(
        createTableRowNode().append(createTableCellNode().append(createParagraphNode())))
    }
    func tablesHTML() throws -> [String] {
      try editor.read(.latest) {
        try getRoot().getChildren().map { try $0.createDOM(editor._config, editor).outerHTML }
      }
    }
    try editor.update(discrete: true) {
      try getRoot().clear().append(smallTable(), smallTable().setStyle("background-color: blue;"))
      try setSelection(nil)
    }
    var tables = try tablesHTML()
    try #require(tables.count == 2)
    expectTableHtml(tables[0], #"<table class="test-table-class"></table>"#, hasHorizontalScroll)
    expectTableHtml(
      tables[1], #"<table class="test-table-class" style="background-color: blue;"></table>"#,
      hasHorizontalScroll)

    // updateDOM moves the style from the previous version's to this one's.
    var updated: [Bool] = []
    try editor.update(discrete: true) {
      for (i, node) in try getRoot().getChildren().enumerated() {
        guard let table = node as? TableNode else { continue }
        let prev = try table.getLatest()
        let dom = try prev.createDOM(editor._config, editor)
        let next = try table.setStyle("--table-index: \(i);")
        updated.append(try next.updateDOM(prev, dom, editor._config))
        expectTableHtml(
          dom.outerHTML,
          "<table class=\"test-table-class\" style=\"--table-index: \(i);\"></table>",
          hasHorizontalScroll)
      }
    }
    #expect(updated == [false, false])
    tables = try tablesHTML()
    expectTableHtml(
      tables[0], #"<table class="test-table-class" style="--table-index: 0;"></table>"#,
      hasHorizontalScroll)
    expectTableHtml(
      tables[1], #"<table class="test-table-class" style="--table-index: 1;"></table>"#,
      hasHorizontalScroll)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.exportDOM() with range selection"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.exportDOM() with range selection"
    ),
    arguments: [false, true])
  func exportDOMWithRangeSelection(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      let tableNode = try createTableNodeWithDimensions(2, 2).setColWidths([100, 200])
      for (i, node) in try tableNode.getAllTextNodes().enumerated() {
        try node.setTextContent(String(i))
      }
      try getRoot().clear().append(tableNode)
      let html = try generateHtmlFromNodes(editor, getRoot().select(0))
      #expect(
        prettifyHTML(html)
          == prettifyHTML(
            """
            <table class="test-table-class">
              <colgroup><col style="width: 100px;" /><col style="width: 200px;" /></colgroup>
              <tbody>
                <tr>
                  <th style="\(exportedHeaderCell)"><p><span style="white-space: pre-wrap;">0</span></p></th>
                  <th style="\(exportedHeaderCell)"><p><span style="white-space: pre-wrap;">1</span></p></th>
                </tr>
                <tr>
                  <th style="\(exportedHeaderCell)"><p><span style="white-space: pre-wrap;">2</span></p></th>
                  <td style="\(exportedCell)"><p><span style="white-space: pre-wrap;">3</span></p></td>
                </tr>
              </tbody>
            </table>
            """))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.exportDOM() with partial table selection"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.exportDOM() with partial table selection"
    ),
    arguments: [false, true])
  func exportDOMWithPartialTableSelection(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      let tableNode = try createTableNodeWithDimensions(2, 2).setColWidths([100, 200])
      for (i, node) in try tableNode.getAllTextNodes().enumerated() {
        try node.setTextContent(String(i))
      }
      try getRoot().append(tableNode)
      // `$dfs(tableNode)` filtered to cells: the cells in document order.
      let cells = try tableNode.getChildren().flatMap { row in
        try #require(row as? TableRowNode).getChildren().compactMap { $0 as? TableCellNode }
      }
      // second column
      let tableSelection = try createTableSelectionFrom(tableNode, cells[1], cells[3])
      let html = try generateHtmlFromNodes(editor, tableSelection)
      #expect(
        prettifyHTML(html)
          == prettifyHTML(
            """
            <table class="test-table-class">
              <colgroup><col style="width: 200px;" /></colgroup>
              <tbody>
                <tr>
                  <th style="\(exportedHeaderCell)"><p><span style="white-space: pre-wrap;">1</span></p></th>
                </tr>
                <tr>
                  <td style="\(exportedCell)"><p><span style="white-space: pre-wrap;">3</span></p></td>
                </tr>
              </tbody>
            </table>
            """))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Update column widths"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Update column widths"
    ),
    arguments: [false, true])
  func updateColumnWidths(hasHorizontalScroll: Bool) throws {
    // Upstream compares the reconciled table element, rows included; here
    // the table's `createDOM`, which renders the `<colgroup>`, and the
    // column count stand in for it.
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update { try getRoot().append(createTableNodeWithDimensions(4, 2, true)) }

    try editor.read(.forceCommit) {
      expectTableHtml(
        try lastTable().createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"></table>"#, hasHorizontalScroll)
    }

    // Set widths
    try editor.update { _ = try lastTable().setColWidths([50, 50]) }

    try editor.read(.forceCommit) {
      let table = try lastTable()
      expectTableHtml(
        try table.createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"><colgroup><col style="width: 50px;"><col style="width: 50px;"></colgroup></table>"#,
        hasHorizontalScroll)
      // colWidths should be immutable: a Swift array is a copy.
      var colWidths = try #require(try table.getColWidths())
      colWidths.append(100)
      #expect(try table.getColWidths() == [50, 50])
      #expect(try table.getColumnCount() == 2)
    }

    // Add a column
    try editor.update {
      let table = try lastTable()
      // `getCellNodeFromCords(0, 0, DOMTable)`: the first cell.
      let row = try #require(try table.getFirstChild() as? TableRowNode)
      let cell = try #require(try row.getFirstChild() as? TableCellNode)
      try setSelection(createTableSelectionFrom(table, cell, cell))
      _ = try insertTableColumnAtSelection()
      _ = try table.setColWidths([50, 50, 100])
    }

    try editor.read(.forceCommit) {
      let table = try lastTable()
      expectTableHtml(
        try table.createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"><colgroup><col style="width: 50px;"><col style="width: 50px;"><col style="width: 100px;"></colgroup></table>"#,
        hasHorizontalScroll)
      #expect(try table.getColWidths() == [50, 50, 100])
      #expect(try table.getColumnCount() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Toggle row striping ON/OFF"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Toggle row striping ON/OFF"
    ),
    arguments: [false, true])
  func toggleRowStriping(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(4, 4, true))
    }
    try editor.update(discrete: true) { try lastTable().setRowStriping(true) }
    try editor.read(.forceCommit) {
      expectTableHtml(
        try lastTable().createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class test-table-row-striping-class" data-lexical-row-striping="true"></table>"#,
        hasHorizontalScroll)
    }
    try editor.update { try lastTable().setRowStriping(false) }
    try editor.read(.forceCommit) {
      expectTableHtml(
        try lastTable().createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"></table>"#, hasHorizontalScroll)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Toggle frozen first column ON/OFF"
    ))
  func toggleFrozenFirstColumn() throws {
    let editor = try makeEditor(true)
    try editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(4, 4, true))
    }
    try editor.update { try lastTable().setFrozenColumns(1) }
    try editor.read(.forceCommit) {
      let html = try lastTable().createDOM(editorConfig).outerHTML
      #expect(
        prettifyHTML(html)
          == prettifyHTML(
            #"<div class="table-scrollable-wrapper test-table-frozen-column-class"><table class="test-table-class" data-lexical-frozen-column="true"></table></div>"#
          ))
    }
    let actual = JSONValue.object(try editor.getEditorState().toJSON())
    #expect(canonical(actual) == canonical(try JSONValue.parse(frozenFirstColumnEditorState)))

    try editor.update { try lastTable().setFrozenColumns(0) }
    try editor.read(.forceCommit) {
      let html = try lastTable().createDOM(editorConfig).outerHTML
      #expect(
        prettifyHTML(html)
          == prettifyHTML(
            #"<div class="table-scrollable-wrapper"><table class="test-table-class"></table></div>"#
          ))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Change Table-level alignment"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Change Table-level alignment"
    ),
    arguments: [false, true])
  func changeTableAlignment(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(4, 4, true))
    }
    try editor.update { try lastTable().setFormat("center") }
    try editor.read(.forceCommit) {
      expectTableHtml(
        try lastTable().createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class test-table-alignment-center"></table>"#,
        hasHorizontalScroll)
    }
    try editor.update { try lastTable().setFormat("left") }
    try editor.read(.forceCommit) {
      expectTableHtml(
        try lastTable().createDOM(editorConfig).outerHTML,
        #"<table class="test-table-class"></table>"#, hasHorizontalScroll)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=rtl"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=ltr"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=undefined"
    ),
    arguments: ["rtl", "ltr", nil] as [String?])
  func importDOMPreservesDir(expected: String?) throws {
    let attr = expected.map { " dir=\"\($0)\"" } ?? ""
    let inputHtml = "<table\(attr)><tr\(attr)><td\(attr)>a</td></tr></table>"
    let editor = try createMountedTestEditor(nodes: tableTestNodes, theme: theme)
    try editor.update(discrete: true) {
      let dom = DOMParser().parseFromString(inputHtml)
      try getRoot().clear().append(contentsOf: generateNodesFromDOM(editor, dom))
    }
    try editor.read {
      let table = try #require(getRoot().getFirstChild() as? TableNode)
      let row = try #require(table.getFirstChild() as? TableRowNode)
      let cell = try #require(row.getFirstChild() as? TableCellNode)
      #expect(try table.getDirection() == expected)
      #expect(try row.getDirection() == expected)
      #expect(try cell.getDirection() == expected)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > round trips the dir attribute through exportDOM"
    ))
  func roundTripsDir() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes, theme: theme)
    try editor.update(discrete: true) {
      let table = try createTableNodeWithDimensions(1, 1, false)
      try table.setDirection("rtl")
      try #require(table.getFirstChild() as? TableRowNode).setDirection("rtl")
      try getRoot().clear().append(table)
    }
    let exported = try editor.read { try generateHtmlFromNodes(editor) }
    try editor.update(discrete: true) {
      let dom = DOMParser().parseFromString(exported)
      try getRoot().clear().append(contentsOf: generateNodesFromDOM(editor, dom))
    }
    try editor.read {
      let table = try #require(getRoot().getFirstChild() as? TableNode)
      let row = try #require(table.getFirstChild() as? TableRowNode)
      #expect(try table.getDirection() == "rtl")
      #expect(try row.getDirection() == "rtl")
    }
  }
}
