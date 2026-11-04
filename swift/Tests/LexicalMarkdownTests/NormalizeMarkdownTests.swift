import LexicalMarkdown
import LexicalTesting
import Testing

// Port of the `normalizeMarkdown - shouldMergeAdjacentLines = true` and
// `= false` describe blocks of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.
// Documents are written as arrays of lines so that trailing whitespace shows.

private func lines(_ lines: String...) -> String { lines.joined(separator: "\n") }

private let codeBlocksMarkdown = lines(
  "", "A1", "A2", "", "A3", "", "```md", "B1", "B2", "", "B3", "```", "", "C1", "C2", "", "C3", "",
  "```js", "D1", "D2", "", "D3", "```", "", "```single line code```", "", "E1", "E2", "", "E3", "")

private let tableMarkdown = lines("", "| a | b |", "| --- | --- |", "| c | d |", "")

@Suite struct NormalizeMarkdownMergeTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > should combine lines separated by a single \n unless they are in a codeblock"
    ))
  func combinesOutsideCodeBlocks() {
    #expect(
      normalizeMarkdown(codeBlocksMarkdown, true)
        == lines(
          "", "A1 A2", "", "A3", "", "```md", "B1", "B2", "", "B3", "```", "", "C1 C2", "", "C3",
          "", "```js", "D1", "D2", "", "D3", "```", "", "```single line code```", "", "E1 E2", "",
          "E3", ""))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > keeps fence-like lines that carry an info string as code content"
    ))
  func infoStringFenceIsContent() {
    // The inner ```js carries an info string, so it is part of the code
    // block's content; the block stays open until the bare ``` fence.
    let markdown = lines("", "```ts", "const a = 1", "```js", "const b = 2", "```", "", "After", "")
    #expect(normalizeMarkdown(markdown, true) == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not close a longer fence on a shorter inner fence"
    ))
  func longerFenceStaysOpen() {
    let markdown = lines("", "````", "```", "inner", "```", "````", "")
    #expect(normalizeMarkdown(markdown, true) == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > closes a shorter opening fence with a longer closing fence"
    ))
  func longerClosingFence() {
    // Per CommonMark a closing fence may be longer than the opening fence.
    let markdown = lines("```", "line one", "line two", "````", "after one", "after two")
    #expect(
      normalizeMarkdown(markdown, true)
        == lines("```", "line one", "line two", "````", "after one after two"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > keeps content unmerged in an unclosed code block"
    ))
  func unclosedCodeBlock() {
    // The end of the document closes a fenced code block too.
    let markdown = lines("```", "line one", "line two")
    #expect(normalizeMarkdown(markdown, true) == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > tables"
    ))
  func tables() {
    #expect(normalizeMarkdown(tableMarkdown, true) == tableMarkdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges adjacent plain text lines with a single space"
    ))
  func mergesPlainLines() {
    #expect(normalizeMarkdown(lines("Hello", "world"), true) == "Hello world")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges while trimming the next line and inserting a single space"
    ))
  func mergesTrimmed() {
    #expect(normalizeMarkdown(lines("Hello", "   world   "), true) == "Hello world")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not merge across HTML-like tags (opening, content, closing, after)"
    ))
  func htmlLikeTags() {
    let md = lines("<div>", "content", "</div>", "after")
    #expect(normalizeMarkdown(md, true) == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not merge the fence line with the first line after a code block"
    ))
  func closingFenceStaysOnItsLine() {
    #expect(normalizeMarkdown("```\ncode\n```\nNext line", true) == "```\ncode\n```\nNext line")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves hard-break trailing spaces when merging adjacent lines"
    ))
  func hardBreakSpaces() {
    let md = lines("foo  ", "bar")
    #expect(normalizeMarkdown(md, true) == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves exact hard-break trailing spaces when merging adjacent lines"
    ))
  func exactHardBreakSpaces() {
    let md = lines("foo   ", "bar")
    #expect(normalizeMarkdown(md, true) == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves backslash hard-breaks when merging adjacent lines"
    ))
  func backslashHardBreak() {
    let md = lines("foo\\", "bar")
    #expect(normalizeMarkdown(md, true) == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges a soft break before a hard-breaking line"
    ))
  func softBreakBeforeHardBreak() {
    #expect(normalizeMarkdown(lines("foo", "bar  ", "baz"), true) == lines("foo bar  ", "baz"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > treats whitespace-only lines as empty separators (no merge across them)"
    ))
  func whitespaceOnlySeparator() {
    #expect(normalizeMarkdown(lines("A1", "     ", "A2"), true) == lines("A1", "", "A2"))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > mdx start tag followed by content, than closing tag preceded by content"
    ))
  func mdxTagsWithContent() {
    #expect(
      normalizeMarkdown(lines("<MyComponent>Line 1", "Some Text</MyComponent>"), true)
        == "<MyComponent>Line 1 Some Text</MyComponent>")
  }
}

@Suite struct NormalizeMarkdownNoMergeTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > should not combine lines separated by a single \n"
    ))
  func doesNotCombine() {
    #expect(normalizeMarkdown(codeBlocksMarkdown, false) == codeBlocksMarkdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > tables"
    ))
  func tables() {
    #expect(normalizeMarkdown(tableMarkdown, false) == tableMarkdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves trailing whitespace on content lines"
    ))
  func trailingWhitespace() {
    #expect(normalizeMarkdown("foo   \n\nbar", false) == "foo   \n\nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > collapses whitespace-only lines to empty (paragraph separator)"
    ))
  func whitespaceOnlyLines() {
    #expect(normalizeMarkdown("A\n   \nB", false) == "A\n\nB")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves leading whitespace on content lines"
    ))
  func leadingWhitespace() {
    #expect(normalizeMarkdown("   foo\n\nbar", false) == "   foo\n\nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves indented fenced code blocks nested inside tags"
    ))
  func indentedFenceInTags() {
    let markdown = lines("", "<Banner>", "```ts", "  indent 1;", "```", "</Banner>", "")
    #expect(normalizeMarkdown(markdown, false) == markdown)
  }
}
