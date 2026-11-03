import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-html/src/__tests__/unit/DOMImportExtension.test.ts.
// `@lexical/link` is not ported, so `URLLinkNode` stands in for `LinkNode`:
// an inline element with a URL, which is all the rules here use. Upstream's
// AnchorRule reads `el.href`, which jsdom resolves (`https://example.com/`);
// the Swift DOM has no URL resolution, so the rule reads the attribute.

private final class URLLinkNode: ElementNode {
  var __url = ""
  override class func config() -> NodeConfig? {
    NodeConfig(URLLinkNode.self, type: "link", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __url = (prevNode as! URLLinkNode).__url
  }
  override func isInline() throws -> Bool { true }
  func getURL() throws -> String { try getLatest().__url }
}

private func createURLLinkNode(_ url: String) throws -> URLLinkNode {
  let link = try create(URLLinkNode.self)
  link.__url = url
  return link
}

nonisolated(unsafe) private let idState: StateConfig<String?> = createState(
  "id", parse: { if case .string(let s) = $0 { s } else { nil } },
  unparse: { $0.map(JSONValue.string) })

nonisolated(unsafe) private let sourceState: StateConfig<String?> = createState(
  "source", parse: { if case .string(let s) = $0 { s } else { nil } },
  unparse: { $0.map(JSONValue.string) })

private func paragraph(_ text: String) throws -> ParagraphNode {
  try createParagraphNode().append(createTextNode(text))
}

nonisolated(unsafe) private let IdAttributeRule = defineImportRule(
  name: "test/id-decorator", match: sel.any().attr("id", /\S/)
) { _, el, next in
  let out = try next()
  if out.count == 1, let id = el.getAttribute("id"), !id.isEmpty {
    try setState(out[0], idState, id)
  }
  return out
}

nonisolated(unsafe) private let AnchorRule = defineImportRule(
  name: "test/anchor", match: sel.tag("a")
) { ctx, el, next in
  guard let href = el.getAttribute("href"), !href.isEmpty else { return try next() }
  let link = try createURLLinkNode(href)
  try link.splice(0, 0, ctx.importChildren(el, ImportChildrenOpts(schema: InlineSchema)))
  return [link]
}

nonisolated(unsafe) private let ParagraphRule = defineImportRule(
  name: "test/paragraph", match: sel.tag("p")
) { ctx, el, _ in
  [
    try createParagraphNode().splice(
      0, 0, ctx.importChildren(el, ImportChildrenOpts(schema: InlineSchema)))
  ]
}

nonisolated(unsafe) private let TextRule = defineImportRule(
  name: "test/text", match: sel.text()
) { ctx, el, _ in
  if el.data.isEmpty { return [] }
  let text = try createTextNode(el.data)
  let format = try ctx.get(ImportTextFormat)
  if format != 0 { try text.setFormat(format) }
  return [text]
}

nonisolated(unsafe) private let BoldRule = defineImportRule(
  name: "test/bold", match: sel.tag("b", "strong")
) { ctx, el, _ in
  try ctx.importChildren(
    el,
    ImportChildrenOpts(context: [
      contextValue(ImportTextFormat, ctx.get(ImportTextFormat) | IS_BOLD)
    ]))
}

nonisolated(unsafe) private let ItalicRule = defineImportRule(
  name: "test/italic", match: sel.tag("em", "i")
) { ctx, el, _ in
  try ctx.importChildren(
    el,
    ImportChildrenOpts(context: [
      contextValue(ImportTextFormat, ctx.get(ImportTextFormat) | IS_ITALIC)
    ]))
}

// Sibling-emitting: <figure> emits [image-paragraph, caption-paragraph].
nonisolated(unsafe) private let FigureRule = defineImportRule(
  name: "test/figure", match: sel.tag("figure")
) { ctx, el, _ in
  var out: [LexicalNode] = []
  if let img = el.children.first(where: { $0.localName == "img" }) {
    out.append(try paragraph("[img:\(img.getAttribute("alt") ?? "")]"))
  }
  if let cap = el.children.first(where: { $0.localName == "figcaption" }) {
    out.append(
      try createParagraphNode().splice(
        0, 0, ctx.importChildren(cap, ImportChildrenOpts(schema: InlineSchema))))
  }
  return out
}

// Handles <pre> only when a child <code> has a language- class.
nonisolated(unsafe) private let CodeRule = defineImportRule(
  name: "test/code", match: sel.tag("pre")
) { _, el, next in
  guard let code = el.children.first(where: { $0.localName == "code" }),
    code.className.contains("language-")
  else { return try next() }
  let lang = code.className.firstMatch(of: /(?:^|\s)language-(\S+)/).map { String($0.1) } ?? "?"
  return [try paragraph("[code:\(lang)]")]
}

nonisolated(unsafe) private let CodeCaptureRule = defineImportRule(
  name: "test/code-capture", match: sel.tag("pre").attr("data-language", /^(.+)$/, capture: "lang")
) { ctx, _, _ in
  [try paragraph("[capture:\(ctx.captures["lang"]?[1] ?? "")]")]
}

nonisolated(unsafe) private let SourceAwareRule = defineImportRule(
  name: "test/source-aware", match: sel.tag("div")
) { ctx, el, _ in
  let p = try createParagraphNode().splice(
    0, 0, ctx.importChildren(el, ImportChildrenOpts(schema: InlineSchema)))
  try setState(p, sourceState, ctx.get(ImportSource).rawValue)
  return [p]
}

private func buildTestEditor(_ extraRules: [DOMImportRule]) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test-host",
      dependencies: [configExtension(DOMImportExtension) { $0.rules = extraRules.map(\.entry) }],
      nodes: { [.klass(URLLinkNode.self)] }))
}

