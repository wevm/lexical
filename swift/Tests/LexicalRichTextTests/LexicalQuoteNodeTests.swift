import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Ports of LexicalQuoteNode.test.ts, QuoteInsertNewAfter.test.ts and
// LexicalTabNode.test.ts from packages/lexical-rich-text/src/__tests__/unit.

/// The rich text nodes of upstream's `DEFAULT_NODES`.
var richTextTestNodes: [LexicalNodeConfig] { [.klass(HeadingNode.self), .klass(QuoteNode.self)] }

private var editorConfig: EditorConfig {
  EditorConfig(namespace: "", theme: ["quote": "my-quote-class"])
}

@Suite struct LexicalQuoteNodeTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let quoteNode = try createQuoteNode()
      #expect(quoteNode.getType() == "quote")
      #expect(try quoteNode.getTextContent() == "")
    }
    #expect(throws: LexicalError.self) { try createQuoteNode() }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let quoteNode = try createQuoteNode()
      #expect(
        try quoteNode.createDOM(editorConfig).outerHTML
          == #"<blockquote class="my-quote-class"></blockquote>"#)
      #expect(
        try quoteNode.createDOM(EditorConfig(namespace: "")).outerHTML
          == "<blockquote></blockquote>")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.updateDOM()"
    ))
  func updateDOM() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let quoteNode = try createQuoteNode()
      let domElement = try quoteNode.createDOM(editorConfig)
      #expect(domElement.outerHTML == #"<blockquote class="my-quote-class"></blockquote>"#)
      let newQuoteNode = try createQuoteNode()
      #expect(try newQuoteNode.updateDOM(quoteNode, domElement, editorConfig) == false)
      #expect(domElement.outerHTML == #"<blockquote class="my-quote-class"></blockquote>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > QuoteNode.insertNewAfter()"
    ))
  func insertNewAfter() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var quoteNode: QuoteNode!
    try update(editor) {
      quoteNode = try createQuoteNode()
      try getRoot().append(quoteNode)
    }
    #expect(try shape(editor) == "<quote></quote>")
    try update(editor) {
      let result = try quoteNode.insertNewAfter(createRangeSelection())
      #expect(result is ParagraphNode)
      #expect(try (result as? ElementNode)?.getDirection() == quoteNode.getDirection())
    }
    #expect(try shape(editor) == "<quote></quote><paragraph></paragraph>")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalQuoteNode.test.ts::LexicalQuoteNode tests > $createQuoteNode()"
    ))
  func createQuote() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let quoteNode = try createQuoteNode()
      let createdQuoteNode = try createQuoteNode()
      #expect(quoteNode.__type == createdQuoteNode.__type)
      #expect(quoteNode.__parent == createdQuoteNode.__parent)
      #expect(quoteNode.__key != createdQuoteNode.__key)
    }
  }
}

private struct BlockSnapshot: Equatable {
  var format: String
  var style: String
  var textFormat: Int
  var type: String
}

/// Puts the caret at the end of a centred, styled block and presses Enter,
/// then describes the continuation block.
private func pressEnterAtEndOf(
  _ createBlock: @escaping () throws -> ElementNode, _ boldPending: Bool
) throws -> BlockSnapshot {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[test-quote-insert-new-after]", dependencies: [RichTextExtension],
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  defer { editor.dispose() }
  try editor.setRootElement(TestRootElement())
  try editor.update(discrete: true) {
    let block = try createBlock()
    try block.setFormat("center")
    try block.setStyle("color: red")
    try block.append(createTextNode("hello"))
    try getRoot().clear().append(block)
    try block.selectEnd()
  }
  if boldPending {
    try editor.update(discrete: true) {
      try #require(try getSelection() as? RangeSelection).formatText("bold")
    }
  }
  try editor.update(discrete: true) {
    try #require(try getSelection() as? RangeSelection).insertParagraph()
  }
  return try editor.read {
    let children = try getRoot().getChildren()
    try #require(children.count == 2, "expected the block plus its continuation")
    let last = try #require(children.last as? ElementNode)
    return try BlockSnapshot(
      format: last.getFormatType(), style: last.getStyle(), textFormat: last.getTextFormat(),
      type: last.getType())
  }
}

@Suite struct QuoteInsertNewAfterTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteInsertNewAfter.test.ts::QuoteNode.insertNewAfter > keeps the block alignment and style, as ParagraphNode does"
    ))
  func keepsAlignmentAndStyle() throws {
    // The reference behaviour, from core ParagraphNode.insertNewAfter.
    let fromParagraph = try pressEnterAtEndOf({ try createParagraphNode() }, false)
    #expect(fromParagraph.format == "center")
    #expect(fromParagraph.style == "color: red")
    #expect(fromParagraph.type == "paragraph")
    let fromQuote = try pressEnterAtEndOf({ try createQuoteNode() }, false)
    #expect(fromQuote.format == "center")
    #expect(fromQuote.style == "color: red")
    #expect(fromQuote.type == "paragraph")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteInsertNewAfter.test.ts::QuoteNode.insertNewAfter > seeds the continuation block with the pending text format"
    ))
  func seedsPendingTextFormat() throws {
    let fromParagraph = try pressEnterAtEndOf({ try createParagraphNode() }, true)
    let fromQuote = try pressEnterAtEndOf({ try createQuoteNode() }, true)
    #expect(fromParagraph.textFormat != 0)
    #expect(fromQuote.textFormat == fromParagraph.textFormat)
  }
}

@Suite struct RichTextTabNodeTests {
  private func boldAB(_ editor: Editor) throws {
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.select()
    }
    try update(editor) {
      try #require(try getSelection() as? RangeSelection).insertText("ab")
      try #require(try getRoot().getFirstDescendant() as? TextNode).select(0, 2)
    }
    try editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold")
    drainMicrotasks()
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalTabNode.test.ts::LexicalTabNode tests > INSERT_TAB_COMMAND applies selection format and style to TabNode"
    ))
  func insertTabAppliesFormat() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    _ = registerRichText(editor)
    try boldAB(editor)
    try editor.dispatchCommand(INSERT_TAB_COMMAND)
    drainMicrotasks()
    try editor.read {
      let tabNode = try #require(try getRoot().getAllTextNodes().first { $0.getType() == "tab" })
      #expect(try tabNode.getFormat() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalTabNode.test.ts::LexicalTabNode tests > format preserved when typing between tabs inserted in bold text"
    ))
  func formatPreservedBetweenTabs() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    _ = registerRichText(editor)
    try boldAB(editor)
    try update(editor) {
      try #require(try getRoot().getFirstDescendant() as? TextNode).select(1, 1)
    }
    try editor.dispatchCommand(INSERT_TAB_COMMAND)
    try editor.dispatchCommand(INSERT_TAB_COMMAND)
    drainMicrotasks()
    try update(editor) {
      let tabNodes = try getRoot().getAllTextNodes().filter { $0.getType() == "tab" }
      #expect(tabNodes.count == 2)
      try tabNodes[1].selectStart()
    }
    try update(editor) {
      try #require(try getSelection() as? RangeSelection).insertText("x")
    }
    try editor.read {
      let xNode = try #require(
        try getRoot().getAllTextNodes().first { try $0.getTextContent() == "x" })
      #expect(try xNode.getFormat() == 1)
    }
  }
}
