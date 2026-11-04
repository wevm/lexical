import Lexical
import LexicalCodeCore
import LexicalHTML
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import LexicalTesting
import Testing

// Port of the `Markdown` describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts, less
// the loops over `IMPORT_AND_EXPORT`, which ConformanceTests' MarkdownTests
// replays from vectors.

/// `editor.update(fn, {discrete: true})`.
private func discreteUpdate(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(fn, EditorUpdateOptions(discrete: true))
}

/// `$getSelection()` when it is a range selection.
private func rangeSelection() throws -> RangeSelection? {
  try getSelection() as? RangeSelection
}

/// Types `text` one character per discrete update, as the upstream tests do.
private func typeCharacters(_ editor: Editor, _ text: String) throws {
  for character in text {
    try discreteUpdate(editor) { try rangeSelection()?.insertText(String(character)) }
  }
}

private func html(_ editor: Editor) throws -> String {
  try editor.read { try generateHtmlFromNodes(editor) }
}

nonisolated(unsafe) private let CANCELED_HEADING_REPLACE_EXAMPLE = ElementTransformer(
  dependencies: [HeadingNode.self],
  export: { _, _, _ in nil },
  regExp: JSRegExp(#"^(#{1,6})\s"#),
  replace: { _, _, _, _ in false })

@Suite struct MarkdownDescribeTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should not remove leading node and transform if replace returns false"
    ))
  func cancelledReplaceKeepsText() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, [CANCELED_HEADING_REPLACE_EXAMPLE])
    try discreteUpdate(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectEnd()
      try rangeSelection()?.insertText("#")
    }
    try discreteUpdate(editor) { try rangeSelection()?.insertText(" ") }
    #expect(try html(editor) == #"<p><span style="white-space: pre-wrap;"># </span></p>"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should remove leading node and execute transform if replace does not return false"
    ))
  func replaceRunsTransform() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, [HEADING])
    try discreteUpdate(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectEnd()
      try rangeSelection()?.insertText("#")
    }
    try discreteUpdate(editor) { try rangeSelection()?.insertText(" ") }
    #expect(try html(editor) == "<h1><br></h1>")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the \"1. \" list shortcut"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the \"- \" list shortcut"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the \"* \" list shortcut"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the \"+ \" list shortcut"
    ),
    arguments: ["1. ", "- ", "* ", "+ "])
  func headingKeepsListShortcut(_ shortcut: String) throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    try discreteUpdate(editor) {
      let heading = try createHeadingNode("h1")
      let text = try createTextNode("Welcome to the playground")
      try heading.append(text)
      try getRoot().append(heading)
      try text.select(0, 0)
    }
    try typeCharacters(editor, shortcut)
    try editor.read { () throws -> Void in
      let heading = try getRoot().getFirstChild()
      #expect(isHeadingNode(heading))
      #expect(try heading?.getTextContent() == "\(shortcut)Welcome to the playground")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"# \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"## \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"###### \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"1. \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"- \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"* \" shortcut (#7407)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the \"+ \" shortcut (#7407)"
    ),
    arguments: ["# ", "## ", "###### ", "1. ", "- ", "* ", "+ "])
  func quoteKeepsShortcut(_ shortcut: String) throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    try discreteUpdate(editor) {
      let quote = try createQuoteNode()
      let text = try createTextNode("Welcome to the playground")
      try quote.append(text)
      try getRoot().append(quote)
      try text.select(0, 0)
    }
    try typeCharacters(editor, shortcut)
    try editor.read { () throws -> Void in
      let quote = try getRoot().getFirstChild()
      #expect(isQuoteNode(quote))
      #expect(try quote?.getTextContent() == "\(shortcut)Welcome to the playground")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when the code fence shortcut is typed in it (#7407)"
    ))
  func quoteKeepsCodeFence() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    try discreteUpdate(editor) {
      let quote = try createQuoteNode()
      let text = try createTextNode("")
      try quote.append(text)
      try getRoot().append(quote)
      try text.select(0, 0)
    }
    try typeCharacters(editor, "```")
    _ = try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    try editor.read { () throws -> Void in
      let quote = try getRoot().getFirstChild()
      #expect(isQuoteNode(quote))
      #expect(try quote?.getTextContent() == "```")
    }
  }

  private func expectCodeRoundTrip(_ markdown: String) throws {
    let editor = try createMarkdownTestEditor()
    try discreteUpdate(editor) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
    #expect(try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) } == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > can round-trip nested fenced code blocks (4 backticks wrapping 3 backticks)"
    ))
  func nestedFences() throws {
    try expectCodeRoundTrip(
      "````markdown\n" + "# Example\n" + "\n" + "Run this:\n" + "```bash\n" + "npm install\n"
        + "```\n" + "````")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > can round-trip deeply nested fenced code blocks (5 backticks wrapping 4 backticks)"
    ))
  func deeplyNestedFences() throws {
    try expectCodeRoundTrip(
      "`````text\n" + "Top Level 5 ticks\n" + "````markdown\n" + "Level 2 (4 ticks)\n"
        + "```javascript\n" + "console.log(\"Deepest (3 ticks)\");\n" + "```\n" + "````\n"
        + "`````")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > computes fence dynamically when code block content contains backticks"
    ))
  func dynamicFence() throws {
    let editor = try createMarkdownTestEditor(nodes: [.klass(CodeNode.self)])
    try discreteUpdate(editor) {
      // A CodeNode without a fence state (so the default ```), whose content
      // contains ```.
      let codeBlockNode = try createCodeNode("markdown")
      try getRoot().append(codeBlockNode).selectEnd()
        .insertRawText("```js\nconsole.log(\"hello\");\n```")
    }
    // The export makes the fence ```` since the content contains ```.
    let exported = try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
    #expect(exported == "````markdown\n```js\nconsole.log(\"hello\");\n```\n````")
  }
}

