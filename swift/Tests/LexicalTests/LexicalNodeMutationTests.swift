import Lexical
import LexicalTesting
import Testing

// Port of the replace, insertAfter, insertBefore and selectNext tests of the
// `LexicalNode tests` suite in LexicalNode.test.ts. The reconciled HTML that
// upstream compares is compared as the model's shape (see `shape`).

private func run(_ env: NodeTestEnv, _ fn: @escaping (TextNode) throws -> Void) throws {
  let textNode = env.textNode!
  try update(env.editor) { try fn(textNode) }
}

@Suite struct LexicalNodeMutationTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): with a sibling in the same parent"
    ))
  func replaceWithSibling() throws {
    let env = try NodeTestEnv()
    try run(env) { textNode in
      let paragraph = try textNode.getParentOrThrow()
      let barTextNode = try TextNode("bar")
      try paragraph.append(barTextNode)
      try textNode.replace(barTextNode)
      #expect(try paragraph.getChildrenSize() == 1)
      #expect(try paragraph.getChildren().count == 1)
      #expect(try paragraph.getFirstChild()!.getTextContent() == "bar")
    }
    #expect(try shape(env.editor) == "<paragraph>[bar]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): token"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): segmented"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): directionless"
    ),
    arguments: ["text", "token", "segmented", "directionless"])
  func replaceText(_ kind: String) throws {
    let env = try NodeTestEnv()
    #expect(try shape(env.editor) == "<paragraph>[foo]</paragraph>")
    try run(env) { textNode in
      let barTextNode = try TextNode("bar")
      switch kind {
      case "token", "segmented": try barTextNode.setMode(kind)
      case "directionless": try barTextNode.toggleDirectionless()
      default: break
      }
      try textNode.replace(barTextNode)
    }
    #expect(try shape(env.editor) == "<paragraph>[bar]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace() within canBeEmpty: false"
    ))
  func replaceWithinCanBeEmptyFalse() throws {
    let env = try NodeTestEnv()
    try update(env.editor) {
      let textNode = try createTextNode("Hello")
      try getRoot().clear().append(
        createParagraphNode().append(createTestInlineElementNode().append(textNode)))
      try textNode.replace(createTextNode("world"))
    }
    #expect(
      try shape(env.editor)
        == "<paragraph><test_inline_block>[world]</test_inline_block></paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter()"
    ))
  func insertAfter() throws {
    // Upstream calls insertAfter() with no argument, which Swift's types rule
    // out; in a read, and outside one, any insertion throws.
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(throws: (any Error).self) { try env.textNode.insertAfter(TextNode()) }
    }
    #expect(throws: (any Error).self) { try env.textNode.insertAfter(TextNode()) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): token"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): segmented"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter(): directionless"
    ),
    arguments: [
      // Upstream's "segmented" case sets token mode too.
      ("text", "[foobar]"), ("token", "[foo][bar]"), ("segmented", "[foo][bar]"),
      ("directionless", "[foobar]"),
    ])
  func insertAfterText(_ kind: String, _ expected: String) throws {
    let env = try NodeTestEnv()
    #expect(try shape(env.editor) == "<paragraph>[foo]</paragraph>")
    try run(env) { textNode in
      let barTextNode = try TextNode("bar")
      switch kind {
      case "token", "segmented": try barTextNode.setMode("token")
      case "directionless": try barTextNode.toggleDirectionless()
      default: break
      }
      try textNode.insertAfter(barTextNode)
    }
    #expect(try shape(env.editor) == "<paragraph>\(expected)</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter() move blocks around"
    ))
  func insertAfterMoveBlocks() throws {
    let env = try NodeTestEnv()
    var text1: TextNode!
    var block2: ParagraphNode!
    try update(env.editor) {
      let root = try getRoot()
      try root.clear()
      let block1 = try ParagraphNode()
      block2 = try ParagraphNode()
      let block3 = try ParagraphNode()
      text1 = try TextNode("A")
      try block1.append(text1)
      try block2.append(TextNode("B"))
      try block3.append(TextNode("C"))
      try root.append(block1, block2, block3)
    }
    #expect(
      try shape(env.editor)
        == "<paragraph>[A]</paragraph><paragraph>[B]</paragraph><paragraph>[C]</paragraph>")
    try update(env.editor) {
      try text1.insertAfter(block2)
    }
    #expect(
      try shape(env.editor)
        == "<paragraph>[A]<paragraph>[B]</paragraph></paragraph><paragraph>[C]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertAfter() move blocks around #2"
    ))
  func insertAfterMoveBlocks2() throws {
    let env = try NodeTestEnv()
    var texts: [TextNode] = []
    try update(env.editor) {
      let root = try getRoot()
      try root.clear()
      texts = try ["A", "B", "C"].map { try TextNode($0).toggleUnmergeable() }
      for text in texts {
        try root.append(ParagraphNode().append(text))
      }
    }
    #expect(
      try shape(env.editor)
        == "<paragraph>[A]</paragraph><paragraph>[B]</paragraph><paragraph>[C]</paragraph>")
    try update(env.editor) {
      try texts[2].insertAfter(texts[0])
      try texts[2].insertAfter(texts[1])
    }
    #expect(
      try shape(env.editor)
        == "<paragraph></paragraph><paragraph></paragraph><paragraph>[C][B][A]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore()"
    ))
  func insertBefore() throws {
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(throws: (any Error).self) { try env.textNode.insertBefore(TextNode()) }
    }
    #expect(throws: (any Error).self) { try env.textNode.insertBefore(TextNode()) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): from another parent"
    ))
  func insertBeforeFromAnotherParent() throws {
    let env = try NodeTestEnv()
    #expect(try shape(env.editor) == "<paragraph>[foo]</paragraph>")
    try update(env.editor) {
      try getRoot().append(ParagraphNode().append(TextNode("bar")))
    }
    #expect(try shape(env.editor) == "<paragraph>[foo]</paragraph><paragraph>[bar]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): token"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): segmented"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.insertBefore(): directionless"
    ),
    arguments: [
      ("text", "[barfoo]"), ("token", "[bar][foo]"), ("segmented", "[bar][foo]"),
      ("directionless", "[barfoo]"),
    ])
  func insertBeforeText(_ kind: String, _ expected: String) throws {
    let env = try NodeTestEnv()
    #expect(try shape(env.editor) == "<paragraph>[foo]</paragraph>")
    try run(env) { textNode in
      let barTextNode = try TextNode("bar")
      switch kind {
      case "token", "segmented": try barTextNode.setMode(kind)
      case "directionless": try barTextNode.toggleDirectionless()
      default: break
      }
      try textNode.insertBefore(barTextNode)
    }
    #expect(try shape(env.editor) == "<paragraph>\(expected)</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext()"
    ))
  func selectNext() throws {
    let env = try NodeTestEnv()
    try run(env) { textNode in
      let barTextNode = try TextNode("bar")
      try textNode.insertAfter(barTextNode)
      #expect(try barTextNode.isSelected() == false)
      try textNode.selectNext()
      #expect(try barTextNode.isSelected())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext(): no next sibling"
    ))
  func selectNextNoSibling() throws {
    let env = try NodeTestEnv()
    let paragraphNode = env.paragraphNode!
    try run(env) { textNode in
      let selection = try textNode.selectNext()
      #expect(try selection.anchor.getNode() === paragraphNode)
      #expect(selection.anchor.offset == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.selectNext(): non-text node"
    ))
  func selectNextNonText() throws {
    let env = try NodeTestEnv()
    try run(env) { textNode in
      try textNode.insertAfter(TestNode())
      let selection = try textNode.selectNext()
      #expect(try selection.anchor.getNode() === textNode.getParent())
      #expect(selection.anchor.offset == 1)
    }
  }
}
