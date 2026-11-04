import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts.
// Upstream's editor also has TableExtension; the editor here leaves it out
// until @lexical/table is ported, and the case that imports a table inside a
// <pre> is skipped. The GitHub code tables are CodeExtension's own rules,
// which win over TableExtension's upstream.

private class SemanticParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SemanticParagraphNode.self, type: "semantic-paragraph", extends: ParagraphNode.self)
  }
  override func getTextContent() throws -> String { "[\(try super.getTextContent())]" }
  override func getTextContentSize() throws -> Int { try getTextContent().utf16.count }
}

private final class InlineSemanticParagraphNode: SemanticParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InlineSemanticParagraphNode.self, type: "inline-semantic-paragraph",
      extends: SemanticParagraphNode.self)
  }
  override func isInline() throws -> Bool { true }
}

private final class TextDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TextDecoratorNode.self, type: "text-decorator", extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
  override func getTextContent() throws -> String { "[attachment]" }
  override func getTextContentSize() throws -> Int { try getTextContent().utf16.count }
}

private func buildEditor(
  _ rules: [DOMImportRule] = [], replaceParagraphs: Bool = false
) throws
  -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: "[code-content-import]",
      dependencies: [
        RichTextExtension, ListExtension, LinkExtension, CodeExtension,
        configExtension(DOMImportExtension) { $0.rules = rules.map(\.entry) },
      ],
      nodes: {
        [
          .klass(SemanticParagraphNode.self), .klass(InlineSemanticParagraphNode.self),
          .klass(TextDecoratorNode.self),
        ]
          + (replaceParagraphs
            ? [
              .replacement(
                LexicalNodeReplacement(
                  replace: ParagraphNode.self, with: { _ in try SemanticParagraphNode() },
                  withKlass: SemanticParagraphNode.self))
            ] : [])
      }))
}

private func parse(_ html: String) -> DOMDocument {
  DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>", "text/html")
}

private func githubTable(_ contents: String) -> String {
  #"<table class="js-file-line-container">\#(contents)</table>"#
}

private func expectCodeContent(
  _ node: LexicalNode?, _ text: String, sourceLocation: SourceLocation = #_sourceLocation
) throws {
  let node = try #require(node as? CodeNode, sourceLocation: sourceLocation)
  #expect(try node.getTextContent() == text, sourceLocation: sourceLocation)
  let children = try node.getChildren()
  #expect(
    children.allSatisfy { $0 is CodeHighlightNode || $0 is TabNode || $0 is LineBreakNode },
    sourceLocation: sourceLocation)
  for child in children where child is CodeHighlightNode {
    let content = try child.getTextContent()
    #expect(
      !content.unicodeScalars.contains { $0 == "\r" || $0 == "\n" || $0 == "\t" },
      sourceLocation: sourceLocation)
  }
}

struct CodeContentCase: Sendable, CustomTestStringConvertible {
  let name: String
  let html: String
  let text: String
  var testDescription: String { name }
}

