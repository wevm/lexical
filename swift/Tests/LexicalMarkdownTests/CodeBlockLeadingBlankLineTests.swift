import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts.

private func importCodeText(_ markdown: String) throws -> String? {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  var text: String?
  try editor.update(discrete: true) {
    try convertFromMarkdownString(markdown, TRANSFORMERS)
    let first = try getRoot().getFirstChild()
    text = try (first as? CodeNode)?.getTextContent()
  }
  return text
}

private func roundTrip(_ markdown: String) throws -> String {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  try editor.update(discrete: true) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
  return try editor.read { try convertToMarkdownString(TRANSFORMERS) }
}

@Suite struct CodeBlockLeadingBlankLineTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > keeps a blank first line of a fenced code block"
    ))
  func keepsBlankFirstLine() throws {
    #expect(try importCodeText("```js\n\ncode\n```") == "\ncode")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > keeps several blank first lines"
    ))
  func keepsSeveralBlankFirstLines() throws {
    #expect(try importCodeText("```js\n\n\na\nb\n```") == "\n\na\nb")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > round trips a code block that starts with a blank line"
    ))
  func roundTripsLeadingBlankLine() throws {
    let markdown = "```js\n\ncode\n```"
    #expect(try roundTrip(markdown) == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > still drops nothing from a code block without leading blank lines"
    ))
  func noLeadingBlankLines() throws {
    #expect(try importCodeText("```js\ncode\n```") == "code")
    #expect(try roundTrip("```js\ncode\n```") == "```js\ncode\n```")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > still treats text after the opening fence as content"
    ))
  func textAfterOpeningFence() throws {
    #expect(try importCodeText("``` code\nmore\n```") == "code\nmore")
    #expect(try importCodeText("```javascript Incomplete tag") == "Incomplete tag")
  }
}
