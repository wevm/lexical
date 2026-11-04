import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts.

private let MarkdownShortcutTestExtension = defineExtension(
  name: "MarkdownShortcutTest",
  dependencies: [
    HistoryExtension, LinkExtension, RichTextExtension, ListExtension, CodeExtension,
  ],
  register: { editor, _, _ in try registerMarkdownShortcuts(editor) })

private func buildShortcutEditor() throws -> Editor {
  try buildEditorFromExtensions(MarkdownShortcutTestExtension)
}

private func typeMarkdown(_ editor: Editor, _ text: String) throws {
  try editor.update {
    let selection = try getSelection()
    if !((selection as? RangeSelection)?.isCollapsed() ?? false) {
      _ = try getRoot().selectEnd()
    }
  }
  for char in text.unicodeScalars {
    try editor.update(discrete: true) { try getSelection()?.insertText(String(char)) }
  }
  // Markdown shortcuts issues a cascading update that is not
  // discrete, so force sync reconciliation with a read.
  try editor.read {}
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(
    try getRoot().getFirstChildOrThrow() as? ParagraphNode, "Root child must be a paragraph")
}

@Suite struct MarkdownTransformersLinkTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > text before a markdown link is preserved"
    ))
  func textBeforeLinkPreserved() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "Start [test](url)")
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["Start ", "test"])
      #expect(children[1] is LinkNode, "Second child must be a LinkNode")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > formatted text before a markdown link is preserved"
    ))
  func formattedTextBeforeLinkPreserved() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "**Bold** [Link](url)")
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["Bold", " ", "Link"])
      let linkNode = try #require(children[2] as? LinkNode, "Third child must be a LinkNode")
      #expect(try linkNode.getTextContent() == "Link")
      #expect(try linkNode.getURL() == "url")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > LINK is not too greedy if there is a preceding match that was not processed"
    ))
  func linkNotTooGreedy() throws {
    // https://github.com/meta/lexical/issues/8129
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    // Set up initial condition, since we are not typing a character at a time
    // it's not handled by markdown shortcuts in this update
    try editor.update(discrete: true) {
      try getRoot().selectEnd().insertRawText(
        "[a](https://a.example.com) [b](https://b.example.com")
    }
    try typeMarkdown(editor, ")")
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(
        try children.map { try $0.getTextContent() } == ["[a](https://a.example.com) ", "b"])
      let linkNode = try #require(children[1] as? LinkNode, "Second child must be a LinkNode")
      #expect(try linkNode.getTextContent() == "b")
      #expect(try linkNode.getURL() == "https://b.example.com")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a destination between angle brackets keeps the whitespace in the URL"
    ))
  func angleBracketDestination() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "[test](<https://example.com/a b>)")
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(children.count == 1)
      let linkNode = try #require(children.first as? LinkNode, "Expected a link")
      #expect(try linkNode.getTextContent() == "test")
      #expect(try linkNode.getURL() == "https://example.com/a b")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a title ending in an escaped backslash closes at the next quote"
    ))
  func titleEndingInEscapedBackslash() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "[test](url \"a\\\\\")")
    try editor.read { () throws -> Void in
      let linkNode = try #require(
        try getRoot().getAllTextNodes()[0].getParent() as? LinkNode, "Expected a link")
      #expect(try linkNode.getURL() == "url")
      #expect(try linkNode.getTitle() == "a\\")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > a title whose closing quote is escaped makes no link"
    ))
  func escapedClosingQuote() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "[test](url \"a\\\")")
    try editor.read { () throws -> Void in
      #expect(try getRoot().getTextContent() == "[test](url \"a\\\")")
      #expect(try !getRoot().getAllTextNodes().contains { try $0.getParent() is LinkNode })
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::LINK > markdown link should not be created inside another link."
    ))
  func noLinkInsideLink() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().selectEnd().insertNodes([
        createParagraphNode().append(createLinkNode("link").append(createTextNode("hello")))
      ])
    }
    // moves the cursor inside the link text: hell‸o
    try editor.update(discrete: true) { _ = try getRoot().getAllTextNodes()[0].select(4, 4) }
    try typeMarkdown(editor, "[world](www)")
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(children.count == 1)
      let linkNode = try #require(children.first as? LinkNode, "Expected a link")
      #expect(try linkNode.getTextContent() == "hell[world](www)o")
      #expect(try linkNode.getURL() == "link")
    }
  }
}

/// The single text child of the first paragraph.
private func onlyText() throws -> TextNode {
  let children = try firstParagraph().getChildren()
  #expect(children.count == 1)
  return try #require(children.first as? TextNode, "Child must be a TextNode")
}

