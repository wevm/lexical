import Lexical
import LexicalLink
import LexicalMarkdown
import Testing

// Ports of the link describe blocks of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts, from
// `link destination ends at whitespace` to `link import`.

private struct ImportedLink: Equatable, CustomStringConvertible {
  let title: String?
  let url: String
  var description: String { "{title: \(title.debugDescription), url: \(url.debugDescription)}" }
}

private func createLinkEditor() throws -> Editor {
  try createMarkdownTestEditor(nodes: [.klass(LinkNode.self)])
}

/// The links of the text nodes `md` imports to, in order.
private func linksOfTextNodes() throws -> [LinkNode] {
  try getRoot().getAllTextNodes().compactMap { try $0.getParent() as? LinkNode }
}

private func importLinks(_ md: String) throws -> [ImportedLink] {
  let editor = try createLinkEditor()
  try editor.update(discrete: true) { try convertFromMarkdownString(md, TRANSFORMERS) }
  return try editor.read(.latest) {
    try linksOfTextNodes().map { ImportedLink(title: try $0.getTitle(), url: try $0.getURL()) }
  }
}

/// A case whose `md` imports to `links`.
struct MarkdownLinkImportCase: Sendable, CustomTestStringConvertible {
  let md: String
  let links: [(title: String?, url: String)]
  var testDescription: String { md.debugDescription }
  init(_ md: String, _ links: [(title: String?, url: String)]) {
    self.md = md
    self.links = links
  }
}

private func expectLinks(_ testCase: MarkdownLinkImportCase) throws {
  #expect(
    try importLinks(testCase.md)
      == testCase.links.map { ImportedLink(title: $0.title, url: $0.url) })
}