private func generate(
  _ html: String, _ options: GenerateNodesFromDOMOptions? = nil
) throws
  -> [LexicalNode]
{
  let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom, options)
}

private func importInto(
  _ editor: Editor, _ html: String, _ options: GenerateNodesFromDOMOptions? = nil
) throws {
  try editor.update(discrete: true) {
    let nodes = try generate(html, options)
    try getRoot().clear().splice(0, 0, nodes)
  }
}

private func rootParagraphs() throws -> [ParagraphNode] {
  try getRoot().getChildren().compactMap { $0 as? ParagraphNode }
}

private func rootTexts(_ editor: Editor) throws -> [String] {
  try editor.read { try rootParagraphs().map { try $0.getTextContent() } }
}

@Suite struct DOMImportExtensionTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > basic anchor import + id decorator"
    ))
  func anchorAndId() throws {
    let editor = try buildTestEditor([IdAttributeRule, AnchorRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, #"<p><a id="x" href="https://example.com">link</a></p>"#)
    try editor.read { () throws in
      let para = try #require(try rootParagraphs().first)
      let link = try #require(try para.getFirstChild() as? URLLinkNode)
      #expect(try getState(link, idState) == "x")
      #expect(try link.getURL() == "https://example.com")
      #expect(try (link.getFirstChild() as? TextNode)?.getTextContent() == "link")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > text format propagation via ImportTextFormat context"
    ))
  func formatPropagation() throws {
    let editor = try buildTestEditor([BoldRule, ItalicRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, "<p>plain <b>bold <i>italicbold</i></b></p>")
    try editor.read { () throws in
      #expect(try getRoot().getAllTextNodes().count == 3)
      #expect(try findText("plain ").getFormat() == 0)
      #expect(try findText("bold ").hasFormat("bold"))
      #expect(try !findText("bold ").hasFormat("italic"))
      #expect(try findText("italicbold").hasFormat("bold"))
      #expect(try findText("italicbold").hasFormat("italic"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > RootSchema wraps stray inline runs in paragraphs"
    ))
  func rootSchema() throws {
    let editor = try buildTestEditor([ParagraphRule, TextRule, BoldRule])
    defer { editor.dispose() }
    try importInto(editor, "orphan <b>inlines</b> at root")
    try editor.read { () throws in
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try rootParagraphs().first?.getTextContent() == "orphan inlines at root")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > sibling-emitting rule (<figure> -> two paragraphs)"
    ))
  func siblingEmitting() throws {
    let editor = try buildTestEditor([FigureRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(
      editor, #"<figure><img src="x" alt="cat"/><figcaption>A cat.</figcaption></figure>"#)
    #expect(try rootTexts(editor) == ["[img:cat]", "A cat."])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > $next() deferral: code rule defers to next on non-language code"
    ))
  func nextDeferral() throws {
    let editor = try buildTestEditor([CodeRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(
      editor, #"<pre><code class="language-js">x</code></pre><pre><code>y</code></pre>"#)
    #expect(try rootTexts(editor) == ["[code:js]", "y"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > regex capture is exposed on ctx.captures without re-running"
    ))
  func regexCapture() throws {
    let editor = try buildTestEditor([CodeCaptureRule])
    defer { editor.dispose() }
    try importInto(editor, #"<pre data-language="rust">irrelevant</pre>"#)
    #expect(try rootTexts(editor) == ["[capture:rust]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > per-call context: ImportSource flows to rule body"
    ))
  func perCallContext() throws {
    let editor = try buildTestEditor([SourceAwareRule, TextRule])
    defer { editor.dispose() }
    for _ in 0..<2 {
      try importInto(
        editor, "<div>x</div>",
        GenerateNodesFromDOMOptions(context: [contextValue(ImportSource, .paste)]))
      try editor.read { () throws in
        let p = try #require(try rootParagraphs().first)
        #expect(try getState(p, sourceState) == "paste")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > per-call context default is \"unknown\""
    ))
  func perCallDefault() throws {
    let editor = try buildTestEditor([SourceAwareRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, "<div>x</div>")
    try editor.read { () throws in
      let p = try #require(try rootParagraphs().first)
      #expect(try getState(p, sourceState) == "unknown")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > rule priority: the earlier entry in `rules` runs first; can call $next()"
    ))
  func rulePriority() throws {
    let editor = try buildTestEditor([IdAttributeRule, AnchorRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, #"<p><a id="link-x" href="/y">click</a></p>"#)
    try editor.read { () throws in
      let link = try #require(try rootParagraphs().first?.getFirstChild() as? URLLinkNode)
      #expect(try getState(link, idState) == "link-x")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > rules contributed by dependents outrank their dependencies"
    ))
  func dependentsOutrank() throws {
    nonisolated(unsafe) var visited: [String] = []
    func traceRule(_ name: String) -> DOMImportRule {
      defineImportRule(name: "test/trace-\(name)", match: sel.tag("p")) { _, _, next in
        visited.append(name)
        return try next()
      }
    }
    func makeExtension(
      _ name: String, _ dependencies: [any AnyLexicalExtensionArgument] = []
    )
      -> any AnyLexicalExtensionArgument
    {
      defineExtension(
        name: "test-\(name)",
        dependencies: dependencies + [
          configExtension(DOMImportExtension) {
            $0.rules = [traceRule("\(name)-a").entry, traceRule("\(name)-b").entry]
          }
        ])
    }
    let leaf = makeExtension("leaf")
    let mid = makeExtension("mid", [leaf])
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "test-root", dependencies: [mid]),
      configExtension(DOMImportExtension) { $0.rules = [traceRule("builder").entry] })
    defer { editor.dispose() }
    try importInto(editor, "<p>x</p>")
    #expect(visited == ["builder", "mid-a", "mid-b", "leaf-a", "leaf-b"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > among sibling dependencies, the later-listed one outranks"
    ))
  func siblingOrder() throws {
    nonisolated(unsafe) var visited: [String] = []
    func makeExtension(_ name: String) -> any AnyLexicalExtensionArgument {
      let rule = defineImportRule(name: "test/sibling-\(name)", match: sel.tag("p")) {
        _, _, next in
        visited.append(name)
        return try next()
      }
      return defineExtension(
        name: "test-sibling-\(name)",
        dependencies: [configExtension(DOMImportExtension) { $0.rules = [rule.entry] }])
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test-sibling-root", dependencies: [makeExtension("first"), makeExtension("second")]))
    defer { editor.dispose() }
    try importInto(editor, "<p>x</p>")
    #expect(visited == ["second", "first"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser: parseSelector(\"p.foo\") matches as expected"
    ))
  func parseSelectorClass() throws {
    let cssRule = defineImportRule(name: "test/css-pfoo", match: try parseSelector("p.foo")) {
      _, _, _ in [try paragraph("[matched]")]
    }
    let editor = try buildTestEditor([cssRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, #"<p class="foo">x</p><p>y</p>"#)
    #expect(try rootTexts(editor) == ["[matched]", "y"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser via sel.css() chains with combinators"
    ))
  func cssChains() throws {
    let chained = defineImportRule(
      name: "test/css-combinator",
      match: try sel.css("span.highlight").attr("data-lang", /^(.+)$/, capture: "lang")
    ) { ctx, _, _ in [try paragraph("[combo:\(ctx.captures["lang"]?[1] ?? "")]")] }
    let editor = try buildTestEditor([chained])
    defer { editor.dispose() }
    try importInto(editor, #"<span class="highlight" data-lang="go">x</span>"#)
    #expect(try rootTexts(editor) == ["[combo:go]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS selector lists dispatch tag-restricted and unrestricted groups"
    ))
  func selectorLists() throws {
    func importedTags(_ selector: String) throws -> [String] {
      let rule = defineImportRule(name: "test/css-selector-list", match: try sel.css(selector)) {
        _, el, _ in [try paragraph(el.nodeName.lowercased())]
      }
      let editor = try buildTestEditor([rule])
      defer { editor.dispose() }
      try importInto(editor, #"<p></p><div class="foo"></div><article></article>"#)
      return try rootTexts(editor)
    }
    #expect(try importedTags("p, .foo") == ["p", "div"])
    #expect(try importedTags(".foo, p") == ["p", "div"])
    #expect(try importedTags("p, .foo, article") == ["p", "div", "article"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser rejects an empty selector or a hole in a selector list"
    ))
  func parserRejects() throws {
    for selector in ["", "   ", "h1,", ",h1", "h1,,h2", "h1, ,h2"] {
      #expect("\(selector)") {
        _ = try parseSelector(selector)
      } throws: {
        "\($0)".contains("expected a selector")
      }
    }
    _ = try parseSelector("*")
    _ = try parseSelector("h1, *")
    _ = try parseSelector("*.foo")
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > isElementOfTag narrows correctly without instanceof"
    ))
  func elementOfTag() throws {
    let body = try #require(
      DOMParser().parseFromString(
        #"<!doctype html><html><body><a href="x"></a><p></p></body></html>"#
      )
      .body)
    let anchor = try #require(body.children.first)
    let para = try #require(body.children.last)
    #expect(isElementOfTag(anchor, "a"))
    #expect(!isElementOfTag(anchor, "p"))
    #expect(isElementOfTag(para, "p"))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > compileImportRules: unknown tags hit wildcard bucket"
    ))
  func wildcardBucket() throws {
    let editor = try buildTestEditor([IdAttributeRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, #"<my-thing id="custom">z</my-thing>"#)
    try editor.read { () throws in
      let p = try #require(try rootParagraphs().first)
      #expect(try p.getTextContent() == "z")
      let text = try #require(try p.getFirstChild() as? TextNode)
      #expect(try getState(text, idState) == "custom")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::BlockSchema / InlineSchema > BlockSchema wraps inline runs in paragraphs"
    ))
  func blockSchema() throws {
    let blockRule = defineImportRule(name: "test/section", match: sel.tag("section")) {
      ctx, el, _ in
      [
        try paragraph(
          "[section:\(try ctx.importChildren(el, ImportChildrenOpts(schema: BlockSchema)).count)]")
      ]
    }
    let editor = try buildTestEditor([blockRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try importInto(editor, "<section>inline<p>block</p>more inline</section>")
    #expect(try rootTexts(editor) == ["[section:3]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::regression sanity for the existing $generateNodesFromDOM > importer state does not leak between $generateNodesFromDOM calls"
    ))
  func stateDoesNotLeak() throws {
    let editor = try buildTestEditor([BoldRule, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let a = try generate("<p><b>x</b></p>")
      let b = try generate("<p>y</p>")
      try getRoot().clear().splice(0, 0, a + b)
    }
    try editor.read { () throws in
      let ps = try rootParagraphs()
      try #require(ps.count == 2)
      #expect(try (ps[0].getFirstChild() as? TextNode)?.hasFormat("bold") == true)
      #expect(try (ps[1].getFirstChild() as? TextNode)?.hasFormat("bold") == false)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::$importChildren `rules` overlay > overlay rules take precedence over main rules inside the subtree, and are absent outside"
    ))
  func overlayPrecedence() throws {
    nonisolated(unsafe) var overlayHits = 0
    let overlay = defineOverlayRules([
      defineImportRule(name: "test/overlay-x", match: sel.tag("x")) { _, _, _ in
        overlayHits += 1
        return [try paragraph("[overlay-x]")]
      }
    ])
    let wrapRule = defineImportRule(name: "test/wrap", match: sel.tag("wrap")) { ctx, el, _ in
      try ctx.importChildren(el, ImportChildrenOpts(rules: overlay))
    }
    let plainX = defineImportRule(name: "test/main-x", match: sel.tag("x")) { _, _, _ in
      [try paragraph("[main-x]")]
    }
    let editor = try buildTestEditor([wrapRule, plainX])
    defer { editor.dispose() }
    try importInto(editor, "<wrap><x></x></wrap><x></x>")
    #expect(try rootTexts(editor) == ["[overlay-x]", "[main-x]"])
    #expect(overlayHits == 1)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::$importChildren `rules` overlay > overlay rule `$next()` falls through to the main dispatcher"
    ))
  func overlayFallsThrough() throws {
    let plainX = defineImportRule(name: "test/main-x", match: sel.tag("x")) { _, _, _ in
      [try paragraph("[main-x]")]
    }
    let overlay = defineOverlayRules([
      defineImportRule(name: "test/overlay-x-deferred", match: sel.tag("x")) { _, _, next in
        try next()
      }
    ])
    let wrapRule = defineImportRule(name: "test/wrap", match: sel.tag("wrap")) { ctx, el, _ in
      try ctx.importChildren(el, ImportChildrenOpts(rules: overlay))
    }
    let editor = try buildTestEditor([wrapRule, plainX])
    defer { editor.dispose() }
    try importInto(editor, "<wrap><x></x></wrap>")
    #expect(try rootTexts(editor) == ["[main-x]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::overlay composition via defineOverlayRules > inlines nested overlays in priority order (earlier entry wins)"
    ))
  func nestedOverlays() throws {
    let overlayA = defineOverlayRules([
      defineImportRule(name: "test/A-x", match: sel.tag("x")) { _, _, _ in [try paragraph("[A]")] }
    ])
    let overlayB = defineOverlayRules([
      defineImportRule(name: "test/B-x", match: sel.tag("x")) { _, _, _ in [try paragraph("[B-x]")]
      },
      defineImportRule(name: "test/B-y", match: sel.tag("y")) { _, _, _ in [try paragraph("[B-y]")]
      },
    ])
    let merged = defineOverlayRules([.overlay(overlayA), .overlay(overlayB)])
    let wrapRule = defineImportRule(name: "test/wrap-merged", match: sel.tag("wrap")) {
      ctx, el, _ in try ctx.importChildren(el, ImportChildrenOpts(rules: merged))
    }
    let editor = try buildTestEditor([wrapRule])
    defer { editor.dispose() }
    try importInto(editor, "<wrap><x>x</x><y>y</y></wrap>")
    #expect(try rootTexts(editor) == ["[A]", "[B-y]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::overlay composition via defineOverlayRules > lower-priority overlay rule fires when the higher one calls $next()"
    ))
  func lowerOverlayFires() throws {
    let overlayA = defineOverlayRules([
      defineImportRule(name: "test/A-defers", match: sel.tag("x")) { _, _, next in try next() }
    ])
    let overlayB = defineOverlayRules([
      defineImportRule(name: "test/B-fallback", match: sel.tag("x")) { _, _, _ in
        [try paragraph("[B-fallback]")]
      }
    ])
    let merged = defineOverlayRules([.overlay(overlayA), .overlay(overlayB)])
    let wrapRule = defineImportRule(name: "test/wrap-defers", match: sel.tag("wrap")) {
      ctx, el, _ in try ctx.importChildren(el, ImportChildrenOpts(rules: merged))
    }
    let editor = try buildTestEditor([wrapRule])
    defer { editor.dispose() }
    try importInto(editor, "<wrap><x>x</x></wrap>")
    #expect(try rootTexts(editor) == ["[B-fallback]"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > $getImportContextValue reads default outside an active import"
    ))
  func contextDefault() throws {
    let editor = try buildTestEditor([])
    defer { editor.dispose() }
    try editor.read { () throws in
      #expect(try getImportContextValue(ImportSource) == .unknown)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > createImportState creates a fresh state with its own default"
    ))
  func freshState() throws {
    let myState = createImportState("test/my-state") { 42 }
    let editor = try buildTestEditor([])
    defer { editor.dispose() }
    try editor.read { () throws in
      #expect(try getImportContextValue(myState) == 42)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > an import session chains to the ambient import context"
    ))
  func ambientContext() throws {
    let myState = createImportState("test/ambient") { "default" }
    let probe = defineImportRule(name: "test/ambient-probe", match: sel.tag("cite")) {
      ctx, _, _ in [try createTextNode("saw:\(try ctx.get(myState))")]
    }
    let editor = try buildTestEditor([probe, ParagraphRule, TextRule])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      func textOf(_ nodes: [LexicalNode]) throws -> String {
        try nodes.map { try $0.getTextContent() }.joined()
      }
      #expect(try textOf(generate("<p><cite></cite></p>")) == "saw:default")
      try withImportContext([contextValue(myState, "outer")]) { () throws in
        #expect(try textOf(generate("<p><cite></cite></p>")) == "saw:outer")
        #expect(
          try textOf(
            generate(
              "<p><cite></cite></p>",
              GenerateNodesFromDOMOptions(context: [contextValue(myState, "per-call")])))
            == "saw:per-call")
      }
      #expect(try getImportContextValue(myState) == "default")
    }
  }
}