// MARK: - overlapping inline formats (#4895)

private struct Run: Equatable, CustomStringConvertible {
  var text: String
  var format: Int
  init(_ text: String, _ format: Int) {
    self.text = text
    self.format = format
  }
  var description: String { "[\(text.debugDescription), \(format)]" }
}

private let BOLD = TEXT_TYPE_TO_FORMAT["bold"]!
private let ITALIC = TEXT_TYPE_TO_FORMAT["italic"]!
private let STRIKE = TEXT_TYPE_TO_FORMAT["strikethrough"]!
private let INLINE_CODE = TEXT_TYPE_TO_FORMAT["code"]!

/// `$textRuns()`: the document as merged (text, format) runs.
private func textRuns() throws -> [Run] {
  var out: [Run] = []
  for node in try getRoot().getAllTextNodes() {
    let format = try node.getFormat()
    let text = try node.getTextContent()
    if let last = out.last, last.format == format {
      out[out.count - 1].text += text
    } else {
      out.append(Run(text, format))
    }
  }
  return out
}

@Suite struct MarkdownOverlappingInlineFormatsTests {
  /// Builds a one-paragraph document from the runs, checks its export, and
  /// that importing the export restores the runs.
  private func expectRoundTrip(_ runs: [Run], _ expected: String) throws {
    let editor = try createMarkdownTestEditor()
    try discreteUpdate(editor) {
      let paragraph = try createParagraphNode()
      for run in runs {
        try paragraph.append(createTextNode(run.text).setFormat(run.format))
      }
      try getRoot().clear().append(paragraph)
    }
    let markdown = try editor.read { try convertToMarkdownString(TRANSFORMERS) }
    #expect(markdown == expected)
    let reimported = try createMarkdownTestEditor()
    try discreteUpdate(reimported) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
    #expect(try reimported.read(textRuns) == runs)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips bold overlapping italic (the issue example)"
    ))
  func boldOverlappingItalic() throws {
    try expectRoundTrip(
      [Run("he", 0), Run("llo", BOLD), Run("wor", BOLD | ITALIC), Run("ld", ITALIC), Run("!", 0)],
      "he**llo*wor****ld*!")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips italic overlapping bold"
    ))
  func italicOverlappingBold() throws {
    try expectRoundTrip(
      [Run("a", 0), Run("b", ITALIC), Run("c", ITALIC | BOLD), Run("d", BOLD), Run("e", 0)],
      "a*b**c*****d**e")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips italic then bold italic between punctuation"
    ))
  func italicThenBoldItalic() throws {
    try expectRoundTrip(
      [Run(")", ITALIC), Run("(", BOLD), Run(")", BOLD | ITALIC)], "*)***(*)***")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips strikethrough overlapping bold"
    ))
  func strikethroughOverlappingBold() throws {
    try expectRoundTrip(
      [Run("a", STRIKE), Run("b", STRIKE | BOLD), Run("c", BOLD)], "~~a**b**~~**c**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips a code span inside a bold run"
    ))
  func codeSpanInsideBold() throws {
    try expectRoundTrip(
      [Run("a", BOLD), Run("b", BOLD | INLINE_CODE), Run("c", BOLD)], "**a`b`c**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > imports a partially consumed delimiter run like CommonMark"
    ))
  func partiallyConsumedDelimiterRun() throws {
    // `**llo*wor****ld*` pairs `*wor*` first, leaving `***` of the four-marker
    // run; the remaining `**` must still close the `**` opener and the final
    // `*` opens the trailing emphasis.
    let editor = try createMarkdownTestEditor()
    try discreteUpdate(editor) {
      try convertFromMarkdownString("he**llo*wor****ld*!", TRANSFORMERS)
    }
    #expect(
      try editor.read(textRuns) == [
        Run("he", 0), Run("llo", BOLD), Run("wor", BOLD | ITALIC), Run("ld", ITALIC), Run("!", 0),
      ])
  }
}

