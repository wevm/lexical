import Lexical
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of the `markdown Safari compatibility (issue #8012)` describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts:
// negative lookbehinds throw at RegExp construction in Safari < 16.4, so the
// code span regex captures the preceding character instead.

private func importMarkdown(_ md: String) throws -> Editor {
  let editor = try createMarkdownTestEditor()
  try editor.update(
    { try convertFromMarkdownString(md, TRANSFORMERS) }, EditorUpdateOptions(discrete: true))
  return editor
}

private func roundtrip(_ md: String) throws -> String {
  try importMarkdown(md).read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
}

@Suite struct MarkdownSafariCompatibilityTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not throw when constructing markdown regex patterns"
    ))
  func doesNotThrow() {
    #expect(throws: Never.self) { _ = try importMarkdown("`hello`") }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses a code span at the start of a string"
    ))
  func codeSpanAtStart() throws {
    #expect(try roundtrip("`hello` world") == "`hello` world")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses a code span in the middle of text"
    ))
  func codeSpanInMiddle() throws {
    #expect(try roundtrip("foo `code` bar") == "foo `code` bar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses multiple code spans on the same line"
    ))
  func multipleCodeSpans() throws {
    #expect(try roundtrip("`a` and `b`") == "`a` and `b`")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not parse a backtick preceded by a backslash as a code span"
    ))
  func escapedBacktick() throws {
    let editor = try importMarkdown("\\`not code\\`")
    #expect(try editor.read(.latest) { try getRoot().getTextContent() } == "`not code`")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > correctly captures code span content"
    ))
  func codeSpanContent() throws {
    #expect(try roundtrip("`hello world`") == "`hello world`")
    #expect(try roundtrip("foo `bar baz` qux") == "foo `bar baz` qux")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not apply emphasis formatting inside a code span"
    ))
  func noEmphasisInCodeSpan() throws {
    #expect(try roundtrip("`**not bold**`") == "`**not bold**`")
  }
}
