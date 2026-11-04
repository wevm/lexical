import Lexical
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import Testing

// Port of the `$convertSelectionToMarkdownString` describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.

/// Builds the document and selection in `setUp`, then exports the selection.
private func exportSelection(_ setUp: @escaping () throws -> Void) throws -> String {
  let editor = try createMarkdownTestEditor()
  try editor.update(discrete: true) { try setUp() }
  return try editor.read(.latest) {
    try convertSelectionToMarkdownString(TRANSFORMERS, getSelection())
  }
}

private func selectRange(
  _ anchor: TextNode, _ anchorOffset: TextOffset, _ focus: TextNode, _ focusOffset: TextOffset
) throws {
  try setSelectionFromCaretRange(
    getCaretRange(
      getTextPointCaret(anchor, .next, anchorOffset), getTextPointCaret(focus, .next, focusOffset)
    ))
}

@Suite struct ConvertSelectionToMarkdownStringTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts full selection to markdown"
    ))
  func fullSelection() throws {
    let result = try exportSelection {
      let text = try createTextNode("Hello World")
      try getRoot().append(createParagraphNode().append(text))
      try text.select(0)
    }
    #expect(result == "Hello World")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts partial text selection to markdown"
    ))
  func partialTextSelection() throws {
    let result = try exportSelection {
      let text = try createTextNode("Hello World")
      try getRoot().append(createParagraphNode().append(text))
      try text.select(6, 11)
    }
    #expect(result == "World")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts selection with bold text to markdown"
    ))
  func boldText() throws {
    let result = try exportSelection {
      let normal = try createTextNode("Hello ")
      let bold = try createTextNode("Bold").toggleFormat("bold")
      let after = try createTextNode(" World")
      try getRoot().append(createParagraphNode().append(normal, bold, after))
      try selectRange(normal, 0, bold, 4)
    }
    #expect(result == "Hello **Bold**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > does not prefix a newline when the selection starts after the first block"
    ))
  func noNewlinePrefixAfterFirstBlock() throws {
    let result = try exportSelection {
      let firstText = try createTextNode("First")
      let secondText = try createTextNode("Second")
      let thirdText = try createTextNode("Third")
      try getRoot().append(
        createParagraphNode().append(firstText), createParagraphNode().append(secondText),
        createParagraphNode().append(thirdText))
      try selectRange(secondText, 0, thirdText, 5)
    }
    #expect(result == "Second\n\nThird")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > returns empty string for null selection"
    ))
  func nullSelection() throws {
    #expect(try convertSelectionToMarkdownString(TRANSFORMERS, nil) == "")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > returns empty string for collapsed selection"
    ))
  func collapsedSelection() throws {
    let result = try exportSelection {
      let text = try createTextNode("Hello World")
      try getRoot().append(createParagraphNode().append(text))
      try text.select(5, 5)
    }
    #expect(result == "")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts backward selection to markdown"
    ))
  func backwardSelection() throws {
    let result = try exportSelection {
      let text = try createTextNode("Hello World")
      try getRoot().append(createParagraphNode().append(text))
      try text.select(5, 0)
    }
    #expect(result == "Hello")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts multi-paragraph selection to markdown"
    ))
  func multiParagraphSelection() throws {
    let result = try exportSelection {
      let t1 = try createTextNode("First paragraph")
      let t2 = try createTextNode("Second paragraph")
      try getRoot().append(createParagraphNode().append(t1), createParagraphNode().append(t2))
      try selectRange(t1, 0, t2, .direction(.next))
    }
    #expect(result == "First paragraph\n\nSecond paragraph")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts selection within a list to markdown"
    ))
  func selectionWithinList() throws {
    let result = try exportSelection {
      let t1 = try createTextNode("Item 1")
      let t2 = try createTextNode("Item 2")
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(t1), createListItemNode().append(t2)))
      try selectRange(t1, 0, t2, .direction(.next))
    }
    #expect(result == "- Item 1\n- Item 2")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > preserves link when partially selecting link text"
    ))
  func partialLinkText() throws {
    let result = try exportSelection {
      let linkText = try createTextNode("link text")
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("before "), createLinkNode("https://example.com").append(linkText),
          createTextNode(" after")))
      try linkText.select(0, 4)
    }
    #expect(result == "[link](https://example.com)")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > list partial selection only includes selected items"
    ))
  func listPartialSelection() throws {
    let result = try exportSelection {
      let t1 = try createTextNode("Item 1")
      let t2 = try createTextNode("Item 2")
      let t3 = try createTextNode("Item 3")
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(t1), createListItemNode().append(t2),
          createListItemNode().append(t3)))
      try selectRange(t2, 0, t3, 2)
    }
    #expect(result == "- Item 2\n- It")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > quote partial selection only includes selected text"
    ))
  func quotePartialSelection() throws {
    let result = try exportSelection {
      let t2 = try createTextNode("Line 2")
      try getRoot().append(
        createQuoteNode().append(createTextNode("Line 1"), createLineBreakNode(), t2))
      try t2.select(0)
    }
    #expect(result == "> Line 2")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > nested list partial selection only includes selected nested items"
    ))
  func nestedListPartialSelection() throws {
    let result = try exportSelection {
      let nt1 = try createTextNode("Nested A")
      let nestedList = try createListNode("bullet").append(
        createListItemNode().append(nt1),
        createListItemNode().append(createTextNode("Nested B")))
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("Item 1")),
          createListItemNode().append(nestedList),
          createListItemNode().append(createTextNode("Item 3"))))
      try nt1.select(0)
    }
    #expect(result == "    - Nested A")
  }
}