// MARK: - formatted whitespace-only text (#9403)

private struct FormattedChar: Equatable, CustomStringConvertible {
  var char: String
  var format: Int
  var url: String?
  var description: String { "[\(char.debugDescription), \(format), \(url ?? "null")]" }
}

private func importMarkdown(_ markdown: String) throws -> Editor {
  let editor = try createMarkdownTestEditor()
  try discreteUpdate(editor) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
  return editor
}

/// `$chars()`: the format and link URL of every non-whitespace character.
/// Whitespace is left out because an exported tag cannot wrap it.
private func formattedChars() throws -> [FormattedChar] {
  var chars: [FormattedChar] = []
  for node in try getRoot().getAllTextNodes() {
    let parent = try node.getParent()
    let url = try (parent as? LinkNode)?.getURL()
    for scalar in try node.getTextContent().unicodeScalars {
      let char = String(scalar)
      if jsTrim(char) != "" {
        chars.append(FormattedChar(char: char, format: try node.getFormat(), url: url))
      }
    }
  }
  return chars
}

private func rootText() throws -> String { try getRoot().getTextContent() }
private func exportMarkdown() throws -> String { try convertToMarkdownString(TRANSFORMERS) }

private func formatted(_ text: String, _ format: Int) throws -> TextNode {
  try createTextNode(text).setFormat(format)
}

/// A case of the whitespace-only export table: the nodes of the paragraph
/// and the expected export, a string or a pattern.
struct WhitespaceOnlyExportCase: Sendable, CustomTestStringConvertible {
  let name: String
  let nodes: @Sendable () throws -> [LexicalNode]
  let expected: String
  let isPattern: Bool
  var testDescription: String { name }
}

