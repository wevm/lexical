import Lexical

/// `createTestEditor()` with `_headless = true`, as the upstream utils tests
/// make it.
func createHeadlessTestEditor(nodes: [LexicalNodeConfig] = []) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: nodes, onError: { throw $0 }, theme: [:], headless: true))
}

/// `await editor.update(fn)`: the update, then its commit.
func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}

/// `TestElementNode` from packages/lexical/src/__tests__/utils.
final class TestElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestElementNode.self, type: "test_block", extends: ElementNode.self)
  }
}

func createTestElementNode() throws -> TestElementNode { try TestElementNode() }

func createTextAndParagraphWithDepth(_ depth: Int) throws -> [LexicalNode] {
  if depth <= 0 { return [try createTextNode("<\(depth) />")] }
  return [
    try createTextNode("<\(depth)>"),
    try createParagraphNode().append(contentsOf: createTextAndParagraphWithDepth(depth - 1)),
    try createTextNode("</\(depth)>"),
  ]
}

func textContentForDepth(_ i: Int) -> String {
  i > 0 ? "<\(i)>\(textContentForDepth(i - 1))</\(i)>" : "<\(i) />"
}

func fiveTextNodes() throws -> ParagraphNode {
  try createParagraphNode().splice(0, 0, (0..<5).map { try createTextNode("\($0)") })
}

func texts(_ parent: ElementNode) throws -> [String] {
  try parent.getAllTextNodes().map { try $0.getTextContent() }
}

/// `TestShadowRootNode` from packages/lexical/src/__tests__/utils.
final class TestShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestShadowRootNode.self, type: "test_shadow_root", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

func createTestShadowRootNode() throws -> TestShadowRootNode { try TestShadowRootNode() }

/// `TestDecoratorNode` from packages/lexical/src/__tests__/utils: inline
/// unless `setIsInline(false)`.
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
