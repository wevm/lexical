import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalSelectionCaretText.test.ts.

private func run(_ body: @escaping () throws -> Void) throws {
  let editor = try createEditor(
    CreateEditorArgs(namespace: "caret-selection-test", onError: { throw $0 }))
  try editor.update(discrete: true, body)
}

private func endpoints(_ selection: RangeSelection, backward: Bool) -> (Point, Point) {
  backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
}

@Suite struct LexicalSelectionCaretTextTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::selected text ends at the same boundary for an element or text point (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::selected text ends at the same boundary for an element or text point (backward=true)"
    ), arguments: [false, true])
  func sameBoundaryForElementOrText(backward: Bool) throws {
    try run {
      let first = try createTextNode("abcd")
      let second = try createTextNode("efgh").setFormat("italic")
      let paragraph = try createParagraphNode().append(
        first, second, try createTextNode("ijkl").setFormat("bold"))
      try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      let (start, end) = endpoints(selection, backward: backward)
      try start.set(first.getKey(), 1, .text)
      try end.set(second.getKey(), 4, .text)
      #expect(try selection.getTextContent() == "bcdefgh")
      try end.set(paragraph.getKey(), 2, .element)
      #expect(try selection.getTextContent() == "bcdefgh")
      #expect(try paragraph.getTextContent() == "abcdefghijkl")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::equivalent mixed text and element points contain no selected text (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::equivalent mixed text and element points contain no selected text (backward=true)"
    ), arguments: [false, true])
  func equivalentPointsSelectNothing(backward: Bool) throws {
    try run {
      let first = try createTextNode("abcd")
      let paragraph = try createParagraphNode().append(
        first, try createTextNode("efgh").setFormat("italic"))
      try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      let (start, end) = endpoints(selection, backward: backward)
      try start.set(first.getKey(), 4, .text)
      try end.set(paragraph.getKey(), 1, .element)
      #expect(try selection.getTextContent() == "")
      #expect(try paragraph.getTextContent() == "abcdefgh")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::collapsed element point beside a slot host contains no text (offset=0)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretText.test.ts::collapsed element point beside a slot host contains no text (offset=1)"
    ), arguments: [0, 1])
  func collapsedPointBesideSlotHost(offset: Int) throws {
    try run {
      let root = try getRoot()
      let host = try createParagraphNode()
      try root.clear().append(host)
      try setSlot(host, "title", try createParagraphNode().append(try createTextNode("Title")))
      let selection = createRangeSelection()
      try selection.anchor.set(root.getKey(), offset, .element)
      try selection.focus.set(root.getKey(), offset, .element)
      #expect(try selection.getNodes().map { $0.getKey() } == [host.getKey()])
      #expect(try selection.getTextContent() == "")
      #expect(try host.getTextContent() == "Title")
    }
  }
}