private let CASES: [CodeContentCase] = [
  CodeContentCase(
    name: "PRE with two nonempty DIV lines", html: "<pre><div>alpha</div><div>beta</div></pre>",
    text: "alpha\nbeta"),
  CodeContentCase(
    name: "PRE with an explicitly empty paragraph", html: "<pre><p><br></p></pre>", text: ""),
  CodeContentCase(
    name: "PRE with consecutive empty paragraphs",
    html: "<pre><p><br></p><p><br></p><p><br></p></pre>", text: "\n\n"),
  CodeContentCase(
    name: "PRE with a leading empty paragraph", html: "<pre><p><br></p><p>alpha</p></pre>",
    text: "\nalpha"),
  CodeContentCase(
    name: "PRE with an empty paragraph between lines",
    html: "<pre><p>alpha</p><p><br></p><p>beta</p></pre>", text: "alpha\n\nbeta"),
  CodeContentCase(
    name: "PRE with a trailing empty paragraph", html: "<pre><p>alpha</p><p><br></p></pre>",
    text: "alpha\n"),
  CodeContentCase(
    name: "PRE with mixed inline and block segments", html: "<pre>alpha<p>beta</p>gamma</pre>",
    text: "alpha\nbeta\ngamma"),
  CodeContentCase(
    name: "PRE with nested transparent block wrappers",
    html: "<pre><section><div>alpha</div><div>beta</div></section><p>gamma</p></pre>",
    text: "alpha\nbeta\ngamma"),
  CodeContentCase(
    name: "multiline CODE with DIV lines",
    html: "<code><div>alpha<br></div><div>beta</div></code>", text: "alpha\nbeta"),
  CodeContentCase(
    name: "multiline CODE with an empty paragraph",
    html: "<code><p>alpha</p><p><br></p><p>beta</p></code>", text: "alpha\n\nbeta"),
  CodeContentCase(
    name: "monospace DIV with SECTION lines",
    html:
      #"<div style="font-family: monospace"><section>alpha</section><section>beta</section></div>"#,
    text: "alpha\nbeta"),
  CodeContentCase(
    name: "monospace DIV with an empty paragraph",
    html: #"<div style="font-family: monospace"><p>alpha</p><p><br></p><p>beta</p></div>"#,
    text: "alpha\n\nbeta"),
  CodeContentCase(
    name: "GitHub code table with DIV lines in separate rows",
    html: githubTable(
      #"<tr><td class="js-file-line"><div>alpha</div></td></tr>"#
        + #"<tr><td class="js-file-line"><div>beta</div></td></tr>"#),
    text: "alpha\nbeta"),
  CodeContentCase(
    name: "GitHub code table with an empty paragraph between lines",
    html: githubTable(
      #"<tr><td class="js-file-line"><p>alpha</p><p><br></p><p>beta</p></td></tr>"#),
    text: "alpha\n\nbeta"),
  CodeContentCase(
    name: "GitHub code table with an explicitly empty paragraph",
    html: githubTable(#"<tr><td class="js-file-line"><p><br></p></td></tr>"#), text: ""),
  CodeContentCase(
    name: "PRE with flat text tabs and linebreaks stays unchanged",
    html: "<pre>\talpha\nbeta</pre>", text: "\talpha\nbeta"),
  CodeContentCase(
    name: "nested multiline CODE alongside paragraph and inline text",
    html: "<pre>a<code>alpha\nbeta</code><p>after</p>b</pre>", text: "a\nalpha\nbeta\nafter\nb"),
  CodeContentCase(
    name: "heading followed by a paragraph", html: "<pre><h2>heading</h2><p>body</p></pre>",
    text: "heading\nbody"),
  CodeContentCase(
    name: "link text followed by a paragraph",
    html: #"<pre><a href="https://example.com/">alpha</a><p>beta</p></pre>"#,
    text: "alpha\nbeta"),
  CodeContentCase(
    name: "list text preserves its own item separators",
    html: "<pre><ul><li>alpha</li><li>beta</li></ul><p>after</p></pre>",
    text: "alpha\n\nbeta\nafter"),
  CodeContentCase(
    name: "hard linebreaks and tabs", html: "<pre>alpha<br>beta\tgamma<br><br>delta</pre>",
    text: "alpha\nbeta\tgamma\n\ndelta"),
  CodeContentCase(
    name: "VS Code Chrome wrapper",
    html:
      "<div style=\"font-family:monospace;white-space:pre\"><div>\talpha</div><br><div>beta</div></div>",
    text: "\talpha\n\nbeta"),
  CodeContentCase(
    name: "VS Code Safari sibling run",
    html:
      "<div style=\"font-family:monospace;white-space:pre\">\talpha</div><br style=\"font-family:monospace;white-space:pre\"><div style=\"font-family:monospace;white-space:pre\">beta</div>",
    text: "\talpha\n\nbeta"),
  CodeContentCase(
    name: "plain monospace DIV with nested DIV lines",
    html: #"<div style="font-family:monospace"><div>alpha</div><div>beta</div></div>"#,
    text: "alpha\nbeta"),
]

@Suite struct CodeContentImportTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with two nonempty DIV lines"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with an explicitly empty paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with consecutive empty paragraphs"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with a leading empty paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with an empty paragraph between lin…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with a trailing empty paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with mixed inline and block segments"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with nested transparent block wrapp…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > multiline CODE with DIV lines"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > multiline CODE with an empty paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > monospace DIV with SECTION lines"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > monospace DIV with an empty paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with DIV lines in sep…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with an empty paragra…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with an explicitly em…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with flat text tabs and linebreaks …"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > nested multiline CODE alongside paragra…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > heading followed by a paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > link text followed by a paragraph"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > list text preserves its own item separa…"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > hard linebreaks and tabs"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > VS Code Chrome wrapper"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > VS Code Safari sibling run"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > plain monospace DIV with nested DIV lin…"
    ),
    arguments: CASES)
  func normalizesContent(_ c: CodeContentCase) throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(parse(c.html))
      #expect(nodes.count == 1)
      try expectCodeContent(nodes.first, c.text)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom quote node"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom semantic-paragraph node"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom inline-semantic-paragraph node"
    ),
    arguments: ["quote", "semantic-paragraph", "inline-semantic-paragraph"])
  func preservesCustomTextSemantics(_ type: String) throws {
    let editor = try buildEditor([
      defineImportRule(name: "custom-code-child", match: sel.tag("div").classAll("custom-block")) {
        _, _, _ in
        let custom: ElementNode =
          switch type {
          case "quote": try createQuoteNode()
          case "semantic-paragraph": try create(SemanticParagraphNode.self)
          default: try create(InlineSemanticParagraphNode.self)
          }
        try custom.append(createTextNode("kept"))
        return type == "inline-semantic-paragraph"
          ? [try createTextNode("before"), custom, try createTextNode("after")]
          : [custom, try createParagraphNode().append(createTextNode("plain"))]
      }
    ])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(
        parse(#"<pre><div class="custom-block">ignored</div></pre>"#))
      try expectCodeContent(
        nodes.first,
        type == "quote"
          ? "kept\nplain" : type == "semantic-paragraph" ? "[kept]\nplain" : "before[kept]after")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > uses configured paragraph replacements for their text semantics"
    ))
  func paragraphReplacements() throws {
    let editor = try buildEditor(replaceParagraphs: true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(
        parse("<pre><p>alpha</p><p><br></p><p>beta</p></pre>"))
      try expectCodeContent(nodes.first, "[alpha]\n[]\n[beta]")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > normalizes raw CRLF and tabs returned by a custom importer"
    ))
  func rawCRLFAndTabs() throws {
    let editor = try buildEditor([
      defineImportRule(name: "custom-raw-code-text", match: sel.tag("span").classAll("custom-text"))
      {
        _, _, _ in [try createTextNode("alpha\r\n\tbeta")]
      }
    ])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(
        parse(#"<pre><span class="custom-text">ignored</span></pre>"#))
      try expectCodeContent(nodes.first, "alpha\n\tbeta")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > uses a block decorator text fallback between inline runs"
    ))
  func blockDecoratorFallback() throws {
    let editor = try buildEditor([
      defineImportRule(
        name: "custom-code-attachment", match: sel.tag("span").classAll("attachment")
      ) {
        _, _, _ in [try create(TextDecoratorNode.self)]
      }
    ])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(
        parse(#"<pre>before<span class="attachment"></span>after</pre>"#))
      try expectCodeContent(nodes.first, "before\n[attachment]\nafter")
    }
  }
}
