import Lexical
import LexicalExtension
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts.

// "\\" is one literal backslash.
private let ONE = "foo\\"
private let TWO = "foo\\\\"
private let THREE = "foo\\\\\\"

private func importText(_ markdown: String) throws -> String {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  var text = ""
  try editor.update(discrete: true) {
    try convertFromMarkdownString(markdown, TRANSFORMERS, nil, false, true)
    text = try getRoot().getTextContent()
  }
  return text
}

/// `parseMarkdownHardLineBreak(line)` as the JS array (`null` is `nil`).
private func parse(_ line: String) -> [String]? {
  parseMarkdownHardLineBreak(line).map { [$0.0, $0.1] }
}

@Suite struct EscapedBackslashHardLineBreakTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > treats a lone trailing backslash as a hard line break"
    ))
  func loneTrailingBackslash() {
    #expect(parse(ONE) == ["foo", "\\"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > does not treat an escaped backslash as a hard line break"
    ))
  func escapedBackslash() {
    #expect(parse(TWO) == nil)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > treats an odd run of backslashes as a hard line break"
    ))
  func oddRunOfBackslashes() {
    #expect(parse(THREE) == ["foo\\\\", "\\"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > still recognises the two-space hard line break"
    ))
  func twoSpaceHardLineBreak() {
    #expect(parse("foo  ") == ["foo", "  "])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > merges a line ending in an escaped backslash with the next line"
    ))
  func mergesEscapedBackslashLine() {
    #expect(normalizeMarkdown(TWO + "\nbar", true) == TWO + " bar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > keeps a real hard line break unmerged"
    ))
  func keepsRealHardLineBreak() {
    #expect(normalizeMarkdown(ONE + "\nbar", true) == ONE + "\nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > keeps the literal backslash in the imported text"
    ))
  func keepsLiteralBackslash() throws {
    // `foo\\` is an escaped backslash, so the paragraph text is `foo\ bar`.
    #expect(try importText(TWO + "\nbar") == "foo\\ bar")
  }
}
