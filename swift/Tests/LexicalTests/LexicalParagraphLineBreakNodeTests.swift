import Lexical
import LexicalTesting
import Testing

// Ports of nodes/__tests__/unit/LexicalParagraphNode.test.ts and
// LexicalLineBreakNode.test.ts, except createDOM, updateDOM and importDOM,
// which are dom-only. The reconciled HTML upstream compares is the document
// `shape` here. upstream's insertNewAfter test compares the HTML inside the
// update, before the new paragraph is reconciled, so only the model checks
// are ported.

@Suite struct LexicalParagraphNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.constructor"
    ))
  func constructor() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraphNode = try ParagraphNode()
      #expect(paragraphNode.getType() == "paragraph")
      #expect(try paragraphNode.getTextContent() == "")
    }
    #expect(throws: (any Error).self) { try ParagraphNode() }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.exportJSON() should return and object conforming to the expected schema"
    ))
  func exportJSONSchema() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(
        try createParagraphNode().exportJSON().stringify()
          == #"{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.insertNewAfter()"
    ))
  func insertNewAfter() throws {
    let editor = try createTestEditor()
    var paragraphNode: ParagraphNode?
    try update(editor) {
      paragraphNode = try ParagraphNode()
      try getRoot().append(try #require(paragraphNode))
    }
    #expect(try shape(editor) == "<paragraph></paragraph>")
    try update(editor) {
      let paragraphNode = try #require(paragraphNode)
      let selection = try paragraphNode.select()
      let result = try paragraphNode.insertNewAfter(selection, restoreSelection: false)
      let newParagraph = try #require(result as? ParagraphNode)
      #expect(try newParagraph.getDirection() == paragraphNode.getDirection())
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > $createParagraphNode()"
    ))
  func createParagraphNodeFactory() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraphNode = try ParagraphNode()
      let createdParagraphNode = try createParagraphNode()
      #expect(paragraphNode.__type == createdParagraphNode.__type)
      #expect(paragraphNode.__parent == createdParagraphNode.__parent)
      #expect(paragraphNode.__key != createdParagraphNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > $isParagraphNode()"
    ))
  func isParagraphNodeGuard() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(isParagraphNode(try ParagraphNode()))
    }
  }
}

@Suite struct LexicalLineBreakNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.constructor"
    ))
  func constructor() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let lineBreakNode = try createLineBreakNode()
      #expect(lineBreakNode.getType() == "linebreak")
      #expect(try lineBreakNode.getTextContent() == "\n")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.exportJSON() should return and object conforming to the expected schema"
    ))
  func exportJSONSchema() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(
        try createLineBreakNode().exportJSON().stringify() == #"{"type":"linebreak","version":1}"#)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.$isLineBreakNode()"
    ))
  func isLineBreakNodeGuard() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(isLineBreakNode(try createLineBreakNode()))
    }
  }
}
