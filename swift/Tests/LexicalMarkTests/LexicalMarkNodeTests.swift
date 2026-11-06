import Lexical
import LexicalHTML
import LexicalMark
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-mark/__tests__/unit/LexicalMarkNode.test.ts.

/// The test editor with a paragraph in its root, as the suite's
/// `beforeEach` leaves it.
private func markEditor(clearOnly: Bool = false) throws -> Editor {
  let editor = try createMountedTestEditor(nodes: [.klass(MarkNode.self)])
  try editor.update(
    {
      let root = try getRoot().clear()
      if !clearOnly { try root.append(createParagraphNode()) }
    }, EditorUpdateOptions(discrete: true))
  return editor
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChild() as? ParagraphNode)
}

@Suite struct LexicalMarkNodeTests {
  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > wraps a whole text node"
    ))
  func wrapsAWholeTextNode() throws {
    let editor = try markEditor()
    try editor.update {
      let textNode = try createTextNode("marked")
      let paragraphNode = try firstParagraph()
      try paragraphNode.append(textNode)
      let selection = createRangeSelection()
      try selection.anchor.set(textNode.getKey(), 0, .text)
      try selection.focus.set(textNode.getKey(), try textNode.getTextContent().utf16.count, .text)
      try wrapSelectionInMarkNode(selection, false, "my-id")

      #expect(try paragraphNode.getChildren().count == 1)
      let markNode = try #require(try paragraphNode.getFirstChild() as? MarkNode)
      #expect(try markNode.getType() == "mark")
      #expect(try markNode.getIDs() == ["my-id"])
      #expect(try markNode.getChildren().count == 1)
      #expect(try markNode.getFirstChildOrThrow().getKey() == textNode.getKey())
      #expect(try markNode.getFirstChildOrThrow().getTextContent() == "marked")
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > splits a text node if the selection is not at the start/end"
    ))
  func splitsATextNode() throws {
    let editor = try markEditor()
    try editor.update {
      let textNode = try createTextNode("unmarked marked unmarked")
      let paragraphNode = try firstParagraph()
      try paragraphNode.append(textNode)
      let selection = createRangeSelection()
      try selection.anchor.set(textNode.getKey(), "unmarked ".utf16.count, .text)
      try selection.focus.set(textNode.getKey(), "unmarked marked".utf16.count, .text)
      try wrapSelectionInMarkNode(selection, false, "my-id")

      #expect(try paragraphNode.getTextContent() == "unmarked marked unmarked")
      #expect(try paragraphNode.getChildren().map { try $0.getType() } == ["text", "mark", "text"])
      #expect(
        try paragraphNode.getChildren().map { try $0.getTextContent() } == [
          "unmarked ", "marked", " unmarked",
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > includes inline decorator nodes"
    ))
  func includesInlineDecoratorNodes() throws {
    let editor = try markEditor()
    try editor.update {
      let decoratorNode = try createTestDecoratorNode()
      let textNode = try createTextNode("more text")
      let paragraphNode = try firstParagraph()
      try paragraphNode.append(decoratorNode, textNode)
      let selection = createRangeSelection()
      try selection.anchor.set(paragraphNode.getKey(), 0, .element)
      try selection.focus.set(paragraphNode.getKey(), try paragraphNode.getChildrenSize(), .element)
      try wrapSelectionInMarkNode(selection, false, "my-id")

      #expect(try paragraphNode.getChildren().count == 1)
      let markNode = try #require(try paragraphNode.getFirstChild() as? MarkNode)
      #expect(try markNode.getType() == "mark")
      #expect(
        try markNode.getChildren().map { $0.getKey() } == [
          decoratorNode.getKey(), textNode.getKey(),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > keeps decorator nodes inside an existing mark"
    ))
  func keepsDecoratorNodesInsideAnExistingMark() throws {
    let editor = try markEditor()
    try editor.update {
      let paragraphNode = try firstParagraph()
      let textNode = try createTextNode("aaa x^2 bbb")
      try paragraphNode.append(textNode)

      let selection = try textNode.select(0)
      try wrapSelectionInMarkNode(selection, false, "comment-id")

      let markNode = try #require(try paragraphNode.getFirstChildOrThrow() as? MarkNode)
      let markedTextNode = try #require(try markNode.getFirstChildOrThrow() as? TextNode)
      let equationSelection = try markedTextNode.select(
        "aaa ".utf16.count, "aaa x^2".utf16.count)
      try setSelection(equationSelection)

      try insertNodeIntoLeaf(createTestDecoratorNode())

      #expect(try paragraphNode.getChildren().map { try $0.getType() } == ["mark"])
      let updatedMarkNode = try #require(try paragraphNode.getFirstChildOrThrow() as? MarkNode)
      #expect(try updatedMarkNode.getIDs() == ["comment-id"])
      let markChildren = try updatedMarkNode.getChildren()
      #expect(markChildren.count == 3)
      #expect(try markChildren[0].getTextContent() == "aaa ")
      #expect(try markChildren[1].getType() == "test_decorator")
      #expect(try markChildren[2].getTextContent() == " bbb")
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > includes inline element nodes"
    ))
  func includesInlineElementNodes() throws {
    let editor = try markEditor()
    try editor.update {
      let elementNode = try createTestInlineElementNode()
      let textNode = try createTextNode("more text")
      let paragraphNode = try firstParagraph()
      try paragraphNode.append(elementNode, textNode)
      let selection = createRangeSelection()
      try selection.anchor.set(paragraphNode.getKey(), 0, .element)
      try selection.focus.set(paragraphNode.getKey(), try paragraphNode.getChildrenSize(), .element)
      try wrapSelectionInMarkNode(selection, false, "my-id")

      #expect(try paragraphNode.getChildren().count == 1)
      let markNode = try #require(try paragraphNode.getFirstChild() as? MarkNode)
      #expect(try markNode.getType() == "mark")
      #expect(
        try markNode.getChildren().map { $0.getKey() } == [elementNode.getKey(), textNode.getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > does not include block element nodes"
    ))
  func doesNotIncludeBlockElementNodes() throws {
    let editor = try markEditor()
    try editor.update {
      let elementNode = try createTestElementNode()
      let textNode = try createTextNode("more text")
      let paragraphNode = try firstParagraph()
      try paragraphNode.append(elementNode, textNode)
      let selection = createRangeSelection()
      try selection.anchor.set(paragraphNode.getKey(), 0, .element)
      try selection.focus.set(paragraphNode.getKey(), try paragraphNode.getChildrenSize(), .element)
      try wrapSelectionInMarkNode(selection, false, "my-id")

      #expect(try paragraphNode.getChildren().count == 2)
      #expect(try paragraphNode.getChildAtIndex(0)?.getKey() == elementNode.getKey())

      // the text part of the selection should still be marked
      let markNode = try #require(try paragraphNode.getChildAtIndex(1) as? MarkNode)
      #expect(try markNode.getType() == "mark")
      #expect(try markNode.getChildren().count == 1)
      #expect(try markNode.getTextContent() == "more text")
    }
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $generateNodesFromDOM > retains spaces around mark elements"
    ))
  func retainsSpacesAroundMarkElements() throws {
    let editor = try markEditor(clearOnly: true)
    try editor.update {
      let dom = DOMParser().parseFromString(
        "<html><body><p><span>Foo </span><mark>Bar</mark><span> !</span></p></body></html>",
        "text/html")
      let nodes = try generateNodesFromDOM(editor, dom)

      #expect(nodes.count == 1)
      let paragraphNode = try #require(nodes[0] as? ElementNode)
      #expect(try paragraphNode.getChildren().count == 3)
      // The <mark> element is imported as plain text in this test, so all
      // three children are TextNodes.
      let textNode1 = try #require(try paragraphNode.getChildAtIndex(0) as? TextNode)
      let markNode = try #require(try paragraphNode.getChildAtIndex(1) as? TextNode)
      let textNode2 = try #require(try paragraphNode.getChildAtIndex(2) as? TextNode)

      #expect(try textNode1.getTextContent() == "Foo ")
      #expect(try markNode.getTextContent() == "Bar")
      #expect(try textNode2.getTextContent() == " !")
    }
  }
}

/// `MarkNode overlap theme class`. JS reads the class off the element the
/// reconciler made; here `createDOM` and `updateDOM` run on one element as
/// the reconciler calls them.
@Suite struct MarkNodeOverlapThemeClassTests {
  private let config = EditorConfig(
    namespace: "test", theme: ["mark": "mk", "markOverlap": "mk-overlap"])

  private func markElement(_ ids: [String], _ nextIDs: [String]? = nil) throws -> DOMElement {
    let editor = try createMountedTestEditor(
      nodes: [.klass(MarkNode.self)], theme: ["mark": "mk", "markOverlap": "mk-overlap"])
    var element: DOMElement? = nil
    var key = ""
    try editor.update(
      {
        let markNode = try createMarkNode(ids)
        try markNode.append(createTextNode("marked"))
        key = markNode.getKey()
        try getRoot().clear().append(createParagraphNode().append(markNode))
        element = try markNode.createDOM(config)
      }, EditorUpdateOptions(discrete: true))
    if let nextIDs {
      try editor.update(
        {
          let prev = try #require(try getNodeByKey(key) as? MarkNode)
          let next = try prev.setIDs(nextIDs)
          _ = try next.updateDOM(prev, element!, config)
        }, EditorUpdateOptions(discrete: true))
    }
    return try #require(element)
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::MarkNode overlap theme class > createDOM adds the overlap class for more than one id"
    ))
  func createDOMAddsTheOverlapClass() throws {
    #expect(try markElement(["a", "b", "c"]).className.contains("mk-overlap"))
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::MarkNode overlap theme class > updateDOM adds the overlap class when going from 1 to 3 ids"
    ))
  func updateDOMAddsTheOverlapClass() throws {
    let element = try markElement(["a"], ["a", "b", "c"])
    #expect(element.className.contains("mk-overlap"))
    #expect(element.className.contains("mk"))
  }

  @Test(
    .upstream(
      "lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::MarkNode overlap theme class > updateDOM removes the overlap class when going from 2 to 0 ids"
    ))
  func updateDOMRemovesTheOverlapClass() throws {
    let element = try markElement(["a", "b"], [])
    #expect(!element.className.contains("mk-overlap"))
    #expect(element.className.contains("mk"))
  }
}