private let whitespaceOnlyExportCases: [WhitespaceOnlyExportCase] = [
  WhitespaceOnlyExportCase(
    name: "closes before unformatted text",
    nodes: {
      [try formatted("a", BOLD | ITALIC), try formatted(" ", BOLD), try formatted("b", 0)]
    },
    expected: "***a*** b", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "closes at the end of a link",
    nodes: {
      [
        try createLinkNode("u").append(formatted("a", BOLD | ITALIC), formatted(" ", BOLD)),
        try formatted("c", 0),
      ]
    },
    expected: "[***a*** ](u)c", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "closes before consecutive formatted spaces",
    nodes: {
      [
        try formatted("a", BOLD), try formatted(" ", BOLD | ITALIC),
        try formatted(" ", BOLD | STRIKE), try formatted("c", 0),
      ]
    },
    expected: "**a**  c", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "closes before the trailing space of the text it formats",
    nodes: {
      [try formatted("a ", BOLD), try formatted(" ", BOLD | ITALIC), try formatted("c", 0)]
    },
    expected: "**a**  c", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "does not reopen a tag it kept open",
    nodes: {
      [try formatted("a", BOLD), try formatted(" ", BOLD | ITALIC), try formatted("c", BOLD)]
    },
    expected: "**a c**", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "keeps a tag open across consecutive formatted spaces",
    nodes: {
      [
        try formatted("a", BOLD), try formatted(" ", BOLD | ITALIC),
        try formatted(" ", BOLD | STRIKE), try formatted("c", BOLD),
      ]
    },
    expected: "**a  c**", isPattern: false),
  WhitespaceOnlyExportCase(
    name: "closes before a formatted line break",
    nodes: {
      [try formatted("a", BOLD), try formatted("\n", BOLD | ITALIC), try formatted("c", BOLD)]
    },
    expected: "**a**\n**c**", isPattern: false),
  // The code span's padding is not what the inline code cases test, so
  // their expected export leaves it open.
  WhitespaceOnlyExportCase(
    name: "leaves the formats of inline code that holds only a space off",
    nodes: {
      [try formatted("a", 0), try formatted(" ", BOLD | INLINE_CODE), try formatted("b", 0)]
    },
    expected: "^a` +`b$", isPattern: true),
  WhitespaceOnlyExportCase(
    name: "keeps a tag open across inline code that holds only a space",
    nodes: {
      [try formatted("a", BOLD), try formatted(" ", BOLD | INLINE_CODE), try formatted("b", BOLD)]
    },
    expected: #"^\*\*a` +`b\*\*$"#, isPattern: true),
]

@Suite struct MarkdownFormattedWhitespaceOnlyTextTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips \"**_a_ [b](u)**\" with a stable export"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips \"**_a_ [b](u) c**\" with a stable export"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips \"*a **b** [c](u)*\" with a stable export"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips \"**a [b](u)**\" with a stable export"
    ),
    arguments: ["**_a_ [b](u)**", "**_a_ [b](u) c**", "*a **b** [c](u)*", "**a [b](u)**"])
  func stableRoundTrip(_ original: String) throws {
    let first = try importMarkdown(original)
    let chars = try first.read(formattedChars)
    // Every delimiter was consumed and the link was parsed.
    #expect(!chars.contains { $0.char == "*" || $0.char == "_" })
    #expect(chars.contains { $0.url != nil })

    let exported = try first.read(exportMarkdown)
    let second = try importMarkdown(exported)
    #expect(try second.read(formattedChars) == chars)
    #expect(try second.read(rootText) == first.read(rootText))
    #expect(try second.read(exportMarkdown) == exported)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before unformatted text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes at the end of a link"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before consecutive formatted spaces"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before the trailing space of the text it formats"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > does not reopen a tag it kept open"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > keeps a tag open across consecutive formatted spaces"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before a formatted line break"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > leaves the formats of inline code that holds only a space off"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > keeps a tag open across inline code that holds only a space"
    ),
    arguments: whitespaceOnlyExportCases)
  func whitespaceOnlyExport(_ testCase: WhitespaceOnlyExportCase) throws {
    let editor = try createMarkdownTestEditor()
    try discreteUpdate(editor) {
      try getRoot().clear().append(createParagraphNode().append(contentsOf: testCase.nodes()))
    }
    let exported = try editor.read(exportMarkdown)
    if testCase.isPattern {
      #expect(JSRegExp(testCase.expected).test(exported), "\(exported.debugDescription)")
    } else {
      #expect(exported == testCase.expected)
    }
    #expect(try importMarkdown(exported).read(formattedChars) == editor.read(formattedChars))
  }
}

