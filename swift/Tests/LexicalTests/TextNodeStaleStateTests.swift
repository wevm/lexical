import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/TextNodeStaleState.test.ts. The returned
// reference is captured during the seeding update, so from the next update
// on it is the previous version of the node.

private func seedStaleTextNode(_ editor: Editor) throws -> TextNode {
  var text: TextNode!
  try editor.update(discrete: true) {
    text = try createTextNode("a")
    try getRoot().clear().append(try createParagraphNode().append(text))
  }
  return text
}

private func editor() throws -> Editor {
  try createEditor(CreateEditorArgs(namespace: "textnode-stale-state", onError: { throw $0 }))
}

@Suite struct TextNodeStaleStateTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > setTextContent can restore the original text through a stale reference"
    ))
  func setTextContentRestores() throws {
    let editor = try editor()
    let text = try seedStaleTextNode(editor)
    try editor.update(discrete: true) {
      _ = try text.setTextContent("b")
      _ = try text.setTextContent("a")
    }
    #expect(try editor.read { try text.getTextContent() } == "a")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > setMode can restore the original mode through a stale reference"
    ))
  func setModeRestores() throws {
    let editor = try editor()
    let text = try seedStaleTextNode(editor)
    try editor.update(discrete: true) {
      _ = try text.setMode("token")
      _ = try text.setMode("normal")
    }
    #expect(try editor.read { try text.getMode() } == "normal")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > isSimpleText reflects the latest mode"
    ))
  func isSimpleTextReflectsLatestMode() throws {
    let editor = try editor()
    let text = try seedStaleTextNode(editor)
    try editor.update(discrete: true) { _ = try text.setMode("token") }
    try editor.read { () throws in
      #expect(try text.isToken())
      #expect(try text.isSimpleText() == false)
    }
  }
}
