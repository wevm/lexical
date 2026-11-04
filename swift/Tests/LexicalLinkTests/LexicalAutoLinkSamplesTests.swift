import Lexical
import LexicalExtension
import LexicalLink
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts.

private let urls = [
  "http://example.com",
  "https://example.com",
  "http://www.example.com",
  "https://www.example.com",
  "www.example.com",
  "http://example.org",
  "https://example.net",
  "http://example.co.uk",
  "https://example.xyz",
  "http://example.com/path/to/resource",
  "https://www.example.com/path/to/resource",
  "http://example.com/path?name=value",
  "https://www.example.com/path?name=value&another=value2",
  "http://example.com/path#section",
  "https://www.example.com/path/to/resource#fragment",
  "http://example.com:8080",
  "https://www.example.com:443/path",
  "http://192.168.0.1",
  "https://127.0.0.1",
  "http://example.com/path/to/res+ource",
  "https://example.com/path/to/res%20ource",
  "http://example.com/path?name=va@lue",
  "https://example.com/path?name=value&another=val%20ue",
  "http://subdomain.example.com",
  "https://sub.subdomain.example.com",
  "http://example.museum",
  "https://example.travel",
  "http://foo.bar",
  "https://foo.bar",
]
private let emails = [
  "email@domain.com",
  "firstname.lastname@domain.com",
  "email@subdomain.domain.com",
  "firstname+lastname@domain.com",
  "email@[123.123.123.123]",
  "\"email\"@domain.com",
  "1234567890@domain.com",
  "email@domain-one.com",
  "_______@domain.com",
  "email@domain.name",
  "email@domain.co.uk",
  "firstname-lastname@domain.com",
]
private let invalidUrls = [
  "example.com",
  "htp://example.com",
  "htps://example.com",
  "http://exa mple.com",
  "https://example .com",
  "http://example!.com",
  "http://.com",
  "https://.org",
  "http://",
  "https://",
  "not_a_url",
  "this is not a url",
  "example",
  "ftp://example.com",
]
private let invalidEmails = [
  "@domain.com",
  "@subdomain.domain.com",
  "email@domain!.com",
  "email@domain.c",
  "email@.com",
  "email@.org",
  "email@",
  "not_an_email",
]

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[test autolink samples]",
      dependencies: [
        configExtension(AutoLinkExtension) {
          $0.matchers = [autoLinkUrlMatcher, autoLinkEmailMatcher]
        }
      ],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          try getRoot().append(createParagraphNode()).selectEnd()
        })))
}

private func insertText(_ editor: Editor, _ text: String, _ incremental: Bool) throws {
  // Commit between characters so matching, splitting and link edits run at
  // every insertion boundary, as they do during typing. `Array.from(text)`
  // splits by code point.
  for chunk in incremental ? text.unicodeScalars.map { String($0) } : [text] {
    try editor.update(discrete: true) {
      let selection = try #require(try getSelection() as? RangeSelection)
      try selection.insertText(chunk)
    }
  }
}

struct LinkSample: Sendable, CustomTestStringConvertible {
  var prefix: String
  var suffix: String
  var text: String
  var url: String
  var testDescription: String { text }
}

private let links: [LinkSample] =
  urls.map {
    LinkSample(
      prefix: "", suffix: " ltr", text: $0, url: $0.hasPrefix("http") ? $0 : "https://\($0)")
  }
  + emails.map { LinkSample(prefix: "", suffix: " ltr", text: $0, url: "mailto:\($0)") }
  + [
    LinkSample(
      prefix: "مرحبا ", suffix: " end", text: "https://qabilah.com/posts/عربي",
      url: "https://qabilah.com/posts/عربي"),
    LinkSample(
      prefix: "go ", suffix: " done", text: "http://예시.한국/경로?키=값#부분",
      url: "http://예시.한국/경로?키=값#부분"),
  ]

