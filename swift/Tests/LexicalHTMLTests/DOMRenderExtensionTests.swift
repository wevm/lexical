import Lexical
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts
// and DOMRenderConditionalOverrides.test.ts. The tests that assert the DOM
// the reconciler renders into a root element are `dom-only` skips: the
// native views replace that reconciler.

nonisolated(unsafe) private let idState: StateConfig<String?> = createState(
  "id", parse: { if case .string(let s) = $0 { s } else { nil } },
  unparse: { $0.map(JSONValue.string) })

private final class TextNodeA: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TextNodeA.self, type: "text-a", extends: TextNode.self)
  }
}

private func buildEditor(
  _ overrides: [DOMRenderMatch], nodes: [LexicalNodeConfig] = [],
  _ initialState: @escaping () throws -> Void
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "root",
      dependencies: [configExtension(DOMRenderExtension) { $0.overrides = overrides }],
      nodes: { nodes },
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() })))
}

private func expectHtmlToBeEqual(
  _ actual: String, _ expected: String, sourceLocation: SourceLocation = #_sourceLocation
) {
  #expect(prettifyHTML(actual) == prettifyHTML(expected), sourceLocation: sourceLocation)
}

@Suite struct DOMRenderExtensionTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > $createDOM overrides apply to HTML export (with root: false)"
    ))
  func createDOMOverridesApplyToExport() throws {
    let editor = try buildEditor([
      domOverride(
        all: DOMRenderMatchConfig(createDOM: { node, next, _ in
          let dom = try next()
          dom.setAttribute("data-node-type", node.getType())
          return dom
        }))
    ]) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("plain"), createLineBreakNode(),
          createTextNode("bold").setFormat("bold")))
    }
    defer { editor.dispose() }
    try editor.read {
      expectHtmlToBeEqual(
        try generateHtmlFromNodes(editor),
        """
        <p data-node-type="paragraph">
          <span style="white-space: pre-wrap;" data-node-type="text">
            plain
          </span>
          <br data-node-type="linebreak" />
          <b>
            <strong style="white-space: pre-wrap;" data-node-type="text">
              bold
            </strong>
          </b>
        </p>
        """)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > can override DOM export"
    ))
  func overrideDOMExport() throws {
    let editor = try buildEditor([
      domOverride(
        all: DOMRenderMatchConfig(exportDOM: { node, next, _ in
          let result = try next()
          if let id = try getState(node, idState), let element = result.element as? DOMElement {
            element.setAttribute("id", id)
          }
          return result
        })),
      domOverride(
        [TextNode.self],
        DOMRenderMatchConfig(exportDOM: { _, next, _ in
          let result = try next()
          if try getRenderContextValue(RenderContextRoot),
            let element = result.element as? DOMElement,
            element.style.getPropertyValue("white-space") == "pre-wrap",
            // we know there aren't tabs or newlines but if there are
            // leading, trailing, or adjacent spaces then we need the
            // pre-wrap to preserve the content
            !element.textContent.contains(/^\s|\s$|\s\s/)
          {
            _ = element.style.removeProperty("white-space")
            if element.style.cssText == "" {
              element.removeAttribute("style")
            }
          }
          return result
        })),
    ]) {
      try setState(getRoot(), idState, "root").append(
        setState(createParagraphNode(), idState, "paragraph").append(
          setState(createTextNode("text!"), idState, "text")))
    }
    defer { editor.dispose() }
    try editor.read {
      expectHtmlToBeEqual(
        try generateDOMFromRoot(getDocument().createElement("div")).innerHTML,
        """
        <div id="root" role="textbox">
          <p id="paragraph">
            <span id="text">text!</span>
          </p>
        </div>
        """)
    }
  }

  // Upstream renders into a root element and reads `data-order` from the
  // reconciled DOM. Swift runs the compiled `createDOM` and `decorateDOM`
  // hooks, the two calls the reconciler makes for a created node.
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > $decorateDOM overrides all run in order (base, specific, wildcard last)"
    ))
  func decorateOrder() throws {
    func append(_ dom: DOMElement, _ s: String) {
      dom.setAttribute("data-order", (dom.getAttribute("data-order") ?? "") + s)
    }
    let editor = try buildEditor(
      [
        domOverride(
          all: DOMRenderMatchConfig(decorateDOM: { _, _, dom, _ in append(dom, "wildcard;") })),
        domOverride(
          [TextNodeA.self],
          DOMRenderMatchConfig(decorateDOM: { _, _, dom, _ in append(dom, "text-a;") })),
        domOverride(
          [TextNode.self],
          DOMRenderMatchConfig(decorateDOM: { _, _, dom, _ in append(dom, "text;") })),
      ], nodes: [.klass(TextNodeA.self)]
    ) {
      try getRoot().append(
        createParagraphNode().append(try create(TextNodeA.self).setTextContent("text a")))
    }
    defer { editor.dispose() }
    try editor.read {
      let dom = getEditorDOMRenderConfig(editor)
      let paragraph = try #require(try getRoot().getFirstChild())
      let text = try #require(try getRoot().getFirstDescendant())
      let p = try dom.createDOM(paragraph, editor)
      try dom.decorateDOM(paragraph, nil, p, editor)
      let span = try dom.createDOM(text, editor)
      try dom.decorateDOM(text, nil, span, editor)
      #expect(p.getAttribute("data-order") == "wildcard;")
      #expect(span.getAttribute("data-order") == "text;text-a;wildcard;")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > type merge"
    ))
  func typeMerge() throws {
    let editor = try buildEditor(
      [
        domOverride(
          [TextNode.self],
          DOMRenderMatchConfig(exportDOM: { node, _, _ in
            let span = getDocument().createElement("span")
            span.append(try node.getTextContent())
            return DOMExportOutput(element: span)
          })),
        domOverride(
          [TextNodeA.self],
          DOMRenderMatchConfig(exportDOM: { node, next, _ in
            // We want to call next to get any export behavior that applies
            // to all TextNode, if we return a new value we will only get
            // export behavior that applies to predicates, wildcards, and
            // TextNodeA
            let r = try next()
            (r.element as? DOMElement)?.setAttribute("data-lexical-type", node.getType())
            return r
          })),
        domOverride(
          [TextNode.self],
          DOMRenderMatchConfig(exportDOM: { _, next, _ in
            let r = try next()
            (r.element as? DOMElement)?.setAttribute("data-did-override", "true")
            return r
          })),
      ], nodes: [.klass(TextNodeA.self)]
    ) {
      try getRoot().append(
        createParagraphNode().append(
          try create(TextNodeA.self).setTextContent("text a"), createLineBreakNode(),
          createTextNode().setTextContent("plain text")))
    }
    defer { editor.dispose() }
    try editor.read {
      expectHtmlToBeEqual(
        try generateDOMFromRoot(getDocument().createElement("div")).innerHTML,
        """
        <div role="textbox">
          <p>
            <span data-did-override="true" data-lexical-type="text-a">
              text a
            </span>
            <br />
            <span data-did-override="true">plain text</span>
          </p>
        </div>
        """)
    }
  }
}

