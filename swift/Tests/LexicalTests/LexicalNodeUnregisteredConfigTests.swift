import LexicalTesting
import Testing

@testable import Lexical

// Port of the `LexicalNode.$config() without registration` and `a setter
// returns the version it wrote to` suites of LexicalNode.test.ts. JS throws
// from `static getType()` for a class without a type; in Swift `getType()`
// returns "" for it and registration throws the same error.

private final class IncorrectCustomDecoratorNode: DecoratorNode {
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private final class CorrectCustomDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CorrectCustomDecoratorNode.self, type: "correct-custom-decorator")
  }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private class ParentConfigNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ParentConfigNode.self, type: "parent-config-node", extends: TextNode.self)
  }
}

private final class ChildConfigNode: ParentConfigNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ChildConfigNode.self, type: "child-config-node", extends: ParentConfigNode.self)
  }
}

private final class SiblingConfigNode: ParentConfigNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      SiblingConfigNode.self, type: "sibling-config-node", extends: ParentConfigNode.self)
  }
}

private class OuterConfigNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(OuterConfigNode.self, type: "outer-config-node", extends: TextNode.self)
  }
}

private final class InnerConfigNode: OuterConfigNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InnerConfigNode.self, type: "inner-config-node", extends: OuterConfigNode.self)
  }
}

private final class Transformed: @unchecked Sendable {
  var types: [String] = []
}
private let abstractBaseTransformed = Transformed()

/// An abstract base: no `type` (JS keys its config by a symbol), but a
/// `$transform` its concrete subclasses share.
private class AbstractBaseNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      AbstractBaseNode.self, extends: ElementNode.self,
      transform: { node in abstractBaseTransformed.types.append(node.getType()) })
  }
}

private final class ConcreteChildNode: AbstractBaseNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ConcreteChildNode.self, type: "concrete-child-node", extends: AbstractBaseNode.self)
  }
}

private func discreteEditor(_ nodes: [LexicalNode.Type] = []) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

@Suite struct LexicalNodeUnregisteredConfigTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.clone()"))
  func baseCloneThrows() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      // JS mocks `getType` for this suite so the constructor gets as far as
      // `clone`; without the mock, as here, construction throws first.
      #expect(throws: LexicalError.self) {
        let node = try LexicalNode(key: "__custom_key__")
        _ = try LexicalNode.clone(node)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > static getType() before registration"
    ))
  func staticGetTypeBeforeRegistration() throws {
    // Twice, so that a cached record cannot hold the wrong thing.
    for _ in 0..<2 {
      #expect(IncorrectCustomDecoratorNode.getType() == "")
      #expect(throws: LexicalError.self) {
        try discreteEditor([IncorrectCustomDecoratorNode.self])
      }
      do {
        _ = try discreteEditor([IncorrectCustomDecoratorNode.self])
      } catch {
        #expect("\(error)".contains("does not implement .getType"))
      }
      #expect(CorrectCustomDecoratorNode.getType() == "correct-custom-decorator")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > subclass static getType() is not shadowed by a superclass synthesized getType"
    ))
  func subclassGetTypeNotShadowed() {
    #expect(ParentConfigNode.getType() == "parent-config-node")
    #expect(ChildConfigNode.getType() == "child-config-node")
    #expect(SiblingConfigNode.getType() == "sibling-config-node")
    #expect(ParentConfigNode.getType() == "parent-config-node")
    #expect(ChildConfigNode.getType() == "child-config-node")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > subclass static getType() resolves correctly when read before the superclass"
    ))
  func subclassGetTypeReadFirst() {
    #expect(InnerConfigNode.getType() == "inner-config-node")
    #expect(OuterConfigNode.getType() == "outer-config-node")
    #expect(InnerConfigNode.getType() == "inner-config-node")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > abstract base class declares shared $config under a Symbol key"
    ))
  func abstractBaseSharedConfig() throws {
    #expect(AbstractBaseNode.getType() == "")
    let editor = try discreteEditor([ConcreteChildNode.self])
    try editor.update(discrete: true) {
      try getRoot().append(ConcreteChildNode())
    }
    #expect(ConcreteChildNode.getType() == "concrete-child-node")
    #expect(abstractBaseTransformed.types == ["concrete-child-node"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode.$config() without registration > a structurally-identical subclass stays narrowable"
    ))
  func structurallyIdenticalSubclassNarrows() throws {
    func narrowFromTextNode(_ node: TextNode) -> String {
      if let tab = node as? TabNode {
        return tab.getType()
      }
      return node.getType()
    }
    let editor = try discreteEditor()
    try editor.update(discrete: true) { () throws in
      let tab = try createTabNode()
      let text = try createTextNode("x")
      #expect(isTabNode(tab))
      #expect(isTabNode(text) == false)
      #expect(narrowFromTextNode(text) == "text")
      #expect(narrowFromTextNode(tab) == "tab")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::a setter returns the version it wrote to > setFormat/setStyle/setIndent return the writable, not the receiver"
    ))
  func settersReturnTheWritable() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode())
    }
    // A node is cloned only across updates.
    try editor.update(discrete: true) { () throws in
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      let afterIndent = try paragraph.setIndent(2)
      #expect(afterIndent !== paragraph)
      #expect(try afterIndent === paragraph.getLatest())
      #expect(try afterIndent.setStyle("color: red") === paragraph.getLatest())
      #expect(try afterIndent.setFormat("center") === paragraph.getLatest())
    }
  }
}
