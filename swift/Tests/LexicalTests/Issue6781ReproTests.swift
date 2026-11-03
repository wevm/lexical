import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/Issue6781Repro.test.ts.

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

@Suite(.serialized) struct Issue6781ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue6781Repro.test.ts::Backspacing an empty block updates the pending format (#6781) > merging into the previous block adopts that block text format and style"
    ))
  func mergeAdoptsPreviousFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let text = try createTextNode("hello").setFormat(IS_BOLD).setStyle("color: red")
      let paragraph = try createParagraphNode().append(text)
      // The empty paragraph the caret sits in carries its own pending format
      // and style, as it would after the toolbar is used on an empty line.
      let emptyParagraph = try createParagraphNode().setTextFormat(IS_ITALIC).setTextStyle(
        "color: blue")
      try getRoot().clear().append(paragraph, emptyParagraph)
      let selection = try emptyParagraph.select(0, 0)
      selection.setFormat(IS_ITALIC)
      selection.setStyle("color: blue")
    }
    try editor.update(discrete: true) { try rangeSelection().deleteCharacter(true) }
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(selection.format == IS_BOLD)
      #expect(selection.style == "color: red")
    }
    try editor.update(discrete: true) { try rangeSelection().insertText("X") }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getTextContent() == "helloX")
      let text = try #require(try root.getLastDescendant() as? TextNode)
      #expect(try text.getFormat() == IS_BOLD)
      #expect(try text.getStyle() == "color: red")
      // The inserted text merges into the existing node.
      #expect(try (root.getFirstChild() as! ElementNode).getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue6781Repro.test.ts::Backspacing an empty block updates the pending format (#6781) > backspacing inside a block leaves the pending format alone"
    ))
  func backspaceInsideKeepsPendingFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let text = try createTextNode("hello").setFormat(IS_BOLD).setStyle("color: red")
      try getRoot().clear().append(try createParagraphNode().append(text))
      let selection = try text.select(5, 5)
      selection.setFormat(IS_ITALIC)
      selection.setStyle("color: blue")
    }
    try editor.update(discrete: true) { try rangeSelection().deleteCharacter(true) }
    try editor.read { () throws in
      let selection = try rangeSelection()
      // The caret never left the text node, so the chosen format stays.
      #expect(selection.format == IS_ITALIC)
      #expect(selection.style == "color: blue")
    }
  }
}
