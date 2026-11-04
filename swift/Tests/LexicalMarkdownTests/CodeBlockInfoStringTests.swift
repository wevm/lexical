import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts.

private func roundTrip(_ markdown: String) throws -> String {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  try editor.update(discrete: true) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
  return try editor.read { try convertToMarkdownString(TRANSFORMERS) }
}

/// `{language, text}` of the first block when it is a code block; JS
/// `language: undefined` (no code block, or none set) is `nil`.
private struct CodeBlockInfo: Equatable {
  var language: String?
  var text: String?
}

private func importCodeBlock(_ markdown: String) throws -> CodeBlockInfo {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  var result = CodeBlockInfo()
  try editor.update(discrete: true) {
    try convertFromMarkdownString(markdown, TRANSFORMERS)
    if let first = try getRoot().getFirstChild() as? CodeNode {
      result = CodeBlockInfo(language: try first.getLanguage(), text: try first.getTextContent())
    }
  }
  return result
}

@Suite struct CodeBlockInfoStringTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a title attribute into the code content"
    ))
  func titleAttribute() throws {
    #expect(
      try importCodeBlock("```js title=\"x\"\ncode\n```")
        == CodeBlockInfo(language: "js", text: "code"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a line-highlight range into the code content"
    ))
  func lineHighlightRange() throws {
    #expect(
      try importCodeBlock("```ts {1,3}\ncode\n```") == CodeBlockInfo(language: "ts", text: "code"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a bare info-string word into the code content"
    ))
  func bareInfoStringWord() throws {
    #expect(
      try importCodeBlock("```js showLineNumbers\na\nb\n```")
        == CodeBlockInfo(language: "js", text: "a\nb"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > keeps content that follows a fence with no language"
    ))
  func fenceWithNoLanguage() throws {
    #expect(
      try importCodeBlock("``` code\nmore\n```") == CodeBlockInfo(language: nil, text: "code\nmore")
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > leaves a plain language fence unchanged"
    ))
  func plainLanguageFence() throws {
    #expect(
      try importCodeBlock("```javascript\nCode\n```")
        == CodeBlockInfo(language: "javascript", text: "Code"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > round trips the info string of a closed fence"
    ))
  func roundTripsInfoString() throws {
    #expect(try roundTrip("```js title=\"x\"\ncode\n```") == "```js title=\"x\"\ncode\n```")
    #expect(try roundTrip("```ts {1,3}\ncode\n```") == "```ts {1,3}\ncode\n```")
    #expect(
      try roundTrip("```js showLineNumbers\na\nb\n```") == "```js showLineNumbers\na\nb\n```")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > adds nothing to a fence that has no info string tail"
    ))
  func noInfoStringTail() throws {
    #expect(try roundTrip("```js\ncode\n```") == "```js\ncode\n```")
    #expect(try roundTrip("```\ncode\n```") == "```\ncode\n```")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > leaves an unterminated fence unchanged"
    ))
  func unterminatedFence() throws {
    #expect(
      try importCodeBlock("```javascript Incomplete tag")
        == CodeBlockInfo(language: "javascript", text: "Incomplete tag"))
  }
}
