import Lexical
import LexicalClipboard
import LexicalHTML
import LexicalTable
import LexicalTesting
import Testing

// Port of the Copy, Cut and plain text cases of
// packages/lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx.
// Upstream mounts the `initializeUnitTest` editor with
// `<TablePlugin hasHorizontalScroll>`, which sets scrollable tables and
// registers `registerTablePlugin` and `registerTableSelectionObserver`; this
// does the same. Upstream compares the reconciled `innerHTML`; here
// `renderedHTML` builds it from each node's `createDOM` the way the
// reconciler's `$createNode` does.

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

/// `wrapTableHtml(expected)`.
private func wrapTableHtml(_ expected: String) -> String {
  expected.replacing(/<table( dir="(?:auto|ltr|rtl)")?/) { match in
    "<div\(match.1 ?? "") class=\"table-scrollable-wrapper\"><table"
  }.replacing("</table>", with: "</table></div>")
}

/// `prettifyHtml(html)`: prettier also formats each `style` attribute as
/// CSS (`a: b` becomes `a: b;`), which `prettifyHTML` does not.
private func prettify(_ html: String) -> String {
  guard let body = DOMParser().parseFromString(prettifyHTML(html)).body else { return html }
  for element in body.descendantElements() where element.hasAttribute("style") {
    element.setAttribute("style", element.style.cssText)
  }
  return body.innerHTML
}

/// `expectReconciledTableHtmlToBeEqual(actual, expected, dir)`.
private func expectReconciledTableHtml(
  _ actual: String, _ expected: String, _ hasHorizontalScroll: Bool, dir: String = "auto",
  sourceLocation: SourceLocation = #_sourceLocation
) {
  let expected = expected.replacing("<table", with: "<table dir=\"\(dir)\"")
  #expect(
    prettify(actual) == prettify(hasHorizontalScroll ? wrapTableHtml(expected) : expected),
    sourceLocation: sourceLocation)
}

/// `expectHtmlToBeEqual(actual, expected)`.
private func expectHtml(
  _ actual: String, _ expected: String, sourceLocation: SourceLocation = #_sourceLocation
) {
  #expect(prettify(actual) == prettify(expected), sourceLocation: sourceLocation)
}

/// `emptyTableContentsHtml(rows, cols, HeadingTableHTMLGenerator)`.
private func emptyHeadingTableContentsHtml(_ rows: Int, _ cols: Int) -> String {
  (0..<rows).map { row in
    let cells = (0..<cols).map { col in
      let tag = col == 0 || row == 0 ? "th" : "td"
      return
        "<\(tag) dir=\"auto\"><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\" /></p></\(tag)>"
    }
    return "<tr dir=\"auto\">\(cells.joined())</tr>"
  }.joined()
}

/// The `initializeUnitTest` editor with `<TablePlugin>` and the
/// `beforeEach` paragraph, selected.
private func makeEditor(_ hasHorizontalScroll: Bool) throws -> Editor {
  let editor = try createMountedTestEditor(nodes: tableTestNodes, theme: theme)
  setScrollableTablesActive(editor, hasHorizontalScroll)
  _ = try registerTablePlugin(editor)
  _ = try registerTableSelectionObserver(editor)
  try editor.update(discrete: true) {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    try paragraph.select()
  }
  return editor
}

/// The root element's `innerHTML` as the reconciler renders the committed
/// document: each node's `createDOM`, `data-lexical-text` on text, the
/// reconciled direction and format of each element, its children in its
/// `getDOMSlot`, and the managed `<br>` of an empty block or one ending in a
/// line break.
private func renderedHTML(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    let container = getDocument().createElement("div")
    for child in try getRoot().getChildren() {
      _ = container.appendChild(try renderDOM(child, editor))
    }
    return container.innerHTML
  }
}

/// `$getReconciledDirection(node)`.
private func reconciledDirection(_ node: ElementNode) throws -> String? {
  if let direction = node.__dir {
    return direction
  }
  guard let parent = try node.getParent() else {
    return "auto"
  }
  if try !isRootOrShadowRoot(parent) || parent.__dir != nil {
    return nil
  }
  return "auto"
}

