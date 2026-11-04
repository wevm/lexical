import Lexical
import LexicalLink
import LexicalMarkdown
import Testing

// Port of the `decimal character references that name no character`
// describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.
// CommonMark reads at most seven digits, and replaces U+0000 and an invalid
// code point with U+FFFD.
@Suite struct MarkdownDecimalCharacterReferenceTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#1114112;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#9999999;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#0;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#55296;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#12345678;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#1114111;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads \"a&#35;b\""
    ),
    arguments: [
      ("a&#1114112;b", "a\u{FFFD}b"),
      ("a&#9999999;b", "a\u{FFFD}b"),
      ("a&#0;b", "a\u{FFFD}b"),
      ("a&#55296;b", "a\u{FFFD}b"),
      ("a&#12345678;b", "a&#12345678;b"),
      ("a&#1114111;b", "a\u{10FFFF}b"),
      ("a&#35;b", "a#b"),
    ])
  func reads(_ md: String, _ text: String) throws {
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) { try convertFromMarkdownString(md, TRANSFORMERS) }
    #expect(try editor.read(.latest) { try getRoot().getTextContent() } == text)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads one in a link destination and title"
    ))
  func readsInLinkDestinationAndTitle() throws {
    let editor = try createMarkdownTestEditor(nodes: [.klass(LinkNode.self)])
    try editor.update(discrete: true) {
      try convertFromMarkdownString(
        #"[x](https://example.com/&#9999999; "t&#1114112;")"#, TRANSFORMERS)
    }
    let links = try editor.read(.latest) {
      try getRoot().getAllTextNodes().compactMap { try $0.getParent() as? LinkNode }
        .map { [try $0.getTitle(), try $0.getURL()] }
    }
    #expect(links == [["t\u{FFFD}", "https://example.com/\u{FFFD}"]])
  }
}
