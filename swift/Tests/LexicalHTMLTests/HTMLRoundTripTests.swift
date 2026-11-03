import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalTesting
import Testing

// Ports of packages/lexical-html/src/__tests__/unit LineBreakRoundTrip,
// Issue8391WhitespaceAroundUnknownInline, ExportAfterFragment,
// LexicalHtmlBackwardCompat and SlotNodeSelectionExport. The
// LineBreakRoundTrip case with a link needs @lexical/link and is skipped.

private func richTextEditor(
  nodes: [LexicalNodeConfig] = [], _ initial: InitialEditorStateType = .none
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[html-round-trip]", dependencies: [RichTextExtension], nodes: { nodes },
      editorConfig: InitialEditorConfig(initialEditorState: initial)))
}

private func seed(_ editor: Editor, _ children: @escaping () throws -> [LexicalNode]) throws {
  try editor.update(discrete: true) { () throws in
    try getRoot().clear().append(createParagraphNode().append(contentsOf: children()))
  }
}

private func exportHtml(_ editor: Editor) throws -> String {
  try editor.read { try generateHtmlFromNodes(editor, nil) }
}

private func parse(_ html: String) -> DOMDocument {
  DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
}

private func childTypes(_ nodes: [LexicalNode]) throws -> [String] {
  guard let first = nodes.first as? ElementNode else { return [] }
  return try first.getChildren().map { $0.getType() }
}

private let importers: [String] = ["legacy", "extension"]

private func runImporter(_ name: String, _ editor: Editor, _ html: String) throws -> [LexicalNode] {
  name == "legacy"
    ? try generateNodesFromDOM(editor, parse(html))
    : try generateNodesFromDOMViaExtension(parse(html))
}

@Suite struct LineBreakRoundTripTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > a trailing line break exports with a terminating <br>"
    ))
  func trailingExport() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createTextNode("a"), try createLineBreakNode()] }
    #expect(
      try exportHtml(editor)
        == #"<p><span style="white-space: pre-wrap;">a</span><br><br data-lexical-managed-linebreak="true"></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > a lone line break exports with a terminating <br>"
    ))
  func loneExport() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createLineBreakNode()] }
    #expect(try exportHtml(editor) == #"<p><br><br data-lexical-managed-linebreak="true"></p>"#)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > the exported terminator carries the reconciler marker"
    ))
  func terminatorMarker() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createTextNode("a"), try createLineBreakNode()] }
    let brs = try #require(parse(exportHtml(editor)).body).getElementsByTagName("br")
    try #require(brs.count == 2)
    #expect(!brs[0].hasAttribute("data-lexical-managed-linebreak"))
    #expect(brs[1].getAttribute("data-lexical-managed-linebreak") == "true")
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > an interior line break exports unchanged"
    ))
  func interiorExport() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) {
      [try createTextNode("a"), try createLineBreakNode(), try createTextNode("b")]
    }
    #expect(
      try exportHtml(editor)
        == #"<p><span style="white-space: pre-wrap;">a</span><br><span style="white-space: pre-wrap;">b</span></p>"#
    )
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a trailing line break"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a trailing line break"
    ),
    arguments: importers)
  func restoresTrailing(importer: String) throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createTextNode("a"), try createLineBreakNode()] }
    let html = try exportHtml(editor)
    try editor.update(discrete: true) { () throws in
      #expect(try childTypes(runImporter(importer, editor, html)) == ["text", "linebreak"])
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a lone line break"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a lone line break"
    ),
    arguments: importers)
  func restoresLone(importer: String) throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createLineBreakNode()] }
    let html = try exportHtml(editor)
    try editor.update(discrete: true) { () throws in
      #expect(try childTypes(runImporter(importer, editor, html)) == ["linebreak"])
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores consecutive trailing line breaks"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores consecutive trailing line breaks"
    ),
    arguments: importers)
  func restoresConsecutive(importer: String) throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) {
      [try createTextNode("a"), try createLineBreakNode(), try createLineBreakNode()]
    }
    let html = try exportHtml(editor)
    try editor.update(discrete: true) { () throws in
      #expect(
        try childTypes(runImporter(importer, editor, html)) == ["text", "linebreak", "linebreak"])
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a trailing line break with the marker stripped"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a trailing line break with the marker stripped"
    ),
    arguments: importers)
  func restoresStripped(importer: String) throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try seed(editor) { [try createTextNode("a"), try createLineBreakNode()] }
    let html = try exportHtml(editor).replacing(
      #" data-lexical-managed-linebreak="true""#, with: "")
    #expect(!html.contains("data-lexical-managed-linebreak"))
    try editor.update(discrete: true) { () throws in
      #expect(try childTypes(runImporter(importer, editor, html)) == ["text", "linebreak"])
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > still drops an unmarked trailing <br> from external HTML"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > still drops an unmarked trailing <br> from external HTML"
    ),
    arguments: importers)
  func dropsUnmarked(importer: String) throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      #expect(try childTypes(runImporter(importer, editor, "<p>a<br></p>")) == ["text"])
    }
  }
}

