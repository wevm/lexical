import Lexical
import LexicalExtension
import LexicalSelection
import LexicalTesting
import Testing

// The two styling tests of
// packages/lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts,
// which use $patchStyleText from @lexical/selection; the rest of that file is
// in LexicalTests.

@Suite struct LexicalSelectionCaretSlicesStyleTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > styling retains the selected prefix at an element start (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > styling retains the selected prefix at an element start (backward=true)"
    ),
    arguments: [false, true])
  func stylingRetainsPrefix(backward: Bool) throws {
    let editor = try buildEditorFromExtensions(defineExtension(name: "slice-style"))
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("abc")
      let paragraph = try createParagraphNode().append(text)
      _ = try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      let (start, end) =
        backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try start.set(paragraph.getKey(), 0, .element)
      try end.set(text.getKey(), 1, .text)
      try setSelection(selection)

      try patchStyleText(selection, ["color": "red"])
      #expect(try selection.getTextContent() == "a")
      try patchStyleText(selection, ["color": "blue"])
      #expect(try paragraph.getFirstChildOrThrow().getTextContent() == "a")
      let anchorNode = try #require(try selection.anchor.getNode() as? TextNode)
      #expect(try anchorNode.getStyle() == "color: blue;")
      #expect(try paragraph.getLastChildOrThrow().getTextContent() == "bc")
    }
  }
}
