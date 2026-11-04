import Lexical
import LexicalClipboard
import LexicalList
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts.
// Upstream compares the reconciled HTML; `shape` stands in for it, with the
// text format and style checked on the model. The code cases need
// @lexical/code, which is not ported (parity/skips.json).

/// `beforeEach`: an empty, selected paragraph.
private func pasteEditor() throws -> Editor {
  // Upstream's default test nodes include the list nodes.
  let editor = try createMountedTestEditor(nodes: [
    .klass(ListNode.self), .klass(ListItemNode.self),
  ])
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

  /// Each element below the root, in document order, with what upstream's
  /// HTML shows of it: `type(text)|format|direction`, a list as
  /// `list(listType)`, and a list item with its checked state (an item that
  /// only wraps a nested list as `item(<nested>)`, whose state the HTML does
  /// not show).
  private func listSummary(_ editor: Editor) throws -> [String] {
    try editor.read(.latest) {
      var out: [String] = []
      func walk(_ node: LexicalNode) throws {
        guard let element = node as? ElementNode else { return }
        let tail = "|\(try element.getFormatType())|\(try element.getDirection() ?? "nil")"
        if let list = element as? ListNode {
          out.append("list(\(try list.getListType()))" + tail)
        } else if let item = element as? ListItemNode {
          if try item.getFirstChild() is ListNode {
            out.append("item(<nested>)" + tail)
          } else {
            let checked = try item.getChecked().map { "\($0)" } ?? "nil"
            out.append("item(\(try item.getTextContent()))|\(checked)" + tail)
          }
        } else {
          out.append("\(element.getType())(\(try element.getTextContent()))" + tail)
        }
        try element.getChildren().forEach(walk)
      }
      try getRoot().getChildren().forEach(walk)
      return out
    }
  }

  private static let listCases: [(String, String)] = [
    (
      "google doc list with right alignment",
      ##"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-61c25daf-7fff-957d-d4b5-8e006d20766d"><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;"><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><p dir="ltr" style="line-height:1.38;text-align: right;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Hello</span></p></li><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><p dir="ltr" style="line-height:1.38;text-align: right;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">World</span></p></li></ul></b>"##
    ),
    (
      "google doc checklist",
      ##"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1980f960-7fff-f4df-4ba3-26c6e1508542"><ul style="margin-top:0;margin-bottom:0;padding-inline-start:28px;"><li dir="ltr" role="checkbox" aria-checked="true" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAABbElEQVR4Ae3bsU4CYRDEcRsxodZE8Q0BbS258l5MwESJNL6HOfrPKdhyxeBcwk5mkn9F98sGIOSuPM/zPM/zPI+xG/SEtuiAWpEOaIOWaDIWziP6RK14OzSjX44ITvTBvqRn1MRaMIHeBIE2TKBBEGhgArWkKmtJBjKQgQxkIANd/Aw0NVC+O7RHvYFynHasN1COE/UGynGiXgOIjxOtdIH4OGJAfBwxID6OGBAfRwiIjyMARMCpCjRF5+72Dzhd5R+rHfpC92NeTlWgLl5PkQg4RYBynBSJgFMGKMNJkQg4lYFeUDuFRMCpBXQOEgGnDtA/kPg4xT7m2y/tCd9zKgOdviTC5RQEIiAFjh4QASlw9IAISIEjCURAWvmf1UDKcQwUSDmOgWLdMcxA7BnIQAYykIEM5EcRvplAW0GgNRNoKQg0ZwJN0E4I5x1dI+pmgSSA84BG2QQt0LrYG/eAXtGccjme53me53me9wPjPWZWjhktAQAAAABJRU5ErkJggg==" width="18.4px" height="18.4px" alt="checked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">done</span></p></li><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li><ul style="margin-top:0;margin-bottom:0;padding-inline-start:28px;"><li dir="ltr" role="checkbox" aria-checked="true" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;" aria-level="2"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAABbElEQVR4Ae3bsU4CYRDEcRsxodZE8Q0BbS258l5MwESJNL6HOfrPKdhyxeBcwk5mkn9F98sGIOSuPM/zPM/zPI+xG/SEtuiAWpEOaIOWaDIWziP6RK14OzSjX44ITvTBvqRn1MRaMIHeBIE2TKBBEGhgArWkKmtJBjKQgQxkIANd/Aw0NVC+O7RHvYFynHasN1COE/UGynGiXgOIjxOtdIH4OGJAfBwxID6OGBAfRwiIjyMARMCpCjRF5+72Dzhd5R+rHfpC92NeTlWgLl5PkQg4RYBynBSJgFMGKMNJkQg4lYFeUDuFRMCpBXQOEgGnDtA/kPg4xT7m2y/tCd9zKgOdviTC5RQEIiAFjh4QASlw9IAISIEjCURAWvmf1UDKcQwUSDmOgWLdMcxA7BnIQAYykIEM5EcRvplAW0GgNRNoKQg0ZwJN0E4I5x1dI+pmgSSA84BG2QQt0LrYG/eAXtGccjme53me53me9wPjPWZWjhktAQAAAABJRU5ErkJggg==" width="18.4px" height="18.4px" alt="checked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">done</span></p></li><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="2"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li></ul><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li></ul></b>"##
    ),
    (
      "github checklist",
      ##"<meta charset='utf-8'><p dir="auto" style="box-sizing: border-box; margin-top: 0px !important; margin-bottom: 16px; color: rgb(31, 35, 40); font-family: -apple-system, &quot;system-ui&quot;, &quot;Segoe UI&quot;, &quot;Noto Sans&quot;, Helvetica, Arial, sans-serif, &quot;Apple Color Emoji&quot;, &quot;Segoe UI Emoji&quot;; font-size: 14px; font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: start; text-indent: 0px; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">checklist</p><ul class="contains-task-list" style="box-sizing: border-box; padding: 0px; margin-top: 0px; margin-bottom: 0px !important; position: relative; color: rgb(31, 35, 40); font-family: -apple-system, &quot;system-ui&quot;, &quot;Segoe UI&quot;, &quot;Noto Sans&quot;, Helvetica, Arial, sans-serif, &quot;Apple Color Emoji&quot;, &quot;Segoe UI Emoji&quot;; font-size: 14px; font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: start; text-indent: 0px; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;"><li class="task-list-item enabled" style="box-sizing: border-box; list-style-type: none; padding: 2px 15px 2px 42px; margin-right: -15px; margin-left: -15px; line-height: 1.5; border: 0px;"><span class="handle" style="box-sizing: border-box; display: block; float: left; width: 20px; padding: 2px 0px 0px 2px; margin-left: -43px; opacity: 0;"><svg class="drag-handle" aria-hidden="true" width="16" height="16"><path d="M10 13a1 1 0 100-2 1 1 0 000 2zm-4 0a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zm3 1a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zM6 5a1 1 0 100-2 1 1 0 000 2z"></path></svg></span><input type="checkbox" id="" class="task-list-item-checkbox" checked="" style="box-sizing: border-box; font: inherit; margin: 0px 0.2em 0.25em -1.4em; overflow: visible; padding: 0px; vertical-align: middle;"><span></span>done</li><li class="task-list-item enabled" style="box-sizing: border-box; list-style-type: none; margin-top: 0px; padding: 2px 15px 2px 42px; margin-right: -15px; margin-left: -15px; line-height: 1.5; border: 0px;"><span class="handle" style="box-sizing: border-box; display: block; float: left; width: 20px; padding: 2px 0px 0px 2px; margin-left: -43px; opacity: 0;"><svg class="drag-handle" aria-hidden="true" width="16" height="16"><path d="M10 13a1 1 0 100-2 1 1 0 000 2zm-4 0a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zm3 1a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zM6 5a1 1 0 100-2 1 1 0 000 2z"></path></svg></span><input type="checkbox" id="" class="task-list-item-checkbox" style="box-sizing: border-box; font: inherit; margin: 0px 0.2em 0.25em -1.4em; overflow: visible; padding: 0px; vertical-align: middle;"><span></span>todo</li></ul>"##
    ),
    (
      "joplin checklist",
      ##"<ul data-is-checklist="1"><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-0" checked="checked"><label id="cb-label-md-checkbox-0" for="md-checkbox-0" class="checkbox-label-checked">Milk</label></div></li><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-1"><label id="cb-label-md-checkbox-1" for="md-checkbox-1" class="checkbox-label-unchecked">Eggs</label></div></li><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-2" checked="checked"><label id="cb-label-md-checkbox-2" for="md-checkbox-2" class="checkbox-label-checked">Beer</label></div></li></ul>"##
    ),
  ]

  private static let expectedLists: [String: (String, [String])] = [
    "google doc list with right alignment": (
      "<list><listitem>[Hello]</listitem><listitem>[World]</listitem></list>",
      ["list(bullet)||nil", "item(Hello)|nil|right|ltr", "item(World)|nil|right|ltr"]
    ),
    "google doc checklist": (
      "<list><listitem>[done]</listitem><listitem>[todo]</listitem><listitem><list>"
        + "<listitem>[done]</listitem><listitem>[todo]</listitem></list></listitem>"
        + "<listitem>[todo]</listitem></list>",
      [
        "list(check)||nil", "item(done)|true||ltr", "item(todo)|false||ltr",
        "item(<nested>)||nil", "list(check)||nil", "item(done)|true||ltr",
        "item(todo)|false||ltr", "item(todo)|false||ltr",
      ]
    ),
    "github checklist": (
      "<paragraph>[checklist]</paragraph><list><listitem>[done]</listitem>"
        + "<listitem>[todo]</listitem></list>",
      [
        "paragraph(checklist)|start|nil", "list(check)||nil", "item(done)|true|start|nil",
        "item(todo)|false|start|nil",
      ]
    ),
    "joplin checklist": (
      "<list><listitem>[Milk]</listitem><listitem>[Eggs]</listitem><listitem>[Beer]</listitem></list>",
      ["list(check)||nil", "item(Milk)|true||nil", "item(Eggs)|false||nil", "item(Beer)|true||nil"]
    ),
  ]

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: google doc list with right alignment"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: google doc checklist"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: github checklist"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: joplin checklist"
    ))
  func htmlCopyPasteLists() throws {
    for (name, pastedHTML) in Self.listCases {
      let editor = try pasteEditor()
      defer { editor.dispose() }
      let dataTransfer = DataTransfer()
      dataTransfer.setData("text/html", pastedHTML)
      try update(editor) {
        let selection = try #require(
          try getSelection() as? RangeSelection, "isRangeSelection(selection)")
        try insertDataTransferForRichText(dataTransfer, selection, editor)
      }
      let (expectedShape, expectedSummary) = try #require(Self.expectedLists[name])
      #expect(try shape(editor) == expectedShape, "\(name)")
      #expect(try listSummary(editor) == expectedSummary, "\(name)")
    }
  }
}