// MARK: - list marker

@Suite struct MarkdownListMarkerMemoryTests {
  private func listEditor() throws -> Editor {
    try createMarkdownTestEditor(nodes: [.klass(ListNode.self), .klass(ListItemNode.self)])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember marker used on import"
    ))
  func markerOnImport() throws {
    let editor = try listEditor()
    try discreteUpdate(editor) {
      try convertFromMarkdownString("+ hello", TRANSFORMERS, nil, true, false)
    }
    try editor.read(.latest) { () throws -> Void in
      let node = try getRoot().getFirstChild()
      #expect(node is ListNode)
      #expect(try node.map { try getState($0, listMarkerState) } == "+")
    }
  }

  private func typeCheckList(_ text: String) throws -> String? {
    let editor = try listEditor()
    try registerMarkdownShortcuts(editor, [CHECK_LIST])
    try discreteUpdate(editor) {
      try getRoot().clear().append(createParagraphNode()).selectEnd().insertText(text)
    }
    try discreteUpdate(editor) { try #require(try getSelection()).insertText(" ") }
    return try editor.read { () throws -> String? in
      let node = try getRoot().getFirstChild()
      #expect(node is ListNode)
      return try node.map { try getState($0, listMarkerState) }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should not use [ as a marker for an implicit check list"
    ))
  func implicitCheckListMarker() throws {
    #expect(try typeCheckList("[]") == "-")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember the marker for checkbox with an explicit marker"
    ))
  func explicitCheckListMarker() throws {
    #expect(try typeCheckList("* []") == "*")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember marker used on export"
    ))
  func markerOnExport() throws {
    let editor = try listEditor()
    try discreteUpdate(editor) {
      let listNode = try createListNode()
      try setState(listNode, listMarkerState, "+")
      let listItemNode = try createListItemNode()
      try listItemNode.append(createTextNode("hello"))
      try listNode.append(listItemNode)
      try listNode.setListType("bullet")
      try getRoot().select()
      try insertNodes([listNode])
    }
    try editor.read(.latest) { () throws -> Void in
      #expect(try convertToMarkdownString(TRANSFORMERS, nil, true) == "+ hello")
    }
  }
}

// MARK: - Enter key triggers

private func typeLineAndPressEnter(_ editor: Editor, _ text: String) throws {
  try discreteUpdate(editor) {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    try paragraph.selectEnd()
    try rangeSelection()?.insertText(text)
  }
  try discreteUpdate(editor) { _ = try editor.dispatchCommand(KEY_ENTER_COMMAND, nil) }
}

@Suite struct MarkdownEnterKeyTriggersTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should create an empty code block when ``` is typed and Enter is pressed"
    ))
  func emptyCodeBlock() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    try typeLineAndPressEnter(editor, "```")
    #expect(try html(editor) == #"<pre spellcheck="false"></pre>"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should create a code block with language when ```javascript is typed and Enter is pressed"
    ))
  func codeBlockWithLanguage() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    try typeLineAndPressEnter(editor, "```javascript")
    #expect(try html(editor) == #"<pre spellcheck="false" data-language="javascript"></pre>"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should not transform on Enter when replace returns false"
    ))
  func cancelledReplaceOnEnter() throws {
    let canceledCode = MultilineElementTransformer(
      dependencies: [CodeNode.self],
      regExpStart: JSRegExp(#"^`{3,}(\w+)?"#),
      regExpEnd: MultilineRegExpEnd(optional: true, regExp: JSRegExp(#"^[ \t]*`{3,}$"#)),
      replace: { _, _, _, _, _, _ in false })
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, [canceledCode])
    try typeLineAndPressEnter(editor, "```")
    #expect(try html(editor) == #"<p><span style="white-space: pre-wrap;">```</span></p>"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should transform element markdown on Enter when trailing space was not required (custom element transformer)"
    ))
  func elementTriggeredOnEnter() throws {
    let elementTriggeredFence = ElementTransformer(
      dependencies: [CodeNode.self],
      export: { _, _, _ in nil },
      regExp: JSRegExp(#"^`{3,}(\w+)?$"#),
      replace: { parentNode, children, match, isImport in
        let node = try createCodeNode(match[1])
        try node.append(contentsOf: children)
        try parentNode.replace(node)
        if !isImport {
          try node.select(0, 0)
        }
        return nil
      },
      triggerOnEnter: true)
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, [elementTriggeredFence])
    try typeLineAndPressEnter(editor, "```")
    #expect(try html(editor) == #"<pre spellcheck="false"></pre>"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should transform heading on Enter when a line was inserted at once (no trailing space listener trigger)"
    ))
  func headingOnEnter() throws {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, [HEADING])
    try typeLineAndPressEnter(editor, "## ")
    #expect(try html(editor) == "<h2><br></h2>")
  }
}

// MARK: - composition-end trigger characters (#7026)

@Suite struct MarkdownCompositionEndTriggerTests {
  private func buildEditor() throws -> Editor {
    let editor = try createMarkdownTestEditor()
    try registerMarkdownShortcuts(editor, TRANSFORMERS)
    return editor
  }

  private func getTextNode() throws -> TextNode {
    let paragraph = try #require(
      try getRoot().getFirstChildOrThrow() as? ParagraphNode, "expected ParagraphNode at root[0]")
    return try #require(
      try paragraph.getFirstChildOrThrow() as? TextNode, "expected TextNode at paragraph[0]")
  }

  private func setUpBacktickHello(_ editor: Editor) throws {
    try discreteUpdate(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(createTextNode("`hello"))
      try paragraph.selectEnd()
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > composition-end trigger characters (#7026) > applies inline code when the closing backtick is committed via compositionend"
    ))
  func closingBacktickViaCompositionEnd() throws {
    let editor = try buildEditor()
    // "`hello" with the caret at offset 6.
    try setUpBacktickHello(editor)
    // compositionupdate in progress: the backtick is in the text node, the
    // caret after it, and the editor composing.
    try discreteUpdate(editor) {
      let textNode = try getTextNode()
      try setCompositionKey(textNode.getKey())
      try textNode.setTextContent("`hello`")
      try textNode.select(7, 7)
    }
    // compositionend: the text is in place, the caret stays, and the
    // COMPOSITION_END_TAG is added.
    try discreteUpdate(editor) {
      try setCompositionKey(nil)
      try addUpdateTag(COMPOSITION_END_TAG)
      try getTextNode().markDirty()
    }
    #expect(
      try html(editor)
        == #"<p><code spellcheck="false" style="white-space: pre-wrap;"><span>hello</span></code></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > composition-end trigger characters (#7026) > applies inline code when compositionend commits the trigger as a multi-character chunk"
    ))
  func multiCharacterCompositionEnd() throws {
    let editor = try buildEditor()
    // The caret stays at offset 6 across the composition, then jumps to 8
    // when compositionend commits "a`".
    try setUpBacktickHello(editor)
    try discreteUpdate(editor) { try setCompositionKey(getTextNode().getKey()) }
    try discreteUpdate(editor) {
      try setCompositionKey(nil)
      try addUpdateTag(COMPOSITION_END_TAG)
      let textNode = try getTextNode()
      try textNode.setTextContent("`helloa`")
      try textNode.select(8, 8)
    }
    #expect(
      try html(editor)
        == #"<p><code spellcheck="false" style="white-space: pre-wrap;"><span>helloa</span></code></p>"#
    )
  }
}