// Port of the `$convertSelectionToMarkdownString whitespace slices` describe
// block.
@Suite struct ConvertSelectionToMarkdownStringWhitespaceSliceTests {
  private func createEditor() throws -> Editor {
    try createMarkdownTestEditor(nodes: [.klass(LinkNode.self)])
  }

  private func export(_ editor: Editor) throws -> String {
    try editor.read(.latest) {
      try convertSelectionToMarkdownString(TRANSFORMERS, getSelection())
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > does not emit a dangling closing tag when the selection slices a format down to whitespace"
    ))
  func noDanglingClosingTag() throws {
    let editor = try createEditor()
    try editor.update(discrete: true) {
      let first = try createTextNode("a  ").toggleFormat("bold").setStyle("color: red")
      let second = try createTextNode("b").toggleFormat("bold")
      try getRoot().append(createParagraphNode().append(first, second))
      try selectRange(first, 1, second, 1)
    }
    #expect(try export(editor) == "  **b**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes a tag before a formatted space at the end of a selected link"
    ))
  func closesBeforeFormattedSpaceAtLinkEnd() throws {
    let editor = try createEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(
        createLinkNode("u").append(
          createTextNode("a").setFormat(IS_BOLD | IS_ITALIC),
          createTextNode(" ").setFormat(IS_BOLD)),
        createTextNode("c"))
      try getRoot().append(paragraph)
      try paragraph.select(0, paragraph.getChildrenSize())
    }
    #expect(try export(editor) == "[***a*** ](u)c")
  }

  struct SliceCase: Sendable, CustomTestStringConvertible {
    let name: String
    let formats: [(text: String, format: Int)]
    let selection: (startNode: Int, start: Int, endNode: Int, end: Int)
    let expected: String
    var testDescription: String { name }
  }

  static let sliceCases: [SliceCase] = [
    SliceCase(
      name: "closes the tags of the previous node before a slice cut down to whitespace",
      formats: [("a", IS_BOLD | IS_ITALIC), (" b", IS_BOLD)], selection: (0, 0, 1, 1),
      expected: "***a*** "),
    SliceCase(
      name: "keeps a tag open across consecutive formatted spaces",
      formats: [
        ("a", IS_BOLD), (" ", IS_BOLD | IS_ITALIC), (" ", IS_BOLD | IS_STRIKETHROUGH),
        ("cd", IS_BOLD),
      ], selection: (0, 0, 3, 1), expected: "**a  c**"),
    SliceCase(
      name: "closes before a slice cut down to a formatted space",
      formats: [("a ", IS_BOLD), (" b", IS_BOLD | IS_ITALIC)], selection: (0, 0, 1, 1),
      expected: "**a**  "),
    SliceCase(
      name: "closes when the format continues only after the selection",
      formats: [("a", IS_BOLD | IS_ITALIC), (" x", IS_BOLD), ("c", IS_BOLD | IS_ITALIC)],
      selection: (0, 0, 1, 1), expected: "***a*** "),
    SliceCase(
      name: "closes every tag at the end of a sliced node",
      formats: [("a", IS_BOLD), ("bc", IS_BOLD | IS_ITALIC), ("d", IS_BOLD)],
      selection: (0, 0, 1, 1), expected: "**a*b***"),
    SliceCase(
      name: "leaves out inline code the selection only touches",
      formats: [("a", IS_BOLD), ("xy", IS_BOLD | IS_CODE)], selection: (0, 0, 1, 0),
      expected: "**a**"),
    SliceCase(
      name: "leaves out inline code the selection starts at the end of",
      formats: [("xy", IS_BOLD | IS_CODE), ("a", IS_BOLD)], selection: (0, 2, 1, 1),
      expected: "**a**"),
  ]

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes the tags of the previous node before a slice cut down to whitespace"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > keeps a tag open across consecutive formatted spaces"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes before a slice cut down to a formatted space"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes when the format continues only after the selection"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes every tag at the end of a sliced node"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > leaves out inline code the selection only touches"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > leaves out inline code the selection starts at the end of"
    ), arguments: sliceCases)
  func slice(_ slice: SliceCase) throws {
    let editor = try createEditor()
    try editor.update(discrete: true) {
      let nodes = try slice.formats.map { try createTextNode($0.text).setFormat($0.format) }
      try getRoot().append(createParagraphNode().append(contentsOf: nodes))
      let (startNode, start, endNode, end) = slice.selection
      try selectRange(nodes[startNode], .offset(start), nodes[endNode], .offset(end))
    }
    #expect(try export(editor) == slice.expected)
  }
}
