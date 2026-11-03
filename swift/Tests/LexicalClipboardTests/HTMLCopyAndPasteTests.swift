import Lexical
import LexicalClipboard
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts.
// Upstream compares the reconciled HTML; `shape` stands in for it, with the
// text format and style checked on the model. The list, checklist and code
// cases need @lexical/list and @lexical/code, which are not ported
// (parity/skips.json).

/// `beforeEach`: an empty, selected paragraph.
private func pasteEditor() throws -> Editor {
  let editor = try createMountedTestEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    _ = try getRoot().append(paragraph)
    _ = try paragraph.select()
  }
  return editor
}

struct HTMLPasteCase: Sendable, CustomTestStringConvertible {
  let name: String
  let pastedHTML: String
  let expectedShape: String
  var plainTextInsert: String? = nil
  var testDescription: String { name }

  static let all: [HTMLPasteCase] = [
    HTMLPasteCase(
      name: "plain DOM text node", pastedHTML: "Hello!",
      expectedShape: "<paragraph>[Hello!]</paragraph>"),
    HTMLPasteCase(
      name: "a paragraph element", pastedHTML: "<p>Hello!<p>",
      expectedShape: "<paragraph>[Hello!]</paragraph><paragraph></paragraph>"),
    HTMLPasteCase(
      name: "a single div",
      pastedHTML: """
        123
                    <div>
                      456
                    </div>
        """,
      expectedShape: "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"),
    HTMLPasteCase(
      name: "multiple nested spans and divs",
      pastedHTML: """
        <div>
                    a b
                    <span>
                      c d
                      <span>e</span>
                    </span>
                    <div>
                      f
                      <span>g h</span>
                    </div>
                  </div>
        """,
      expectedShape: "<paragraph>[a b c d e]</paragraph><paragraph>[f g h]</paragraph>"),
    HTMLPasteCase(
      name: "nested span in a div",
      pastedHTML: """
        <div>
                    <span>
                      123
                      <div>456</div>
                    </span>
                  </div>
        """,
      expectedShape: "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"),
    HTMLPasteCase(
      name: "nested div in a span", pastedHTML: " <span>123<div>456</div></span>",
      expectedShape: "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"),
    HTMLPasteCase(
      name: "pasting inheritance", pastedHTML: "<strong>hello</strong>",
      expectedShape: "<paragraph>[hello world]</paragraph>", plainTextInsert: " world"),
    HTMLPasteCase(
      name: "div with display:inline should not insert linebreaks",
      pastedHTML:
        #"<div><div style="display: inline;">hello</div> <div style="display: inline;">world</div></div>"#,
      expectedShape: "<paragraph>[hello world]</paragraph>"),
  ]
}

@Suite struct HTMLCopyAndPasteTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: plain DOM text node"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: a paragraph element"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: a single div"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: multiple nested spans and divs"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: nested span in a div"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: nested div in a span"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: pasting inheritance"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: div with display:inline should not insert linebreaks"
    ),
    arguments: HTMLPasteCase.all)
  func htmlCopyPaste(testCase: HTMLPasteCase) throws {
    let editor = try pasteEditor()
    defer { editor.dispose() }
    let dataTransfer = DataTransfer()
    dataTransfer.setData("text/html", testCase.pastedHTML)
    try update(editor) {
      let selection = try #require(
        try getSelection() as? RangeSelection, "isRangeSelection(selection)")
      try insertDataTransferForRichText(dataTransfer, selection, editor)
      if let plainTextInsert = testCase.plainTextInsert {
        let newSelection = try #require(
          try getSelection() as? RangeSelection,
          "isRangeSelection(newSelection) for plainTextInsert")
        try newSelection.insertText(plainTextInsert)
      }
    }
    #expect(try shape(editor) == testCase.expectedShape)
    if testCase.plainTextInsert != nil {
      // Upstream: `<strong class="editor-text-bold">`.
      try editor.read(.latest) { () throws in
        let texts = try getRoot().getAllTextNodes()
        #expect(try texts.allSatisfy { try $0.hasFormat("bold") })
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > iOS fix: Word predictions should be handled as plain text to maintain selection formatting"
    ))
  func wordPredictionsAsPlainText() throws {
    let editor = try pasteEditor()
    defer { editor.dispose() }
    // An iOS Safari autocorrect or word prediction pastes the word as both
    // text/plain and text/html; it is inserted as HTML only when they
    // differ.
    let dataTransfer = DataTransfer()
    dataTransfer.setData("text/plain", "Prediction")
    dataTransfer.setData("text/html", "Prediction")
    try update(editor) {
      let selection = try #require(
        try getSelection() as? RangeSelection, "isRangeSelection(selection)")
      try patchStyleText(selection, ["background-color": "rgb(255,170,45)"])
      try insertDataTransferForRichText(dataTransfer, selection, editor)
    }
    // The selection's style is kept because the text went in as plain text.
    #expect(try shape(editor) == "<paragraph>[Prediction]</paragraph>")
    try editor.read(.latest) { () throws in
      let texts = try getRoot().getAllTextNodes()
      try #require(texts.count == 1)
      // Upstream's DOM shows `background-color: rgb(255, 170, 45);`, the
      // jsdom normalization of this model style.
      #expect(try texts[0].getStyle() == "background-color: rgb(255,170,45);")
    }
  }
}
