import Lexical

/// `TestInlineElementNode` from packages/lexical/src/__tests__/utils.
final class TestInlineElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestInlineElementNode.self, type: "test_inline_block", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func canBeEmpty() throws -> Bool { false }
}

/// `await editor.update(fn)`: the update, then its commit.
func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}
