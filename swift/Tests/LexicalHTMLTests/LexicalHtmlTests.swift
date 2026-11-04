import Lexical
import LexicalHTML
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-html/src/__tests__/unit/LexicalHtml.test.ts.
// Upstream registers list, code and link nodes; the Swift editors register
// the rich-text and list nodes (code and links are not ported).

private func headlessEditor(
  nodes: [LexicalNodeConfig] = [], html: HTMLConfig? = nil
) throws -> Editor {
  try createEditor(
    CreateEditorArgs(nodes: nodes, onError: { throw $0 }, headless: true, html: html))
}

private func richTextNodes() -> [LexicalNodeConfig] {
  [
    .klass(HeadingNode.self), .klass(ListNode.self), .klass(ListItemNode.self),
    .klass(QuoteNode.self),
  ]
}

private func twoParagraphs() throws -> (TextNode, TextNode) {
  let text1 = try createTextNode("Hello")
  let text2 = try createTextNode("World")
  try getRoot().append(createParagraphNode().append(text1)).append(
    createParagraphNode().append(text2))
  return (text1, text2)
}

private func importHTML(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) {
    let nodes = try generateNodesFromDOM(editor, DOMParser().parseFromString(html))
    try getRoot().append(contentsOf: nodes)
  }
}

@Suite struct LexicalHtmlTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Empty editor state"
    ))
  func emptyEditorState() throws {
    let editor = try headlessEditor(nodes: richTextNodes())
    try editor.update(discrete: true) { try getRoot().append(createParagraphNode()) }
    #expect(try editor.read(.latest) { try generateHtmlFromNodes(editor) } == "<p><br></p>")
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Nested ordered list numbering (Regression #7207)"
    ))
  func nestedOrderedListNumbering() throws {
    let editor = try headlessEditor(nodes: richTextNodes())
    try editor.update(discrete: true) {
      func item(_ text: String) throws -> ListItemNode {
        try createListItemNode().append(createTextNode(text))
      }
      let nestedWrapper = try createListItemNode().append(
        createListNode("number").append(item("LA"), item("TX")))
      try getRoot().append(
        createListNode("number").append(
          item("Canada"), item("USA"), nestedWrapper, item("Germany")))
    }
    // #7207: the nested list is inside its parent <li>, not a separate <li>.
    #expect(
      try editor.read(.latest) { try generateHtmlFromNodes(editor) }
        == #"<ol><li value="1"><span style="white-space: pre-wrap;">Canada</span></li><li value="2"><span style="white-space: pre-wrap;">USA</span><ol><li value="1"><span style="white-space: pre-wrap;">LA</span></li><li value="2"><span style="white-space: pre-wrap;">TX</span></li></ol></li><li value="3"><span style="white-space: pre-wrap;">Germany</span></li></ol>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Use provided selection"
    ))
  func providedSelection() throws {
    let editor = try headlessEditor(nodes: richTextNodes())
    nonisolated(unsafe) var selection: RangeSelection? = nil
    try editor.update(discrete: true) {
      let (text1, text2) = try twoParagraphs()
      _ = try text1.getParentOrThrow().select(0, text1.getTextContentSize())
      let s = createRangeSelection()
      try s.setTextNodeRange(text2, 0, text2, text2.getTextContentSize())
      selection = s
    }
    var html = ""
    try editor.update { html = try generateHtmlFromNodes(editor, selection) }
    #expect(html == #"<span style="white-space: pre-wrap;">World</span>"#)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Default selection (undefined) should serialize entire editor state"
    ))
  func defaultSelection() throws {
    let editor = try headlessEditor(nodes: richTextNodes())
    try editor.update(discrete: true) {
      let (text1, _) = try twoParagraphs()
      _ = try text1.getParentOrThrow().select(0, text1.getTextContentSize())
    }
    var html = ""
    try editor.update { html = try generateHtmlFromNodes(editor) }
    #expect(
      html
        == #"<p><span style="white-space: pre-wrap;">Hello</span></p><p><span style="white-space: pre-wrap;">World</span></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > If alignment is set on the paragraph, don't overwrite from parent empty format"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > If alignment is set on the paragraph, it should take precedence over its parent block alignment"
    ),
    arguments: [
      #"<div><p style="text-align: center;">Hello world!</p></div>"#,
      #"<div style="text-align: right;"><p style="text-align: center;">Hello world!</p></div>"#,
    ])
  func paragraphAlignmentWins(input: String) throws {
    let editor = try headlessEditor()
    try importHTML(editor, input)
    var html = ""
    try editor.update { html = try generateHtmlFromNodes(editor) }
    #expect(
      html
        == #"<p style="text-align: center;"><span style="white-space: pre-wrap;">Hello world!</span></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > It should output correctly nodes whose export is DocumentFragment"
    ))
  func documentFragmentExport() throws {
    let editor = try headlessEditor(
      html: HTMLConfig(export: [
        (
          ParagraphNode.self,
          { _, _ in DOMExportOutput(element: getDocument().createDocumentFragment()) }
        )
      ]))
    try editor.update(discrete: true) { _ = try twoParagraphs() }
    var html = ""
    try editor.update { html = try generateHtmlFromNodes(editor) }
    #expect(
      html
        == #"<span style="white-space: pre-wrap;">Hello</span><span style="white-space: pre-wrap;">World</span>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > existing inline styles are preserved after inlining pass"
    ))
  func inlineStylesPreserved() throws {
    let editor = try headlessEditor()
    try editor.update(discrete: true) {
      let dom = DOMParser().parseFromString(
        #"<html><head><style>.colored { color: red; }</style></head><body><p><span class="colored" style="color: blue;">Hello</span></p></body></html>"#
      )
      _ = try generateNodesFromDOM(editor, dom)
      let span = try #require(
        dom.documentElement?.descendantElements().first { $0.classList.contains("colored") })
      #expect(span.style.getPropertyValue("color") == "blue")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph with dir=\"rtl\""
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph with dir=\"ltr\""
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph without dir"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > heading with dir=\"rtl\""
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > blockquote with dir=\"rtl\""
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > list with dir=\"rtl\""
    ),
    arguments: [
      (#"<p dir="rtl">مرحبا</p>"#, "rtl"),
      (#"<p dir="ltr">Hello</p>"#, "ltr"),
      ("<p>Hello</p>", nil),
      (#"<h1 dir="rtl">عنوان</h1>"#, "rtl"),
      (#"<blockquote dir="rtl">اقتباس</blockquote>"#, "rtl"),
      (#"<ul dir="rtl"><li>عنصر</li></ul>"#, "rtl"),
    ] as [(String, String?)])
  func importPreservesDir(html: String, expected: String?) throws {
    let editor = try headlessEditor(nodes: richTextNodes())
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOM(editor, DOMParser().parseFromString(html))
      _ = try getRoot().selectEnd()
      try getRoot().select(0, nil).insertNodes(nodes)
    }
    let direction = try editor.read {
      try (getRoot().getFirstChild() as? ElementNode)?.getDirection()
    }
    #expect(direction == expected)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: slots are not auto-serialized to HTML"
    ))
  func slotsNotSerialized() throws {
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "slot", nodes: [.klass(TestShadowRootNode.self)], onError: { throw $0 },
        headless: true))
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode().append(
        createParagraphNode().append(createTextNode("SLOTTEXT")))
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
    }
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(!html.contains("SLOTTEXT"))
  }
}
