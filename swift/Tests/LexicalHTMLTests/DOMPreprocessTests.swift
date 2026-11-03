import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-html/src/__tests__/unit/DOMPreprocess.test.ts and
// HorizontalRuleImportExtension.test.ts. `HorizontalRuleExtension` is not
// ported yet (it needs NodeSelectionExtension); the `<hr>` tests register
// `HorizontalRuleNode`, which is all the import rule checks for.

private func elements(_ dom: DOMNode, _ tag: String) -> [DOMElement] {
  let root = (dom as? DOMDocument)?.documentElement ?? (dom as? DOMElement)
  return root?.descendantElements().filter { $0.localName == tag } ?? []
}

private func buildHost(_ deps: [any AnyLexicalExtensionArgument]) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "host", dependencies: [CoreImportExtension] + deps))
}

@Suite struct DOMPreprocessTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > default $inlineStylesFromStyleSheets resolves <style> rules to inline styles"
    ))
  func inlinesStyleSheets() throws {
    let editor = try buildHost([])
    defer { editor.dispose() }
    try importInto(
      editor, #"<style>.xl1 { font-weight: 700; }</style><p><span class="xl1">bold</span></p>"#)
    try editor.read { () throws in
      #expect(try getRoot().getAllTextNodes().first?.hasFormat("bold") == true)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > app preprocess can mutate the DOM before walking"
    ))
  func mutatesDOM() throws {
    let stripScripts: DOMPreprocessFn = { dom, _, next in
      for el in elements(dom, "script") { el.remove() }
      try next()
    }
    let editor = try buildHost([
      configExtension(DOMImportExtension) { $0.preprocess = [stripScripts] }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<p>before<script>alert(1)</script>after</p>")
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "beforeafter")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocess can write to the session for the rest of the import"
    ))
  func writesSessionFromMeta() throws {
    let sourceState = createImportState("test/import-source") { "unknown" }
    let readSourceMeta: DOMPreprocessFn = { dom, ctx, next in
      if let meta = elements(dom, "meta").first(where: {
        $0.getAttribute("name") == "lexical-source"
      }),
        let content = meta.getAttribute("content"), !content.isEmpty
      {
        ctx.session.set(sourceState, content)
      }
      try next()
    }
    let divRule = defineImportRule(name: "test/source-aware-div", match: sel.tag("div")) {
      ctx, _, _ in
      [try createParagraphNode().append(createTextNode("[\(try ctx.get(sourceState))]"))]
    }
    let editor = try buildHost([
      configExtension(DOMImportExtension) {
        $0.preprocess = [readSourceMeta]
        $0.rules = [divRule.entry]
      }
    ])
    defer { editor.dispose() }
    try importInto(editor, #"<meta name="lexical-source" content="paste"><div>x</div>"#)
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "[paste]")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocess can write to the import session (shared with rules)"
    ))
  func sharesSession() throws {
    let stylesheets = createImportState("test/stylesheets") { [String]() }
    let collect: DOMPreprocessFn = { dom, ctx, next in
      for el in elements(dom, "style") {
        ctx.session.update(stylesheets) { $0 + [el.textContent] }
      }
      try next()
    }
    let articleRule = defineImportRule(name: "test/article", match: sel.tag("article")) {
      ctx, _, _ in
      [
        try createParagraphNode().append(
          createTextNode("[saw \(ctx.session.get(stylesheets).count) sheets]"))
      ]
    }
    let editor = try buildHost([
      configExtension(DOMImportExtension) {
        $0.preprocess = [collect]
        $0.rules = [articleRule.entry]
      }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<style>.a{}</style><style>.b{}</style><article>body</article>")
    try editor.read { () throws in
      #expect(try firstParagraph().getTextContent() == "[saw 2 sheets]")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > middleware chain: a wrapper preprocess can defer to a lower one via $next()"
    ))
  func middlewareChain() throws {
    nonisolated(unsafe) var log: [String] = []
    let inner: DOMPreprocessFn = { _, _, next in
      log.append("inner-before")
      try next()
      log.append("inner-after")
    }
    let wrapper: DOMPreprocessFn = { _, _, next in
      log.append("wrapper-before")
      try next()
      log.append("wrapper-after")
    }
    let editor = try buildHost([
      configExtension(DOMImportExtension) { $0.preprocess = [inner, wrapper] }
    ])
    defer { editor.dispose() }
    try importInto(editor, "<p>x</p>")
    #expect(log == ["wrapper-before", "inner-before", "inner-after", "wrapper-after"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > per-call preprocess runs in addition to config-level ones"
    ))
  func perCallPreprocess() throws {
    nonisolated(unsafe) var log: [String] = []
    let configPre: DOMPreprocessFn = { _, _, next in
      log.append("config")
      try next()
    }
    let perCall: DOMPreprocessFn = { _, _, next in
      log.append("per-call")
      try next()
    }
    let editor = try buildHost([configExtension(DOMImportExtension) { $0.preprocess = [configPre] }]
    )
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let dep = try getExtensionDependencyFromEditor(editor, DOMImportExtension)
      let dom = DOMParser().parseFromString("<!doctype html><html><body><p>x</p></body></html>")
      let nodes = try dep.output.generateNodesFromDOM(
        dom, GenerateNodesFromDOMOptions(preprocess: [perCall]))
      try getRoot().clear().splice(0, 0, nodes)
    }
    #expect(log == ["per-call", "config"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocessors contributed by dependents run before their dependencies"
    ))
  func dependentsFirst() throws {
    nonisolated(unsafe) var log: [String] = []
    func trace(_ name: String) -> DOMPreprocessFn {
      { _, _, next in
        log.append(name)
        try next()
      }
    }
    func makeExtension(
      _ name: String, _ deps: [any AnyLexicalExtensionArgument] = []
    )
      -> any AnyLexicalExtensionArgument
    {
      defineExtension(
        name: "preprocess-\(name)",
        dependencies: deps + [
          configExtension(DOMImportExtension) {
            $0.preprocess = [trace("\(name)-a"), trace("\(name)-b")]
          }
        ])
    }
    let leaf = makeExtension("leaf")
    let mid = makeExtension("mid", [leaf])
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "preprocess-root", dependencies: [CoreImportExtension, mid]))
    defer { editor.dispose() }
    try importInto(editor, "<p>x</p>")
    #expect(log == ["mid-b", "mid-a", "leaf-b", "leaf-a"])
  }
}

