import Lexical
import LexicalClipboard
import LexicalTesting
import Testing

// The paste tests of packages/lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx,
// which need @lexical/clipboard; the rest are in LexicalTests. Upstream
// compares the reconciled HTML; `shape` stands in for it, and the `dir` of
// an imported paragraph is checked on the model.

/// `beforeEach`: an empty, selected paragraph.
private func tabNodeEditor() throws -> Editor {
  let editor = try createMountedTestEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    _ = try getRoot().append(paragraph)
    _ = try paragraph.select()
  }
  return editor
}

private func paste(_ editor: Editor, _ insert: @escaping (RangeSelection) throws -> Void) throws {
  try update(editor) {
    try insert(try #require(try getSelection() as? RangeSelection, "isRangeSelection(selection)"))
  }
}

private func plainText(_ text: String) -> DataTransfer {
  let dataTransfer = DataTransfer()
  dataTransfer.setData("text/plain", text)
  return dataTransfer
}

@Suite struct LexicalTabNodePasteTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste plain text with tabs and newlines in plain text"
    ))
  func plainTextInPlainText() throws {
    let editor = try tabNodeEditor()
    defer { editor.dispose() }
    let dataTransfer = plainText("hello\tworld\nhello\tworld")
    try paste(editor) { try insertDataTransferForPlainText(dataTransfer, $0) }
    #expect(try shape(editor) == "<paragraph>[hello][\t][world]<br>[hello][\t][world]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste plain text with tabs and newlines in rich text"
    ))
  func plainTextInRichText() throws {
    let editor = try tabNodeEditor()
    defer { editor.dispose() }
    let dataTransfer = plainText("hello\tworld\nhello\tworld")
    try paste(editor) { try insertDataTransferForRichText(dataTransfer, $0, editor) }
    #expect(
      try shape(editor)
        == "<paragraph>[hello][\t][world]</paragraph><paragraph>[hello][\t][world]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can paste HTML with tabs and new lines (2)"
    ))
  func htmlWithTabs() throws {
    let editor = try tabNodeEditor()
    defer { editor.dispose() }
    // A Google Docs 2-liner "Hello\tworld".
    let dataTransfer = DataTransfer()
    let span =
      #"<span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">"#
    let tab =
      #"\#(span)<span class="Apple-tab-span" style="white-space:pre;">"# + "\t</span></span>"
    dataTransfer.setData(
      "text/html",
      #"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-123"><p dir="ltr" style="line-height:1.38;margin-left: 36pt;margin-top:0pt;margin-bottom:0pt;">"#
        + "\(span)Hello</span>\(tab)\(span)world</span></p>"
        + "\(span)Hello</span>\(tab)\(span)world</span></b>")
    try paste(editor) { try insertDataTransferForRichText(dataTransfer, $0, editor) }
    #expect(
      try shape(editor)
        == "<paragraph>[Hello][\t][world]</paragraph><paragraph>[Hello][\t][world]</paragraph>")
    // Upstream: `<p dir="ltr">` then `<p dir="auto">`.
    try editor.read(.latest) { () throws in
      let paragraphs = try getRoot().getChildren().compactMap { $0 as? ElementNode }
      try #require(paragraphs.count == 2)
      #expect(try paragraphs[0].getDirection() == "ltr")
      #expect(try paragraphs[1].getDirection() == nil)
    }
  }
}
