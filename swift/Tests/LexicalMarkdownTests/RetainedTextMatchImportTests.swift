import Lexical
import LexicalLink
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts.

nonisolated(unsafe) private let RETAINED_TRANSFORMERS: [any Transformer] =
  TEXT_FORMAT_TRANSFORMERS + [LINK]
nonisolated(unsafe) private let FORMAT_IN_PLACE = TextMatchTransformer(
  dependencies: [], importRegExp: JSRegExp("@[^ ]+"), regExp: JSRegExp("@[^ ]+"),
  replace: { node, _ in
    _ = try node.toggleFormat("highlight")
    return nil
  })

/// One imported text node: its text, format, and the URL of its parent
/// link (`nil` when the parent is not a link).
private struct ImportedText: Equatable {
  var format: Int
  var text: String
  var url: String?
}

private func importText(
  _ markdown: String, _ transformers: [any Transformer], _ detached: Bool
)
  throws -> [ImportedText]
{
  let editor = try createMarkdownTestEditor(nodes: [.klass(LinkNode.self)])
  var result: [ImportedText] = []
  try editor.update(discrete: true) {
    let container: ElementNode
    if detached {
      let nodes = try generateNodesFromMarkdownString(markdown, transformers)
      let paragraph = try #require(nodes.first as? ElementNode)
      #expect(try paragraph.isAttached() == false)
      #expect(try getRoot().getChildrenSize() == 0)
      container = paragraph
    } else {
      try convertFromMarkdownString(markdown, transformers)
      container = try getRoot()
    }
    result = try container.getAllTextNodes().map { node in
      let parent = try node.getParent()
      return ImportedText(
        format: try node.getFormat(), text: try node.getTextContent(),
        url: try (parent as? LinkNode)?.getURL())
    }
  }
  return result
}

/// A case of the `describe.each` blocks: detached or not, the prefix, and
/// the markdown and expected text of an `it.each` row.
struct RetainedCase: Sendable, CustomTestStringConvertible {
  let detached: Bool
  let prefix: String
  let md: String
  let text: String
  init(_ detached: Bool, _ prefix: String, _ md: String = "", _ text: String = "") {
    self.detached = detached
    self.prefix = prefix
    self.md = md
    self.text = text
  }
  func with(_ md: String) -> RetainedCase { RetainedCase(detached, prefix, md, text) }
  var testDescription: String { "detached: \(detached), prefix \"\(prefix)\": \(md)" }
}

/// `read(markdown, transformer)` of upstream's `describe.each`.
private func read(
  _ c: RetainedCase, _ transformer: TextMatchTransformer? = nil
) throws
  -> [ImportedText]
{
  try importText(
    c.prefix + c.md,
    transformer.map { [$0] + RETAINED_TRANSFORMERS } ?? RETAINED_TRANSFORMERS, c.detached)
}