private func buildHRHost() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "hr-host", dependencies: [CoreImportExtension],
      nodes: { [.klass(HorizontalRuleNode.self)] }))
}

@Suite struct HorizontalRuleImportExtensionTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> imports as HorizontalRuleNode"
    ))
  func importsHR() throws {
    let editor = try buildHRHost()
    defer { editor.dispose() }
    try importInto(editor, "<hr>")
    try editor.read { () throws in
      #expect(isHorizontalRuleNode(try getRoot().getFirstChild()))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> between paragraphs preserves surrounding structure"
    ))
  func hrBetweenParagraphs() throws {
    let editor = try buildHRHost()
    defer { editor.dispose() }
    try importInto(editor, "<p>before</p><hr><p>after</p>")
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(children[0] is ParagraphNode)
      #expect(isHorizontalRuleNode(children[1]))
      #expect(children[2] is ParagraphNode)
      #expect(try children[0].getTextContent() == "before")
      #expect(try children[2].getTextContent() == "after")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> is dropped when HorizontalRuleNode is not registered"
    ))
  func hrDropped() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "hr-gated-host", dependencies: [CoreImportExtension]))
    defer { editor.dispose() }
    try importInto(editor, "<p>before</p><hr><p>after</p>")
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(children.allSatisfy { $0 is ParagraphNode })
      #expect(try children.map { try $0.getTextContent() } == ["before", "after"])
    }
  }
}
