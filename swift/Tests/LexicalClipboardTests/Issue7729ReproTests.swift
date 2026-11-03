import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue7729Repro.test.ts.

private func setUpEditor(_ initialState: (() throws -> Void)? = nil) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "issue-7729-repro", dependencies: [RichTextExtension],
      editorConfig: InitialEditorConfig(
        initialEditorState: initialState.map { fn in .update { _ in try fn() } })))
}

private func importHtml(_ html: String) throws {
  let editor = try getEditor()
  let dom = DOMParser().parseFromString(html, "text/html")
  let nodes = try generateNodesFromDOM(editor, dom)
  try insertGeneratedNodes(editor, nodes, selectAll())
}

private func importAndReadIndent(_ editor: Editor, _ html: String) throws -> Int {
  try editor.update(discrete: true) { try importHtml(html) }
  return try editor.read { () throws in
    let para = try #require(
      try getRoot().getFirstChildOrThrow() as? ParagraphNode,
      "expected ParagraphNode as first child")
    return try para.getIndent()
  }
}

@Suite struct Issue7729ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > exportDOM emits data-lexical-indent and importDOM round-trips"
    ))
  func roundTrip() throws {
    let editor = try setUpEditor {
      _ = try getRoot().append(createParagraphNode().append(createTextNode("hi")).setIndent(2))
    }
    defer { editor.dispose() }
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html.contains(#"data-lexical-indent="2""#))
    #expect(html.contains("padding-inline-start: 80px"))
    #expect(try importAndReadIndent(editor, html) == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > data-lexical-indent wins over a calc(...) padding-inline-start"
    ))
  func winsOverCalc() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    // What live-DOM copy/paste produces: a calc(...) padding that parseInt
    // cannot recover; the data attribute makes it unambiguous.
    #expect(
      try importAndReadIndent(
        editor,
        #"<p data-lexical-indent="2" style="padding-inline-start: calc(2 * var(--lexical-indent-base-value, 40px));">hi</p>"#
      ) == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > data-lexical-indent wins over a non-40-multiple padding-inline-start"
    ))
  func winsOverNon40Multiple() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    // A custom --lexical-indent-base-value (16px): 32px would round to 1.
    #expect(
      try importAndReadIndent(
        editor, #"<p data-lexical-indent="2" style="padding-inline-start: 32px;">hi</p>"#) == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > falls back to padding heuristic when data-lexical-indent is absent"
    ))
  func paddingHeuristic() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    // HTML from non-Lexical sources still uses the padding heuristic.
    #expect(
      try importAndReadIndent(editor, #"<p style="padding-inline-start: 80px;">hi</p>"#) == 2)
  }
}
