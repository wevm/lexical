import Foundation
import Lexical
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts.

/// `TestInlineNode`: an inline element that exports only its children,
/// like a comment mark.
private final class EscapingTestInlineNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(EscapingTestInlineNode.self, type: "markdown-test-inline", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
}

/// `TestDecoratorNode`: an inline decorator exported as its text content.
private class EscapingTestDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      EscapingTestDecoratorNode.self, type: "markdown-test-decorator", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func getTextContent() throws -> String { "foo" }
}

/// `TestXDecoratorNode`.
private final class EscapingTestXDecoratorNode: EscapingTestDecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      EscapingTestXDecoratorNode.self, type: "markdown-test-x-decorator",
      extends: EscapingTestDecoratorNode.self)
  }
  override func getTextContent() throws -> String { "x" }
}

private let EscapingTestDecoratorExtension = defineExtension(
  name: "@lexical/markdown/test-decorator",
  nodes: { [.klass(EscapingTestDecoratorNode.self), .klass(EscapingTestXDecoratorNode.self)] })

private let EscapingAutoLinkNodeExtension = defineExtension(
  name: "@lexical/markdown/test-autolink", nodes: { [.klass(AutoLinkNode.self)] })

private let EscapingTestInlineExtension = defineExtension(
  name: "@lexical/markdown/test-inline", nodes: { [.klass(EscapingTestInlineNode.self)] })

/// `buildEditorFromExtensions([MarkdownTestExtension, ...extensions])`.
private func escapingEditor(_ extensions: any AnyLexicalExtensionArgument...) throws -> Editor {
  try buildEditorFromExtensions([MarkdownTestExtension] + extensions)
}

/// `editor.update(fn, {discrete: true})`.
private func discrete(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(fn, EditorUpdateOptions(discrete: true))
}

private func rootText(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getTextContent() }
}

private func exportMarkdown(
  _ editor: Editor, _ transformers: [any Transformer] = TRANSFORMERS
) throws -> String {
  try editor.read { try convertToMarkdownString(transformers) }
}

/// `[text.slice(0, split), text.slice(split)]`, in UTF-16 code units.
private func splitText(_ text: String, _ split: Int) -> (String, String) {
  let units = Array(text.utf16)
  return (
    String(decoding: units[..<split], as: UTF16.self),
    String(decoding: units[split...], as: UTF16.self)
  )
}

private func firstParagraph() throws -> ElementNode {
  try #require(getRoot().getFirstChildOrThrow() as? ElementNode)
}

private func links(_ paragraph: ElementNode) throws -> [LinkNode] {
  try paragraph.getChildren().compactMap { $0 as? LinkNode }
}

private func toggleFormats(_ node: TextNode, _ format: String) throws {
  if format == "bold+italic" {
    try node.toggleFormat("bold").toggleFormat("italic")
  } else {
    try node.toggleFormat(format)
  }
}

private func elapsed(_ fn: () throws -> Void) rethrows -> Duration {
  let clock = ContinuousClock()
  let start = clock.now
  try fn()
  return clock.now - start
}

@Suite struct MarkdownEscapingLiteralTextTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"<b>text</b>\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"<https://example.com>\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"<0@a.b>\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"<+@a.b>\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"&#65;\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"&#x41;\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"[label](uri)\" across styled text nodes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes \"[ref]: uri\" across styled text nodes"
    ),
    arguments: [
      ("<b>text</b>", "&#60;b>text&#60;/b>"),
      ("<https://example.com>", "&#60;https://example.com>"),
      ("<0@a.b>", "&#60;0@a.b>"),
      ("<+@a.b>", "&#60;+@a.b>"),
      ("&#65;", "&#38;#65;"),
      ("&#x41;", "&#38;#x41;"),
      ("[label](uri)", "[label&#93;(uri)"),
      ("[ref]: uri", "[ref&#93;: uri"),
    ])
  func escapesAcrossStyledTextNodes(_ text: String, _ expected: String) throws {
    let editor = try escapingEditor()
    for split in 0...text.utf16.count {
      let (head, tail) = splitText(text, split)
      try discrete(editor) {
        try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode(head).setStyle("color: red"),
            createTextNode(tail).setStyle("color: blue")))
      }
      let written = try exportMarkdown(editor)
      #expect(written == expected)
      try discrete(editor) { try convertFromMarkdownString(written) }
      #expect(try rootText(editor) == text)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: \"AT&T\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: \"a < b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: \"&nbsp;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: \"&copy;\""
    ),
    arguments: ["AT&T", "a < b", "&nbsp;", "&copy;"])
  func keepsLegacyReadableText(_ text: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
    }
    let written = try exportMarkdown(editor)
    #expect(written == text)
    try discrete(editor) { try convertFromMarkdownString(written) }
    #expect(try rootText(editor) == text)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: bold"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: italic"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: strikethrough"
    ),
    arguments: ["bold", "italic", "strikethrough"])
  func preservesFormattedMarkersAfterPlainText(_ format: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().append(
        createParagraphNode().append(createTextNode("a"), createTextNode("*").toggleFormat(format)))
    }
    let written = try exportMarkdown(editor)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      #expect(try getRoot().getTextContent() == "a*")
      let nodes = try getRoot().getAllTextNodes()
      try #require(nodes.count >= 2)
      #expect(try nodes[0].getFormat() == 0)
      #expect(try nodes[1].hasFormat(format) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: \"*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: \"**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: \"~~\""
    ),
    arguments: ["*", "**", "~~"])
  func keepsLiteralDelimitersBesideEscapedPunctuation(_ delimiter: String) throws {
    let editor = try escapingEditor()
    for inside in ["[", "&", "<"] {
      for outside in ["[", "&", "<"] {
        let source = "a\(delimiter)\\\(inside)\(delimiter)\\\(outside)"
        try discrete(editor) { try convertFromMarkdownString(source) }
        try editor.read {
          #expect(
            try getRoot().getTextContent() == "a\(delimiter)\(inside)\(delimiter)\(outside)",
            "\(source)")
          for text in try getRoot().getAllTextNodes() {
            #expect(try text.getFormat() == 0, "\(source)")
          }
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: [\"&\",\"#65;\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: [\"&\",\"copy;\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: [\"<\",\"b>text</b>\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: [\"AT&\",\"T\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: [\"a <\",\" b\"]"
    ),
    arguments: [
      (["&", "#65;"], "&#38;#65;"),
      (["&", "copy;"], "&#38;copy;"),
      (["<", "b>text</b>"], "&#60;b>text&#60;/b>"),
      (["AT&", "T"], "AT&#38;T"),
      (["a <", " b"], "a &#60; b"),
    ])
  func guardsAmbiguousNodeBoundaries(_ pieces: [String], _ expected: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          contentsOf: pieces.enumerated().map { index, text in
            try createTextNode(text).setStyle(index != 0 ? "color: blue" : "color: red")
          }))
    }
    #expect(try exportMarkdown(editor) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"\\\\<b>text\\\\</b>\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"&#60;b&#62;text&#60;/b&#62;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"\\\\&#65;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"\\\\&#x41;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"&#38;#65;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"&#x26;#x41;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"\\\\[label\\\\](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: \"\\\\[label](uri)\""
    ),
    arguments: [
      ("\\<b>text\\</b>", "<b>text</b>"),
      ("&#60;b&#62;text&#60;/b&#62;", "<b>text</b>"),
      ("\\&#65;", "&#65;"),
      ("\\&#x41;", "&#x41;"),
      ("&#38;#65;", "&#65;"),
      ("&#x26;#x41;", "&#x41;"),
      ("\\[label\\](uri)", "[label](uri)"),
      ("\\[label](uri)", "[label](uri)"),
    ])
  func keepsImportedLiteralTextThroughRepeatedConversion(_ source: String, _ text: String) throws {
    let editor = try escapingEditor()
    var written = source
    for round in 0..<3 {
      let markdown = written
      try discrete(editor) { try convertFromMarkdownString(markdown) }
      #expect(try rootText(editor) == text, "round \(round)")
      written = try exportMarkdown(editor)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes only the selected literal content"
    ))
  func escapesOnlyTheSelectedLiteralContent() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      let text = try createTextNode("before <b>&copy; [x](u)</b> after")
      try getRoot().clear().append(createParagraphNode().append(text))
      try text.select(7, text.getTextContentSize() - 6)
    }
    #expect(
      try editor.read { try convertSelectionToMarkdownString(TRANSFORMERS, getSelection()) }
        == "&#60;b>&copy; [x&#93;(u)&#60;/b>")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label \"[label]\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label \"a]b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label \"a[b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label \"a](other)\""
    ),
    arguments: ["[label]", "a]b", "a[b", "a](other)"])
  func preservesRealLinkLabel(_ label: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(createLinkNode("uri").append(createTextNode(label))))
    }
    let written = try exportMarkdown(editor)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let paragraph = try firstParagraph()
      let link = try #require(paragraph.getFirstChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "uri")
      #expect(try link.getTextContent() == label)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label \"[\\\\]\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label \"\\\\[x]\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label \"[\\\\]`\""
    ),
    arguments: ["[\\]", "\\[x]", "[\\]`"])
  func preservesBackslashesInCodeFormattedLinkLabel(_ label: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createLinkNode("uri").append(createTextNode(label).toggleFormat("code"))))
    }
    let written = try exportMarkdown(editor)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let paragraph = try firstParagraph()
      let link = try #require(paragraph.getFirstChildOrThrow() as? LinkNode)
      let text = try #require(link.getFirstChildOrThrow() as? TextNode)
      #expect(try text.getTextContent() == label)
      #expect(try text.hasFormat("code") == true)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves mixed code/plain brackets in a link: \"[\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves mixed code/plain brackets in a link: \"]\""
    ),
    arguments: [("[", "]"), ("]", "[")])
  func preservesMixedCodePlainBracketsInALink(_ code: String, _ plain: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createLinkNode("uri").append(
            createTextNode(code).toggleFormat("code"), createTextNode(plain))))
    }
    let written = try exportMarkdown(editor)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let paragraph = try firstParagraph()
      #expect(try paragraph.getChildrenSize() == 1)
      let link = try #require(paragraph.getFirstChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "uri")
      let children = try link.getChildren().map { node in
        let text = try #require(node as? TextNode)
        return "\(try text.getTextContent()):\(try text.hasFormat("code"))"
      }
      #expect(children == ["\(code):true", "\(plain):false"])
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > continues searching after an escaped link opener"
    ))
  func continuesSearchingAfterAnEscapedLinkOpener() throws {
    let editor = try escapingEditor()
    try discrete(editor) { try convertFromMarkdownString("\\[literal](a) [real](b)") }
    try editor.read {
      let paragraph = try firstParagraph()
      #expect(try links(paragraph).map { try $0.getURL() } == ["b"])
      #expect(try paragraph.getTextContent() == "[literal](a) real")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > does not let an escaped closer absorb a later real link"
    ))
  func doesNotLetAnEscapedCloserAbsorbALaterRealLink() throws {
    let editor = try escapingEditor()
    try discrete(editor) { try convertFromMarkdownString("[a]\\](u) [b](v)") }
    try editor.read {
      let paragraph = try firstParagraph()
      #expect(try paragraph.getTextContent() == "[a]](u) b")
      #expect(
        try links(paragraph).map { [try $0.getTextContent(), try $0.getURL()] } == [["b", "v"]])
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: false, wrapped replace: false)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: false, wrapped replace: true)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: true, wrapped replace: false)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: true, wrapped replace: true)"
    ),
    arguments: [(false, false), (false, true), (true, false), (true, true)])
  func preservesCopiedLinkImports(_ clonedPattern: Bool, _ wrappedReplace: Bool) throws {
    let editor = try escapingEditor()
    let exportLink = try #require(LINK.export)
    let pattern = try #require(LINK.importRegExp)
    let replaceLink = try #require(LINK.replace)
    // `{...LINK, export, importRegExp, replace}`
    let customLink = LINK.copy()
    customLink.export = { node, exportChildren, exportFormat in
      try exportLink(node, exportChildren, exportFormat)
    }
    customLink.importRegExp = clonedPattern ? JSRegExp(pattern.source, pattern.flags) : pattern
    customLink.replace =
      wrappedReplace ? { node, match in try replaceLink(node, match) } : replaceLink
    let transformers: [any Transformer] = TRANSFORMERS.map {
      $0 === LINK ? customLink : $0
    }
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("[a]](u) "),
          createLinkNode("v", LinkAttributes(title: "title")).append(createTextNode("b"))))
    }
    let written = try exportMarkdown(editor, transformers)
    try discrete(editor) { try convertFromMarkdownString(written, transformers) }
    try editor.read {
      let children = try firstParagraph().getChildren()
      try #require(children.count >= 2)
      let text = try #require(children[0] as? TextNode)
      #expect(try text.getTextContent() == "[a]](u) ")
      let link = try #require(children[1] as? LinkNode)
      #expect(try link.getTextContent() == "b")
      #expect(try link.getURL() == "v")
      #expect(try link.getTitle() == "title")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: \"<\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: \"&\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: \"[\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: \"]\""
    ),
    arguments: ["<", "&", "[", "]"])
  func retainsBoldBeforeAnExportedLiteralEscape(_ literal: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("!").toggleFormat("bold"), createTextNode(literal)))
    }
    let written = try exportMarkdown(editor)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let children = try firstParagraph().getChildren().map { node in
        let text = try #require(node as? TextNode)
        return "\(try text.getTextContent()):\(try text.hasFormat("bold"))"
      }
      #expect(children == ["!:true", "\(literal):false"])
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: bold"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: italic"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: strikethrough"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: bold+italic"
    ),
    arguments: ["bold", "italic", "strikethrough", "bold+italic"])
  func preservesFormattedBackslashesNextToPlainText(_ format: String) throws {
    let editor = try escapingEditor()
    for before in [false, true] {
      let text = before ? "\\path" : "path\\"
      var expectedFormat = 0
      try discrete(editor) {
        let formatted = try createTextNode(text)
        try toggleFormats(formatted, format)
        expectedFormat = try formatted.getFormat()
        let plain = try createTextNode("file")
        try getRoot().clear().append(
          createParagraphNode().append(
            contentsOf: before ? [plain, formatted] : [formatted, plain]))
      }
      let written = try exportMarkdown(editor)
      try discrete(editor) { try convertFromMarkdownString(written) }
      try editor.read {
        var expected = ["\(text):\(expectedFormat)", "file:0"]
        if before { expected.reverse() }
        let children = try firstParagraph().getChildren().map { node in
          let text = try #require(node as? TextNode)
          return "\(try text.getTextContent()):\(try text.getFormat())"
        }
        #expect(children == expected)
      }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: \"[a\\\\]\\\\](u) \""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: \"[&#65;\\\\]\\\\](u) \""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: \"[\\\\&#65;\\\\]\\\\](u) \""
    ),
    arguments: [
      ("[a\\]\\](u) ", "[a]](u) "),
      ("[&#65;\\]\\](u) ", "[A]](u) "),
      ("[\\&#65;\\]\\](u) ", "[&#65;]](u) "),
    ])
  func decodesEscapedTextBeforeALaterLink(_ prefix: String, _ expected: String) throws {
    try expectTextThenLink(prefix + "[ok](v)", expected, "v", "ok")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves balanced bracket prefixes before a later link: \"[a\\\\][]\\\\](u) \""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves balanced bracket prefixes before a later link: \"[&#65;\\\\][]\\\\](u) \""
    ),
    arguments: [
      ("[a\\][]\\](u) ", "[a][]](u) "),
      ("[&#65;\\][]\\](u) ", "[A][]](u) "),
    ])
  func preservesBalancedBracketPrefixesBeforeALaterLink(
    _ prefix: String, _ expected: String
  )
    throws
  {
    try expectTextThenLink(prefix + "[ok](v)", expected, "v", "ok")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves nested openers after an escaped prefix: \"[[b](v)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves nested openers after an escaped prefix: \"[a[b](v)\""
    ),
    arguments: [("[[b](v)", "["), ("[a[b](v)", "[a")])
  func preservesNestedOpenersAfterAnEscapedPrefix(_ suffix: String, _ prefix: String) throws {
    try expectTextThenLink("[a\\]\\](u) " + suffix, "[a]](u) " + prefix, "v", "b")
  }

  /// Imports `markdown` and expects a text node `text`, then a link.
  private func expectTextThenLink(
    _ markdown: String, _ text: String, _ url: String, _ label: String
  ) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try convertFromMarkdownString(markdown) }
    try editor.read {
      let children = try firstParagraph().getChildren()
      try #require(children.count >= 2)
      let textNode = try #require(children[0] as? TextNode)
      #expect(try textNode.getTextContent() == text)
      let link = try #require(children[1] as? LinkNode)
      #expect(try link.getURL() == url)
      #expect(try link.getTextContent() == label)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[]~~~](u)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[]~~~](u)\" #2"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[\\\\a]~~~](u)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[\\\\a]~~~](u)\" #2"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[\\\\]]~~~](u)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: \"~~[\\\\]]~~~](u)\" #2"
    ),
    arguments: [
      ("~~[]~~~](u)", "~~[]~~~](u)", ""),
      ("~~[]~~~](u)", "~~[]~~~](u)", " [ok](v)"),
      ("~~[\\a]~~~](u)", "~~[\\a]~~~](u)", ""),
      ("~~[\\a]~~~](u)", "~~[\\a]~~~](u)", " [ok](v)"),
      ("~~[\\]]~~~](u)", "~~[]]~~~](u)", ""),
      ("~~[\\]]~~~](u)", "~~[]]~~~](u)", " [ok](v)"),
    ])
  func keepsLegacyGroupingForMalformedLinkCandidates(
    _ source: String, _ expected: String, _ suffix: String
  ) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try convertFromMarkdownString(source + suffix) }
    try editor.read {
      let root = try getRoot()
      #expect(try root.getTextContent() == expected + (suffix.isEmpty ? "" : " ok"))
      for text in try root.getAllTextNodes() {
        #expect(try text.getFormat() == 0)
      }
      if !suffix.isEmpty {
        let paragraph = try firstParagraph()
        let link = try #require(paragraph.getLastChildOrThrow() as? LinkNode)
        #expect(try link.getURL() == "v")
        #expect(try link.getTextContent() == "ok")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > runs import transformers after escaped link text: code"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > runs import transformers after escaped link text: custom"
    ),
    arguments: ["code", "custom"])
  func runsImportTransformersAfterEscapedLinkText(_ kind: String) throws {
    let editor = try escapingEditor()
    var calls = 0
    let custom = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#"@\w+"#), regExp: JSRegExp(#"@\w+$"#),
      replace: { node, _ in
        calls += 1
        try node.setTextContent("MENTION")
        return node
      })
    let middle = kind == "code" ? "`code`" : "@name"
    try discrete(editor) {
      try convertFromMarkdownString(
        "[a\\]\\](u) " + middle + " [link](v)",
        kind == "custom" ? [custom] + TRANSFORMERS : TRANSFORMERS)
    }
    try editor.read {
      let root = try getRoot()
      #expect(
        try root.getTextContent() == "[a]](u) " + (kind == "code" ? "code" : "MENTION") + " link")
      if kind == "code" {
        #expect(
          try root.getAllTextNodes().first { try $0.hasFormat("code") }?.getTextContent() == "code")
      }
      let paragraph = try firstParagraph()
      let link = try #require(paragraph.getLastChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "v")
      #expect(try link.getTextContent() == "link")
    }
    #expect(calls == (kind == "custom" ? 1 : 0))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > parses the text before an unbalanced link"
    ))
  func parsesTheTextBeforeAnUnbalancedLink() throws {
    let editor = try escapingEditor()
    // Code spans bind tighter than link brackets
    // (https://spec.commonmark.org/0.31.2/#code-spans), so this is `[`, a code
    // span and a link.
    try discrete(editor) { try convertFromMarkdownString("[`\\`[x](u)") }
    try editor.read {
      let children = try firstParagraph().getChildren()
      try #require(children.count >= 3)
      let text = try #require(children[0] as? TextNode)
      #expect(try text.getTextContent() == "[")
      let code = try #require(children[1] as? TextNode)
      #expect(try code.hasFormat("code") == true)
      #expect(try code.getTextContent() == "\\")
      let link = try #require(children[2] as? LinkNode)
      #expect(try link.getURL() == "u")
      #expect(try link.getTextContent() == "x")
    }
    #expect(try exportMarkdown(editor) == "[`\\`[x](u)")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps the existing raw-source newline mode"
    ))
  func keepsTheExistingRawSourceNewlineMode() throws {
    let editor = try escapingEditor()
    let source = "\\&#65; \\<b> \\[x](u)"
    try discrete(editor) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode(source)))
    }
    #expect(try editor.read { try convertToMarkdownString(TRANSFORMERS, nil, true) } == source)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > does not escape code or authored custom transformer output"
    ))
  func doesNotEscapeCodeOrAuthoredCustomTransformerOutput() throws {
    let editor = try escapingEditor()
    let raw = "<b>&copy; [x](u)</b>"
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode(raw).toggleFormat("code"), createTextNode("owned")))
    }
    let custom = TextMatchTransformer(
      dependencies: [],
      export: { node, _, _ in try node.getTextContent() == "owned" ? raw : nil },
      regExp: JSRegExp("$^"))
    #expect(try exportMarkdown(editor, [custom] + TRANSFORMERS) == "`" + raw + "`" + raw)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps custom matchers responsible for their escape rules"
    ))
  func keepsCustomMatchersResponsibleForTheirEscapeRules() throws {
    let editor = try escapingEditor()
    var calls = 0
    let custom = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#"@\w+"#), regExp: JSRegExp(#"@\w+$"#),
      replace: { node, _ in
        calls += 1
        try node.setTextContent("seen")
        return nil
      })
    try discrete(editor) { try convertFromMarkdownString("\\@name", [custom]) }
    #expect(calls == 1)
    #expect(try rootText(editor) == "\\seen")
  }
}