// MARK: - DOMRenderConditionalOverrides.test.ts

private let WrapDisabled = createRenderState("wrapDisabled") { false }
private let Terse = createRenderState("terse") { false }

private let WRAP_ATTR = "data-wrap"

private func makeEditor() throws -> Editor {
  try buildEditor([
    // Structural override gated per-editor: wraps each <br> in a <span>.
    domOverride(
      [LineBreakNode.self],
      DOMRenderMatchConfig(
        createDOM: { _, next, _ in
          let inner = try next()
          let wrapper = getDocument().createElement("span")
          wrapper.setAttribute(WRAP_ATTR, "true")
          wrapper.appendChild(inner)
          return wrapper
        },
        updateDOM: { _, _, dom, next, _ in
          if !(dom.tagName == "SPAN" && dom.hasAttribute(WRAP_ATTR)) {
            return true
          }
          return try next()
        }),
      DOMOverrideOptions(disabledForEditor: { $0.get(WrapDisabled) })),
    // Export-only override gated per-session: tags every element.
    domOverride(
      all: DOMRenderMatchConfig(exportDOM: { _, next, _ in
        let rval = try next()
        (rval.element as? DOMElement)?.setAttribute("data-terse", "true")
        return rval
      }),
      DOMOverrideOptions(disabledForSession: { !$0.get(Terse) })),
  ]) {
    try getRoot().append(
      createParagraphNode().append(
        createTextNode("a"), createLineBreakNode(), createTextNode("b")))
  }
}

private func getLineBreakKey(_ editor: Editor) throws -> NodeKey {
  try editor.read {
    let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
    return try #require(try paragraph.getChildren().first { $0 is LineBreakNode }).getKey()
  }
}

@Suite struct DOMRenderConditionalOverridesTests {
  // Upstream mounts a root element first; the node identity it checks is
  // in the editor state.
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderConditionalOverrides.test.ts::DOMRender conditional overrides > disabledForEditor re-render reuses node instances (no node map clone)"
    ))
  func reusesNodeInstances() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    let lbKey = try getLineBreakKey(editor)
    let before = try editor.read { try getNodeByKeyOrThrow(lbKey) }
    try setRenderContextValue(WrapDisabled, true, editor)
    // A full reconcile reuses the same node instance: marking a read-only
    // node writable always returns a new instance, so identity equality
    // proves the node was never cloned.
    try editor.read { () throws in
      #expect(try getNodeByKeyOrThrow(lbKey) === before)
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/DOMRenderConditionalOverrides.test.ts::DOMRender conditional overrides > disabledForSession gates export participation"
    ))
  func sessionGatesExport() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    // Cold export: the per-session override is not installed.
    try editor.read { () throws in
      #expect(!(try generateHtmlFromNodes(editor)).contains("data-terse"))
    }
    // Terse export: the per-session override is installed for this walk.
    try editor.read {
      let html = try withRenderContext([contextValue(Terse, true)], editor) {
        try generateHtmlFromNodes(editor)
      }
      #expect(html.contains("data-terse"))
    }
  }
}