private func checkLink(_ sample: LinkSample, incremental: Bool) throws {
  let editor = try buildEditor()
  defer { editor.dispose() }
  let input = sample.prefix + sample.text + sample.suffix
  try insertText(editor, input, incremental)
  try editor.read { () throws -> Void in
    let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
    #expect(try getRoot().getChildrenSize() == 1)
    #expect(try paragraph.getTextContent() == input)
    #expect(try paragraph.getChildrenSize() == (sample.prefix.isEmpty ? 2 : 3))
    let link = try #require(
      try paragraph.getChildAtIndex(sample.prefix.isEmpty ? 0 : 1) as? AutoLinkNode)
    #expect(try link.getURL() == sample.url)
    #expect(try link.getIsUnlinked() == false)
    #expect(try link.getChildrenSize() == 1)
    #expect(try link.getFirstChildOrThrow().getTextContent() == sample.text)
    if !sample.prefix.isEmpty {
      let before = try #require(try paragraph.getFirstChildOrThrow() as? TextNode)
      #expect(try before.getTextContent() == sample.prefix)
    }
    let after = try #require(try paragraph.getLastChildOrThrow() as? TextNode)
    #expect(try after.getTextContent() == sample.suffix)
  }
}

private let invalidSamples = [invalidUrls, invalidEmails]

private func checkUnchanged(_ samples: [String], incremental: Bool) throws {
  let editor = try buildEditor()
  defer { editor.dispose() }
  let text = samples.joined(separator: " ")
  try insertText(editor, text, incremental)
  try editor.read { () throws -> Void in
    let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
    #expect(try getRoot().getChildrenSize() == 1)
    #expect(try paragraph.getChildrenSize() == 1)
    let child = try #require(try paragraph.getFirstChildOrThrow() as? TextNode)
    #expect(try child.getTextContent() == text)
  }
}

@Suite struct LexicalAutoLinkSamplesTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.org"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.net"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.co.uk"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.xyz"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path/to/resource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path/to/resource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path?name=value"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path?name=value…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path#section"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path/to/resourc…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com:8080"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com:443/path"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://192.168.0.1"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://127.0.0.1"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path/to/res+ource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com/path/to/res%20ource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path?name=va@lue"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com/path?name=value&ano…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://subdomain.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://sub.subdomain.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.museum"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.travel"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://foo.bar"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://foo.bar"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname.lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@subdomain.domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname+lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@[123.123.123.123]"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > \"email\"@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > 1234567890@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain-one.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > _______@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.name"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.co.uk"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname-lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://qabilah.com/posts/عربي"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://예시.한국/경로?키=값#부분"
    ),
    arguments: links)
  func transform(_ sample: LinkSample) throws {
    try checkLink(sample, incremental: false)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > leaves invalid URLs unchanged"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > leaves invalid emails unchanged"
    ),
    arguments: invalidSamples)
  func leavesInvalidUnchanged(_ samples: [String]) throws {
    try checkUnchanged(samples, incremental: false)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.org"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.net"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.co.uk"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.xyz"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path/to/resource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path/to/resource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path?name=value"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path?name=value…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path#section"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path/to/resourc…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com:8080"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com:443/path"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://192.168.0.1"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://127.0.0.1"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path/to/res+ource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com/path/to/res%20ource"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path?name=va@lue"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com/path?name=value&ano…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://subdomain.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://sub.subdomain.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.museum"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.travel"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://foo.bar"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://foo.bar"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname.lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@subdomain.domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname+lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@[123.123.123.123]"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > \"email\"@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > 1234567890@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain-one.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > _______@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.name"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.co.uk"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname-lastname@domain.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://qabilah.com/posts/عربي"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://예시.한국/경로?키=값#부분"
    ),
    arguments: links)
  func transformIncremental(_ sample: LinkSample) throws {
    try checkLink(sample, incremental: true)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > leaves invalid URLs unchanged"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > leaves invalid emails unchanged"
    ),
    arguments: invalidSamples)
  func leavesInvalidUnchangedIncremental(_ samples: [String]) throws {
    try checkUnchanged(samples, incremental: true)
  }
}
