import Lexical
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts.

private var editorConfig: EditorConfig {
  EditorConfig(namespace: "", theme: ["tableCell": "test-table-cell-class"])
}

private func makeEditor() throws -> Editor {
  try createMountedTestEditor(nodes: tableTestNodes)
}

/// `expectTableCellNode(result)`.
private func expectTableCellNode(_ result: DOMConversionOutput?) throws -> TableCellNode {
  let node = try #require(result?.node)
  try #require(node.count == 1, "Expected a single node, but got an array")
  return try #require(node[0] as? TableCellNode, "Expected result.node to be a TableCellNode")
}

@Suite struct LexicalTableCellNodeTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > importJSON coerces out-of-domain span/width/verticalAlign"
    ))
  func importJSONCoercesOutOfDomain() throws {
    let editor = try makeEditor()
    try update(editor) {
      let json = JSONObject([
        ("colSpan", 0), ("headerState", .int(TableCellHeaderStates.NO_STATUS)),
        ("rowSpan", -3), ("type", "tablecell"), ("verticalAlign", "nonsense"), ("width", 0),
      ])
      let cell = try #require(TableCellNode.importJSON(json) as? TableCellNode)
      #expect(try cell.getColSpan() == 1)
      #expect(try cell.getRowSpan() == 1)
      #expect(try cell.getWidth() == nil)
      #expect(try cell.getVerticalAlign() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > importJSON round-trips in-domain values"
    ))
  func importJSONRoundTrips() throws {
    let editor = try makeEditor()
    try update(editor) {
      let json = JSONObject([
        ("backgroundColor", "#fff"), ("children", []), ("colSpan", 2), ("direction", nil),
        ("format", ""), ("headerState", .int(TableCellHeaderStates.ROW)), ("indent", 0),
        ("rowSpan", 3), ("type", "tablecell"), ("version", 1), ("verticalAlign", "middle"),
        ("width", 120),
      ])
      let cell = try #require(TableCellNode.importJSON(json) as? TableCellNode)
      #expect(try cell.getColSpan() == 2)
      #expect(try cell.getRowSpan() == 3)
      #expect(try cell.getWidth() == 120)
      #expect(try cell.getVerticalAlign() == "middle")
      #expect(try cell.getBackgroundColor() == "#fff")
      #expect(try cell.getHeaderStyles() == TableCellHeaderStates.ROW)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.constructor"
    ))
  func constructor() throws {
    let editor = try makeEditor()
    try update(editor) {
      #expect(throws: Never.self) { try createTableCellNode(TableCellHeaderStates.NO_STATUS) }
    }
    #expect(throws: (any Error).self) { try createTableCellNode(TableCellHeaderStates.NO_STATUS) }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try makeEditor()
    try update(editor) {
      let cellNode = try createTableCellNode(TableCellHeaderStates.NO_STATUS)
      #expect(
        try cellNode.createDOM(editorConfig).outerHTML
          == #"<td class="test-table-cell-class"></td>"#)

      let headerCellNode = try createTableCellNode(TableCellHeaderStates.ROW)
      #expect(
        try headerCellNode.createDOM(editorConfig).outerHTML
          == #"<th class="test-table-cell-class"></th>"#)

      let cellWithRowSpanNode = try createTableCellNode(TableCellHeaderStates.NO_STATUS, colSpan: 2)
      #expect(
        try cellWithRowSpanNode.createDOM(editorConfig).outerHTML
          == #"<td colspan="2" class="test-table-cell-class"></td>"#)

      let cellWithCustomWidthNode = try createTableCellNode(
        TableCellHeaderStates.NO_STATUS, width: 200)
      #expect(
        try cellWithCustomWidthNode.createDOM(editorConfig).outerHTML
          == #"<td style="width: 200px;" class="test-table-cell-class"></td>"#)

      let cellWithIgnoredVerticalAlignNode = try createTableCellNode(
        TableCellHeaderStates.NO_STATUS)
      try cellWithIgnoredVerticalAlignNode.setVerticalAlign("top")
      #expect(
        try cellWithIgnoredVerticalAlignNode.createDOM(editorConfig).outerHTML
          == #"<td class="test-table-cell-class"></td>"#)

      let cellWithValidVerticalAlignNode = try createTableCellNode(
        TableCellHeaderStates.NO_STATUS)
      try cellWithValidVerticalAlignNode.setVerticalAlign("middle")
      #expect(
        try cellWithValidVerticalAlignNode.createDOM(editorConfig).outerHTML
          == #"<td style="vertical-align: middle;" class="test-table-cell-class"></td>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.importDOM"
    ))
  func importDOM() throws {
    let editor = try makeEditor()
    let cases: [(String, String)] = [
      (
        """
        <table>
          <tr>
            <td>
              <test-decorator></test-decorator>
              1
            </td>
          </tr>
        </table>
        """,
        """
        <table>
          <tbody>
            <tr>
              <td
                style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                <p>
                  <test-decorator></test-decorator>
                  <span style="white-space: pre-wrap;">1</span>
                </p>
              </td>
            </tr>
          </tbody>
        </table>
        """
      ),
      (
        """
        <table>
          <tr>
            <td>
              1
              <br />
              <br />
              <br />
              <br />
              2
            </td>
          </tr>
        </table>
        """,
        """
        <table>
          <tbody>
            <tr>
              <td
                style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                <p>
                  <span style="white-space: pre-wrap;">1</span>
                  <br />
                  <br />
                  <br />
                  <br />
                  <span style="white-space: pre-wrap;">2</span>
                </p>
              </td>
            </tr>
          </tbody>
        </table>
        """
      ),
      (
        """
        <table>
          <tr>
            <td>
              <p>1</p>
              <br />
              <p>2</p>
            </td>
          </tr>
        </table>
        """,
        """
        <table>
          <tbody>
            <tr>
              <td
                style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                <p>
                  <span style="white-space: pre-wrap;">1</span>
                </p>
                <p>
                  <br />
                </p>
                <p>
                  <span style="white-space: pre-wrap;">2</span>
                </p>
              </td>
            </tr>
          </tbody>
        </table>
        """
      ),
    ]
    try update(editor) {
      let root = try getRoot()
      for (input, output) in cases {
        try root.clear()
        let dom = DOMParser().parseFromString(input)
        try root.append(contentsOf: generateNodesFromDOM(editor, dom))
        let html = try generateHtmlFromNodes(editor)
        #expect(prettifyHTML(html) == prettifyHTML(output))
      }
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> with scope=\"col\" becomes COLUMN header"
    ))
  func thScopeCol() throws {
    let editor = try makeEditor()
    try update(editor) {
      let th = getDocument().createElement("th")
      th.setAttribute("scope", "col")
      let node = try expectTableCellNode(runDOMConversion(editor, th))
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.COLUMN)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> with scope=\"row\" becomes ROW header"
    ))
  func thScopeRow() throws {
    let editor = try makeEditor()
    try update(editor) {
      let th = getDocument().createElement("th")
      th.setAttribute("scope", "row")
      let node = try expectTableCellNode(runDOMConversion(editor, th))
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.ROW)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> without scope defaults to ROW header"
    ))
  func thWithoutScope() throws {
    let editor = try makeEditor()
    try update(editor) {
      let document = getDocument()
      let table = document.createElement("table")
      let tr = document.createElement("tr")
      let th = document.createElement("th")
      tr.appendChild(th)
      table.appendChild(tr)
      let node = try expectTableCellNode(runDOMConversion(editor, th))
      // First row, first column → BOTH (ROW from header row + COLUMN from
      // first column)
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.BOTH)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in first row without scope becomes ROW header"
    ))
  func thInFirstRow() throws {
    let editor = try makeEditor()
    try update(editor) {
      let document = getDocument()
      let table = document.createElement("table")
      let tr = document.createElement("tr")
      let th = document.createElement("th")
      let td = document.createElement("td")
      tr.appendChild(th)
      tr.appendChild(td)
      table.appendChild(tr)
      let node = try expectTableCellNode(runDOMConversion(editor, th))
      // First row, first column → BOTH
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.BOTH)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in first column of non-first row becomes COLUMN header"
    ))
  func thInFirstColumnOfLaterRow() throws {
    let editor = try makeEditor()
    try update(editor) {
      let document = getDocument()
      let table = document.createElement("table")
      let tr1 = document.createElement("tr")
      let tr2 = document.createElement("tr")
      let th1 = document.createElement("th")
      let td1 = document.createElement("td")
      let th2 = document.createElement("th")
      let td2 = document.createElement("td")
      tr1.appendChild(th1)
      tr1.appendChild(td1)
      tr2.appendChild(th2)
      tr2.appendChild(td2)
      table.appendChild(tr1)
      table.appendChild(tr2)
      let node = try expectTableCellNode(runDOMConversion(editor, th2))
      // Non-first row, first column → COLUMN
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.COLUMN)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in thead without scope becomes ROW header"
    ))
  func thInThead() throws {
    let editor = try makeEditor()
    try update(editor) {
      let document = getDocument()
      let table = document.createElement("table")
      let thead = document.createElement("thead")
      let tr = document.createElement("tr")
      let th = document.createElement("th")
      let th2 = document.createElement("th")
      tr.appendChild(th)
      tr.appendChild(th2)
      thead.appendChild(tr)
      table.appendChild(thead)
      // Second th in thead → ROW (not first column, so only ROW from first
      // row)
      let node = try expectTableCellNode(runDOMConversion(editor, th2))
      #expect(try node.getHeaderStyles() == TableCellHeaderStates.ROW)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with style.backgroundColor reads inline background-color"
    ))
  func tdBackgroundColor() throws {
    let editor = try makeEditor()
    try update(editor) {
      let td = getDocument().createElement("td")
      td.style.setProperty("background-color", "#F4B084")
      let node = try expectTableCellNode(runDOMConversion(editor, td))
      // Browsers normalize hex to rgb when set via .style
      #expect(try node.getBackgroundColor() == td.style["background-color"])
      #expect(try node.getBackgroundColor() == "rgb(244, 176, 132)")
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with no background color sets backgroundColor to null"
    ))
  func tdNoBackgroundColor() throws {
    let editor = try makeEditor()
    try update(editor) {
      let td = getDocument().createElement("td")
      let node = try expectTableCellNode(runDOMConversion(editor, td))
      #expect(try node.getBackgroundColor() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with color propagates color to child TextNodes via after callback"
    ))
  func tdColorPropagates() throws {
    let editor = try makeEditor()
    try update(editor) {
      let td = getDocument().createElement("td")
      td.style.setProperty("color", "blue")
      let result = try runDOMConversion(editor, td)
      _ = try expectTableCellNode(result)
      // The after callback propagates td color to child TextNodes
      let textNode = try createTextNode("Hello")
      let after = try #require(result?.after)
      _ = try after([textNode])
      #expect(try textNode.getStyle().contains("color: blue"))
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> color does not overwrite existing child TextNode color"
    ))
  func tdColorKeepsExisting() throws {
    let editor = try makeEditor()
    try update(editor) {
      let td = getDocument().createElement("td")
      td.style.setProperty("color", "blue")
      let result = try runDOMConversion(editor, td)
      _ = try expectTableCellNode(result)
      let textNode = try createTextNode("Hello")
      try textNode.setStyle("color: red;")
      let after = try #require(result?.after)
      _ = try after([textNode])
      // Existing color should not be overwritten
      #expect(try textNode.getStyle().contains("color: red"))
      #expect(try !textNode.getStyle().contains("color: blue"))
    }
  }
}
