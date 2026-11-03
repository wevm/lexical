import Lexical
import LexicalTesting
import Testing

// Port of the composition and regression suites of
// __tests__/unit/LexicalSelection.test.ts. TestElementNode stands in for
// QuoteNode, a block that holds text.

nonisolated(unsafe) private let noteState = createState("note", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let otherState = createState("other", parse: { $0?.stringValue ?? "" })

/// `ReplacementTextNode` of the NodeState replacement test.
final class ReplacementTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ReplacementTextNode.self, type: "test-replacement-text", extends: TextNode.self)
  }
}

/// `InlineElementNode` of the #6701 test.
final class InlineElementNode6701: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineElementNode6701.self, type: "inline-element-node", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
}

private func segmentedEditor(_ text: String = "JohnSmith") throws -> (Editor, TextNode) {
  let editor = try createTestEditor()
  var segmented: TextNode!
  try editor.update(discrete: true) {
    segmented = try createTextNode(text).setMode("segmented")
    try getRoot().clear().append(createParagraphNode().append(segmented))
  }
  return (editor, segmented)
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

@Suite struct LexicalSelectionCompositionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText during composition preserves node key"
    ))
  func compositionPreservesKey() throws {
    let (editor, segmented) = try segmentedEditor()
    try editor.update(discrete: true) {
      let key = segmented.getKey()
      try setCompositionKey(key)
      try segmented.select(4, 4).insertText("")
      let latest = try segmented.getLatest()
      #expect(latest.getKey() == key)
      #expect(try latest.isSegmented() == false)
      #expect(try latest.getTextContent() == "JohnSmith")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition replaces segmented node"
    ))
  func noCompositionReplacesSegmented() throws {
    let (editor, segmented) = try segmentedEditor()
    try editor.update(discrete: true) {
      try segmented.select(4, 4).insertText("")
      #expect(try segmented.isAttached() == false)
      let allText = try getRoot().getAllTextNodes()
      #expect(allText.count == 1)
      #expect(allText[0].getKey() != segmented.getKey())
      #expect(try allText[0].getTextContent() == "JohnSmith")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition types into a segmented node"
    ))
  func noCompositionTypesIntoSegmented() throws {
    let (editor, segmented) = try segmentedEditor()
    try editor.update(discrete: true) {
      try segmented.select(4, 4).insertText("X")
      #expect(try segmented.isAttached() == false)
      let allText = try getRoot().getAllTextNodes()
      #expect(allText.count == 1)
      #expect(try allText[0].isSegmented() == false)
      #expect(try allText[0].getTextContent() == "JohnXSmith")
      let selection = try rangeSelection()
      #expect(selection.isCollapsed())
      #expect(selection.anchor.key == allText[0].getKey())
      #expect(selection.anchor.offset == 5)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText without composition keeps the NodeState and detail of the segmented node"
    ))
  func noCompositionKeepsStateAndDetail() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let segmented = try setState(
        createTextNode("JohnSmith").setMode("segmented").toggleUnmergeable(), noteState, "c1")
      try getRoot().clear().append(createParagraphNode().append(segmented))
      try segmented.select(4, 4).insertText("X")
      let allText = try getRoot().getAllTextNodes()
      #expect(allText.count == 1)
      #expect(try allText[0].isSegmented() == false)
      #expect(try allText[0].getTextContent() == "JohnXSmith")
      #expect(try getState(allText[0], noteState) == "c1")
      #expect(try allText[0].isUnmergeable())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > a TextNode replacement keeps the NodeState it sets, and takes the rest"
    ))
  func replacementKeepsItsOwnState() throws {
    let editor = try createTestEditor(nodes: [
      .klass(ReplacementTextNode.self),
      .replacement(
        LexicalNodeReplacement(
          replace: TextNode.self,
          with: { node in
            try setState(
              create(ReplacementTextNode.self).setTextContent(node.getTextContent()), noteState,
              "replacement")
          },
          withKlass: ReplacementTextNode.self)),
    ])
    try editor.update(discrete: true) {
      let segmented = try setState(
        setState(
          create(ReplacementTextNode.self).setTextContent("JohnSmith").setMode("segmented"),
          noteState, "c1"),
        otherState, "o1")
      try getRoot().clear().append(createParagraphNode().append(segmented))
      try segmented.select(4, 4).insertText("X")
      let text = try getRoot().getAllTextNodes()[0]
      #expect(try text.getTextContent() == "JohnXSmith")
      #expect(try getState(text, noteState) == "replacement")
      #expect(try getState(text, otherState) == "o1")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > removing text from inside a segmented node to the next block keeps its NodeState and detail"
    ))
  func removeFromSegmentedKeepsState() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let segmented = try setState(
        createTextNode("$AB").setMode("segmented").toggleUnmergeable(), noteState, "c1")
      let next = try createTextNode("z")
      try getRoot().clear().append(
        createParagraphNode().append(segmented), createParagraphNode().append(next))
      let selection = createRangeSelection()
      try selection.anchor.set(segmented.getKey(), 2, .text)
      try selection.focus.set(next.getKey(), 0, .text)
      try setSelection(selection)
      try selection.removeText()
      let nodes = try getRoot().getAllTextNodes()
      #expect(try getRoot().getTextContent() == "$Az")
      let kept = try #require(try nodes.first { try $0.getTextContent().hasPrefix("$A") })
      #expect(try getState(kept, noteState) == "c1")
      #expect(try kept.isUnmergeable())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Segmented node composition (#5065) > insertText during composition preserves format and style"
    ))
  func compositionPreservesFormat() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let segmented = try createTextNode("JohnSmith").setMode("segmented").setFormat(
        "bold" as TextFormatType)
      try getRoot().clear().append(createParagraphNode().append(segmented))
      let key = segmented.getKey()
      try setCompositionKey(key)
      let sel = try segmented.select(4, 4)
      sel.format = 1
      try sel.insertText("")
      let latest = try segmented.getLatest()
      #expect(try latest.isAttached())
      #expect(latest.getKey() == key)
      #expect(try latest.getFormat() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Non-collapsed selection + composition preserves node identity > spliceText into existing node instead of creating new one"
    ))
  func compositionSplicesExistingNode() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let textNode = try createTextNode("Hello World")
      try getRoot().clear().append(createParagraphNode().append(textNode))
      let key = textNode.getKey()
      try setCompositionKey(key)
      try textNode.select(5, 11).insertText("!")
      let latest = try textNode.getLatest()
      #expect(latest.getKey() == key)
      #expect(try latest.getTextContent() == "Hello!")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Non-collapsed selection + composition preserves node identity > without composition creates a new node"
    ))
  func noCompositionCreatesNode() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let textNode = try createTextNode("Hello World")
      try getRoot().clear().append(createParagraphNode().append(textNode))
      let key = textNode.getKey()
      try textNode.select(5, 11).insertText("!")
      let allText = try getRoot().getAllTextNodes()
      #expect(allText.count == 2)
      #expect(try allText[0].getTextContent() == "Hello")
      #expect(try allText[1].getTextContent() == "!")
      #expect(allText[1].getKey() != key)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #6701 > insertNodes fails an invariant when there is no Block ancestor"
    ))
  func insertNodesWithoutBlockAncestor() throws {
    let editor = try createTestEditor(nodes: [.klass(InlineElementNode6701.self)])
    let error = #expect(throws: LexicalError.self) {
      try editor.update(discrete: true) {
        let textNode = try createTextNode("test")
        try getRoot().clear().append(InlineElementNode6701().append(textNode))
        try textNode.select().insertNodes([createTextNode("more text")])
      }
    }
    #expect(
      error?.message.contains(
        "Expected node TextNode of type text to have a block ElementNode ancestor")
        == true)
  }
}

