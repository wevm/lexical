import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-html/src/__tests__/unit/CoreImportExtension.test.ts.

func buildImportEditor(_ extra: [any AnyLexicalExtensionArgument] = []) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "core-host", dependencies: [CoreImportExtension] + extra))
}

func generateNodes(_ html: String) throws -> [LexicalNode] {
  let editor = try getEditor()
  let dep = try getExtensionDependencyFromEditor(editor, DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom)
}

func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) {
    let nodes = try generateNodes(html)
    try getRoot().clear().append(contentsOf: nodes)
  }
}

func findText(_ content: String) throws -> TextNode {
  try #require(try getRoot().getAllTextNodes().first { try $0.getTextContent() == content })
}

func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChild() as? ParagraphNode)
}

@Suite struct CoreImportExtensionTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > paragraph + text"
    ))
  func paragraphText() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p>Hello world</p>")
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "Hello world")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > inline format tags propagate via ImportTextFormat"
    ))
  func inlineFormats() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p>a <strong>b <em>c</em></strong> <code>d</code></p>")
    try editor.read { () throws in
      #expect(try findText("a ").getFormat() == 0)
      #expect(try findText("b ").hasFormat("bold"))
      #expect(try !findText("b ").hasFormat("italic"))
      #expect(try findText("c").hasFormat("bold"))
      #expect(try findText("c").hasFormat("italic"))
      #expect(try findText("d").hasFormat("code"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > span with Google-Docs-style CSS pushes formats into context"
    ))
  func spanStyles() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(
      editor,
      #"<p><span style="font-weight:700">bold</span> <span style="font-style:italic">italic</span> <span style="text-decoration:underline line-through">both</span></p>"#
    )
    try editor.read { () throws in
      #expect(try findText("bold").hasFormat("bold"))
      #expect(try findText("italic").hasFormat("italic"))
      #expect(try findText("both").hasFormat("underline"))
      #expect(try findText("both").hasFormat("strikethrough"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <b style=\"font-weight:normal\"> (Google Docs wrapper) does NOT add bold"
    ))
  func googleDocsWrapper() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<b style="font-weight:normal"><p>plain</p></b>"#)
    try editor.read { () throws in
      #expect(try !findText("plain").hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <pre> preserves whitespace, splits on \\n into LineBreakNode"
    ))
  func preWhitespace() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<pre>line1\nline2</pre>")
    try editor.read { () throws in
      let children = try firstParagraph().getChildren()
      try #require(children.count == 3)
      #expect(try (children[0] as? TextNode)?.getTextContent() == "line1")
      #expect(children[1] is LineBreakNode)
      #expect(try (children[2] as? TextNode)?.getTextContent() == "line2")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace collapsing matches legacy behavior"
    ))
  func collapse() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p>  hello   world  </p>")
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "hello world")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <br> creates a LineBreakNode"
    ))
  func lineBreak() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p>a<br>b</p>")
    try editor.read { () throws in
      let children = try firstParagraph().getChildren()
      try #require(children.count == 3)
      #expect(children[1] is LineBreakNode)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > paragraph align attribute fallback"
    ))
  func alignAttribute() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<p align="center">center</p>"#)
    try editor.read { () throws in
      #expect(try firstParagraph().getFormatType() == "center")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > inline style with font-weight:normal clears inherited bold"
    ))
  func normalClearsBold() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(
      editor,
      #"<p><strong>bold <span style="font-weight: normal">plain</span> bold</strong></p>"#)
    try editor.read { () throws in
      #expect(try findText("bold ").hasFormat("bold"))
      #expect(try !findText("plain").hasFormat("bold"))
      #expect(try findText(" bold").hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > sub/sup mutex: <sub><sup>x</sup></sub> ⇒ superscript only"
    ))
  func subSupMutex() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p><sub><sup>x</sup></sub></p>")
    try editor.read { () throws in
      let text = try #require(try getRoot().getAllTextNodes().first)
      #expect(try text.hasFormat("superscript"))
      #expect(try !text.hasFormat("subscript"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > text-decoration:none clears inherited underline/strikethrough"
    ))
  func decorationNone() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(
      editor, #"<p><u><s>both <span style="text-decoration: none">neither</span></s></u></p>"#)
    try editor.read { () throws in
      #expect(try findText("both ").hasFormat("underline"))
      #expect(try findText("both ").hasFormat("strikethrough"))
      #expect(try !findText("neither").hasFormat("underline"))
      #expect(try !findText("neither").hasFormat("strikethrough"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace config can override what counts as preserving whitespace"
    ))
  func customPreserve() throws {
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) {
        $0.contextDefaults = [
          contextValue(
            ImportWhitespaceConfig,
            WhitespaceImportConfig(preservesWhitespace: { node in
              defaultPreservesWhitespace(node)
                || ((node as? DOMElement)?.classList.contains("keep-ws") ?? false)
            }))
        ]
      }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<div class=\"keep-ws\">a   b\nc</div>")
    try editor.read { () throws in
      let children = try firstParagraph().getChildren()
      try #require(children.count == 3)
      #expect(try (children[0] as? TextNode)?.getTextContent() == "a   b")
      #expect(children[1] is LineBreakNode)
      #expect(try (children[2] as? TextNode)?.getTextContent() == "c")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace config can override what counts as an inline sibling"
    ))
  func customInline() throws {
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) {
        $0.contextDefaults = [
          contextValue(
            ImportWhitespaceConfig,
            WhitespaceImportConfig(isInline: { node in
              node.nodeName == "HR" || defaultIsInline(node)
            }))
        ]
      }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<p>a <hr>b</p>")
    try editor.read { () throws in
      // With <hr> treated as inline, the trailing space after "a" is kept.
      _ = try findText("a ")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > session can be written by an early rule and read by a later one"
    ))
  func sessionAcrossRules() throws {
    let styleSheets = createImportState("collected-styles") { [String]() }
    let collect = defineImportRule(name: "test/collect-style", match: sel.tag("style")) {
      ctx, el, _ in
      ctx.session.update(styleSheets) { $0 + [el.textContent] }
      return []
    }
    let styleAware = defineImportRule(name: "test/style-aware", match: sel.tag("article")) {
      ctx, el, _ in
      let sheets = ctx.session.get(styleSheets)
      return [
        try createParagraphNode().append(
          createTextNode("[saw \(sheets.count) sheets] \(el.textContent)"))
      ]
    }
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) { $0.rules = [collect.entry, styleAware.entry] }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<style>.a{}</style><style>.b{}</style><article>body</article>")
    try editor.read { () throws in
      #expect(try getRoot().getAllTextNodes().first?.getTextContent() == "[saw 2 sheets] body")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <style> can be overridden by an app-specific rule (default is now a rule, not a framework skip)"
    ))
  func styleOverride() throws {
    nonisolated(unsafe) var captured: String? = nil
    let capture = defineImportRule(name: "test/style-capture", match: sel.tag("style")) {
      _, el, _ in
      captured = el.textContent
      return []
    }
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) { $0.rules = [capture.entry] }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<style>.foo{color:red}</style><p>x</p>")
    #expect(captured == ".foo{color:red}")
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "x")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > unconverted block elements (not just <div>) preserve block boundaries"
    ))
  func blockBoundaries() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, "<section>a</section><article>b</article>")
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["a", "b"])
      #expect(children.allSatisfy { $0 is ParagraphNode })
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > text-align on a non-<div> block element is propagated to its paragraph"
    ))
  func textAlignPropagates() throws {
    let editor = try buildImportEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<header style="text-align: right">x</header>"#)
    try editor.read { () throws in
      #expect(try firstParagraph().getFormatType() == "right")
    }
  }
}
