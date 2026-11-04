import Lexical
import LexicalHTML
import LexicalMarkdown
import Testing

// Port of the `$generateNodesFromMarkdownString` describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.

@Suite struct GenerateNodesFromMarkdownStringTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > returns nodes without modifying the root"
    ))
  func leavesRootAlone() throws {
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("existing")))
    }
    var types: [String] = []
    try editor.update(discrete: true) {
      types = try generateNodesFromMarkdownString("# Heading\n\nParagraph", TRANSFORMERS)
        .map { $0.getType() }
    }
    #expect(types == ["heading", "paragraph"])
    #expect(try editor.read { try getRoot().getTextContent() } == "existing")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > produces the same nodes as $convertFromMarkdownString"
    ))
  func sameNodesAsConvert() throws {
    let md = "# Title\n\n- item 1\n- item 2\n\n> quote\n\n```\ncode\n```"

    let convertEditor = try createMarkdownTestEditor()
    try convertEditor.update(discrete: true) { try convertFromMarkdownString(md, TRANSFORMERS) }
    let convertHtml = try convertEditor.read { try generateHtmlFromNodes(convertEditor) }

    let generateEditor = try createMarkdownTestEditor()
    try generateEditor.update(discrete: true) {
      let nodes = try generateNodesFromMarkdownString(md, TRANSFORMERS)
      try getRoot().clear().append(contentsOf: nodes)
    }
    let generateHtml = try generateEditor.read { try generateHtmlFromNodes(generateEditor) }

    #expect(generateHtml == convertHtml)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > returned nodes can be inserted at selection"
    ))
  func insertAtSelection() throws {
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("before")),
        createParagraphNode().append(createTextNode("after")))
    }
    try editor.update(discrete: true) {
      try getRoot().select(1, 1)
      let nodes = try generateNodesFromMarkdownString("**bold**", TRANSFORMERS)
      if let selection = try getSelection() as? RangeSelection {
        try selection.insertNodes(nodes)
      }
    }
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html.contains("before"))
    #expect(html.contains("<strong"))
    #expect(html.contains("after"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > handles adjacent line merging (commonmark)"
    ))
  func mergesAdjacentLines() throws {
    let editor = try createMarkdownTestEditor()
    var types: [String] = []
    try editor.update(discrete: true) {
      types = try generateNodesFromMarkdownString("line 1\nline 2", TRANSFORMERS, false, true)
        .map { $0.getType() }
    }
    #expect(types == ["paragraph"])
  }
}

// Port of the `Markdown export of long runs of formatted spaces` describe
// block. Each space differs in format from its neighbours, so none of them
// merge. Upstream also spies on `TextNode.prototype.getNextSibling` and
// `getPreviousSibling` and asserts fewer than `10 * SPACES` calls; Swift has
// no way to count calls to a method, so the port asserts the export only.
@Suite struct MarkdownLongFormattedSpaceRunTests {
  static let spaces = 200
  static let half = spaces / 2

  enum Export: Sendable, CustomTestStringConvertible {
    case document, textPoints, elementPoints
    var testDescription: String {
      switch self {
      case .document: "the document"
      case .textPoints: "a selection between text points"
      case .elementPoints: "a selection between element points"
      }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown export of long runs of formatted spaces > visits each sibling a bounded number of times exporting the document"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown export of long runs of formatted spaces > visits each sibling a bounded number of times exporting a selection between text points"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown export of long runs of formatted spaces > visits each sibling a bounded number of times exporting a selection between element points"
    ), arguments: [Export.document, .textPoints, .elementPoints])
  func boundedSiblingVisits(_ export: Export) throws {
    let spaces = Self.spaces
    let half = Self.half
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("a").setFormat(IS_BOLD))
      for i in 0..<spaces {
        try paragraph.append(
          createTextNode(" ").setFormat(IS_BOLD | (i % 2 != 0 ? IS_ITALIC : IS_STRIKETHROUGH)))
      }
      try paragraph.append(createTextNode("c").setFormat(IS_BOLD))
      try getRoot().append(paragraph)
      switch export {
      case .document: break
      case .textPoints:
        let first = try #require(try paragraph.getFirstChild() as? TextNode)
        let middle = try #require(try paragraph.getChildAtIndex(half) as? TextNode)
        try setSelectionFromCaretRange(
          getCaretRange(getTextPointCaret(first, .next, 0), getTextPointCaret(middle, .next, 1)))
      case .elementPoints:
        try paragraph.select(half + 1, paragraph.getChildrenSize())
      }
    }
    let result = try editor.read {
      export == .document
        ? try convertToMarkdownString(TRANSFORMERS)
        : try convertSelectionToMarkdownString(TRANSFORMERS, getSelection())
    }
    let space = " "
    let expected =
      switch export {
      case .document: "**a\(String(repeating: space, count: spaces))c**"
      case .textPoints: "**a**\(String(repeating: space, count: half))"
      case .elementPoints: "\(String(repeating: space, count: spaces - half))**c**"
      }
    #expect(result == expected)
  }
}