@Suite struct RetainedTextMatchImportTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text formatted in place exactly once: @a\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text formatted in place exactly once: @a\\\\\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text formatted in place exactly once: @a&#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text formatted in place exactly once: @a&#38;#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a\\\\\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a&#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a&#38;#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text formatted in place exactly once: @a\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text formatted in place exactly once: @a\\\\\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text formatted in place exactly once: @a&#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text formatted in place exactly once: @a&#38;#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a\\\\\\*b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a&#65;b"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text formatted in place exactly once: @a&#38;#65;b"
    ),
    arguments: [
      RetainedCase(false, "", "@a\\*b", "@a*b"),
      RetainedCase(false, "", "@a\\\\\\*b", "@a\\*b"),
      RetainedCase(false, "", "@a&#65;b", "@aAb"),
      RetainedCase(false, "", "@a&#38;#65;b", "@a&#65;b"),
      RetainedCase(false, "prefix ", "@a\\*b", "@a*b"),
      RetainedCase(false, "prefix ", "@a\\\\\\*b", "@a\\*b"),
      RetainedCase(false, "prefix ", "@a&#65;b", "@aAb"),
      RetainedCase(false, "prefix ", "@a&#38;#65;b", "@a&#65;b"),
      RetainedCase(true, "", "@a\\*b", "@a*b"),
      RetainedCase(true, "", "@a\\\\\\*b", "@a\\*b"),
      RetainedCase(true, "", "@a&#65;b", "@aAb"),
      RetainedCase(true, "", "@a&#38;#65;b", "@a&#65;b"),
      RetainedCase(true, "prefix ", "@a\\*b", "@a*b"),
      RetainedCase(true, "prefix ", "@a\\\\\\*b", "@a\\*b"),
      RetainedCase(true, "prefix ", "@a&#65;b", "@aAb"),
      RetainedCase(true, "prefix ", "@a&#38;#65;b", "@a&#65;b"),
    ])
  func unescapesFormattedInPlaceOnce(_ c: RetainedCase) throws {
    let nodes = try read(c, FORMAT_IN_PLACE)
    #expect(nodes.map(\.text).joined() == c.prefix + c.text)
    #expect(nodes.last?.format == TEXT_TYPE_TO_FORMAT["highlight"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text a link transformer declines: [a\\*b]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes text a link transformer declines: [a&#65;]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text a link transformer declines: [a\\*b]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes text a link transformer declines: [a&#65;]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text a link transformer declines: [a\\*b]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes text a link transformer declines: [a&#65;]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text a link transformer declines: [a\\*b]](url)"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes text a link transformer declines: [a&#65;]](url)"
    ),
    arguments: [
      RetainedCase(false, "", "[a\\*b]](url)", "[a*b]](url)"),
      RetainedCase(false, "", "[a&#65;]](url)", "[aA]](url)"),
      RetainedCase(false, "prefix ", "[a\\*b]](url)", "[a*b]](url)"),
      RetainedCase(false, "prefix ", "[a&#65;]](url)", "[aA]](url)"),
      RetainedCase(true, "", "[a\\*b]](url)", "[a*b]](url)"),
      RetainedCase(true, "", "[a&#65;]](url)", "[aA]](url)"),
      RetainedCase(true, "prefix ", "[a\\*b]](url)", "[a*b]](url)"),
      RetainedCase(true, "prefix ", "[a&#65;]](url)", "[aA]](url)"),
    ])
  func unescapesTextLinkDeclines(_ c: RetainedCase) throws {
    let nodes = try read(c)
    #expect(nodes.map(\.text).joined() == c.prefix + c.text)
    #expect(nodes.allSatisfy { $0.url == nil })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > unescapes a matched node reused inside a link without returning it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > unescapes a matched node reused inside a link without returning it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > unescapes a matched node reused inside a link without returning it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > unescapes a matched node reused inside a link without returning it"
    ),
    arguments: [
      RetainedCase(false, ""),
      RetainedCase(false, "prefix "),
      RetainedCase(true, ""),
      RetainedCase(true, "prefix "),
    ])
  func matchedNodeReusedInLink(_ c: RetainedCase) throws {
    let transformer = FORMAT_IN_PLACE.copy()
    transformer.dependencies = [LinkNode.self]
    transformer.replace = { node, _ in
      let link = try createLinkNode("https://example.com")
      _ = try node.replace(link)
      _ = try link.append(node)
      return nil
    }
    let nodes = try read(c.with(#"@a\\\*b"#), transformer)
    #expect(
      nodes.last == ImportedText(format: 0, text: #"@a\*b"#, url: "https://example.com"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > does not unescape a returned node twice"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > does not unescape a returned node twice"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > does not unescape a returned node twice"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > does not unescape a returned node twice"
    ),
    arguments: [
      RetainedCase(false, ""),
      RetainedCase(false, "prefix "),
      RetainedCase(true, ""),
      RetainedCase(true, "prefix "),
    ])
  func returnedNodeNotUnescapedTwice(_ c: RetainedCase) throws {
    let transformer = FORMAT_IN_PLACE.copy()
    transformer.replace = { node, _ in
      try node.setTextContent(node.getTextContent().jsSlice(1))
    }
    let nodes = try read(c.with(#"@a\\\*b"#), transformer)
    #expect(nodes.map(\.text).joined() == c.prefix + #"a\*b"#)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > keeps literal content when a transformer formats the node as code"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > keeps literal content when a transformer formats the node as code"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > keeps literal content when a transformer formats the node as code"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > keeps literal content when a transformer formats the node as code"
    ),
    arguments: [
      RetainedCase(false, ""),
      RetainedCase(false, "prefix "),
      RetainedCase(true, ""),
      RetainedCase(true, "prefix "),
    ])
  func literalContentFormattedAsCode(_ c: RetainedCase) throws {
    let text = #"@a\*&#65;b"#
    let transformer = FORMAT_IN_PLACE.copy()
    transformer.replace = { node, _ in
      _ = try node.toggleFormat("code")
      return nil
    }
    let nodes = try read(c.with(text), transformer)
    #expect(nodes.last == ImportedText(format: TEXT_TYPE_TO_FORMAT["code"]!, text: text, url: nil))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"\" > leaves newly generated replacement text literal"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix \"prefix \" > leaves newly generated replacement text literal"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"\" > leaves newly generated replacement text literal"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix \"prefix \" > leaves newly generated replacement text literal"
    ),
    arguments: [
      RetainedCase(false, ""),
      RetainedCase(false, "prefix "),
      RetainedCase(true, ""),
      RetainedCase(true, "prefix "),
    ])
  func generatedReplacementTextLiteral(_ c: RetainedCase) throws {
    let text = #"literal\*&#65;"#
    let transformer = FORMAT_IN_PLACE.copy()
    transformer.replace = { node, _ in
      _ = try node.replace(createTextNode(text))
      return nil
    }
    let nodes = try read(c.with("@replace"), transformer)
    #expect(nodes.map(\.text).joined() == c.prefix + text)
  }
}
