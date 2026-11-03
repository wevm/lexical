import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts.

private func buildEditor(_ initial: @escaping () throws -> Void) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [DecoratorTextExtension, RichTextExtension],
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initial() })))
}

/// The decorator and the text node, after FORMAT_TEXT_COMMAND bold.
private func formatsAfterBold(_ editor: Editor) throws -> (decorator: Int, text: Int) {
  try editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold")
  return try editor.read {
    let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
    let children = try paragraph.getChildren()
    let decorator = try #require(children.first as? DecoratorTextNode)
    let text = try #require(children.last as? TextNode)
    return (try decorator.getFormat(), try text.getFormat())
  }
}

@Suite struct DecoratorTextExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > aligns DecoratorTextNode to not-bold when TextNode is bold"
    ))
  func alignsToNotBold() throws {
    let editor = try buildEditor {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      // The decorator starts not bold, the text bold.
      let decorator = try create(DecoratorTextNode.self)
      let text = try createTextNode("hello").setFormat(IS_BOLD)
      try paragraph.append(decorator, text)
      _ = try selectAll()
      // onSelectionChange doesn't run headless, so set the AND-intersection
      // format the browser would compute.
      if let selection = try getSelection() as? RangeSelection { selection.format = IS_BOLD }
    }
    defer { editor.dispose() }
    let formats = try formatsAfterBold(editor)
    #expect(formats.decorator == 0)
    #expect(formats.text == 0)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > aligns DecoratorTextNode to bold when TextNode is not-bold"
    ))
  func alignsToBold() throws {
    let editor = try buildEditor {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let decorator = try create(DecoratorTextNode.self).setFormat(IS_BOLD)
      try paragraph.append(decorator, createTextNode("hello"))
      _ = try selectAll()
    }
    defer { editor.dispose() }
    let formats = try formatsAfterBold(editor)
    #expect(formats.decorator == IS_BOLD)
    #expect(formats.text == IS_BOLD)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/DecoratorTextExtension.test.ts::DecoratorTextExtension FORMAT_TEXT_COMMAND > removes bold from all nodes when every node in selection is already bold"
    ))
  func removesBold() throws {
    let editor = try buildEditor {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let decorator = try create(DecoratorTextNode.self).setFormat(IS_BOLD)
      let text = try createTextNode("hello").setFormat(IS_BOLD)
      try paragraph.append(decorator, text)
      _ = try selectAll()
      if let selection = try getSelection() as? RangeSelection { selection.format = IS_BOLD }
    }
    defer { editor.dispose() }
    let formats = try formatsAfterBold(editor)
    #expect(formats.decorator == 0)
    #expect(formats.text == 0)
  }
}
