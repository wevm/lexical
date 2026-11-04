import Lexical
import LexicalLink
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of the `markdown hard line break import` describe block of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.

/// The test file's `HIGHLIGHT_TEXT_MATCH_IMPORT`: `LINK` with an import
/// regex that highlights `$text$`.
nonisolated(unsafe) private let HIGHLIGHT_TEXT_MATCH_IMPORT: TextMatchTransformer = {
  let t = LINK.copy()
  t.importRegExp = JSRegExp(#"\$([^$]+?)\$"#)
  t.replace = { textNode, _ in
    try textNode.toggleFormat("highlight")
    return nil
  }
  return t
}()

@Suite struct MarkdownHardLineBreakImportTests {
  private func expectPreservedRoundTrip(_ md: String) throws {
    let editor = try createMarkdownTestEditor()
    try editor.update(
      {
        try convertFromMarkdownString(md, TRANSFORMERS + [HIGHLIGHT_TEXT_MATCH_IMPORT], nil, true)
      },
      EditorUpdateOptions(discrete: true))
    #expect(
      try editor.read(.latest) {
        try convertToMarkdownString(TRANSFORMERS + [HIGHLIGHT_TEXT_MATCH_IMPORT], nil, true)
      } == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown hard line break import > preserves hard line break when shouldPreserveNewLines is true"
    ))
  func twoSpaces() throws {
    try expectPreservedRoundTrip("foo  \nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown hard line break import > preserves backslash hard line break when shouldPreserveNewLines is true"
    ))
  func backslash() throws {
    try expectPreservedRoundTrip("foo\\\nbar")
  }
}
