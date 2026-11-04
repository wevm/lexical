import Lexical
import LexicalLink
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of the `markdown whitespace import (default mode)` describe block of
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

nonisolated(unsafe) private let transformers: [any Transformer] =
  TRANSFORMERS + [HIGHLIGHT_TEXT_MATCH_IMPORT]

private func discreteUpdate(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(fn, EditorUpdateOptions(discrete: true))
}

private func exported(_ editor: Editor) throws -> String {
  try editor.read(.latest) { try convertToMarkdownString(transformers) }
}

private func importDefault(
  _ md: String, _ shouldMergeAdjacentLines: Bool = false
) throws
  -> Editor
{
  let editor = try createMarkdownTestEditor()
  try discreteUpdate(editor) {
    try convertFromMarkdownString(md, transformers, nil, false, shouldMergeAdjacentLines)
  }
  return editor
}

private func expectRoundTrip(_ md: String, _ shouldMergeAdjacentLines: Bool = false) throws {
  let editor = try importDefault(md, shouldMergeAdjacentLines)
  #expect(try exported(editor) == md)
}

private func getHardLineBreakMarker(_ line: String) throws -> String {
  let parsed = try #require(parseMarkdownHardLineBreak(line), "Expected a hard line break marker")
  return parsed.1
}

/// The first block and its first line break.
private func firstBlockAndLineBreak() throws -> (ElementNode, LexicalNode) {
  let block = try #require(
    try getRoot().getFirstChildOrThrow() as? ElementNode, "Expected an element block")
  let lineBreakNode = try #require(
    try block.getChildren().first { $0 is LineBreakNode }, "Expected a line break node")
  return (block, lineBreakNode)
}

private func expectImportedHardLineBreakMarker(_ md: String, _ marker: String) throws {
  let editor = try importDefault(md)
  try editor.read(.latest) { () throws -> Void in
    let (block, lineBreakNode) = try firstBlockAndLineBreak()
    let texts = try block.getChildren().compactMap { $0 as? TextNode }.map {
      try $0.getTextContent()
    }
    #expect(!texts.contains(marker))
    #expect(try block.getTextContent() == "foo\nbar")
    #expect(try getState(lineBreakNode, hardLineBreakState) == marker)
  }
  #expect(try exported(editor) == md)
}

@Suite struct MarkdownWhitespaceImportDefaultModeTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves trailing whitespace on a standalone paragraph line (default mode)"
    ))
  func trailingWhitespace() throws {
    // A paragraph with trailing spaces, separated by an empty line from the next.
    try expectRoundTrip("hello world   \n\nnext paragraph")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves leading whitespace on a standalone paragraph line (default mode)"
    ))
  func leadingWhitespace() throws {
    try expectRoundTrip("   hello world\n\nnext paragraph")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break in default mode"
    ))
  func twoSpaceHardBreak() throws {
    // normalizeMarkdown keeps the trailing "  ", which $importBlocks turns
    // into a LineBreakNode.
    try expectRoundTrip("foo  \nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces in default mode"
    ))
  func exactSpacesHardBreak() throws {
    try expectRoundTrip("foo   \nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break in default mode"
    ))
  func backslashHardBreak() throws {
    try expectRoundTrip("foo\\\nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after formatted text"
    ))
  func markerAfterFormattedText() throws {
    try expectImportedHardLineBreakMarker("**foo**  \nbar", "  ")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores exact hard line break spaces after formatted text"
    ))
  func exactMarkerAfterFormattedText() throws {
    try expectImportedHardLineBreakMarker("**foo**   \nbar", "   ")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after a link"
    ))
  func markerAfterLink() throws {
    try expectImportedHardLineBreakMarker("[foo](https://lexical.dev)  \nbar", "  ")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > does not infer a hard line break marker from spaces inside a link"
    ))
  func noMarkerFromSpacesInLink() throws {
    let md = "[foo  ](https://lexical.dev)\nbar"
    let editor = try importDefault(md)
    try editor.read(.latest) { () throws -> Void in
      let (block, lineBreakNode) = try firstBlockAndLineBreak()
      #expect(try block.getTextContent() == "foo  \nbar")
      #expect(try getState(lineBreakNode, hardLineBreakState) == "")
    }
    #expect(try exported(editor) == md)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after formatted blockquote text"
    ))
  func markerInBlockquote() throws {
    try expectImportedHardLineBreakMarker("> **foo**  \n> bar", "  ")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break in merge-adjacent mode"
    ))
  func twoSpaceMergeAdjacent() throws {
    try expectRoundTrip("foo  \nbar", true)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces in merge-adjacent mode"
    ))
  func exactSpacesMergeAdjacent() throws {
    try expectRoundTrip("foo   \nbar", true)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break in merge-adjacent mode"
    ))
  func backslashMergeAdjacent() throws {
    try expectRoundTrip("foo\\\nbar", true)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break between blockquote lines"
    ))
  func twoSpaceBlockquote() throws {
    try expectRoundTrip("> foo  \n> bar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces between blockquote lines"
    ))
  func exactSpacesBlockquote() throws {
    try expectRoundTrip("> foo   \n> bar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break between blockquote lines"
    ))
  func backslashBlockquote() throws {
    try expectRoundTrip("> foo\\\n> bar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > exports exact hard line break markers from line break state"
    ))
  func exportsMarkerFromState() throws {
    let editor = try createMarkdownTestEditor()
    let marker = try getHardLineBreakMarker("foo   ")
    try discreteUpdate(editor) {
      let lineBreakNode = try createLineBreakNode()
      try setState(lineBreakNode, hardLineBreakState, marker)
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("foo"), lineBreakNode, createTextNode("bar")))
    }
    #expect(try exported(editor) == "foo   \nbar")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > does not copy markdown hard line break markers when line breaks are copied"
    ))
  func copyResetsMarker() throws {
    let editor = try createMarkdownTestEditor()
    let marker = try getHardLineBreakMarker("foo   ")
    try discreteUpdate(editor) {
      let lineBreakNode = try createLineBreakNode()
      try setState(lineBreakNode, hardLineBreakState, marker)
      #expect(try getState(lineBreakNode, hardLineBreakState) == "   ")
      let copy = try copyNode(lineBreakNode)
      #expect(try getState(copy, hardLineBreakState) == "")
      #expect(try getState(lineBreakNode, hardLineBreakState) == "   ")
    }
  }
}