/// `$exportParagraph(...texts)`: a paragraph of `texts` in distinct styles.
private func exportParagraph(_ texts: String...) throws {
  try getRoot().clear().append(
    createParagraphNode().append(
      contentsOf: texts.enumerated().map { index, text in
        try createTextNode(text).setStyle("color: #\(index)\(index)\(index)")
      }))
}

/// `$describe(node)` of "round trips text with brackets before a link".
private func describeNode(_ node: LexicalNode) throws -> String {
  if let link = node as? LinkNode {
    return "{\(try link.getURL()):\(try link.getChildren().map(describeNode).joined())}"
  } else if let text = node as? TextNode {
    return "<\(try text.getFormat()):\(try text.getTextContent())>"
  }
  if let element = node as? ElementNode {
    return try element.getChildren().map(describeNode).joined()
  }
  return try node.getTextContent()
}

/// `TRANSFORMERS.filter(transformer => transformer !== LINK)`.
nonisolated(unsafe) private let WITHOUT_LINK = TRANSFORMERS.filter { $0 !== LINK }

@Suite struct MarkdownEscapingBracketTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"cleared review [1][2]\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"Narrative claim [1].\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"done [x] and [ ] todo\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"array[0] and map[key]\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"see [1]: the note\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"a [label] (not a link)\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"[label]\\n(next line)\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"nested [[1]] and [a [b] c]\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"[\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"]\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"][\" as typed"
    ),
    arguments: [
      "cleared review [1][2]",
      "Narrative claim [1].",
      "done [x] and [ ] todo",
      "array[0] and map[key]",
      "see [1]: the note",
      "a [label] (not a link)",
      "[label]\n(next line)",
      "nested [[1]] and [a [b] c]",
      "[",
      "]",
      "][",
    ])
  func leavesAsTyped(_ text: String) throws {
    let editor = try escapingEditor()
    for split in 0...text.utf16.count {
      let (head, tail) = splitText(text, split)
      try discrete(editor) { try exportParagraph(head, tail) }
      let written = try exportMarkdown(editor)
      #expect(written == text, "split \(split)")
      try discrete(editor) { try convertFromMarkdownString(written) }
      #expect(try rootText(editor) == text, "split \(split)")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"[a](b)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"![a](b)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"[a]: b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"   [a]: b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"[a](b) and [c](d)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"x](y)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of \"[[1]](url)\""
    ),
    arguments: [
      ("[a](b)", "[a&#93;(b)"),
      ("![a](b)", "![a&#93;(b)"),
      ("[a]: b", "[a&#93;: b"),
      ("   [a]: b", "   [a&#93;: b"),
      ("[a](b) and [c](d)", "[a&#93;(b) and [c&#93;(d)"),
      ("x](y)", "x&#93;(y)"),
      ("[[1]](url)", "[[1]&#93;(url)"),
    ])
  func escapesTheClosingBracket(_ text: String, _ expected: String) throws {
    let editor = try escapingEditor()
    for split in 0...text.utf16.count {
      let (head, tail) = splitText(text, split)
      try discrete(editor) { try exportParagraph(head, tail) }
      let written = try exportMarkdown(editor)
      #expect(written == expected, "split \(split)")
      try discrete(editor) { try convertFromMarkdownString(written) }
      #expect(try rootText(editor) == text, "split \(split)")
      #expect(try editor.read { try links(firstParagraph()).isEmpty }, "split \(split)")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a definition label after a line break"
    ))
  func escapesADefinitionLabelAfterALineBreak() throws {
    let editor = try escapingEditor()
    try discrete(editor) { try exportParagraph("intro\n[a]: b") }
    #expect(try exportMarkdown(editor) == "intro\n[a&#93;: b")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"[1]\" as \"[[1]](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"claim [2]\" as \"[claim [2]](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"[a [b] c]\" as \"[[a [b] c]](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"a]b\" as \"[a&#93;b](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"a[b\" as \"[a&#91;b](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"[x](y)\" as \"[&#91;x&#93;(y)](uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text \"a](other)\" as \"[a&#93;(other)](uri)\""
    ),
    arguments: [
      ("[1]", "[[1]](uri)"),
      ("claim [2]", "[claim [2]](uri)"),
      ("[a [b] c]", "[[a [b] c]](uri)"),
      ("a]b", "[a&#93;b](uri)"),
      ("a[b", "[a&#91;b](uri)"),
      ("[x](y)", "[&#91;x&#93;(y)](uri)"),
      ("a](other)", "[a&#93;(other)](uri)"),
    ])
  func writesLinkText(_ label: String, _ expected: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(createLinkNode("uri").append(createTextNode(label))))
    }
    let written = try exportMarkdown(editor)
    #expect(written == expected)
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let paragraph = try firstParagraph()
      #expect(try paragraph.getChildrenSize() == 1)
      let link = try #require(paragraph.getFirstChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "uri")
      #expect(try link.getTextContent() == label)
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker \"[x] done\" at the start of a line"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker \"[X] done\" at the start of a line"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker \"[ ] todo\" at the start of a line"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker \"[] todo\" at the start of a line"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker \"  [x] indented\" at the start of a line"
    ),
    arguments: [
      ("[x] done", "[x&#93; done"),
      ("[X] done", "[X&#93; done"),
      ("[ ] todo", "[ &#93; todo"),
      ("[] todo", "[&#93; todo"),
      ("  [x] indented", "  [x&#93; indented"),
    ])
  func escapesTheTaskMarkerAtTheStartOfALine(_ text: String, _ expected: String) throws {
    let editor = try escapingEditor()
    for split in 0...text.utf16.count {
      let (head, tail) = splitText(text, split)
      try discrete(editor) { try exportParagraph(head, tail) }
      let written = try exportMarkdown(editor)
      #expect(written == expected, "split \(split)")
      try discrete(editor) { try convertFromMarkdownString(written, [CHECK_LIST] + TRANSFORMERS) }
      try editor.read {
        let block = try getRoot().getFirstChildOrThrow()
        #expect(!(block is ListNode), "split \(split)")
        #expect(try block.getTextContent() == text, "split \(split)")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker that starts a bullet item"
    ))
  func escapesATaskMarkerThatStartsABulletItem() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("[ ] todo"))))
    }
    let written = try exportMarkdown(editor, [CHECK_LIST] + TRANSFORMERS)
    #expect(written == "- [ &#93; todo")
    try discrete(editor) { try convertFromMarkdownString(written, [CHECK_LIST] + TRANSFORMERS) }
    try editor.read {
      let list = try #require(getRoot().getFirstChildOrThrow() as? ListNode)
      #expect(try list.getListType() == "bullet")
      #expect(try list.getTextContent() == "[ ] todo")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > keeps a real check list item as a check list"
    ))
  func keepsARealCheckListItemAsACheckList() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try convertFromMarkdownString("- [ ] todo\n- [x] done", [CHECK_LIST] + TRANSFORMERS)
    }
    #expect(try exportMarkdown(editor, [CHECK_LIST] + TRANSFORMERS) == "- [ ] todo\n- [x] done")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in [\"[a\\\\\",\"]: /url\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in [\"[a](x]: /url\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in [\"[foo\",\"\\n\",\"bar]: /url\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in [\"intro\",\"\\n\",\"[a]: b\"]"
    ),
    arguments: [
      // [a\ + ]: /url in two nodes: the backslash is exported doubled.
      (["[a\\", "]: /url"], "[a\\\\&#93;: /url"),
      // The first ] is escaped, so the second still closes a label.
      (["[a](x]: /url"], "[a&#93;(x&#93;: /url"),
      (["[foo", "\n", "bar]: /url"], "[foo\nbar&#93;: /url"),
      (["intro", "\n", "[a]: b"], "intro\n[a&#93;: b"),
    ])
  func escapesTheDefinitionLabelIn(_ parts: [String], _ expected: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          contentsOf: parts.enumerated().map { index, part -> LexicalNode in
            part == "\n"
              ? try createLineBreakNode()
              : try createTextNode(part).setStyle("color: #\(index)\(index)\(index)")
          }))
    }
    #expect(try exportMarkdown(editor) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > treats the start of an exported selection as a line start"
    ))
  func treatsTheStartOfAnExportedSelectionAsALineStart() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      let intro = try createTextNode("intro ")
      let definition = try createTextNode("[a]: /url").setStyle("color: red")
      try getRoot().clear().append(createParagraphNode().append(intro, definition))
      try definition.select(0, definition.getTextContentSize())
    }
    #expect(
      try editor.read { try convertSelectionToMarkdownString(TRANSFORMERS, getSelection()) }
        == "[a&#93;: /url")
  }

  // across inline elements

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a bracket inside an inline element followed by ("
    ))
  func escapesABracketInsideAnInlineElementFollowedByParen() throws {
    let editor = try escapingEditor(EscapingTestInlineExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          create(EscapingTestInlineNode.self).append(createTextNode("[a]")),
          createTextNode("(b)")))
    }
    let written = try exportMarkdown(editor)
    #expect(written == "[a&#93;(b)")
    try discrete(editor) { try convertFromMarkdownString(written) }
    #expect(try rootText(editor) == "[a](b)")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a pair inside link text that an inline element splits from ("
    ))
  func escapesAPairInsideLinkTextThatAnInlineElementSplitsFromParen() throws {
    let editor = try escapingEditor(EscapingTestInlineExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createLinkNode("url").append(
            create(EscapingTestInlineNode.self).append(createTextNode("[x]")),
            createTextNode("(y)"))))
    }
    let written = try exportMarkdown(editor)
    #expect(written == "[&#91;x&#93;(y)](url)")
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let link = try #require(firstParagraph().getFirstChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "url")
      #expect(try link.getTextContent() == "[x](y)")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a definition label split by an inline element: [\"[a]\",\": b\"] (inline first)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a definition label split by an inline element: [\"[a\",\"]: /url\"] (inline second)"
    ),
    arguments: [(["[a]", ": b"], "inline first"), (["[a", "]: /url"], "inline second")])
  func escapesADefinitionLabelSplitByAnInlineElement(_ parts: [String], _ which: String) throws {
    let editor = try escapingEditor(EscapingTestInlineExtension)
    try discrete(editor) {
      let first = parts[0]
      let second = parts[1]
      try getRoot().clear().append(
        createParagraphNode().append(
          which == "inline first"
            ? create(EscapingTestInlineNode.self).append(createTextNode(first))
            : createTextNode(first),
          which == "inline first"
            ? createTextNode(second)
            : create(EscapingTestInlineNode.self).append(createTextNode(second))))
    }
    #expect(try exportMarkdown(editor).contains("&#93;:"))
  }

  // round trips text with brackets before a link

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"Claim [1] is *important*, see [source](https://example.com)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[1] **bold** [link](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[1] `c` [link](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"**[1]** [l](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[a](b) **x** [l](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[a&#93;(b) **x** [l](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"see [1] and ~~[2]~~ [link](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[h[ello](u)\" and exports it again unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports \"[foo [bar](/uri)&#93;(/uri)\" and exports it again unchanged"
    ),
    arguments: [
      (
        "Claim [1] is *important*, see [source](https://example.com)",
        "<0:Claim [1] is ><2:important><0:, see >{https://example.com:<0:source>}"
      ),
      ("[1] **bold** [link](u)", "<0:[1] ><1:bold><0: >{u:<0:link>}"),
      ("[1] `c` [link](u)", "<0:[1] ><16:c><0: >{u:<0:link>}"),
      ("**[1]** [l](u)", "<1:[1]><0: >{u:<0:l>}"),
      ("[a](b) **x** [l](u)", "{b:<0:a>}<0: ><1:x><0: >{u:<0:l>}"),
      ("[a&#93;(b) **x** [l](u)", "<0:[a](b) ><1:x><0: >{u:<0:l>}"),
      ("see [1] and ~~[2]~~ [link](u)", "<0:see [1] and ><4:[2]><0: >{u:<0:link>}"),
      ("[h[ello](u)", "<0:[h>{u:<0:ello>}"),
      ("[foo [bar](/uri)&#93;(/uri)", "<0:[foo >{/uri:<0:bar>}<0:](/uri)>"),
    ])
  func importsAndExportsAgainUnchanged(_ markdown: String, _ nodes: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try convertFromMarkdownString(markdown) }
    #expect(try editor.read { try describeNode(getRoot()) } == nodes)
    let written = try exportMarkdown(editor)
    #expect(written == markdown)
    try discrete(editor) { try convertFromMarkdownString(written) }
    #expect(try editor.read { try describeNode(getRoot()) } == nodes)
  }

  // around a decorator

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestDecoratorNode]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestXDecoratorNode]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestDecoratorNode] #2"
    ),
    arguments: [
      (false, "]: done", "&#91;foo&#93;: done"),
      (true, "] todo", "&#91;x&#93; todo"),
      (false, "](b)", "&#91;foo&#93;(b)"),
    ])
  func escapesTheBracketsAroundADecorator(_ isX: Bool, _ after: String, _ expected: String) throws {
    // The class is passed as a flag: `TestXDecoratorNode` or `TestDecoratorNode`.
    let editor = try escapingEditor(EscapingTestDecoratorExtension)
    try discrete(editor) {
      let decorator: LexicalNode =
        isX
        ? try create(EscapingTestXDecoratorNode.self) : try create(EscapingTestDecoratorNode.self)
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("["), decorator, createTextNode(after)))
    }
    #expect(try exportMarkdown(editor, [CHECK_LIST] + TRANSFORMERS) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > leaves a bracket away from a decorator as typed"
    ))
  func leavesABracketAwayFromADecoratorAsTyped() throws {
    let editor = try escapingEditor(EscapingTestDecoratorExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          create(EscapingTestDecoratorNode.self), createTextNode(" cleared review [1][2]")))
    }
    #expect(try exportMarkdown(editor) == "foo cleared review [1][2]")
  }

  /// A paragraph of a plain text node and a code-formatted one.
  private func exportPlainThenCode(_ parts: [String]) throws -> String {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode(parts[0]), createTextNode(parts[1]).toggleFormat("code")))
    }
    return try exportMarkdown(editor)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code [\"[a\",\"]:b\"] would close"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code [\"[a b\",\"c]: d\"] would close"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code [\"[a\",\"b]\"] would close"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code [\"[a\",\"[b]: c\"] would close"
    ),
    arguments: [
      (["[a", "]:b"], "&#91;a`]:b`"),
      (["[a b", "c]: d"], "&#91;a b`c]: d`"),
      (["[a", "b]"], "[a`b]`"),
      (["[a", "[b]: c"], "[a`[b]: c`"),
    ])
  func escapesTheOpenerOfALabelThatInlineCodeWouldClose(
    _ parts: [String], _ expected: String
  )
    throws
  {
    #expect(try exportPlainThenCode(parts) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: [\"[a](b) \",\"]:x\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: [\"[] \",\"]:url\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: [\"[x] see \",\"]:url\"]"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: [\"[a]: b \",\"]:c\"]"
    ),
    arguments: [
      (["[a](b) ", "]:x"], "&#91;a&#93;(b) `]:x`"),
      (["[] ", "]:url"], "&#91;&#93; `]:url`"),
      (["[x] see ", "]:url"], "&#91;x&#93; see `]:url`"),
      (["[a]: b ", "]:c"], "&#91;a&#93;: b `]:c`"),
    ])
  func escapesALineStartOpenerLeftOpenToInlineCode(_ parts: [String], _ expected: String) throws {
    #expect(try exportPlainThenCode(parts) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > does not read a link whose syntax is inside inline code"
    ))
  func doesNotReadALinkWhoseSyntaxIsInsideInlineCode() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("see ["), createTextNode("](").toggleFormat("code"),
          createTextNode("x) now")))
    }
    let written = try exportMarkdown(editor)
    #expect(written == "see [`](`x) now")
    try discrete(editor) { try convertFromMarkdownString(written) }
    try editor.read {
      let paragraph = try firstParagraph()
      #expect(try links(paragraph).isEmpty)
      #expect(try paragraph.getTextContent() == "see [](x) now")
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker after a decorator that may end a line"
    ))
  func escapesATaskMarkerAfterADecoratorThatMayEndALine() throws {
    let editor = try escapingEditor(EscapingTestDecoratorExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("a "), create(EscapingTestDecoratorNode.self),
          createTextNode("[x] done")))
    }
    #expect(try exportMarkdown(editor) == "a foo[x&#93; done")
  }

  // custom transformers

  nonisolated(unsafe) private static let TEST_LINK = TextMatchTransformer(
    dependencies: [EscapingTestInlineNode.self],
    export: { node, exportChildren, _ in
      guard let node = node as? EscapingTestInlineNode else { return nil }
      return "[\(try exportChildren(node))](custom)"
    },
    importRegExp: JSRegExp("$^"), regExp: JSRegExp("$^"))

  nonisolated(unsafe) private static let HTML_LINK = TextMatchTransformer(
    dependencies: [],
    export: { node, exportChildren, _ in
      guard let node = node as? LinkNode else { return nil }
      return "<a href=\"\(try node.getURL())\">\(try exportChildren(node))</a>"
    },
    importRegExp: JSRegExp("$^"), regExp: JSRegExp("$^"))

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > applies the link text rules inside an element a transformer writes as [text](url)"
    ))
  func appliesTheLinkTextRulesInsideAnElementATransformerWritesAsLink() throws {
    let editor = try escapingEditor(EscapingTestInlineExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          create(EscapingTestInlineNode.self).append(createTextNode("a]"))))
    }
    #expect(try exportMarkdown(editor, [Self.TEST_LINK] + TRANSFORMERS) == "[a&#93;](custom)")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > escapes at the edge of an element a transformer writes with \"(note)\" after it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > escapes at the edge of an element a transformer writes with \": note\" after it"
    ),
    arguments: [
      ("(note)", "see [", "a]", "see [a&#93;(note)"),
      (": note", "", "[a]", "&#91;a&#93;: note"),
    ])
  func escapesAtTheEdgeOfAnElementATransformerWrites(
    _ suffix: String, _ before: String, _ inner: String, _ expected: String
  ) throws {
    let NOTE = TextMatchTransformer(
      dependencies: [EscapingTestInlineNode.self],
      export: { node, exportChildren, _ in
        guard let node = node as? EscapingTestInlineNode else { return nil }
        return "\(try exportChildren(node))\(suffix)"
      },
      importRegExp: JSRegExp("$^"), regExp: JSRegExp("$^"))
    let editor = try escapingEditor(EscapingTestInlineExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          contentsOf: (before.isEmpty ? [] : [try createTextNode(before)])
            + [try create(EscapingTestInlineNode.self).append(createTextNode(inner))]))
    }
    #expect(try exportMarkdown(editor, [NOTE] + TRANSFORMERS) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > reads a link a transformer writes in another syntax as unknown"
    ))
  func readsALinkATransformerWritesInAnotherSyntaxAsUnknown() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("["), createLinkNode("u").append(createTextNode("a")),
          createTextNode("]: /x")))
    }
    #expect(
      try exportMarkdown(editor, [Self.HTML_LINK] + TRANSFORMERS)
        == "&#91;<a href=\"u\">a</a>&#93;: /x")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > reads typed control characters as text"
    ))
  func readsTypedControlCharactersAsText() throws {
    let editor = try escapingEditor()
    try discrete(editor) { try exportParagraph("[a\u{0001}]: /url") }
    #expect(try exportMarkdown(editor) == "[a\u{0001}&#93;: /url")
  }

  // links not written in brackets

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a definition label around autolinks"
    ))
  func escapesADefinitionLabelAroundAutolinks() throws {
    let editor = try escapingEditor(EscapingAutoLinkNodeExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("[see "),
          createAutoLinkNode("https://example.com").append(createTextNode("https://example.com")),
          createTextNode(" today]: "),
          createAutoLinkNode("https://example.com").append(createTextNode("https://example.com"))))
    }
    #expect(
      try exportMarkdown(editor) == "[see https://example.com today&#93;: https://example.com")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a bracket at the end of an autolink followed by ("
    ))
  func escapesABracketAtTheEndOfAnAutolinkFollowedByParen() throws {
    let editor = try escapingEditor(EscapingAutoLinkNodeExtension)
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createAutoLinkNode("https://example.com").append(
            createTextNode("https://example.com/[a]")),
          createTextNode("(b)")))
    }
    let written = try exportMarkdown(editor)
    #expect(written == "https://example.com/[a&#93;(b)")
    try discrete(editor) { try convertFromMarkdownString(written) }
    #expect(try editor.read { try links(firstParagraph()).isEmpty })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > treats link text as literal text when LINK is not exported"
    ))
  func treatsLinkTextAsLiteralTextWhenLinkIsNotExported() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createLinkNode("url").append(createTextNode("[a]")), createTextNode("(b)")))
    }
    #expect(try exportMarkdown(editor, WITHOUT_LINK) == "[a&#93;(b)")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a definition label across a link when LINK is not exported"
    ))
  func escapesADefinitionLabelAcrossALinkWhenLinkIsNotExported() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("["), createLinkNode("url").append(createTextNode("a")),
          createTextNode("]: /url")))
    }
    #expect(try exportMarkdown(editor, WITHOUT_LINK) == "[a&#93;: /url")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"- [x] done\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"+ [ ] todo\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"* [] todo\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"1. [x] done\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"> [a]: b\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"- [a]: b\" where a container marker precedes it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes \"2) [a]: b\" where a container marker precedes it"
    ),
    arguments: [
      ("- [x] done", "- [x&#93; done"),
      ("+ [ ] todo", "+ [ &#93; todo"),
      ("* [] todo", "\\* [] todo"),
      ("1. [x] done", "1. [x&#93; done"),
      ("> [a]: b", "> [a&#93;: b"),
      ("- [a]: b", "- [a&#93;: b"),
      ("2) [a]: b", "2) [a&#93;: b"),
    ])
  func escapesWhereAContainerMarkerPrecedesIt(_ text: String, _ expected: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try exportParagraph(text) }
    #expect(try exportMarkdown(editor, [CHECK_LIST] + TRANSFORMERS) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"a- [x] done\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"-[x] done\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"1.[a]: b\" as typed"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves \"x > [a]: b\" as typed"
    ),
    arguments: ["a- [x] done", "-[x] done", "1.[a]: b", "x > [a]: b"])
  func leavesMarkerLookalikesAsTyped(_ text: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try exportParagraph(text) }
    #expect(try exportMarkdown(editor) == text)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > imports a code span full of [ before a link in linear time"
    ))
  func importsACodeSpanFullOfOpenersBeforeALinkInLinearTime() throws {
    let editor = try escapingEditor()
    let markdown = "`" + String(repeating: "[", count: 80000) + "`](u)"
    let time = try elapsed {
      try discrete(editor) { try convertFromMarkdownString(markdown) }
    }
    #expect(time < .seconds(10))
    #expect(try editor.read { try links(firstParagraph()).isEmpty })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker indented with Unicode whitespace: \" [x] done\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker indented with Unicode whitespace: \"　[ ] todo\""
    ),
    arguments: [
      ("\u{00A0}[x] done", "\u{00A0}[x&#93; done"),
      ("\u{3000}[ ] todo", "\u{3000}[ &#93; todo"),
    ])
  func escapesATaskMarkerIndentedWithUnicodeWhitespace(_ text: String, _ expected: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) { try exportParagraph(text) }
    let written = try exportMarkdown(editor, [CHECK_LIST] + TRANSFORMERS)
    #expect(written == expected)
    try discrete(editor) { try convertFromMarkdownString(written, [CHECK_LIST] + TRANSFORMERS) }
    #expect(try rootText(editor) == text)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > imports a line of many code spans holding link syntax"
    ))
  func importsALineOfManyCodeSpansHoldingLinkSyntax() throws {
    let editor = try escapingEditor()
    let markdown = String(repeating: "[a`](x)` ", count: 2000)
    let time = try elapsed {
      try discrete(editor) { try convertFromMarkdownString(markdown) }
    }
    #expect(time < .seconds(10))
    #expect(try editor.read { try links(firstParagraph()).isEmpty })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports many styled nodes in linear time"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports one long node in linear time"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports many brackets in one node in linear time"
    ),
    arguments: ["many styled nodes", "one long node", "many brackets in one node"])
  func exportsInLinearTime(_ name: String) throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      let children: [LexicalNode] =
        switch name {
        case "many styled nodes":
          try (0..<8000).map { index in
            try createTextNode("xa]:").setStyle("color: #\(index % 2)00")
          }
        case "one long node":
          [try createTextNode(String(repeating: "xa]:", count: 20000))]
        default:
          [try createTextNode(String(repeating: "[a] ", count: 20000))]
        }
      try getRoot().clear().append(createParagraphNode().append(contentsOf: children))
    }
    // Linear exports take tens of milliseconds; a quadratic one takes tens of
    // seconds, so this bound leaves room for a busy machine.
    let time = try elapsed { _ = try exportMarkdown(editor) }
    #expect(time < .seconds(10))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > reads brackets written as references by earlier versions"
    ))
  func readsBracketsWrittenAsReferencesByEarlierVersions() throws {
    let editor = try escapingEditor()
    try discrete(editor) {
      try convertFromMarkdownString("cleared review &#91;1&#93;&#91;2&#93;")
    }
    #expect(try rootText(editor) == "cleared review [1][2]")
  }
}

@Suite struct MarkdownEscapingNumericReferenceTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#65;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#x41;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#X41;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"\\\\&#65;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"\\\\&#x41;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#38;#65;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#x26;#x41;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"\\\\\\\\&#65;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&copy;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#0;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#xD800;\" once"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes \"&#x110000;\" once"
    ),
    arguments: [
      ("&#65;", "A"),
      ("&#x41;", "A"),
      ("&#X41;", "A"),
      ("\\&#65;", "&#65;"),
      ("\\&#x41;", "&#x41;"),
      ("&#38;#65;", "&#65;"),
      ("&#x26;#x41;", "&#x41;"),
      ("\\\\&#65;", "\\A"),
      ("&copy;", "&copy;"),
      ("&#0;", "\u{FFFD}"),
      ("&#xD800;", "\u{FFFD}"),
      ("&#x110000;", "\u{FFFD}"),
    ])
  func decodesOnce(_ source: String, _ expected: String) {
    #expect(unescapeText(source) == expected)
  }
}