private func renderDOM(_ node: LexicalNode, _ editor: Editor) throws -> DOMElement {
  let dom = try node.createDOM(editor._config, editor)
  if node is TextNode {
    dom.setAttribute("data-lexical-text", "true")
  }
  guard let element = node as? ElementNode else { return dom }
  if let direction = try reconciledDirection(element) {
    dom.setAttribute("dir", direction)
  }
  let slot = element.getDOMSlot(dom)
  let children = try element.getChildren()
  for child in children {
    try slot.insertChild(try renderDOM(child, editor))
  }
  if let align = ELEMENT_FORMAT_TO_TYPE[element.__format], element.__format != 0 {
    dom.style.setProperty("text-align", align)
  }
  if try !element.isInline() {
    let last = children.last
    if try last == nil || last is LineBreakNode || (last is DecoratorNode && last!.isInline()) {
      let br = getDocument().createElement("br")
      br.setAttribute("data-lexical-managed-linebreak", "true")
      try slot.insertChild(br)
    }
  }
  return dom
}

/// Pastes `html` at the selection with `$insertDataTransferForRichText`.
private func paste(_ editor: Editor, _ html: String) throws {
  let dataTransfer = DataTransfer()
  dataTransfer.setData("text/html", html)
  try editor.update(discrete: true) {
    let selection = try #require(
      try getSelection() as? RangeSelection, "isRangeSelection(selection)")
    try insertDataTransferForRichText(dataTransfer, selection, editor)
  }
}

private func cut(_ editor: Editor) throws {
  _ = try editor.dispatchCommand(CUT_COMMAND, .clipboard(ClipboardEvent("cut")))
}

private let emptyParagraph = #"<p dir="auto"><br data-lexical-managed-linebreak="true" /></p>"#

// swift-format-ignore
private let googleDocsTable = #"<html><body><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-16a69100-7fff-6cb9-b829-cb1def16a58d"><div dir="ltr" style="margin-left:0pt;" align="left"><table style="border:none;border-collapse:collapse;table-layout:fixed"><colgroup><col style="width:100px"/><col style="width:200px"/></colgroup><tbody><tr style="height:22.015pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Hello there</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">General Kenobi!</span></p></td></tr><tr style="height:22.015pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Lexical is nice</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><br data-lexical-managed-linebreak="true"></td></tr></tbody></table></div></b><!--EndFragment--></body></html>"#

// https://developer.mozilla.org/en-US/docs/Web/HTML/Element/thead
private let theadTable = """
  <meta charset="utf-8" />
  <table style="box-sizing: border-box; border-collapse: collapse; border: 2px solid rgb(140, 140, 140); font-family: sans-serif; font-size: 0.8rem; letter-spacing: 1px; color: rgb(21, 20, 26); font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; orphans: 2; text-align: start; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">
    <caption style="box-sizing: border-box; caption-side: bottom; padding: 10px;">
      Council budget (in £) 2018
    </caption>
    <thead style="box-sizing: border-box; background-color: rgb(44, 94, 119); color: rgb(255, 255, 255);">
      <tr style="box-sizing: border-box;">
        <th scope="col" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
          Items
        </th>
        <th scope="col" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
          Expenditure
        </th>
      </tr>
    </thead>
    <tbody style="box-sizing: border-box; background-color: rgb(228, 240, 245);">
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
          Donuts
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
          3,000
        </td>
      </tr>
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
          Stationery
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
          18,000
        </td>
      </tr>
    </tbody>
    <tfoot style="box-sizing: border-box; background-color: rgb(44, 94, 119); color: rgb(255, 255, 255);">
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
          Totals
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
          21,000
        </td>
      </tr>
    </tfoot>
  </table>
  """

