import Lexical

// The shared helpers of packages/lexical/src/__tests__/utils, which upstream
// tests of every package import: the test editor, the test nodes it
// registers, and the model-shape helpers that stand in for comparing
// reconciled HTML.

/// `createTestEditor` from packages/lexical/src/__tests__/utils: keys start
/// at 1, and errors are rethrown rather than reported.
public func createTestEditor(
  nodes: [LexicalNodeConfig] = [], theme: EditorThemeClasses = [:], headless: Bool = false
) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: defaultTestNodes + nodes, onError: { throw $0 }, theme: theme,
      headless: headless))
}

/// Stands in for the contenteditable `initializeUnitTest` mounts: with it
/// attached, commits run the reconciler.
public final class TestRootElement {
  public init() {}
}

/// `createTestEditor` mounted on a `TestRootElement`, as `initializeUnitTest`
/// mounts its editor.
public func createMountedTestEditor(
  nodes: [LexicalNodeConfig] = [], theme: EditorThemeClasses = [:]
) throws -> Editor {
  let editor = try createTestEditor(nodes: nodes, theme: theme)
  try editor.setRootElement(TestRootElement())
  return editor
}

/// `await editor.update(fn)`: the update, then its commit.
public func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}

/// `TestDecoratorNode` from the upstream test utils: inline unless
/// `setIsInline(false)`.
public final class TestDecoratorNode: DecoratorNode {
  var __block = false
  public override class func config() -> NodeConfig? {
    NodeConfig(TestDecoratorNode.self, type: "test_decorator", extends: DecoratorNode.self)
  }
  public override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __block = (prevNode as! TestDecoratorNode).__block
  }
  public override func isInline() throws -> Bool { try !getLatest().__block }
  @discardableResult
  public func setIsInline(_ inline: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__block = !inline
    return writable
  }
  public override class func importDOM() -> DOMConversionMap? {
    [
      "test-decorator": { _ in
        DOMConversion { _ in DOMConversionOutput(node: try createTestDecoratorNode()) }
      }
    ]
  }
  public override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    DOMExportOutput(element: getDocument().createElement("test-decorator"))
  }
  public override func getTextContent() throws -> String { "Hello world" }
  public override func createDOM(
    _ config: EditorConfig, _ editor: Editor? = nil
  ) throws
    -> DOMElement
  {
    getDocument().createElement(__block ? "div" : "span")
  }
  public override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    __block != (prevNode as! TestDecoratorNode).__block
  }
  public override func decorate(_ editor: Editor) throws -> Any? { "test-decorator" }
}

public func createTestDecoratorNode() throws -> TestDecoratorNode { try TestDecoratorNode() }

/// Stands in for `LinkNode` from @lexical/link with the overrides core
/// editing consults: inline, never empty, no text inserted at its edges,
/// and split by copying itself.
public final class TestLinkNode: ElementNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(TestLinkNode.self, type: "link", extends: ElementNode.self)
  }
  public override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws
    -> LexicalNode?
  {
    let linkNode = try copyNode(self)
    try insertAfter(linkNode, restoreSelection: restoreSelection)
    return linkNode
  }
  public override func canInsertTextBefore() throws -> Bool { false }
  public override func canInsertTextAfter() throws -> Bool { false }
  public override func canBeEmpty() throws -> Bool { false }
  public override func isInline() throws -> Bool { true }
  public override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection else { return false }
    let anchorNode = try selection.anchor.getNode()
    let focusNode = try selection.focus.getNode()
    return try (self.is(anchorNode) || isParentOf(anchorNode))
      && (self.is(focusNode) || isParentOf(focusNode)) && !selection.getTextContent().isEmpty
  }
}

public func createTestLinkNode() throws -> TestLinkNode { try TestLinkNode() }

/// `TestElementNode` from the upstream test utils.
public final class TestElementNode: ElementNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(TestElementNode.self, type: "test_block", extends: ElementNode.self)
  }
}

public func createTestElementNode() throws -> TestElementNode { try TestElementNode() }

/// `TestInlineElementNode` from the upstream test utils.
public final class TestInlineElementNode: ElementNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(TestInlineElementNode.self, type: "test_inline_block", extends: ElementNode.self)
  }
  public override func isInline() throws -> Bool { true }
  public override func canBeEmpty() throws -> Bool { false }
}

