import Lexical
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts,
// which tests `@lexical/text`'s root helpers.

@Suite struct LexicalRootHelpersTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts::LexicalRootHelpers tests > textContent"
    ))
  func textContent() throws {
    let editor = try createMountedTestEditor()
    #expect(try editor.read(.latest) { try rootTextContent() } == "")
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(createTextNode("foo"))
      #expect(try rootTextContent() == "foo")
    }
    #expect(try editor.read(.latest) { try rootTextContent() } == "foo")
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalRootHelpers.test.ts::LexicalRootHelpers tests > isBlank"
    ))
  func isBlank() throws {
    let editor = try createMountedTestEditor()
    #expect(try editor.read(.latest, isRootTextContentEmptyCurry(editor.isComposing())) == true)
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(createTextNode("foo"))
      #expect(try isRootTextContentEmpty(editor.isComposing()) == false)
    }
    #expect(try editor.read(.latest, isRootTextContentEmptyCurry(editor.isComposing())) == false)
  }
}