/// A shadow root holding `children`, selected at `offset`.
private func shadowEditor(
  _ children: @escaping () throws -> [LexicalNode], select offset: Int
) throws -> (Editor, NodeKey) {
  let editor = try createTestEditor()
  var shadowKey = ""
  try editor.update(discrete: true) {
    let shadow = try createTestShadowRootNode()
    try shadow.append(contentsOf: children())
    try getRoot().clear().append(shadow)
    shadowKey = shadow.getKey()
    try shadow.select(offset, offset)
  }
  return (editor, shadowKey)
}

private func blockDecorator() throws -> TestDecoratorNode {
  try createTestDecoratorNode().setIsInline(false)
}

@Suite struct LexicalSelectionShadowRootTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator after the block cursor at the end of a shadow root"
    ))
  func blockDecoratorAtEndOfShadowRoot() throws {
    let (editor, shadowKey) = try shadowEditor({ [try blockDecorator()] }, select: 1)
    try editor.update(discrete: true) { try rangeSelection().insertNodes([blockDecorator()]) }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      let shadow = try #require(try root.getFirstChild() as? TestShadowRootNode)
      #expect(shadow.getKey() == shadowKey)
      let children = try shadow.getChildren()
      #expect(children.count == 2)
      #expect(children.allSatisfy { $0 is DecoratorNode })
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator before the block cursor at the start of a shadow root"
    ))
  func blockDecoratorAtStartOfShadowRoot() throws {
    var existingKey = ""
    let (editor, _) = try shadowEditor(
      {
        let existing = try blockDecorator()
        existingKey = existing.getKey()
        return [existing]
      }, select: 0)
    try editor.update(discrete: true) { try rangeSelection().insertNodes([blockDecorator()]) }
    try editor.read { () throws in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      let children = try shadow.getChildren()
      #expect(children.count == 2)
      #expect(children.allSatisfy { $0 is DecoratorNode })
      #expect(children.count == 2 && children[1].getKey() == existingKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator into an empty shadow root"
    ))
  func blockDecoratorIntoEmptyShadowRoot() throws {
    let (editor, _) = try shadowEditor({ [] }, select: 0)
    try editor.update(discrete: true) { try rangeSelection().insertNodes([blockDecorator()]) }
    try editor.read { () throws in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      let children = try shadow.getChildren()
      #expect(children.count == 1)
      #expect(children.first is DecoratorNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block element at the block cursor inside a shadow root"
    ))
  func blockElementAtBlockCursorInShadowRoot() throws {
    let (editor, _) = try shadowEditor({ [try blockDecorator()] }, select: 1)
    try editor.update(discrete: true) {
      try rangeSelection().insertNodes([createParagraphNode().append(createTextNode("inserted"))])
    }
    try editor.read { () throws in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      let children = try shadow.getChildren()
      try #require(children.count == 2)
      #expect(children[0] is DecoratorNode)
      #expect(children[1] is ParagraphNode)
      #expect(try children[1].getTextContent() == "inserted")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > insertParagraph at an element point on a shadow root seeds into that shadow root"
    ))
  func insertParagraphInShadowRoot() throws {
    let (editor, _) = try shadowEditor({ [try blockDecorator()] }, select: 1)
    try editor.update(discrete: true) {
      let paragraph = try #require(try rangeSelection().insertParagraph())
      #expect(try paragraph.getParent()?.is(getRoot().getFirstChild()) == true)
    }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      let shadow = try #require(try root.getFirstChild() as? TestShadowRootNode)
      #expect(try shadow.getChildrenSize() == 2)
      #expect(try shadow.getLastChild() is ParagraphNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8707 > inserts a block decorator at a root element point without wrapping it in a paragraph"
    ))
  func blockDecoratorAtRootElementPoint() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("existing")))
      try getRoot().select(1, 1)
    }
    try editor.update(discrete: true) { try rangeSelection().insertNodes([blockDecorator()]) }
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(children[0] is ParagraphNode)
      #expect(children[1] is DecoratorNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete removes an empty block before a shadow root"
    ))
  func forwardDeleteRemovesEmptyBlock() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let shadow = try createTestShadowRootNode().append(
        createParagraphNode().append(createTextNode("inside")))
      try getRoot().clear().append(paragraph, shadow)
      try paragraph.selectStart()
    }
    try editor.update(discrete: true) { try rangeSelection().deleteCharacter(false) }
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 1)
      #expect(children[0] is TestShadowRootNode)
      #expect(try children[0].getTextContent() == "inside")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete keeps a non-empty block before a shadow root"
    ))
  func forwardDeleteKeepsNonEmptyBlock() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("before"))
      let shadow = try createTestShadowRootNode().append(
        createParagraphNode().append(createTextNode("inside")))
      try getRoot().clear().append(paragraph, shadow)
      try paragraph.selectEnd()
    }
    try editor.update(discrete: true) { try rangeSelection().deleteCharacter(false) }
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      #expect(try children[0].getTextContent() == "before")
      #expect(children[1] is TestShadowRootNode)
      #expect(try children[1].getTextContent() == "inside")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #8075 > forward delete from an empty block at the end of a shadow root does nothing"
    ))
  func forwardDeleteAtEndOfShadowRoot() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let inner = try createParagraphNode()
      try getRoot().clear().append(
        createTestShadowRootNode().append(inner),
        createTestShadowRootNode().append(createParagraphNode().append(createTextNode("next"))))
      try inner.selectStart()
    }
    try editor.update(discrete: true) { try rangeSelection().deleteCharacter(false) }
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let shadow = try #require(children[0] as? TestShadowRootNode)
      #expect(try shadow.getChildrenSize() == 1)
      #expect(try shadow.getTextContent() == "")
      #expect(try children[1].getTextContent() == "next")
    }
  }
}

