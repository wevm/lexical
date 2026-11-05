import Lexical
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-text/src/__tests__/unit/Issue5466Repro.test.ts.

private final class TestEntityNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestEntityNode.self, type: "test-entity-5466", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

/// Matches `${…}` placeholders, as in the report.
private let getMatch = regexMatch(#"\$\{[^}]*\}"#)

/// The paragraph children as `E:"…"` for entity nodes and `T:"…"` for
/// plain text nodes.
private func describeTextNodes() throws -> String {
  try getRoot().getAllTextNodes().map {
    "\($0 is TestEntityNode ? "E" : "T"):\(JSONValue.string(try $0.getTextContent()).stringify())"
  }.joined(separator: " ")
}

@Suite struct Issue5466ReproTests {
  // The second text node is bold so that it is not normalized into the
  // first one. That leaves the match at the end of a text node that still
  // has a TextNode sibling, which is the shape produced by $insertNodes.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces a match that ends the node and does not start at offset 0"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces the last of several matches in the same node"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces every match when the first one starts at offset 0"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > replaces a match that does not end the node (control)"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/Issue5466Repro.test.ts::registerLexicalTextEntity (#5466) > leaves a match that only completes inside the sibling (control)"
    ),
    arguments: [
      ("x ${A}", " tail", #"T:"x " E:"${A}" T:" tail""#),
      ("x ${A} y ${B}", " tail", #"T:"x " E:"${A}" T:" y " E:"${B}" T:" tail""#),
      ("${A} ${B} ${C}", " tail", #"E:"${A}" T:" " E:"${B}" T:" " E:"${C}" T:" tail""#),
      ("x ${A} y", " tail", #"T:"x " E:"${A}" T:" y" T:" tail""#),
      ("x ${A", "B} tail", #"T:"x ${A" T:"B} tail""#),
    ])
  func replacesMatches(_ first: String, _ second: String, _ expected: String) throws {
    let editor = try buildEntityEditor(nodes: [.klass(TestEntityNode.self)]) { editor in
      try registerLexicalTextEntity(editor, getMatch, TestEntityNode.self) {
        try applyNodeReplacement(TestEntityNode(try $0.getTextContent()))
      }
    }
    defer { editor.dispose() }
    try editor.update {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode(first), createTextNode(second).toggleFormat("bold")))
    }
    #expect(try editor.read { try describeTextNodes() } == expected)
  }
}
