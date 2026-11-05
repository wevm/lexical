import Lexical
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts.

private final class TestEntityNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestEntityNode.self, type: "test-entity", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private func createTestEntityNode(_ text: String) throws -> TestEntityNode {
  try applyNodeReplacement(TestEntityNode(text))
}

private let getMatch = regexMatch(#"#\w+"#)

private func register(_ editor: Editor) throws -> [() -> Void] {
  try registerLexicalTextEntity(editor, getMatch, TestEntityNode.self) {
    try createTestEntityNode($0.getTextContent())
  }
}

@Suite struct RegisterLexicalTextEntityTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts::registerLexicalTextEntity > preserves style and detail when promoting simple text to an entity"
    ))
  func promotingKeepsStyleAndDetail() throws {
    let editor = try buildEntityEditor(nodes: [.klass(TestEntityNode.self)], register: register)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("#lexical").setFormat("bold").setStyle("color: red").setDetail(
            "directionless")))
    }
    try editor.read {
      let node = try #require(try getRoot().getFirstDescendant() as? TextNode)
      #expect(node is TestEntityNode)
      #expect(try node.getTextContent() == "#lexical")
      #expect(try node.hasFormat("bold"))
      #expect(try node.getStyle() == "color: red")
      #expect(try node.isDirectionless())
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntity.test.ts::registerLexicalTextEntity > preserves style and detail when reverting an entity to simple text"
    ))
  func revertingKeepsStyleAndDetail() throws {
    let editor = try buildEntityEditor(nodes: [.klass(TestEntityNode.self)], register: register)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("#lexical")))
    }
    try editor.update(discrete: true) {
      let node = try #require(try getRoot().getFirstDescendant() as? TestEntityNode)
      try node.setFormat("bold").setStyle("color: red").setDetail("directionless")
    }
    // Break the match so the entity reverts to a plain TextNode.
    try editor.update(discrete: true) {
      let node = try #require(try getRoot().getFirstDescendant() as? TextNode)
      try node.setTextContent("lexical")
    }
    try editor.read {
      let node = try #require(try getRoot().getFirstDescendant() as? TextNode)
      #expect(!(node is TestEntityNode))
      #expect(try node.getTextContent() == "lexical")
      #expect(try node.hasFormat("bold"))
      #expect(try node.getStyle() == "color: red")
      #expect(try node.isDirectionless())
    }
  }
}
