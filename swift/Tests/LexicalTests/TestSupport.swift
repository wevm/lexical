import Lexical
import LexicalTesting
import Testing

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

/// The extra nodes of the `LexicalNode tests` suite.
var lexicalNodeTestNodes: [LexicalNodeConfig] {
  [
    .klass(PlainNode.self), .klass(TestNode.self), .klass(InlineDecoratorNode.self),
    .klass(BlockDecoratorNode.self),
  ]
}
