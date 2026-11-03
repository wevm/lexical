import Lexical
import LexicalRichText
import LexicalTesting
import Testing

// The two model tests of packages/lexical/src/__tests__/unit/Issue8922Repro.test.ts.
// The rest pin the boundary anchors the DOM reconciler renders around a
// boundary block decorator, and the DOM selection over them; the views
// replace the DOM reconciler, so those are skipped as dom-only.

private final class TestBlockDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TestBlockDecoratorNode.self, type: "test_block_decorator", extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let dom = getDocument().createElement("div")
    dom.setAttribute("data-test-decorator", "true")
    return dom
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private func setUp() throws -> (Editor, () -> Void) {
  let editor = try createMountedTestEditor(nodes: [.klass(TestBlockDecoratorNode.self)])
  let unregister = registerRichText(editor)
  // decorator, text, decorator
  try editor.update(discrete: true) {
    _ = try getRoot().clear().append(
      TestBlockDecoratorNode(), createParagraphNode().append(createTextNode("hello")),
      TestBlockDecoratorNode())
  }
  return (editor, unregister)
}

/// The selection still spans every top-level node.
private func expectSelectionSpansRoot(_ editor: Editor) throws {
  try editor.read { () throws in
    let selection = try #require(try getSelection() as? RangeSelection)
    let root = try getRoot()
    let topLevel = try selection.getNodes().filter { try $0.getParent()?.is(root) == true }
    #expect(topLevel.map { $0.getKey() } == (try root.getChildren()).map { $0.getKey() })
  }
}

@Suite struct Issue8922ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8922Repro.test.ts::Issue #8922: select-all with a boundary block decorator > anchors do not shift the child offsets $selectAll resolves to"
    ))
  func selectAllOffsets() throws {
    let (editor, unregister) = try setUp()
    defer {
      unregister()
      editor.dispose()
    }
    try editor.update(discrete: true) {
      let selection = try selectAll()
      #expect(selection.anchor.key == "root")
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.offset == 0)
      #expect(selection.focus.key == "root")
      #expect(selection.focus.type == .element)
      #expect(selection.focus.offset == 3)
    }
    try expectSelectionSpansRoot(editor)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8922Repro.test.ts::Issue #8922: select-all with a boundary block decorator > a DOM selection landing outside the anchors still resolves to the full range"
    ))
  func outsideAnchorsResolvesFullRange() throws {
    let (editor, unregister) = try setUp()
    defer {
      unregister()
      editor.dispose()
    }
    // The boundary positions on the far side of each anchor.
    try editor.update(discrete: true) {
      let selection = try selectAll()
      try selection.anchor.set("root", 0, .element)
      try selection.focus.set("root", 3, .element)
    }
    try expectSelectionSpansRoot(editor)
  }
}