// MARK: - Issue8391WhitespaceAroundUnknownInline.test.ts

private let issue8391HTML =
  "<p>In web development, the DOM <tooltip>Document Object Model</tooltip> allows scripts to update content.</p>"
private let TOOLTIP_MARKER = "[TOOLTIP]"

nonisolated(unsafe) private let TooltipRule = defineImportRule(
  name: "test/tooltip", match: sel.tag("tooltip")
) { _, _, _ in [try createTextNode(TOOLTIP_MARKER).setFormat(IS_HIGHLIGHT)] }

private func tooltipInline() -> WhitespaceImportConfig {
  WhitespaceImportConfig(isInline: { defaultIsInline($0) || $0.nodeName == "TOOLTIP" })
}

private func paragraphTexts(_ editor: Editor) throws -> [String] {
  try editor.read { try firstParagraph().getChildren().map { try $0.getTextContent() } }
}

@Suite struct Issue8391Tests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > default config: spaces are trimmed against an unknown <tooltip> (reproduces the bug)"
    ))
  func defaultTrims() throws {
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) { $0.rules = [TooltipRule.entry] }
    ])
    defer { editor.dispose() }
    try importInto(editor, issue8391HTML)
    #expect(
      try paragraphTexts(editor) == [
        "In web development, the DOM", TOOLTIP_MARKER, "allows scripts to update content.",
      ])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > override isInline to recognize <tooltip>: spaces preserved"
    ))
  func overrideKeepsSpaces() throws {
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) {
        $0.contextDefaults = [contextValue(ImportWhitespaceConfig, tooltipInline())]
        $0.rules = [TooltipRule.entry]
      }
    ])
    defer { editor.dispose() }
    try importInto(editor, issue8391HTML)
    #expect(
      try paragraphTexts(editor) == [
        "In web development, the DOM ", TOOLTIP_MARKER, " allows scripts to update content.",
      ])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > per-call override: pass the isInline predicate via the context option"
    ))
  func perCallOverride() throws {
    let editor = try buildImportEditor([
      configExtension(DOMImportExtension) { $0.rules = [TooltipRule.entry] }
    ])
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let dep = try getExtensionDependencyFromEditor(editor, DOMImportExtension)
      let nodes = try dep.output.generateNodesFromDOM(
        parse(issue8391HTML),
        GenerateNodesFromDOMOptions(context: [
          contextValue(ImportWhitespaceConfig, tooltipInline())
        ]))
      try getRoot().clear().splice(0, 0, nodes)
    }
    #expect(
      try paragraphTexts(editor) == [
        "In web development, the DOM ", TOOLTIP_MARKER, " allows scripts to update content.",
      ])
  }
}

// MARK: - ExportAfterFragment.test.ts

/// Exports as `element` and replaces it with a `<section>` in `after`.
private class AfterExportNode: ElementNode {
  class var fragment: Bool { false }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  { false }
  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let doc = getDocument()
    var output = DOMExportOutput(
      element: Self.fragment ? doc.createDocumentFragment() : doc.createElement("div"))
    output.after = { _ in
      let replacement = doc.createElement("section")
      replacement.setAttribute("data-replaced", "true")
      return replacement
    }
    return output
  }
}

private final class FragmentExportNode: AfterExportNode {
  override class var fragment: Bool { true }
  override class func config() -> NodeConfig? {
    NodeConfig(FragmentExportNode.self, type: "fragment-export", extends: ElementNode.self)
  }
}

private final class ElementExportNode: AfterExportNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ElementExportNode.self, type: "element-export", extends: ElementNode.self)
  }
}

@Suite struct ExportAfterFragmentTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/ExportAfterFragment.test.ts::DOMExportOutput.after returning a replacement > applies the replacement when element is a DocumentFragment"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/ExportAfterFragment.test.ts::DOMExportOutput.after returning a replacement > applies the replacement when element is an HTMLElement (control)"
    ),
    arguments: [true, false])
  func afterReplacement(fragment: Bool) throws {
    let editor = try richTextEditor(
      nodes: [.klass(FragmentExportNode.self), .klass(ElementExportNode.self)],
      .update { _ in
        let node: ElementNode =
          fragment ? try create(FragmentExportNode.self) : try create(ElementExportNode.self)
        try getRoot().clear().append(node.append(createTextNode("body")))
      })
    defer { editor.dispose() }
    #expect(try exportHtml(editor).contains(#"data-replaced="true""#))
  }
}

// MARK: - LexicalHtmlBackwardCompat.test.ts

@Suite struct LexicalHtmlBackwardCompatTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtmlBackwardCompat.test.ts::$generateHtmlFromNodes backward compatibility > works inside legacy editor.getEditorState().read(cb) scope (no active editor)"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtmlBackwardCompat.test.ts::$generateHtmlFromNodes backward compatibility > still works inside editor.read() scope (active editor present)"
    ),
    arguments: [true, false])
  func exportsOutsideActiveEditor(legacy: Bool) throws {
    let editor = try createEditor(CreateEditorArgs(onError: { throw $0 }, headless: true))
    try editor.update(discrete: true) { () throws in
      try getRoot().append(createParagraphNode().append(createTextNode("Hello world")))
    }
    let html =
      legacy
      ? try editor.getEditorState().read { try generateHtmlFromNodes(editor) }
      : try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html.contains("Hello world"))
  }
}

