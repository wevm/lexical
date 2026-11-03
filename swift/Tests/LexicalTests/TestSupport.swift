import Lexical
import Testing

/// `createTestEditor` from packages/lexical/src/__tests__/utils: keys start
/// at 1, and errors are rethrown rather than reported.
func createTestEditor(nodes: [LexicalNodeConfig] = []) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: defaultTestNodes + nodes, onError: { throw $0 }, theme: [:]))
}

/// `await editor.update(fn)`: the update, then its commit.
func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}

/// Stands in for constructing a bare `LexicalNode`, which upstream tests do
/// with `getType` mocked to return `'node'`.
final class PlainNode: LexicalNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainNode.self, type: "node", extends: LexicalNode.self)
  }
}

/// `TestNode` from LexicalNode.test.ts.
class TestNode: LexicalNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestNode.self, type: "test", extends: LexicalNode.self)
  }
}

/// `InlineDecoratorNode` from LexicalNode.test.ts.
final class InlineDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineDecoratorNode.self, type: "inline-decorator", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func isParentRequired() -> Bool { true }
  override func decorate(_ editor: Editor) throws -> Any? { "inline-decorator" }
}

/// `BlockDecoratorNode` from LexicalNode.test.ts.
final class BlockDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BlockDecoratorNode.self, type: "block-decorator", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { "block-decorator" }
}

/// `TestDecoratorNode` from the upstream test utils: inline unless
/// `setIsInline(false)`.
final class TestDecoratorNode: DecoratorNode {
  var __block = false
  override class func config() -> NodeConfig? {
    NodeConfig(TestDecoratorNode.self, type: "test_decorator", extends: DecoratorNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __block = (prevNode as! TestDecoratorNode).__block
  }
  override func isInline() throws -> Bool { try !getLatest().__block }
  @discardableResult
  func setIsInline(_ inline: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__block = !inline
    return writable
  }
  override func getTextContent() throws -> String { "Hello world" }
  override func decorate(_ editor: Editor) throws -> Any? { "test-decorator" }
}

func createTestDecoratorNode() throws -> TestDecoratorNode { try TestDecoratorNode() }

/// Stands in for `LinkNode` from @lexical/link with the overrides core
/// editing consults: inline, never empty, no text inserted at its edges,
/// and split by copying itself.
final class TestLinkNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestLinkNode.self, type: "link", extends: ElementNode.self)
  }
  override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws
    -> LexicalNode?
  {
    let linkNode = try copyNode(self)
    try insertAfter(linkNode, restoreSelection: restoreSelection)
    return linkNode
  }
  override func canInsertTextBefore() throws -> Bool { false }
  override func canInsertTextAfter() throws -> Bool { false }
  override func canBeEmpty() throws -> Bool { false }
  override func isInline() throws -> Bool { true }
  override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection else { return false }
    let anchorNode = try selection.anchor.getNode()
    let focusNode = try selection.focus.getNode()
    return try (self.is(anchorNode) || isParentOf(anchorNode))
      && (self.is(focusNode) || isParentOf(focusNode)) && !selection.getTextContent().isEmpty
  }
}

func createTestLinkNode() throws -> TestLinkNode { try TestLinkNode() }

/// `TestElementNode` from the upstream test utils.
final class TestElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestElementNode.self, type: "test_block", extends: ElementNode.self)
  }
}

func createTestElementNode() throws -> TestElementNode { try TestElementNode() }

/// `TestInlineElementNode` from the upstream test utils.
final class TestInlineElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestInlineElementNode.self, type: "test_inline_block", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func canBeEmpty() throws -> Bool { false }
}

func createTestInlineElementNode() throws -> TestInlineElementNode {
  try TestInlineElementNode()
}

/// `TestShadowRootNode` from the upstream test utils.
final class TestShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestShadowRootNode.self, type: "test_shadow_root", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

func createTestShadowRootNode() throws -> TestShadowRootNode { try TestShadowRootNode() }

/// `TestSegmentedNode` from the upstream test utils.
final class TestSegmentedNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestSegmentedNode.self, type: "test_segmented", extends: TextNode.self)
  }
}

func createTestSegmentedNode(_ text: String = "") throws -> TestSegmentedNode {
  try TestSegmentedNode(text).setMode("segmented")
}

/// The upstream `DEFAULT_NODES` that are ported: the test nodes every test
/// editor registers.
var defaultTestNodes: [LexicalNodeConfig] {
  [
    .klass(TestElementNode.self), .klass(TestInlineElementNode.self),
    .klass(TestShadowRootNode.self), .klass(TestSegmentedNode.self),
    .klass(TestDecoratorNode.self), .klass(TestLinkNode.self),
  ]
}

/// The extra nodes of the `LexicalNode tests` suite.
var lexicalNodeTestNodes: [LexicalNodeConfig] {
  [
    .klass(PlainNode.self), .klass(TestNode.self), .klass(InlineDecoratorNode.self),
    .klass(BlockDecoratorNode.self),
  ]
}

/// The committed document as a string, standing in for the reconciled HTML
/// upstream tests compare: an element is `<type>…</type>`, a text node
/// `[text]`, a line break `<br>`, and any other leaf `<type/>`.
func shape(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    try getRoot().getChildren().map(shape).joined()
  }
}

func shape(_ node: LexicalNode) throws -> String {
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
func points(_ editor: Editor) throws -> String {
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
func elementPoints(
  _ anchorKey: NodeKey, _ anchorOffset: Int, _ focusKey: NodeKey, _ focusOffset: Int
)
  -> String
{
  "\(anchorKey):\(anchorOffset):element \(focusKey):\(focusOffset):element"
}
