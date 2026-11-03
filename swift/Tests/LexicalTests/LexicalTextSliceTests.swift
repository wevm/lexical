import LexicalTesting
import Testing

@testable import Lexical

// Port of src/caret/__tests__/unit/LexicalTextSlice.test.ts.

private func run(_ body: @escaping () throws -> Void) throws {
  let editor = try createEditor(
    CreateEditorArgs(namespace: "caret-selection-test", onError: { throw $0 }))
  try editor.update(discrete: true, body)
}

@Suite struct LexicalTextSliceTests {
  @Test(
    .upstream("lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::split a next text slice"),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::split a previous text slice"),
    arguments: [CaretDirection.next, .previous])
  func splitSlice(direction: CaretDirection) throws {
    try run {
      let text = try createTextNode("abcde").setFormat("bold").setStyle("color: red;")
      let paragraph = try createParagraphNode().append(text)
      try getRoot().clear().append(paragraph)
      let caret = try getTextPointCaret(text, direction, direction == .next ? 1 : 4)
      let slice = getTextPointCaretSlice(caret, direction == .next ? 3 : -3)
      let extracted = try #require(try splitTextPointCaretSlice(slice))
      #expect(try paragraph.getChildren().map { try $0.getTextContent() } == ["a", "bcd", "e"])
      #expect(try extracted.getTextContent() == "bcd")
      #expect(try extracted.getFormat() == IS_BOLD)
      #expect(try extracted.getStyle() == "color: red;")
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalTextSlice.test.ts::empty and full text slices leave the node intact"
    ))
  func emptyAndFullSlices() throws {
    try run {
      let text = try createTextNode("abcde")
      let paragraph = try createParagraphNode().append(text)
      try getRoot().clear().append(paragraph)
      #expect(
        try splitTextPointCaretSlice(
          getTextPointCaretSlice(try getTextPointCaret(text, .next, 2), 0)) == nil)
      let full = try splitTextPointCaretSlice(
        getTextPointCaretSlice(try getTextPointCaret(text, .previous, 5), -5))
      #expect(full === text)
      #expect(try paragraph.getChildren().map { $0.getKey() } == [text.getKey()])
    }
  }
}