// MARK: - SlotNodeSelectionExport.test.ts

private final class PlainShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainShadowRootNode.self, type: "plain_shadow_root_html", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  { false }
  override func isShadowRoot() -> Bool { true }
}

private let TARGET_TEXT = "SlotNodeTarget"

private func slottedParagraphEditor() throws -> (Editor, paragraph: NodeKey, text: NodeKey) {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[slot-node-selection-html]", nodes: { [.klass(PlainShadowRootNode.self)] }))
  nonisolated(unsafe) var keys: (NodeKey, NodeKey) = ("", "")
  try editor.update(discrete: true) { () throws in
    let host = try createParagraphNode()
    let slot = try create(PlainShadowRootNode.self)
    let text = try createTextNode(TARGET_TEXT)
    let innerParagraph = try createParagraphNode().append(text)
    try slot.append(innerParagraph)
    _ = try setSlot(host, "media", slot)
    try getRoot().append(host)
    keys = (innerParagraph.getKey(), text.getKey())
  }
  return (editor, keys.0, keys.1)
}

@Suite struct SlotNodeSelectionExportTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/SlotNodeSelectionExport.test.ts::$generateHtmlFromNodes slot frame redirect > a NodeSelection inside a slot exports the node, not empty HTML"
    ))
  func nodeSelection() throws {
    let (editor, paragraph, _) = try slottedParagraphEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let selection = createNodeSelection()
      selection.add(paragraph)
      try setSelection(selection)
    }
    try editor.read { () throws in
      #expect(try generateHtmlFromNodes(editor, getSelection()).contains(TARGET_TEXT))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/SlotNodeSelectionExport.test.ts::$generateHtmlFromNodes slot frame redirect > a RangeSelection inside a slot still exports the node"
    ))
  func rangeSelection() throws {
    let (editor, _, text) = try slottedParagraphEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let selection = createRangeSelection()
      try selection.anchor.set(text, 0, .text)
      try selection.focus.set(text, TARGET_TEXT.utf16.count, .text)
      try setSelection(selection)
    }
    try editor.read { () throws in
      #expect(try generateHtmlFromNodes(editor, getSelection()).contains(TARGET_TEXT))
    }
  }
}

private func firstTextNode(_ nodes: [LexicalNode]) throws -> TextNode {
  var stack = nodes
  while !stack.isEmpty {
    let node = stack.removeFirst()
    if let text = node as? TextNode { return text }
    if let element = node as? ElementNode {
      stack.insert(contentsOf: try element.getChildren(), at: 0)
    }
  }
  throw LexicalError("expected a TextNode")
}

private let capitalizationFormats: [TextFormatType] = ["lowercase", "uppercase", "capitalize"]

@Suite struct Issue8915ReproTests {
  private func seeded(_ format: TextFormatType) throws -> Editor {
    let editor = try richTextEditor()
    try seed(editor) { [try createTextNode("Hello").toggleFormat(format)] }
    return editor
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for lowercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for uppercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > exportDOM writes text-transform for capitalize"
    ),
    arguments: capitalizationFormats)
  func exportWrites(format: TextFormatType) throws {
    let editor = try seeded(format)
    defer { editor.dispose() }
    #expect(try exportHtml(editor).contains("text-transform: \(format)"))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores lowercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores uppercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOM restores capitalize"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores lowercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores uppercase"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > $generateNodesFromDOMViaExtension restores capitalize"
    ),
    arguments: importers, capitalizationFormats)
  func importRestores(importer: String, format: TextFormatType) throws {
    let editor = try seeded(format)
    defer { editor.dispose() }
    let html = try exportHtml(editor)
    try editor.update(discrete: true) { () throws in
      let text = try firstTextNode(try runImporter(importer, editor, html))
      #expect(try text.hasFormat(format))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > text-transform: none clears an inherited capitalization format"
    ))
  func noneClearsInherited() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let text = try firstTextNode(
        try generateNodesFromDOMViaExtension(
          parse(
            #"<p><span style="text-transform: uppercase"><span style="text-transform: none">Hello</span></span></p>"#
          )))
      #expect(try !text.hasFormat("uppercase"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/Issue8915Repro.test.ts::Issue #8915: text-transform survives an HTML round trip > an unrelated text-transform: none does not clear other formats"
    ))
  func unrelatedNone() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let text = try firstTextNode(
        try generateNodesFromDOM(
          editor,
          parse(#"<p><span style="font-weight: bold; text-transform: none;">Hello</span></p>"#)))
      #expect(try text.hasFormat("bold"))
      #expect(try !text.hasFormat("uppercase"))
    }
  }
}
