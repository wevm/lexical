import Lexical
import LexicalTesting
import Testing

// Port of the `getNodes() and extract()` suite of
// __tests__/unit/LexicalSelection.test.ts. TestLinkNode stands in for
// LinkNode and TestElementNode for the list and its items, which the suite
// uses only for structure. Nodes are compared by key, as `toEqual` on the
// latest versions does.

/// The document of the suite.
private struct Document {
  let paragraphNode: ParagraphNode
  let paragraphText: TextNode
  let linkNode: TestLinkNode
  let linkText: TextNode
  let listNode: TestElementNode
  let listItemText1: TextNode
  let listItemText2: TextNode
  let listItem1: TestElementNode
  let listItem2: TestElementNode
  let emptyParagraph: ParagraphNode

  init() throws {
    paragraphText = try createTextNode("paragraph text")
    linkText = try createTextNode("link text")
    linkNode = try createTestLinkNode()
    paragraphNode = try createParagraphNode()
    listItemText1 = try createTextNode("item 1")
    listItemText2 = try createTextNode("item 2")
    listItem1 = try createTestElementNode()
    listItem2 = try createTestElementNode()
    listNode = try createTestElementNode()
    emptyParagraph = try createParagraphNode()
    try getRoot().clear().append(
      paragraphNode.append(paragraphText, linkNode.append(linkText)),
      listNode.append(listItem1.append(listItemText1), listItem2.append(listItemText2)),
      emptyParagraph)
  }

  /// Every node in document order, as `getNodes()` lists a full selection.
  var all: [LexicalNode] {
    [
      paragraphText, linkNode, linkText, paragraphNode, listNode, listItem1, listItemText1,
      listItem2, listItemText2, emptyParagraph,
    ]
  }
}

private func documentEditor() throws -> (Editor, Document) {
  let editor = try createTestEditor()
  var document: Document!
  try update(editor) { document = try Document() }
  return (editor, document)
}

private func keys(_ nodes: [LexicalNode]) -> [NodeKey] { nodes.map(\.__key) }

private func expectPoint(_ point: Point, _ key: NodeKey, _ offset: Int, _ type: PointType) {
  #expect(point.key == key)
  #expect(point.offset == offset)
  #expect(point.type == type)
}

@Suite struct LexicalSelectionGetNodesTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with test document"
    ))
  func selectAllTestDocument() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = try selectAll()
      expectPoint(selection.anchor, d.paragraphText.getKey(), 0, .text)
      expectPoint(selection.focus, d.emptyParagraph.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == keys(d.all))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with leading inline decorator"
    ))
  func selectAllLeadingInlineDecorator() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode()
      try d.paragraphNode.splice(0, 0, [decorator])
      let selection = try selectAll()
      expectPoint(selection.anchor, d.paragraphNode.getKey(), 0, .element)
      expectPoint(selection.focus, d.emptyParagraph.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == keys([decorator] + d.all))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with trailing inline decorator"
    ))
  func selectAllTrailingInlineDecorator() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode()
      try d.emptyParagraph.splice(0, 0, [decorator])
      let selection = try selectAll()
      expectPoint(selection.anchor, d.paragraphText.getKey(), 0, .text)
      expectPoint(selection.focus, d.emptyParagraph.getKey(), 1, .element)
      #expect(try keys(selection.getNodes()) == keys(d.all + [decorator]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with leading empty inline element"
    ))
  func selectAllLeadingEmptyInlineElement() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let inline = try createTestInlineElementNode()
      try d.paragraphNode.splice(0, 0, [inline])
      let selection = try selectAll()
      expectPoint(selection.anchor, inline.getKey(), 0, .element)
      expectPoint(selection.focus, d.emptyParagraph.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == keys([inline] + d.all))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > with trailing empty inline element"
    ))
  func selectAllTrailingEmptyInlineElement() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let inline = try createTestInlineElementNode()
      try d.emptyParagraph.splice(0, 0, [inline])
      let selection = try selectAll()
      expectPoint(selection.anchor, d.paragraphText.getKey(), 0, .text)
      expectPoint(selection.focus, inline.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == keys(d.all + [inline]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > $selectAll() > after removing empty paragraph"
    ))
  func selectAllAfterRemovingEmptyParagraph() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      try d.emptyParagraph.remove()
      let selection = try selectAll()
      expectPoint(selection.anchor, d.paragraphText.getKey(), 0, .text)
      expectPoint(
        selection.focus, d.listItemText2.getKey(), try d.listItemText2.getTextContentSize(), .text)
      #expect(try keys(selection.getNodes()) == keys(Array(d.all.dropLast())))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select all without normalization"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > Manual select all without normalization"
    ))
  func manualSelectAll() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set("root", 0, .element)
      try selection.focus.set("root", getRoot().getChildrenSize(), .element)
      #expect(try keys(selection.getNodes()) == keys(d.all))
      let extracted = try selection.extract()
      #expect(keys(extracted) == keys(d.all))
      #expect(try keys(selection.getNodes()) == keys(extracted))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select all from first text to last empty paragraph"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > Manual select all from first text to last empty paragraph"
    ))
  func manualSelectTextToEmptyParagraph() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(d.paragraphText.getKey(), 0, .text)
      try selection.focus.set(d.emptyParagraph.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == keys(d.all))
      let extracted = try selection.extract()
      #expect(keys(extracted) == keys(d.all))
      #expect(try keys(selection.getNodes()) == keys(extracted))
    }
  }

  /// Two inline decorators in the empty paragraph.
  private func twoDecorators(_ d: Document) throws -> (TestDecoratorNode, TestDecoratorNode) {
    let leading = try createTestDecoratorNode()
    let trailing = try createTestDecoratorNode()
    try d.emptyParagraph.splice(0, 0, [leading, trailing])
    return (leading, trailing)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus collapsed between inline decorators"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus collapsed after inline decorator"
    ),
    arguments: [1, 2])
  func collapsedAmongInlineDecorators(_ offset: Int) throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let (_, trailing) = try twoDecorators(d)
      let key = d.emptyParagraph.getKey()
      let selection = createRangeSelection()
      try selection.anchor.set(key, offset, .element)
      try selection.focus.set(key, offset, .element)
      #expect(selection.isCollapsed())
      expectPoint(selection.anchor, key, offset, .element)
      expectPoint(selection.focus, key, offset, .element)
      #expect(try keys(selection.getNodes()) == [trailing.getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > Manual select with focus between inline decorators"
    ))
  func focusBetweenInlineDecorators() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let (leading, _) = try twoDecorators(d)
      let selection = createRangeSelection()
      try selection.anchor.set(d.paragraphText.getKey(), 0, .text)
      try selection.focus.set(d.emptyParagraph.getKey(), 1, .element)
      expectPoint(selection.anchor, d.paragraphText.getKey(), 0, .text)
      expectPoint(selection.focus, d.emptyParagraph.getKey(), 1, .element)
      #expect(try keys(selection.getNodes()) == keys(d.all + [leading]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select only the paragraph (not normalized)"
    ))
  func selectOnlyParagraph() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let size = try d.paragraphNode.getChildrenSize()
      let selection = try d.paragraphNode.select(0, size)
      expectPoint(selection.anchor, d.paragraphNode.getKey(), 0, .element)
      expectPoint(selection.focus, d.paragraphNode.getKey(), size, .element)
      #expect(try keys(selection.getNodes()) == keys([d.paragraphText, d.linkNode, d.linkText]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select around the paragraph (not normalized)"
    ))
  func selectAroundParagraph() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let index = try d.paragraphNode.getIndexWithinParent()
      let selection = createRangeSelection()
      try selection.anchor.set("root", index, .element)
      try selection.focus.set("root", index + 1, .element)
      expectPoint(selection.anchor, "root", 0, .element)
      expectPoint(selection.focus, "root", 1, .element)
      #expect(
        try keys(selection.getNodes())
          == keys([d.paragraphText, d.linkNode, d.linkText, d.paragraphNode]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > selection collapsed inside an empty element"
    ))
  func collapsedInEmptyElement() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(d.emptyParagraph.getKey(), 0, .element)
      try selection.focus.set(d.emptyParagraph.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == [d.emptyParagraph.getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > getNodes() > select an empty ListItemNode (collapsed)"
    ))
  func collapsedInEmptyListItem() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let emptyListItem = try createTestElementNode()
      try d.listItem2.insertBefore(emptyListItem)
      let selection = createRangeSelection()
      try selection.anchor.set(emptyListItem.getKey(), 0, .element)
      try selection.focus.set(emptyListItem.getKey(), 0, .element)
      expectPoint(selection.anchor, emptyListItem.getKey(), 0, .element)
      expectPoint(selection.focus, emptyListItem.getKey(), 0, .element)
      #expect(try keys(selection.getNodes()) == [emptyListItem.getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select partial TextNode extracts paragraph text"
    ))
  func extractPartialParagraphText() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = try d.paragraphText.select(2, 8)
      let extracted = try selection.extract()
      #expect(try extracted.map { try $0.getTextContent() } == ["ragrap"])
      #expect(try keys(selection.getNodes()) == keys(extracted))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select partial TextNode extracts link text"
    ))
  func extractPartialLinkText() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = try d.linkText.select(1, 4)
      let extracted = try selection.extract()
      #expect(try extracted.map { try $0.getTextContent() } == ["ink"])
      #expect(try keys(selection.getNodes()) == keys(extracted))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select multiple partial TextNode extracts text"
    ))
  func extractMultiplePartialText() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.setTextNodeRange(d.paragraphText, 10, d.linkText, 4)
      let extracted = try selection.extract()
      try #require(extracted.count == 3)
      #expect(try extracted[0] is TextNode && extracted[0].getTextContent() == "text")
      #expect(extracted[1].getKey() == d.linkNode.getKey())
      #expect(try extracted[2] is TextNode && extracted[2].getTextContent() == "link")
      #expect(try keys(selection.getNodes()) == keys(extracted))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select last offset TextNode as first node removes node"
    ))
  func extractFromLastOffset() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.setTextNodeRange(
        d.paragraphText, d.paragraphText.getTextContentSize(), d.linkText, 4)
      let beforeNodes = try selection.getNodes()
      let extracted = try selection.extract()
      try #require(extracted.count == 2)
      #expect(extracted[0].getKey() == d.linkNode.getKey())
      #expect(try extracted[1].getTextContent() == "link")
      #expect(try keys(selection.getNodes()) == keys(beforeNodes))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::getNodes() and extract() > extract() > select 0 offset TextNode as last node removes node"
    ))
  func extractToZeroOffset() throws {
    let (editor, d) = try documentEditor()
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.setTextNodeRange(d.paragraphText, 4, d.linkText, 0)
      let beforeNodes = try selection.getNodes()
      let extracted = try selection.extract()
      try #require(extracted.count == 2)
      #expect(try extracted[0].getTextContent() == "graph text")
      #expect(extracted[1].getKey() == d.linkNode.getKey())
      let next = try #require(try d.paragraphText.getNextSibling())
      #expect(
        try keys(selection.getNodes()) == [next.getKey()] + keys(Array(beforeNodes.dropFirst())))
    }
  }
}