public func createTestInlineElementNode() throws -> TestInlineElementNode {
  try TestInlineElementNode()
}

/// `TestShadowRootNode` from the upstream test utils.
public final class TestShadowRootNode: ElementNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(TestShadowRootNode.self, type: "test_shadow_root", extends: ElementNode.self)
  }
  public override func isShadowRoot() -> Bool { true }
}

public func createTestShadowRootNode() throws -> TestShadowRootNode { try TestShadowRootNode() }

/// `TestSegmentedNode` from the upstream test utils.
public final class TestSegmentedNode: TextNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(TestSegmentedNode.self, type: "test_segmented", extends: TextNode.self)
  }
}

public func createTestSegmentedNode(_ text: String = "") throws -> TestSegmentedNode {
  try TestSegmentedNode(text).setMode("segmented")
}

/// The upstream `DEFAULT_NODES` that are ported: the test nodes every test
/// editor registers.
public var defaultTestNodes: [LexicalNodeConfig] {
  [
    .klass(TestElementNode.self), .klass(TestInlineElementNode.self),
    .klass(TestShadowRootNode.self), .klass(TestSegmentedNode.self),
    .klass(TestDecoratorNode.self), .klass(TestLinkNode.self),
  ]
}

/// Stands in for upstream's `prettifyHtml`, which `expectHtmlToBeEqual`
/// compares both sides with. The repo's prettier config ignores whitespace
/// beside tags (`htmlWhitespaceSensitivity: ignore`) and sorts attributes
/// (`prettier-plugin-organize-attributes`: `class`, then `id` and `name`,
/// the rest, `aria-*`, `data-*`, each group by name), so this drops that
/// whitespace, parses the HTML and sorts each element's attributes the same
/// way. Upstream's `html` template literal is a plain string.
public func prettifyHTML(_ html: String) -> String {
  let stripped = html.replacing("\n", with: "")
    .replacing(/>\s+/, with: ">")
    .replacing(/\s+</, with: "<")
  guard let body = DOMParser().parseFromString(stripped).body else { return stripped }
  func group(_ name: String) -> Int {
    if name == "class" { return 0 }
    if name == "id" || name == "name" { return 1 }
    if name.hasPrefix("aria-") { return 3 }
    if name.hasPrefix("data-") { return 4 }
    return 2
  }
  for element in body.descendantElements() {
    let attributes = element.attributes
    let sorted = attributes.sorted {
      (group($0.name), $0.name) < (group($1.name), $1.name)
    }
    for attribute in attributes { element.removeAttribute(attribute.name) }
    for attribute in sorted { element.setAttribute(attribute.name, attribute.value) }
  }
  return body.innerHTML
}

/// The committed document as a string, standing in for the reconciled HTML
/// upstream tests compare: an element is `<type>…</type>`, a text node
/// `[text]`, a line break `<br>`, and any other leaf `<type/>`.
public func shape(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    try getRoot().getChildren().map(shape).joined()
  }
}

public func shape(_ node: LexicalNode) throws -> String {
  if let element = node as? ElementNode {
    let children = try element.getChildren().map(shape).joined()
    return "<\(element.getType())>\(children)</\(element.getType())>"
  }
  if let text = node as? TextNode {
    return "[\(try text.getTextContent())]"
  }
  if node is LineBreakNode {
    return "<br>"
  }
  return "<\(node.getType())/>"
}

/// The committed range selection's points, `key:offset:type` for the anchor
/// and then the focus, after committing pending updates.
public func points(_ editor: Editor) throws -> String {
  try editor.read {
    guard let selection = try getSelection() as? RangeSelection else {
      return "not a RangeSelection"
    }
    return [selection.anchor, selection.focus].map {
      "\($0.key):\($0.offset):\($0.type.rawValue)"
    }.joined(separator: " ")
  }
}

/// `points` expected text: `key:offset:element` for both points.
public func elementPoints(
  _ anchorKey: NodeKey, _ anchorOffset: Int, _ focusKey: NodeKey, _ focusOffset: Int
)
  -> String
{
  "\(anchorKey):\(anchorOffset):element \(focusKey):\(focusOffset):element"
}
