import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/IndentationLimits.test.ts.
// Upstream's editor also has ListExtension; the paragraph cases here do
// not use it, and the list cases are skipped until @lexical/list is ported.

private func createIndentEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "IndentationLimits",
      dependencies: [
        RichTextExtension, configExtension(TabIndentationExtension) { $0.maxIndent = 7 },
      ]))
}

private func indent(_ editor: Editor, _ times: Int) throws {
  for _ in 0..<times {
    try editor.update(discrete: true) { try editor.dispatchCommand(INDENT_CONTENT_COMMAND) }
  }
}

private func paragraphIndent() throws -> Int {
  try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode).getIndent()
}

@Suite struct IndentationLimitsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for \"\""
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for \"World\""
    ), arguments: ["", "World"])
  func capsParagraphIndentation(_ text: String) throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      if !text.isEmpty { try paragraph.append(createTextNode(text)) }
      try getRoot().clear().append(paragraph)
      _ = try paragraph.selectEnd()
    }
    for i in 1...14 {
      try indent(editor, 1)
      #expect(try editor.read(paragraphIndent) == min(i, 6))
    }
  }

  // JS indents are numbers, so upstream also outdents 0.025, 0.05 and
  // 0.075; a Swift indent is an Int (see the skips).
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0 negative"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 1 negative"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 6 negative"
    ), arguments: [0, 1, 6])
  func outdentNeverNegative(_ initial: Int) throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("Hello"))
      try paragraph.setIndent(initial)
      try getRoot().clear().append(paragraph)
      _ = try paragraph.selectEnd()
    }
    for i in 1...8 {
      try editor.update(discrete: true) { try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND) }
      #expect(try editor.read(paragraphIndent) == max(0, initial - i))
    }
  }
}