// https://developer.mozilla.org/en-US/docs/Web/HTML/Element/caption
private let captionTable = """
  <meta charset="utf-8" />
  <table style="box-sizing: border-box; border-collapse: collapse; border: 2px solid rgb(140, 140, 140); font-family: sans-serif; font-size: 0.8rem; letter-spacing: 1px; color: rgb(21, 20, 26); font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; orphans: 2; text-align: start; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">
    <caption style="box-sizing: border-box; caption-side: bottom; padding: 10px; font-weight: bold;">
      He-Man and Skeletor facts
    </caption>
    <tbody style="box-sizing: border-box;">
      <tr style="box-sizing: border-box;">
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);"></td>
        <th class="heman" scope="col" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230); font: 1.4rem molot; text-shadow: rgb(255, 255, 255) 1px 1px 1px, rgb(0, 0, 0) 2px 2px 1px;">
          He-Man
        </th>
        <th class="skeletor" scope="col" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230); font: 1.7rem rapscallion; letter-spacing: 3px; text-shadow: rgb(255, 255, 255) 1px 1px 0px, rgb(0, 0, 0) 0px 0px 9px;">
          Skeletor
        </th>
      </tr>
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
          Role
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
          Hero
        </td>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
          Villain
        </td>
      </tr>
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
          Weapon
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);">
          Power Sword
        </td>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);">
          Havoc Staff
        </td>
      </tr>
      <tr style="box-sizing: border-box;">
        <th scope="row" style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
          Dark secret
        </th>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
          Expert florist
        </td>
        <td style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
          Cries at romcoms
        </td>
      </tr>
    </tbody>
  </table>
  """

// swift-format-ignore
private let googleSheetsTable = #"<google-sheets-html-origin><style type="text/css"><!--td {border: 1px solid #cccccc;}br {mso-data-placement:same-cell;}--></style><table xmlns="http://www.w3.org/1999/xhtml" cellspacing="0" cellpadding="0" dir="ltr" border="1" style="table-layout:fixed;font-size:10pt;font-family:Arial;width:0px;border-collapse:collapse;border:none" data-sheets-root="1"><colgroup><col width="100"/><col width="189"/><col width="171"/></colgroup><tbody><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-weight:bold;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;Surface&quot;}">Surface</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-style:italic;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;MWP_WORK_LS_COMPOSER&quot;}">MWP_WORK_LS_COMPOSER</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-decoration:underline;text-align:right;" data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:77349}">77349</td></tr><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;Lexical&quot;}">Lexical</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-decoration:line-through;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;XDS_RICH_TEXT_AREA&quot;}">XDS_RICH_TEXT_AREA</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;sdvd sdfvsfs&quot;}" data-sheets-textstyleruns="{&quot;1&quot;:0}{&quot;1&quot;:5,&quot;2&quot;:{&quot;5&quot;:1}}"><span style="font-size:10pt;font-family:Arial;font-style:normal;">sdvd </span><span style="font-size:10pt;font-family:Arial;font-weight:bold;font-style:normal;">sdfvsfs</span></td></tr></tbody></table>"#

/// Appends a paragraph holding text, a 4x4 table with headers and text, as
/// the range-selection Cut cases do, then selects all and cuts.
private func cutRange(_ editor: Editor, before: Bool, after: Bool) throws {
  try editor.update(discrete: true) {
    let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
    if before {
      try paragraph.append(createTextNode("text before the table"))
    }
    try paragraph.append(createTableNodeWithDimensions(4, 4, true))
    if after {
      try paragraph.append(createTextNode("text after the table"))
    }
  }
  try editor.update(discrete: true) {
    try editor.focus()
    _ = try selectAll()
  }
  try editor.update(discrete: true) { try cut(editor) }
}

