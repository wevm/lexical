import Lexical
import LexicalExtension
import LexicalOverflow
import LexicalRichText
import LexicalTesting
import Testing

// Ports of packages/lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts
// and LexicalCharacterLimitPlugin.test.tsx. `lexical-react` is not in the
// parity ledger, so these carry the upstream titles in comments instead of
// `.upstream` ids.

private func makeEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    OverflowExtension,
    defineExtension(
      name: "character-limit-test", nodes: { [.klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
  try editor.setRootElement(TestRootElement())
  return editor
}

private func setupLeftRightOverflow() throws -> (OverflowNode, OverflowNode, ParagraphNode) {
  let root = try getRoot()
  try root.clear()
  let paragraph = try createParagraphNode()
  let overflowLeft = try createOverflowNode()
  let overflowRight = try createOverflowNode()
  try root.append(paragraph)
  try paragraph.append(overflowLeft, overflowRight)
  return (overflowLeft, overflowRight, paragraph)
}

private func paragraph(at index: Int) throws -> ParagraphNode {
  try #require(try getRoot().getChildAtIndex(index) as? ParagraphNode)
}

@Suite struct CharacterLimitTests {
  // useCharacterLimit > $mergePrevious > merges left overflow into right with
  // selection in left
  @Test func mergesLeftOverflowIntoRightWithSelectionInLeft() throws {
    let editor = try makeEditor()
    try editor.update {
      let (overflowLeft, overflowRight, paragraph) = try setupLeftRightOverflow()
      let text1 = try createTextNode("1")
      let text2 = try createTextNode("2")
      let text3 = try createTextNode("3")
      let text4 = try createTextNode("4")
      try overflowLeft.append(text1, text2)
      try text2.toggleFormat("bold")
      try overflowRight.append(text3, text4)
      try text4.toggleFormat("bold")
      try text1.select(1, 1)

      try mergePrevious(overflowRight)

      #expect(try paragraph.getChildrenSize() == 1)
      #expect(isOverflowNode(try paragraph.getFirstChild()))
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == text1.getKey())
      #expect(selection.anchor.offset == 1)
      #expect(selection.focus.key == text1.getKey())
      #expect(selection.focus.offset == 1)
    }
  }

  // useCharacterLimit > $mergePrevious > merges with selection spanning both
  // overflows
  @Test func mergesWithSelectionSpanningBothOverflows() throws {
    let editor = try makeEditor()
    try editor.update {
      let (overflowLeft, overflowRight, paragraph) = try setupLeftRightOverflow()
      let text1 = try createTextNode("1")
      let text2 = try createTextNode("2")
      let text3 = try createTextNode("3")
      let text4 = try createTextNode("4")
      try overflowLeft.append(text1, text2)
      try text2.toggleFormat("bold")
      try overflowRight.append(text3, text4)
      try text4.toggleFormat("bold")
      let selection = try text2.select(0, 0)
      try selection.focus.set(text3.getKey(), 1, .text)

      try mergePrevious(overflowRight)

      #expect(try paragraph.getChildrenSize() == 1)
      let updatedSelection = try #require(try getSelection() as? RangeSelection)
      #expect(updatedSelection.anchor.key == text2.getKey())
      #expect(updatedSelection.anchor.offset == 0)
      #expect(updatedSelection.focus.key == text3.getKey())
      #expect(updatedSelection.focus.offset == 1)
    }
  }

  // useCharacterLimit > $wrapOverflowedNodes > merges adjacent overflow nodes
  // when wrapping before existing overflow
  @Test func mergesAdjacentOverflowNodesWhenWrappingBeforeExistingOverflow() throws {
    let editor = try makeEditor()
    try editor.update {
      let root = try getRoot()
      try root.clear()
      let overflow = try createOverflowNode()
      try overflow.append(createTextNode("f"))
      try root.append(createParagraphNode().append(createTextNode("abcde"), overflow))
    }
    try editor.update {
      if let text = try paragraph(at: 0).getFirstChild() as? TextNode {
        try text.setTextContent("abcdge")
      }
    }
    try editor.update { try wrapOverflowedNodes(5) }
    try editor.read {
      let children = try paragraph(at: 0).getChildren()
      #expect(children.count == 2)
      #expect(try children[0].getTextContent() == "abcdg")
      #expect(isOverflowNode(children[1]))
      #expect(try children[1].getTextContent() == "ef")
    }
  }

  // useCharacterLimit > $wrapOverflowedNodes > counts \n\n separators between
  // paragraphs (#6329)
  @Test func countsSeparatorsBetweenParagraphs() throws {
    let editor = try makeEditor()
    try editor.update {
      let root = try getRoot()
      try root.clear()
      try root.append(
        createParagraphNode().append(createTextNode("123")),
        createParagraphNode().append(createTextNode("456")))
    }
    // "123\n\n456": "123" (3) and the separator (2) fill 5, so all of the
    // second paragraph overflows.
    try editor.update { try wrapOverflowedNodes(5) }
    try editor.read {
      let p1 = try paragraph(at: 0)
      let p2 = try paragraph(at: 1)
      #expect(try p1.getTextContent() == "123")
      #expect(!isOverflowNode(try p1.getFirstChild()))
      let overflow = try #require(try p2.getFirstChild() as? OverflowNode)
      #expect(try overflow.getTextContent() == "456")
    }
  }

  // useCharacterLimit > $wrapOverflowedNodes > splits text correctly with
  // separator budget (#6329)
  @Test func splitsTextWithSeparatorBudget() throws {
    let editor = try makeEditor()
    try editor.update {
      let root = try getRoot()
      try root.clear()
      try root.append(
        createParagraphNode().append(createTextNode("12")),
        createParagraphNode().append(createTextNode("3456")))
    }
    // "12\n\n3456": "12" (2), the separator (2) and "3" (1) fill 5.
    try editor.update { try wrapOverflowedNodes(5) }
    try editor.read {
      let children = try paragraph(at: 1).getChildren()
      #expect(children.count == 2)
      #expect(try children[0].getTextContent() == "3")
      let overflow = try #require(children[1] as? OverflowNode)
      #expect(try overflow.getTextContent() == "456")
    }
  }

  // useCharacterLimit > $wrapOverflowedNodes > counts multiple separators
  // across three paragraphs (#6329)
  @Test func countsSeparatorsAcrossThreeParagraphs() throws {
    let editor = try makeEditor()
    try editor.update {
      let root = try getRoot()
      try root.clear()
      try root.append(
        createParagraphNode().append(createTextNode("abc")),
        createParagraphNode().append(createTextNode("de")),
        createParagraphNode().append(createTextNode("fgh")))
    }
    // "abc\n\nde\n\nfgh": 3 + 2 + 2 + 2 fill 9, so the third paragraph
    // overflows.
    try editor.update { try wrapOverflowedNodes(9) }
    try editor.read {
      let p1 = try paragraph(at: 0)
      let p2 = try paragraph(at: 1)
      #expect(try p1.getTextContent() == "abc")
      #expect(!isOverflowNode(try p1.getFirstChild()))
      #expect(try p2.getTextContent() == "de")
      #expect(!isOverflowNode(try p2.getFirstChild()))
      let overflow = try #require(try paragraph(at: 2).getFirstChild() as? OverflowNode)
      #expect(try overflow.getTextContent() == "fgh")
    }
  }

  // useCharacterLimit > $wrapOverflowedNodes > advances the budget past a
  // slotted decorator so the wrap boundary is exact
  @Test func advancesTheBudgetPastASlottedDecorator() throws {
    let editor = try makeEditor()
    try editor.update {
      let root = try getRoot()
      try root.clear()
      let host = try createParagraphNode()
      try host.append(createTextNode("12345"))
      try root.append(host)
      try setSlot(host, "media", createTestDecoratorNode().setIsInline(false))
    }
    try editor.update { try wrapOverflowedNodes(13) }
    try editor.read {
      let children = try paragraph(at: 0).getChildren()
      #expect(try children[0].getTextContent() == "12")
      #expect(isOverflowNode(children[1]))
      #expect(try children[1].getTextContent() == "345")
    }
  }
}

/// `CharacterLimitPlugin` on an editor that already holds `text`, as when
/// the setting is turned on: the characters left it reports, and the editor.
private func mountWithText(
  _ text: String, strlen: @escaping (String) -> Int = { $0.utf16.count }
) throws -> (remaining: Int?, editor: Editor) {
  let editor = try buildEditorFromExtensions(
    RichTextExtension, OverflowExtension,
    defineExtension(
      name: "character-limit",
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
  try editor.setRootElement(TestRootElement())
  try editor.update(discrete: true) {
    try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
  }
  var remaining: Int?
  _ = try registerCharacterLimit(
    editor, maxCharacters: 5, strlen: strlen, remainingCharacters: { remaining = $0 })
  try editor.read {}
  return (remaining, editor)
}

private func overflowNodeCount(_ editor: Editor) throws -> Int {
  try editor.read {
    try getRoot().getAllTextNodes().filter { try $0.getParent() is OverflowNode }.count
  }
}

@Suite struct CharacterLimitPluginTests {
  // CharacterLimitPlugin > counts the text that is already in the editor when
  // it mounts
  @Test func countsTheTextAlreadyInTheEditor() throws {
    let (remaining, editor) = try mountWithText("hello world")
    #expect(remaining == -6)
    #expect(try overflowNodeCount(editor) > 0)
  }

  // CharacterLimitPlugin > reports a full budget for an editor that is under
  // the limit
  @Test func reportsAFullBudgetUnderTheLimit() throws {
    let (remaining, editor) = try mountWithText("hi")
    #expect(remaining == 3)
    #expect(try overflowNodeCount(editor) == 0)
  }

  // CharacterLimitPlugin > counts in the charset it was given
  @Test func countsInTheCharsetItWasGiven() throws {
    // Three 3-byte characters: 3 UTF-16 code units, 9 UTF-8 bytes.
    let (remaining, _) = try mountWithText("一二三", strlen: { $0.utf8.count })
    #expect(remaining == -4)
  }

}