@Suite struct MarkdownTransformersCodeSpanPrecedenceTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ inside backticks is not formatted as bold"
    ))
  func underscoreBoldInBackticks() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "`__bold__`")
    try editor.read { () throws -> Void in
      let textNode = try onlyText()
      #expect(try textNode.getTextContent() == "__bold__")
      #expect(try textNode.hasFormat("code"))
      #expect(try !textNode.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > **bold** inside backticks is not formatted as bold"
    ))
  func starBoldInBackticks() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "`**bold**`")
    try editor.read { () throws -> Void in
      let textNode = try onlyText()
      #expect(try textNode.getTextContent() == "**bold**")
      #expect(try textNode.hasFormat("code"))
      #expect(try !textNode.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > *italic* inside backticks is not formatted as italic"
    ))
  func italicInBackticks() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "`*italic*`")
    try editor.read { () throws -> Void in
      let textNode = try onlyText()
      #expect(try textNode.getTextContent() == "*italic*")
      #expect(try textNode.hasFormat("code"))
      #expect(try !textNode.hasFormat("italic"))
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ without backticks still formats as bold"
    ))
  func underscoreBoldWithoutBackticks() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "__bold__")
    try editor.read { () throws -> Void in
      let textNode = try onlyText()
      #expect(try textNode.getTextContent() == "bold")
      #expect(try textNode.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::CODE_SPAN_PRECEDENCE > __bold__ after a completed code span still formats as bold"
    ))
  func boldAfterCodeSpan() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "`code` __bold__")
    try editor.read { () throws -> Void in
      let textNodes = try firstParagraph().getChildren().compactMap { $0 as? TextNode }
      let boldNode = try #require(try textNodes.first { try $0.hasFormat("bold") })
      #expect(try boldNode.getTextContent() == "bold")
    }
  }
}

@Suite struct MarkdownTransformersUnderscoreAfterPunctuationTests {
  // Underscore formats do not apply inside a word, so the character before the
  // opening tag has to be whitespace or punctuation. CommonMark counts every
  // Unicode punctuation (P) and symbol (S) character as punctuation, not only
  // the ASCII ones.
  // https://spec.commonmark.org/0.31.2/#unicode-punctuation-character
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing \"See –_here_\" formats the word between the underscores"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing \"See “_here_\" formats the word between the underscores"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing \"See 😀_here_\" formats the word between the underscores"
    ),
    arguments: [
      ("See \u{2013}_here_", "See \u{2013}"),
      ("See \u{201C}_here_", "See \u{201C}"),
      // An emoji (So) outside the Basic Multilingual Plane, read whole.
      ("See \u{1F600}_here_", "See \u{1F600}"),
    ])
  func formatsWordBetweenUnderscores(_ typed: String, _ before: String) throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, typed)
    try editor.read { () throws -> Void in
      let textNodes = try firstParagraph().getChildren().compactMap { $0 as? TextNode }
      #expect(try textNodes.map { try $0.getTextContent() } == [before, "here"])
      #expect(try textNodes.map { try $0.hasFormat("italic") } == [false, true])
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::UNDERSCORE_AFTER_UNICODE_PUNCTUATION > typing an underscore format right after a letter still does not format it"
    ))
  func underscoreAfterLetter() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "See \u{00E9}_here_")
    try editor.read { () throws -> Void in
      let textNodes = try firstParagraph().getChildren().compactMap { $0 as? TextNode }
      #expect(try textNodes.map { try $0.getTextContent() } == ["See \u{00E9}_here_"])
      #expect(try !textNodes[0].hasFormat("italic"))
    }
  }
}

@Suite struct MarkdownTransformersWrappingTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::WRAPPING_PRESERVES_FORMAT > **...** around already-bold text preserves bold"
    ))
  func wrappingBoldText() throws {
    // https://github.com/meta/lexical/issues/8727
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let textNode = try createTextNode("**bold*").toggleFormat("bold")
      try getRoot().selectEnd().insertNodes([createParagraphNode().append(textNode)])
      try textNode.selectEnd().setFormat(textNode.getFormat())
    }
    try typeMarkdown(editor, "*")
    try editor.read { () throws -> Void in
      let textNode = try onlyText()
      #expect(try textNode.getTextContent() == "bold")
      #expect(try textNode.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::WRAPPING_PRESERVES_FORMAT > **...** around mixed-format text formats every wrapped node bold"
    ))
  func wrappingMixedText() throws {
    // https://github.com/meta/lexical/issues/8727
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let plainNode = try createTextNode("**foo")
      let boldNode = try createTextNode("bar*")
      _ = try boldNode.toggleFormat("bold")
      try getRoot().selectEnd().insertNodes([
        createParagraphNode().append(plainNode).append(boldNode)
      ])
      try boldNode.selectEnd().setFormat(boldNode.getFormat())
    }
    try typeMarkdown(editor, "*")
    try editor.read { () throws -> Void in
      let paragraph = try firstParagraph()
      #expect(try paragraph.getTextContent() == "foobar")
      let textNodes = try paragraph.getChildren().compactMap { $0 as? TextNode }
      #expect(try textNodes.allSatisfy { try $0.hasFormat("bold") })
    }
  }
}

@Suite struct MarkdownTransformersHistoryTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownTransformers.test.ts::HISTORY > undo after markdown format transform preserves typed markdown text"
    ))
  func undoAfterFormatTransform() throws {
    let editor = try buildShortcutEditor()
    defer { editor.dispose() }
    try typeMarkdown(editor, "lorem *ipsum*")
    try editor.update(discrete: true) { _ = try editor.dispatchCommand(UNDO_COMMAND) }
    try editor.read { () throws -> Void in
      #expect(try firstParagraph().getTextContent() == "lorem *ipsum*")
    }
  }
}
