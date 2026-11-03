import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts.

private func buildEditor(_ extra: [any AnyLexicalExtensionArgument] = []) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "rich-text-host", dependencies: [RichTextExtension] + extra))
}

private func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) {
    let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
    let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
    try getRoot().clear().append(contentsOf: dep.output.generateNodesFromDOM(dom))
  }
}

private var shadowRootQuoteRules: [any AnyLexicalExtensionArgument] {
  [
    configExtension(DOMImportExtension) { $0.rules = [ShadowRootQuoteRule.entry] }
  ]
}

@Suite struct RichTextImportExtensionTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h1>x</h1> imports as heading h1"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h2>x</h2> imports as heading h2"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h3>x</h3> imports as heading h3"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h4>x</h4> imports as heading h4"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h5>x</h5> imports as heading h5"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h6>x</h6> imports as heading h6"
    ),
    arguments: ["h1", "h2", "h3", "h4", "h5", "h6"])
  func heading(tag: String) throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<\(tag)>x</\(tag)>")
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? HeadingNode)
      #expect(try node.getTag() == tag)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > blockquote imports as quote"
    ))
  func quote() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<blockquote>quoted</blockquote>")
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? QuoteNode)
      #expect(try node.getTextContent() == "quoted")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > blockquote does not import as a shadow root by default"
    ))
  func notShadowRootByDefault() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<blockquote><p>a</p><p>b</p></blockquote>")
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? QuoteNode)
      #expect(!node.isShadowRoot())
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > ShadowRootQuoteRule imports blockquote as a shadow root quote"
    ))
  func shadowRootQuote() throws {
    let editor = try buildEditor(shadowRootQuoteRules)
    defer { editor.dispose() }
    try importInto(editor, "<blockquote><p>a</p><p>b</p></blockquote>")
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? QuoteNode)
      #expect(node.isShadowRoot())
      let children = try node.getChildren()
      #expect(children.count == 2)
      #expect(children.allSatisfy { $0 is ParagraphNode })
      #expect(try children.map { try $0.getTextContent() } == ["a", "b"])
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > ShadowRootQuoteRule wraps bare inline content in a paragraph"
    ))
  func shadowRootQuoteWrapsInline() throws {
    let editor = try buildEditor(shadowRootQuoteRules)
    defer { editor.dispose() }
    try importInto(editor, "<blockquote>quoted</blockquote>")
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? QuoteNode)
      #expect(node.isShadowRoot())
      let children = try node.getChildren()
      try #require(children.count == 1)
      #expect(children[0] is ParagraphNode)
      #expect(try children[0].getTextContent() == "quoted")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > Google Docs title (26pt span) promoted to h1"
    ))
  func googleDocsTitle() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<p><span style="font-size:26pt">Title</span></p>"#)
    try editor.read { () throws in
      let node = try #require(try getRoot().getFirstChild() as? HeadingNode)
      #expect(try node.getTag() == "h1")
      #expect(try node.getTextContent() == "Title")
    }
  }
}
