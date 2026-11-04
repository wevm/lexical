import Lexical
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of the `inline code with backticks (CommonMark code spans)` describe
// block of packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.

private func roundtrip(_ md: String) throws -> String {
  let editor = try createMarkdownTestEditor()
  try editor.update(
    { try convertFromMarkdownString(md, TRANSFORMERS) }, EditorUpdateOptions(discrete: true))
  return try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
}

private func exportCodeSpan(_ content: String) throws -> String {
  let editor = try createMarkdownTestEditor()
  try editor.update(
    {
      let paragraph = try createParagraphNode()
      let text = try createTextNode(content)
      try text.toggleFormat("code")
      try paragraph.append(text)
      try getRoot().append(paragraph)
    }, EditorUpdateOptions(discrete: true))
  return try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
}

@Suite struct MarkdownInlineCodeBackticksTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > round-trips code spans whose content contains backticks"
    ))
  func roundTripsBackticks() throws {
    #expect(try roundtrip("Here: `` a`b ``") == "Here: `` a`b ``")
    #expect(try roundtrip("Code: ``` a``b ```") == "Code: ``` a``b ```")
    #expect(try roundtrip("`` `x ``") == "`` `x ``")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > exports a content-derived fence longer than any backtick run"
    ))
  func contentDerivedFence() throws {
    #expect(try exportCodeSpan("block code") == "`block code`")
    #expect(try exportCodeSpan("a`b") == "`` a`b ``")
    #expect(try exportCodeSpan("a``b") == "``` a``b ```")
    #expect(try exportCodeSpan("`x") == "`` `x ``")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > does not pad a code span made only of spaces"
    ))
  func spacesOnlyNotPadded() throws {
    // CommonMark strips one space from each end only when the content is not
    // all spaces, so padding such a span would add two spaces on import.
    #expect(try exportCodeSpan(" ") == "` `")
    #expect(try exportCodeSpan("   ") == "`   `")
    #expect(try roundtrip("a ` ` b") == "a ` ` b")
    #expect(try roundtrip("a `   ` b") == "a `   ` b")
    #expect(try exportCodeSpan(" x ") == "`  x  `")
    #expect(try exportCodeSpan("\tx\t") == "` \tx\t `")
    #expect(try exportCodeSpan(" \t ") == "`  \t  `")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > normalizes a redundant inline fence to the minimal valid fence"
    ))
  func minimalFence() throws {
    #expect(try roundtrip("a `block code` b") == "a `block code` b")
    #expect(try roundtrip("a ```block code``` b") == "a `block code` b")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > keeps the code fence innermost when combined with bold"
    ))
  func fenceInnermostWithBold() throws {
    #expect(try roundtrip("**`` a`b ``**") == "**`` a`b ``**")
  }
}