/// Appends a 4x4 table with headers, puts text in its first cell, selects
/// from the first cell to `(last, last)` and cuts.
private func cutCells(_ editor: Editor, to last: Int) throws {
  try editor.update(discrete: true) {
    try getRoot().append(createTableNodeWithDimensions(4, 4, true))
  }
  try editor.update(discrete: true) {
    guard let table = try getRoot().getLastChild() as? TableNode else { return }
    let domTable = try getElementForTableNode(editor, table)
    let first = try #require(try table.getCellNodeFromCords(0, 0, domTable))
    try #require(try first.getLastChild() as? ParagraphNode).append(
      createTextNode("some text"))
    try setSelection(
      createTableSelectionFrom(
        table, first, #require(try table.getCellNodeFromCords(last, last, domTable))))
    try cut(editor)
  }
}

@Suite struct LexicalTableNodeClipboardTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table from an external source"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table from an external source"
    ),
    arguments: [false, true])
  func copyFromExternalSource(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try paste(editor, googleDocsTable)
    // Make sure paragraph is inserted inside empty cells
    let emptyCell =
      #"<td dir="auto"><p dir="auto"><br data-lexical-managed-linebreak="true"></p></td>"#
    expectReconciledTableHtml(
      try renderedHTML(editor),
      """
      <table class="test-table-class">
        <colgroup>
          <col style="width: 100px;" />
          <col style="width: 200px;" />
        </colgroup>
        <tr dir="auto">
          <td dir="auto">
            <p dir="ltr">
              <span data-lexical-text="true">Hello there</span>
            </p>
          </td>
          <td dir="auto">
            <p dir="ltr">
              <span data-lexical-text="true">General Kenobi!</span>
            </p>
          </td>
        </tr>
        <tr dir="auto">
          <td dir="auto">
            <p dir="ltr">
              <span data-lexical-text="true">Lexical is nice</span>
            </p>
          </td>
          \(emptyCell)
        </tr>
      </table>
      """, hasHorizontalScroll)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table with caption/tbody/thead/tfoot from an external source"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table with caption/tbody/thead/tfoot from an external source"
    ),
    arguments: [false, true])
  func copyWithTheadTbodyTfoot(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try paste(editor, theadTable)
    func header(_ text: String) -> String {
      #"<th dir="auto"><p dir="auto"><span data-lexical-text="true">\#(text)</span></p></th>"#
    }
    func centered(_ text: String) -> String {
      #"<td dir="auto"><p dir="auto" style="text-align: center;"><span data-lexical-text="true">\#(text)</span></p></td>"#
    }
    let row = #"<tr dir="auto" style="text-align: start">"#
    // Here we are testing the createDOM, not the exportDOM, so the tbody is
    // not there
    expectReconciledTableHtml(
      try renderedHTML(editor),
      """
      <table class="test-table-class">
        \(row)\(header("Items"))\(header("Expenditure"))</tr>
        \(row)\(header("Donuts"))\(centered("3,000"))</tr>
        \(row)\(header("Stationery"))\(centered("18,000"))</tr>
        \(row)\(header("Totals"))\(centered("21,000"))</tr>
      </table>
      """, hasHorizontalScroll)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table with caption from an external source"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table with caption from an external source"
    ),
    arguments: [false, true])
  func copyWithCaption(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try paste(editor, captionTable)
    func header(_ text: String, _ color: String) -> String {
      #"<th dir="auto" style="background-color: \#(color)"><p dir="auto"><span data-lexical-text="true">\#(text)</span></p></th>"#
    }
    func cell(_ text: String, _ color: String) -> String {
      #"<td dir="auto" style="background-color: \#(color)"><p dir="auto" style="text-align: center"><span data-lexical-text="true">\#(text)</span></p></td>"#
    }
    let row = #"<tr dir="auto" style="text-align: start">"#
    let light = "rgb(240, 240, 240)"
    let heading = "rgb(230, 230, 230)"
    let lighter = "rgb(250, 250, 250)"
    // Here we are testing the createDOM, not the exportDOM, so the tbody is
    // not there
    expectReconciledTableHtml(
      try renderedHTML(editor),
      """
      <table class="test-table-class">
        \(row)
          <td dir="auto" style="background-color: \(light)">
            <p dir="auto" style="text-align: center">
              <br data-lexical-managed-linebreak="true" />
            </p>
          </td>
          \(header("He-Man", heading))\(header("Skeletor", heading))
        </tr>
        \(row)\(header("Role", heading))\(cell("Hero", lighter))\(cell("Villain", lighter))</tr>
        \(row)\(header("Weapon", heading))\(cell("Power Sword", light))\(cell("Havoc Staff", light))</tr>
        \(row)\(header("Dark secret", heading))\(cell("Expert florist", lighter))\(cell("Cries at romcoms", lighter))</tr>
      </table>
      """, hasHorizontalScroll)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table from an external source like gdoc with formatting"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table from an external source like gdoc with formatting"
    ),
    arguments: [false, true])
  func copyWithFormatting(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try paste(editor, googleSheetsTable)
    func cell(_ content: String, align: String? = nil) -> String {
      let style = align.map { #" style="text-align: \#($0);""# } ?? ""
      return
        #"<td dir="auto" style="vertical-align: bottom"><p dir="auto"\#(style)>\#(content)</p></td>"#
    }
    func text(_ tag: String, _ text: String) -> String {
      #"<\#(tag) data-lexical-text="true">\#(text)</\#(tag)>"#
    }
    // The pasted table carries dir="ltr", which importDOM now preserves, so
    // the table is explicitly ltr rather than auto and its rows no longer
    // bidi-auto-detect.
    expectReconciledTableHtml(
      try renderedHTML(editor),
      """
      <table class="test-table-class">
        <colgroup>
          <col style="width: 100px" />
          <col style="width: 189px" />
          <col style="width: 171px" />
        </colgroup>
        <tr style="height: 21px;">
          \(cell(text("strong", "Surface")))
          \(cell(text("em", "MWP_WORK_LS_COMPOSER")))
          \(cell(text("span", "77349"), align: "right"))
        </tr>
        <tr style="height: 21px;">
          \(cell(text("span", "Lexical")))
          \(cell(text("span", "XDS_RICH_TEXT_AREA")))
          \(cell(text("span", "sdvd") + text("strong", "sdfvsfs")))
        </tr>
      </table>
      """, hasHorizontalScroll, dir: "ltr")
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table in the middle of a range selection"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table in the middle of a range selection"
    ),
    arguments: [false, true])
  func cutInMiddleOfRange(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try cutRange(editor, before: true, after: true)
    expectHtml(try renderedHTML(editor), emptyParagraph)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table as last node in range selection "
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table as last node in range selection "
    ),
    arguments: [false, true])
  func cutAsLastNode(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try cutRange(editor, before: true, after: false)
    expectHtml(try renderedHTML(editor), emptyParagraph)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table as first node in range selection "
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table as first node in range selection "
    ),
    arguments: [false, true])
  func cutAsFirstNode(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try cutRange(editor, before: false, after: true)
    expectHtml(try renderedHTML(editor), emptyParagraph)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table is whole selection, should remove it"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table is whole selection, should remove it"
    ),
    arguments: [false, true])
  func cutWholeTable(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try cutCells(editor, to: 3)
    expectHtml(try renderedHTML(editor), emptyParagraph)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut subsection of table cells, should just clear contents"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut subsection of table cells, should just clear contents"
    ),
    arguments: [false, true])
  func cutSubsection(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try cutCells(editor, to: 2)
    expectReconciledTableHtml(
      try renderedHTML(editor),
      """
      \(emptyParagraph)
      <table class="test-table-class">
        \(emptyHeadingTableContentsHtml(4, 4))
      </table>
      """, hasHorizontalScroll)
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Table plain text output validation"
    ),
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Table plain text output validation"
    ),
    arguments: [false, true])
  func plainTextOutput(hasHorizontalScroll: Bool) throws {
    let editor = try makeEditor(hasHorizontalScroll)
    try editor.update(discrete: true) {
      try getRoot().append(createTableNodeWithDimensions(4, 4, true))
    }
    try editor.update(discrete: true) {
      let table = try #require(try getRoot().getLastChild() as? TableNode)
      let domTable = try getElementForTableNode(editor, table)
      func put(_ x: Int, _ y: Int, _ text: String) throws {
        let cell = try #require(try table.getCellNodeFromCords(x, y, domTable))
        try #require(try cell.getLastChild() as? ParagraphNode).append(createTextNode(text))
      }
      try put(0, 0, "1")
      try put(1, 0, "")
      try put(2, 0, "2")
      try put(0, 1, "3")
      try put(1, 1, "4")
      try put(2, 1, "")
      let selection = try createTableSelectionFrom(
        table, #require(try table.getCellNodeFromCords(0, 0, domTable)),
        #require(try table.getCellNodeFromCords(2, 1, domTable)))
      #expect(try selection.getTextContent() == "1\t\t2\n3\t4\t\n")
    }
  }
}
