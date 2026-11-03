import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/NormalizeInlineElements.test.ts.

private func setUp(_ dependency: any AnyLexicalExtensionArgument) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [dependency], nodes: { [.klass(TestInlineElementNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          let paragraph = try createParagraphNode()
          try getRoot().append(paragraph)
          try paragraph.append(
            try create(TestInlineElementNode.self), try create(TestInlineElementNode.self))
        })))
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChild() as? ParagraphNode)
}

@Suite struct NormalizeInlineElementsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeInlineElements.test.ts::NormalizeInlineElements > should remove empty inline elements by default"
    ))
  func removesEmptyInlineElements() throws {
    let editor = try setUp(configExtension(NormalizeInlineElementsExtension))
    defer { editor.dispose() }
    try update(editor) {
      let paragraph = try firstParagraph()
      #expect(try paragraph.isEmpty())
      try paragraph.append(
        try create(TestInlineElementNode.self).append(try createTextNode("foo")))
    }
    try update(editor) {
      let inlineNode = try #require(try firstParagraph().getFirstChild() as? TestInlineElementNode)
      try inlineNode.getFirstChildOrThrow().remove()
    }
    try update(editor) { () throws in #expect(try firstParagraph().isEmpty()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NormalizeInlineElements.test.ts::NormalizeInlineElements > should not to remove empty inline elements if extension is disabled"
    ))
  func keepsEmptyInlineElementsWhenDisabled() throws {
    let editor = try setUp(configExtension(NormalizeInlineElementsExtension) { $0.disabled = true })
    defer { editor.dispose() }
    try update(editor) {
      let paragraph = try firstParagraph()
      #expect(try paragraph.getChildrenSize() == 2)
      #expect(try paragraph.getFirstChildOrThrow() is TestInlineElementNode)
      #expect(try paragraph.getLastChildOrThrow() is TestInlineElementNode)
    }
  }
}
