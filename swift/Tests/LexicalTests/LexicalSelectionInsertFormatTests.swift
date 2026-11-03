import Lexical
import LexicalTesting
import Testing

// Port of the insertText, insertNodes and formatting regression suites of
// __tests__/unit/LexicalSelection.test.ts.

private func paragraphTexts(_ paragraph: ElementNode) throws -> [TextNode] {
  try paragraph.getChildren().map { try #require($0 as? TextNode) }
}

/// `bold` then `plain` in one paragraph, `bold` bold, the whole document
/// selected.
private func boldThenPlain(
  _ second: String = "plain", bothBold: Bool = false
) throws -> (
  ParagraphNode, RangeSelection
) {
  let paragraph = try createParagraphNode()
  let first = try createTextNode(bothBold ? "aaa" : "bold").setFormat(IS_BOLD)
  let last = try createTextNode(second)
  if bothBold {
    try last.setFormat(IS_BOLD)
  }
  try paragraph.append(first, last)
  try getRoot().clear().append(paragraph)
  return (paragraph, try selectAll())
}

@Suite struct LexicalSelectionInsertFormatTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #7081 > Firefox selection & paste before linebreak"
    ))
  func pasteBeforeLineBreak() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let textNode = try createTextNode("XXXX").setStyle("color: --color-test")
      let paragraphNode = try createParagraphNode()
      try getRoot().clear().append(
        paragraphNode.append(
          createTextNode("ID: "), textNode, createLineBreakNode(), createTextNode("aa")))
      let selection = try textNode.select(0)
      try selection.focus.set(
        paragraphNode.getKey(), 1 + textNode.getIndexWithinParent(), .element)
      try selection.insertText("123")
      let styledChild = try #require(
        try paragraphNode.getChildren().first {
          try ($0 as? TextNode)?.getStyle() == "color: --color-test"
        })
      #expect(try styledChild.getTextContent() == "123")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #7173 > Can insertNodes of multiple blocks with a target of an initial empty block and the entire next block"
    ))
  func insertBlocksOverEmptyAndFullBlock() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode(), createParagraphNode().append(createTextNode("replaced!")))
      let selection = try selectAll()
      let insertedNodes = try [
        createParagraphNode().append(createTextNode("p1")),
        createParagraphNode().append(createTextNode("p2")),
      ]
      try selection.insertNodes(insertedNodes)
      #expect(try getRoot().getChildren().map(\.__key) == insertedNodes.map(\.__key))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #3181 > Point.isBefore edge case with mixed TextNode & ElementNode and matching descendants"
    ))
  func pointIsBeforeMixed() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let targetText = try createTextNode("target").setMode("token")
      try getRoot().clear().append(
        paragraph.append(
          createTextNode("a").setMode("token"), createTextNode("b").setMode("token"), targetText))
      let selection = try paragraph.select(2, 2)
      try selection.focus.set(targetText.getKey(), 1, .text)
      #expect(selection.anchor.key == paragraph.getKey() && selection.anchor.offset == 2)
      #expect(selection.anchor.type == .element)
      #expect(selection.focus.key == targetText.getKey() && selection.focus.offset == 1)
      #expect(selection.focus.type == .text)
      let caretRange = try caretRangeFromSelection(selection)
      #expect(
        try comparePointCaretNext(
          getCaretInDirection(caretRange.anchor, .next),
          getCaretInDirection(caretRange.focus, .next)) == -1)
      #expect(try selection.anchor.isBefore(selection.focus))
      #expect(try selection.focus.isBefore(selection.anchor) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression #8067 > Formatting issue when replacing text with format"
    ))
  func replaceTextKeepsFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let firstNode = try createTextNode("hello").toggleFormat("bold")
      try paragraph.append(firstNode, createTextNode(" world!"))
      try getRoot().clear().append(paragraph)
      try selectAll().insertText("hello")
      let child = try #require(try paragraph.getChildren().first as? TextNode)
      #expect(try child.getTextContent() == "hello")
      #expect(try child.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText with backward selection inherits first node format > backward selection across bold+plain inherits bold"
    ))
  func backwardSelectionInheritsFirstFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let boldNode = try createTextNode("hello").toggleFormat("bold")
      let plainNode = try createTextNode(" world")
      try paragraph.append(boldNode, plainNode)
      try getRoot().clear().append(paragraph)
      try plainNode.select(6, 6).setTextNodeRange(plainNode, 6, boldNode, 0).insertText("X")
      let child = try #require(try paragraph.getChildren().first as? TextNode)
      #expect(try child.getTextContent() == "X")
      #expect(try child.hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > token node at middle offset replaces entire node"
    ))
  func tokenMiddleReplaces() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let token = try createTextNode("abcdef").setMode("token")
      let paragraph = try createParagraphNode().append(token)
      try getRoot().clear().append(paragraph)
      try token.select(3, 3).insertText("X")
      let children = try paragraphTexts(paragraph)
      #expect(children.count == 1)
      #expect(try children.first?.getTextContent() == "X")
      #expect(try token.isAttached() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > offset 0 on token reuses insertable previous sibling"
    ))
  func tokenStartReusesPrevious() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let prev = try createTextNode("before")
      let token = try createTextNode("locked").setMode("token")
      try getRoot().clear().append(createParagraphNode().append(prev, token))
      try token.select(0, 0)
      try #require(try getSelection() as? RangeSelection).insertText("X")
      #expect(try prev.getLatest().getTextContent() == "beforeX")
      #expect(try token.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText needsRedirect paths > offset at end of token reuses insertable next sibling"
    ))
  func tokenEndReusesNext() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let token = try createTextNode("locked").setMode("token")
      let next = try createTextNode("after")
      try getRoot().clear().append(createParagraphNode().append(token, next))
      try token.select(6, 6)
      try #require(try getSelection() as? RangeSelection).insertText("X")
      #expect(try next.getLatest().getTextContent() == "Xafter")
      #expect(try token.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertText formatDiffers on empty text node > applies format in-place on empty anchor then splices text"
    ))
  func formatAppliedToEmptyAnchor() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let empty = try createTextNode("")
      let paragraph = try createParagraphNode().append(empty)
      try getRoot().clear().append(paragraph)
      let sel = try empty.select(0, 0)
      sel.format = IS_BOLD
      try sel.insertText("typed")
      #expect(try empty.isAttached())
      #expect(try empty.getLatest().getTextContent() == "typed")
      #expect(try empty.getLatest().hasFormat("bold"))
      #expect(try paragraph.getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::RangeSelection.isBackward() caching (#5825) > caches the result and invalidates on Point mutations"
    ))
  func isBackwardCaching() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let text = try createTextNode("hello")
      try getRoot().clear().append(createParagraphNode().append(text))
      let selection = try text.select(0, 5)
      #expect(selection._cachedIsBackward == nil)
      #expect(try selection.isBackward() == false)
      #expect(selection._cachedIsBackward == false)
      #expect(try selection.isBackward() == false)
      try selection.setTextNodeRange(text, 5, text, 0)
      #expect(selection._cachedIsBackward == nil)
      #expect(try selection.isBackward())
      #expect(selection._cachedIsBackward == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Wraps all inline nodes, preserving first linebreak if contain a block element"
    ))
  func wrapInlineNodesKeepsFirstLineBreak() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear()
      let selection = try selectAll()
      let inlineNodes: [LexicalNode] = try [
        createLineBreakNode(), createTextNode("p1"),
        createTextNode("p1").setFormat("bold" as TextFormatType),
      ]
      try selection.insertNodes(inlineNodes + [createParagraphNode()])
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let first = try #require(children[0] as? ParagraphNode)
      #expect(try first.getChildren().map(\.__key) == inlineNodes.map(\.__key))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Collapses a lone linebreak run into an empty paragraph at the end of a non-empty paragraph"
    ))
  func loneLineBreakBecomesEmptyParagraph() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("abc"))
      try getRoot().clear().append(paragraph)
      try paragraph.selectEnd().insertNodes([
        createLineBreakNode(), createParagraphNode().append(createTextNode("x")),
      ])
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(children.allSatisfy { $0 is ParagraphNode })
      #expect(try children[0].getTextContent() == "abc")
      #expect(try (children[1] as? ParagraphNode)?.getChildrenSize() == 0)
      #expect(try children[2].getTextContent() == "x")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Preserves a linebreak followed by inline content when merging into a non-empty paragraph"
    ))
  func lineBreakWithInlineContentMerges() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("abc"))
      try getRoot().clear().append(paragraph)
      try paragraph.selectEnd().insertNodes([
        createLineBreakNode(), createTextNode("tail"),
        createParagraphNode().append(createTextNode("x")),
      ])
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let first = try #require(children[0] as? ParagraphNode)
      #expect(try first.getChildren().map { $0.getType() } == ["text", "linebreak", "text"])
      #expect(try first.getTextContent() == "abc\ntail")
      #expect(children[1] is ParagraphNode)
      #expect(try children[1].getTextContent() == "x")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$wrapInlineNodes regression > Collapses a lone trailing linebreak after a block into an empty paragraph"
    ))
  func trailingLineBreakBecomesEmptyParagraph() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      try getRoot().clear().append(paragraph)
      try paragraph.selectEnd().insertNodes([
        createParagraphNode().append(createTextNode("x")), createLineBreakNode(),
      ])
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(children.allSatisfy { $0 is ParagraphNode })
      #expect(try children[0].getTextContent() == "x")
      #expect(try (children[1] as? ParagraphNode)?.getChildrenSize() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > uses selection.format (AND) instead of first node for toggle direction"
    ))
  func formatTextUsesSelectionFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (paragraph, selection) = try boldThenPlain()
      #expect(selection.hasFormat("bold") == false)
      try formatText(selection, "bold")
      #expect(try paragraphTexts(paragraph).allSatisfy { try $0.hasFormat("bold") })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > toggling off works when all nodes share the format"
    ))
  func formatTextTogglesOff() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (paragraph, selection) = try boldThenPlain("bbb", bothBold: true)
      selection.format = IS_BOLD
      #expect(selection.hasFormat("bold"))
      try formatText(selection, "bold")
      #expect(try paragraphTexts(paragraph).allSatisfy { try !$0.hasFormat("bold") })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$formatText toggle direction (#6935) > explicit alignWithFormat bypasses selection.format reference"
    ))
  func formatTextExplicitAlign() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (paragraph, selection) = try boldThenPlain()
      try formatText(selection, "bold", IS_BOLD)
      #expect(try paragraphTexts(paragraph).allSatisfy { try $0.hasFormat("bold") })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets bold to true on mixed formatting"
    ))
  func setTextFormatBoldOnMixed() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (paragraph, selection) = try boldThenPlain()
      try setTextFormat(selection, [("bold", true)])
      #expect(try paragraphTexts(paragraph).allSatisfy { try $0.hasFormat("bold") })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets bold to false on all-bold text"
    ))
  func setTextFormatClearsBold() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (paragraph, selection) = try boldThenPlain("bbb", bothBold: true)
      selection.format = IS_BOLD
      try setTextFormat(selection, [("bold", false)])
      #expect(try paragraphTexts(paragraph).allSatisfy { try !$0.hasFormat("bold") })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::$setTextFormat (#5518) > sets multiple formats at once"
    ))
  func setTextFormatMultiple() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("hello"))
      try getRoot().clear().append(paragraph)
      try setTextFormat(selectAll(), [("bold", true), ("italic", true)])
      let node = try #require(try paragraph.getFirstChild() as? TextNode)
      #expect(try node.hasFormat("bold"))
      #expect(try node.hasFormat("italic"))
    }
  }

  /// `$pasteAtStartAndDescribeSelection`.
  private func pasteAtStartAndDescribeSelection(_ blocks: [LexicalNode]) throws -> String {
    let text = try createTextNode("asdf")
    try getRoot().clear().append(createParagraphNode().append(text))
    try text.select(0, 0)
    try #require(try getSelection() as? RangeSelection).insertNodes(blocks)
    let after = try #require(try getSelection() as? RangeSelection)
    #expect(after.isCollapsed())
    return
      "\(after.anchor.type.rawValue):\"\(try after.anchor.getNode().getTextContent())\"@\(after.anchor.offset)"
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertNodes() selection > leaves the caret where pasted content ending in an empty block joins"
    ))
  func pasteEndingInEmptyBlock() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let description = try pasteAtStartAndDescribeSelection([
        createParagraphNode().append(createTextNode("AA")),
        createParagraphNode().append(createTextNode("BB")), createParagraphNode(),
      ])
      #expect(try getRoot().getChildren().map { try $0.getTextContent() } == ["AA", "BB", "asdf"])
      #expect(description == "text:\"asdf\"@0")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::insertNodes() selection > leaves the caret where pasted content ending in a text block joins"
    ))
  func pasteEndingInTextBlock() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let description = try pasteAtStartAndDescribeSelection([
        createParagraphNode().append(createTextNode("AA")),
        createParagraphNode().append(createTextNode("BB")),
      ])
      #expect(try getRoot().getChildren().map { try $0.getTextContent() } == ["AA", "BBasdf"])
      #expect(description == "text:\"BB\"@2")
    }
  }
}
