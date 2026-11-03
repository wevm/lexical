import Lexical
import LexicalRichText
import LexicalTesting
import Testing

// The Headings e2e test of packages/lexical/src/caret/__tests__/unit/LexicalCaret.test.ts,
// which needs @lexical/rich-text; the rest of that file is in LexicalTests.

@Suite struct LexicalCaretHeadingTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > ported Headings e2e tests > Pressing return in the middle of a heading creates a new heading below"
    ))
  func returnInMiddleOfHeading() throws {
    let editor = try createMountedTestEditor(nodes: [.klass(HeadingNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let initialTextNode = try createTextNode("[before][after]")
      let headingNode = try createHeadingNode().append(initialTextNode)
      _ = try getRoot().clear().append(headingNode)
      let newHeadingNode = try #require(
        try initialTextNode.select(8, 8).insertParagraph() as? HeadingNode, "paragraph inserted")
      #expect(
        try getRoot().getAllTextNodes().map { try $0.getTextContent() } == ["[before]", "[after]"])
      #expect(
        try getRoot().getChildren().map { $0.getKey() }
          == [headingNode.getKey(), newHeadingNode.getKey()])
      #expect(try initialTextNode.getTextContent() == "[before]")
      #expect(try initialTextNode.getParent()?.is(headingNode) == true)
      let newTextNodes = try newHeadingNode.getAllTextNodes()
      try #require(newTextNodes.count == 1)
      #expect(try newTextNodes[0].getTextContent() == "[after]")
    }
  }
}