@Suite struct MarkdownLinkDestinationWhitespaceEndTests {
  // A backslash escapes punctuation only, so one in front of a space is a
  // literal backslash and the space closes the raw destination.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads \"[x](b\\\\ \\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads \"[x](foo\\\\ bar)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads \"[x](foo\\\\ bar \\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads \"[x](a\\\\ b)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads \"[x](a\\\\ b \\\"t\\\")\""
    ),
    arguments: [
      MarkdownLinkImportCase(#"[x](b\ "t")"#, [("t", #"b\"#)]),
      MarkdownLinkImportCase(#"[x](foo\ bar)"#, []),
      MarkdownLinkImportCase(#"[x](foo\ bar "t")"#, []),
      MarkdownLinkImportCase(#"[x](a\ b)"#, []),
      MarkdownLinkImportCase(#"[x](a\ b "t")"#, []),
    ])
  func reads(_ testCase: MarkdownLinkImportCase) throws { try expectLinks(testCase) }
}

@Suite struct MarkdownLinkDestinationAngleBracketTests {
  // Otherwise a pointy destination that never closes falls through to the raw
  // form and the brackets end up inside the URL.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](<a<b>)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](<>x)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](<a>b)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](<foo)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](<\\\\>)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](a<b>c)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](a<b)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads \"[x](a>b)\""
    ),
    arguments: [
      MarkdownLinkImportCase("[x](<a<b>)", []),
      MarkdownLinkImportCase("[x](<>x)", []),
      MarkdownLinkImportCase("[x](<a>b)", []),
      MarkdownLinkImportCase("[x](<foo)", []),
      MarkdownLinkImportCase(#"[x](<\>)"#, []),
      // anywhere but in first place an angle bracket is an ordinary character
      MarkdownLinkImportCase("[x](a<b>c)", [(nil, "a<b>c")]),
      MarkdownLinkImportCase("[x](a<b)", [(nil, "a<b")]),
      MarkdownLinkImportCase("[x](a>b)", [(nil, "a>b")]),
    ])
  func reads(_ testCase: MarkdownLinkImportCase) throws { try expectLinks(testCase) }
}

@Suite struct MarkdownLinkDestinationTitleShapeTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri \\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri 't')\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri (t))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri  \\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri\\t\\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri\\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x]( \\\"t\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri (a(b)c))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x]( /uri)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](/uri )\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x]( /uri \\\"t\\\" )\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x]()\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x]( )\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](foo(and(bar)))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](a(b(c)d)e)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads \"[x](((a)))\""
    ),
    arguments: [
      // a title takes any of the three spellings, after whitespace
      MarkdownLinkImportCase(#"[x](/uri "t")"#, [("t", "/uri")]),
      MarkdownLinkImportCase("[x](/uri 't')", [("t", "/uri")]),
      MarkdownLinkImportCase("[x](/uri (t))", [("t", "/uri")]),
      MarkdownLinkImportCase(#"[x](/uri  "t")"#, [("t", "/uri")]),
      MarkdownLinkImportCase("[x](/uri\t\"t\")", [("t", "/uri")]),
      // without the whitespace it is all destination
      MarkdownLinkImportCase(#"[x](/uri"t")"#, [(nil, #"/uri"t""#)]),
      MarkdownLinkImportCase(#"[x]( "t")"#, [(nil, #""t""#)]),
      // a parenthesized title may not hold an unescaped parenthesis
      MarkdownLinkImportCase("[x](/uri (a(b)c))", []),
      // whitespace may sit on either side of the destination
      MarkdownLinkImportCase("[x]( /uri)", [(nil, "/uri")]),
      MarkdownLinkImportCase("[x](/uri )", [(nil, "/uri")]),
      MarkdownLinkImportCase(#"[x]( /uri "t" )"#, [("t", "/uri")]),
      // the destination itself is optional
      MarkdownLinkImportCase("[x]()", [(nil, "")]),
      MarkdownLinkImportCase("[x]( )", [(nil, "")]),
      // parentheses nest, up to the depth the pattern is written for
      MarkdownLinkImportCase("[x](foo(and(bar)))", [(nil, "foo(and(bar))")]),
      MarkdownLinkImportCase("[x](a(b(c)d)e)", [(nil, "a(b(c)d)e")]),
      MarkdownLinkImportCase("[x](((a)))", [(nil, "((a))")]),
    ])
  func reads(_ testCase: MarkdownLinkImportCase) throws { try expectLinks(testCase) }
}

@Suite struct MarkdownLinkDestinationBacktrackTests {
  // A pattern that leaves the whitespace runs on either side of an absent
  // destination neighbouring each other walks every split of a run before
  // giving up, so the work grows with the square of the input. Upstream's
  // time limit (5 s) is the assertion.
  static let run = 200000

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run that never closes"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run in front of a quote"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run of tabs"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run on either side of a destination"
    ),
    arguments: [
      String(repeating: " ", count: run),
      String(repeating: " ", count: run) + "\"",
      String(repeating: "\t", count: run),
      String(repeating: " ", count: run) + "/uri" + String(repeating: " ", count: run),
    ])
  func reads(_ rest: String) throws {
    let start = ContinuousClock.now
    let editor = try createLinkEditor()
    try editor.update(discrete: true) { try convertFromMarkdownString("[x](" + rest, TRANSFORMERS) }
    // None of them is a link, and the point is that saying so is quick.
    #expect(try editor.read(.latest) { try linksOfTextNodes() }.isEmpty)
    #expect(ContinuousClock.now - start < .seconds(5))
  }
}

/// Exports a paragraph holding a link labelled `x` and then `after`, with
/// `afterFormat` when given, expects `md`, and expects reading that back to
/// give the same link and text.
private func expectLinkRoundTrip(
  _ url: String, _ title: String?, _ md: String, _ after: String = "",
  _ afterFormat: TextFormatType? = nil
) throws {
  let editor = try createLinkEditor()
  try editor.update(discrete: true) {
    let paragraph = try createParagraphNode().append(
      createLinkNode(url, LinkAttributes(title: .some(title))).append(createTextNode("x")))
    if !after.isEmpty {
      let afterNode = try createTextNode(after)
      try paragraph.append(afterFormat.map { try afterNode.toggleFormat($0) } ?? afterNode)
    }
    try getRoot().clear().append(paragraph)
  }

  let exported = try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
  #expect(exported == md)

  let reimported = try createLinkEditor()
  try reimported.update(discrete: true) { try convertFromMarkdownString(exported, TRANSFORMERS) }
  try reimported.read(.latest) {
    let links = try linksOfTextNodes().map {
      ImportedLink(title: try $0.getTitle(), url: try $0.getURL())
    }
    #expect(links == [ImportedLink(title: title, url: url)])
    #expect(try getRoot().getTextContent() == "x\(after)")
  }
}

@Suite struct MarkdownLinkDestinationRoundTripTests {
  // Export has to write a destination that import can read back: escapes in
  // the raw form, and the pointy form for a URL the raw form cannot hold.
  // The case of "/a\u{7f}b" has no `.upstream` trait: its id holds a raw
  // U+007F, which a Swift string literal cannot hold, so the ledger lists
  // it in skips.json; it runs here all the same.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://en.wikipedia.org/wiki/Ruby_(programming_language)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a)b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a(b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a(b)c(d)e\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/((a))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\\\\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\\\(b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b(c)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b>c\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b<c\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b\\\\\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\tb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"/a\\u0001b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"/a\\u0000b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\r\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a\\rb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a b\\nc\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a<b>c\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a&#10;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a&#x0A;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/a&amp;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"&#38;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/?a=1&b=2\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://example.com/?a=1&b=2;c=3\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"<foo>\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves \"https://lexical.dev\""
    ),
    arguments: [
      (
        "https://en.wikipedia.org/wiki/Ruby_(programming_language)",
        #"[x](https://en.wikipedia.org/wiki/Ruby_\(programming_language\))"#
      ),
      ("https://example.com/a)b", #"[x](https://example.com/a\)b)"#),
      ("https://example.com/a(b", #"[x](https://example.com/a\(b)"#),
      ("https://example.com/a(b)c(d)e", #"[x](https://example.com/a\(b\)c\(d\)e)"#),
      ("https://example.com/((a))", #"[x](https://example.com/\(\(a\)\))"#),
      (#"https://example.com/a\"#, #"[x](https://example.com/a\\)"#),
      (#"https://example.com/a\(b"#, #"[x](https://example.com/a\\\(b)"#),
      ("https://example.com/a b", "[x](<https://example.com/a b>)"),
      ("https://example.com/a b(c)", "[x](<https://example.com/a b(c)>)"),
      ("https://example.com/a b>c", #"[x](<https://example.com/a b\>c>)"#),
      ("https://example.com/a b<c", #"[x](<https://example.com/a b\<c>)"#),
      (#"https://example.com/a b\"#, #"[x](<https://example.com/a b\\>)"#),
      ("https://example.com/a\tb", "[x](<https://example.com/a\tb>)"),
      // Nor a control character, which the pointy form can hold.
      ("/a\u{0001}b", "[x](</a\u{0001}b>)"),
      ("/a\u{007f}b", "[x](</a\u{007f}b>)"),
      ("/a\u{0000}b", "[x](</a\u{0000}b>)"),
      // No destination may hold a line ending, and writing one raw would end
      // the paragraph in the middle of the link.
      ("https://example.com/a\nb", "[x](<https://example.com/a&#10;b>)"),
      ("https://example.com/a\r\nb", "[x](<https://example.com/a&#13;&#10;b>)"),
      ("https://example.com/a\rb", "[x](<https://example.com/a&#13;b>)"),
      ("https://example.com/a b\nc", "[x](<https://example.com/a b&#10;c>)"),
      // Only a `<` in first place would turn the raw form into the pointy
      // one, so an angle bracket anywhere else goes out unescaped.
      ("https://example.com/a<b>c", "[x](https://example.com/a<b>c)"),
      // A URL that already holds a character reference has to keep it.
      ("https://example.com/a&#10;b", "[x](https://example.com/a&#38;#10;b)"),
      ("https://example.com/a&#x0A;b", "[x](https://example.com/a&#38;#x0A;b)"),
      ("https://example.com/a&amp;b", "[x](https://example.com/a&#38;amp;b)"),
      ("&#38;", "[x](&#38;#38;)"),
      // An `&` that begins no reference is ordinary, so a query string keeps
      // the separators it was written with.
      ("https://example.com/?a=1&b=2", "[x](https://example.com/?a=1&b=2)"),
      ("https://example.com/?a=1&b=2;c=3", "[x](https://example.com/?a=1&b=2;c=3)"),
      ("<foo>", #"[x](\<foo>)"#),
      ("", "[x](<>)"),
      ("https://lexical.dev", "[x](https://lexical.dev)"),
    ])
  func preserves(_ url: String, _ md: String) throws {
    try expectLinkRoundTrip(url, nil, md)
  }
}

@Suite struct MarkdownLinkTitleRoundTripTests {
  // A title is read back through `unescapeText` too, so a character reference
  // in one survives only if it goes out as a reference of its own, and a `"`
  // or backslash only with a backslash in front. A line ending goes out as a
  // character reference.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\rb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\r\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\n\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a&#10;\\nb\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a&#10;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a&amp;b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a&b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\\"b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\\\b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"a\\\\\\\"b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves \"plain\""
    ),
    arguments: [
      ("a\nb", #"[x](/uri "a&#10;b")"#),
      ("a\rb", #"[x](/uri "a&#13;b")"#),
      ("a\r\nb", #"[x](/uri "a&#13;&#10;b")"#),
      ("a\n\nb", #"[x](/uri "a&#10;&#10;b")"#),
      ("a&#10;\nb", #"[x](/uri "a&#38;#10;&#10;b")"#),
      ("a&#10;b", #"[x](/uri "a&#38;#10;b")"#),
      ("a&amp;b", #"[x](/uri "a&#38;amp;b")"#),
      ("a&b", #"[x](/uri "a&b")"#),
      (#"a"b"#, #"[x](/uri "a\"b")"#),
      (#"a\b"#, #"[x](/uri "a\\b")"#),
      (#"a\"b"#, #"[x](/uri "a\\\"b")"#),
      ("plain", #"[x](/uri "plain")"#),
    ])
  func preserves(_ title: String, _ md: String) throws {
    try expectLinkRoundTrip("/uri", title, md)
  }
}

@Suite struct MarkdownTextAfterLinkTests {
  // The link markup must not open a construct that the text after it closes:
  // a backslash before the title's closing quote must not escape it, and a
  // backtick must not open a code span, which is read before links are.
  static let after = #" ") `b"#

  struct Case: Sendable, CustomTestStringConvertible {
    let url: String
    let title: String?
    let md: String
    var testDescription: String { "\([url, title ?? "null"])" }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps [\"/uri\",\"a\\\\\"] apart from it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps [\"/uri\",\"a`\"] apart from it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps [\"/a`b\",null] apart from it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps [\"/a `b\",null] apart from it"
    ),
    arguments: [
      Case(url: "/uri", title: #"a\"#, md: #"[x](/uri "a\\") ") \`b"#),
      Case(url: "/uri", title: "a`", md: #"[x](/uri "a\`") ") \`b"#),
      Case(url: "/a`b", title: nil, md: #"[x](/a\`b) ") \`b"#),
      Case(url: "/a `b", title: nil, md: #"[x](</a \`b>) ") \`b"#),
    ])
  func keepsApart(_ testCase: Case) throws {
    try expectLinkRoundTrip(testCase.url, testCase.title, testCase.md, Self.after)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps a backtick in the link apart from inline code after it"
    ))
  func backtickBeforeInlineCode() throws {
    // The code span's own backticks cannot be escaped, so only the escape in
    // the link keeps the two apart.
    try expectLinkRoundTrip("/a`b", nil, #"[x](/a\`b)`c`"#, "c", "code")
  }
}

@Suite struct MarkdownLinkImportTests {
  // Typing reads a link through `LINK.regExp`, which has to agree with the
  // `importRegExp` used here.
  private func shortcutGroups(_ md: String) -> [String?]? {
    LINK.regExp.exec(md).map { Array($0.groups.dropFirst()) }
  }

  private func importGroups(_ md: String) -> [String?]? {
    LINK.importRegExp?.exec(md).map { Array($0.groups.dropFirst()) }
  }

  // Hand-written spellings export never produces: single-quoted and
  // parenthesized titles, and an escaped U+2028 in a title or a pointy
  // destination.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri 'a\\\\\\\\')\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri 'it\\\\'s')\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri (a\\\\)))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri (a\\\\ b))\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri \\\"a\\\\ b\\\")\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](</a\\\\ b> \\\"t\\\")\""
    ),
    arguments: [
      (#"[x](/uri 'a\\')"#, "/uri", #"a\"#),
      (#"[x](/uri 'it\'s')"#, "/uri", "it's"),
      (#"[x](/uri (a\)))"#, "/uri", "a)"),
      ("[x](/uri (a\\\u{2028}b))", "/uri", "a\\\u{2028}b"),
      ("[x](/uri \"a\\\u{2028}b\")", "/uri", "a\\\u{2028}b"),
      ("[x](</a\\\u{2028}b> \"t\")", "/a\\\u{2028}b", "t"),
    ])
  func reads(_ md: String, _ url: String, _ title: String) throws {
    #expect(try importLinks(md) == [ImportedLink(title: title, url: url)])
    #expect(shortcutGroups(md) == importGroups(md))
  }

  // A CR is a line ending in CommonMark, escaped or not, and a pointy
  // destination cannot hold one.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads no link from \"[x](</a\\rb>)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads no link from \"[x](</a\\\\\\rb>)\""
    ), arguments: ["[x](</a\rb>)", "[x](</a\\\rb>)"])
  func readsNoLink(_ md: String) throws {
    #expect(try importLinks(md) == [])
    #expect(shortcutGroups(md) == nil)
  }
}