/// `setupParagraph` and `insertBlocks` of the #9095 suite.
private func pasteEditor(
  _ text: String, _ caretOffset: Int, _ makeNodes: @escaping () throws -> [LexicalNode]
) throws -> Editor {
  let editor = try createTestEditor()
  try editor.update(discrete: true) {
    try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
    let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
    try #require(try paragraph.getFirstChild() as? TextNode).select(caretOffset, caretOffset)
  }
  try editor.update(discrete: true) { try rangeSelection().insertNodes(makeNodes()) }
  return editor
}

/// The root's children as `kind:text`, `p` for a paragraph and `d` for a
/// decorator.
private func rootSummary(_ editor: Editor) throws -> [String] {
  try editor.read {
    try getRoot().getChildren().map { child in
      let kind = child is ParagraphNode ? "p" : child is DecoratorNode ? "d" : child.getType()
      return child is DecoratorNode ? kind : "\(kind):\(try child.getTextContent())"
    }
  }
}

@Suite struct LexicalSelectionPasteBlockTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator at the end of a paragraph leaves no empty paragraph"
    ))
  func blockDecoratorAtEnd() throws {
    let editor = try pasteEditor("hello", 5) { [try blockDecorator()] }
    #expect(try rootSummary(editor) == ["p:hello", "d"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator in the middle of a paragraph keeps the trailing text"
    ))
  func blockDecoratorInMiddle() throws {
    let editor = try pasteEditor("helloworld", 5) { [try blockDecorator()] }
    #expect(try rootSummary(editor) == ["p:hello", "d", "p:world"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a block decorator at the start of a paragraph keeps the paragraph after it"
    ))
  func blockDecoratorAtStart() throws {
    let editor = try pasteEditor("hello", 0) { [try blockDecorator()] }
    #expect(try rootSummary(editor) == ["d", "p:hello"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a quote at the end of a paragraph merges into it (control)"
    ))
  func quoteMergesAtEnd() throws {
    let editor = try pasteEditor("hello", 5) {
      [try createTestElementNode().append(createTextNode("quoted"))]
    }
    #expect(try rootSummary(editor) == ["p:helloquoted"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > pasting a paragraph at the end of a paragraph merges into it (control)"
    ))
  func paragraphMergesAtEnd() throws {
    let editor = try pasteEditor("hello", 5) {
      [try createParagraphNode().append(createTextNode("world"))]
    }
    #expect(try rootSummary(editor) == ["p:helloworld"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::Regression tests for #9095 > typing after pasting a block decorator at the end creates a paragraph on demand"
    ))
  func typingAfterBlockDecorator() throws {
    let editor = try pasteEditor("hello", 5) { [try blockDecorator()] }
    try editor.update(discrete: true) { try rangeSelection().insertText("x") }
    #expect(try rootSummary(editor) == ["p:hello", "d", "p:x"])
  }
}
