import Lexical
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts.
// The reconciled HTML upstream compares is asserted as the model shape
// (`blocks`), and tests that build list nodes wait for @lexical/list.

private var editorConfig: EditorConfig {
  EditorConfig(
    namespace: "",
    theme: [
      "heading": [
        "h1": "my-h1-class", "h2": "my-h2-class", "h3": "my-h3-class", "h4": "my-h4-class",
        "h5": "my-h5-class", "h6": "my-h6-class",
      ]
    ])
}

/// `$describeBlocks`: each block as its tag (headings) or type, then its
/// children as their text (text nodes) or type.
func blocks(_ editor: Editor) throws -> [[String]] {
  try editor.read {
    try getRoot().getChildren().map { block in
      let element = try #require(block as? ElementNode)
      let head = try (element as? HeadingNode)?.getTag() ?? element.getType()
      return try [head]
        + element.getChildren().map { child in
          try child is TextNode ? child.getTextContent() : child.getType()
        }
    }
  }
}

private func replaceRootWithHeading(_ children: LexicalNode...) throws -> HeadingNode {
  let heading = try createHeadingNode("h1").append(contentsOf: children)
  try getRoot().clear().append(heading)
  return heading
}

@Suite struct LexicalHeadingNodeTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let headingNode = try HeadingNode("h1")
      #expect(headingNode.getType() == "heading")
      #expect(try headingNode.getTag() == "h1")
      #expect(try headingNode.getTextContent() == "")
    }
    #expect(throws: LexicalError.self) { try HeadingNode("h1") }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let headingNode = try HeadingNode("h1")
      #expect(
        try headingNode.createDOM(editorConfig).outerHTML == #"<h1 class="my-h1-class"></h1>"#)
      #expect(
        try headingNode.createDOM(EditorConfig(namespace: "", theme: ["heading": [:]]))
          .outerHTML == "<h1></h1>")
      #expect(try headingNode.createDOM(EditorConfig(namespace: "")).outerHTML == "<h1></h1>")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.updateDOM()"
    ))
  func updateDOM() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let headingNode = try createHeadingNode("h1")
      let domElement = try headingNode.createDOM(editorConfig)
      #expect(domElement.outerHTML == #"<h1 class="my-h1-class"></h1>"#)
      let newHeadingNode = try createHeadingNode("h1")
      #expect(try newHeadingNode.updateDOM(headingNode, domElement, editor._config) == false)
      #expect(domElement.outerHTML == #"<h1 class="my-h1-class"></h1>"#)
      // When the tag changes, updateDOM returns true and leaves the DOM to
      // createDOM.
      let newTag = try createHeadingNode("h2")
      #expect(try newTag.updateDOM(headingNode, domElement, editor._config) == true)
      #expect(domElement.outerHTML == #"<h1 class="my-h1-class"></h1>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() empty"
    ))
  func insertNewAfterEmpty() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var headingNode: HeadingNode!
    try update(editor) {
      headingNode = try HeadingNode("h1")
      try getRoot().append(headingNode)
    }
    #expect(try blocks(editor) == [["h1"]])
    try update(editor) {
      // The selection is null here (nothing selected), as in JS.
      #expect(try getSelection() == nil)
      let result = try headingNode.insertNewAfter()
      #expect(result is ParagraphNode)
      #expect(try result.getDirection() == headingNode.getDirection())
    }
    #expect(try blocks(editor) == [["h1"], ["paragraph"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() middle"
    ))
  func insertNewAfterMiddle() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var headingNode: HeadingNode!
    try update(editor) {
      headingNode = try HeadingNode("h1")
      let headingTextNode = try createTextNode("hello world")
      try getRoot().append(headingNode.append(headingTextNode))
      try headingTextNode.select(5, 5)
    }
    #expect(try blocks(editor) == [["h1", "hello world"]])
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      let result = try #require(try headingNode.insertNewAfter(selection) as? ElementNode)
      #expect(result is HeadingNode)
      #expect(try result.getDirection() == headingNode.getDirection())
    }
    #expect(try blocks(editor) == [["h1", "hello world"], ["h1"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() middle keeps format and style"
    ))
  func insertNewAfterMiddleKeepsFormatAndStyle() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var headingNode: HeadingNode!
    try update(editor) {
      headingNode = try HeadingNode("h1")
      try headingNode.setFormat("center")
      try headingNode.setStyle("color: red;")
      let headingTextNode = try createTextNode("hello world")
      try getRoot().append(headingNode.append(headingTextNode))
      try headingTextNode.select(5, 5)
    }
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      let result = try #require(try headingNode.insertNewAfter(selection) as? ElementNode)
      #expect(result is HeadingNode)
      #expect(try result.getFormatType() == "center")
      #expect(try result.getStyle() == "color: red;")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.insertNewAfter() end"
    ))
  func insertNewAfterEnd() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var headingNode: HeadingNode!
    try update(editor) {
      headingNode = try HeadingNode("h1")
      let headingTextNode1 = try createTextNode("hello")
      let headingTextNode2 = try createTextNode(" world").setFormat("bold")
      try getRoot().append(headingNode.append(headingTextNode1, headingTextNode2))
      try headingTextNode2.selectEnd()
    }
    #expect(try blocks(editor) == [["h1", "hello", " world"]])
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      let result = try #require(try headingNode.insertNewAfter(selection) as? ElementNode)
      #expect(result is ParagraphNode)
      #expect(try result.getDirection() == headingNode.getDirection())
    }
    #expect(try blocks(editor) == [["h1", "hello", " world"], ["paragraph"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > HeadingNode.setTag()"
    ))
  func setTag() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try editor.update(discrete: true) {
      try getRoot().append(createHeadingNode("h1"))
    }
    #expect(try blocks(editor) == [["h1"]])
    try editor.update(discrete: true) {
      let heading = try #require(try getRoot().getFirstChild() as? HeadingNode)
      #expect(try heading.getTag() == "h1")
      try heading.setTag("h2")
      #expect(try heading.getTag() == "h2")
    }
    #expect(try blocks(editor) == [["h2"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > $createHeadingNode()"
    ))
  func createHeading() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let headingNode = try HeadingNode("h1")
      let createdHeadingNode = try createHeadingNode("h1")
      #expect(headingNode.__type == createdHeadingNode.__type)
      #expect(headingNode.__parent == createdHeadingNode.__parent)
      #expect(headingNode.__key != createdHeadingNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > $isHeadingNode()"
    ))
  func isHeading() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try update(editor) {
      let headingNode = try HeadingNode("h1")
      #expect(isHeadingNode(headingNode))
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::LexicalHeadingNode tests > creates a h2 with text and can insert a new paragraph after"
    ))
  func h2InsertNewParagraphAfter() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var headingNode: HeadingNode!
    try update(editor) {
      headingNode = try HeadingNode("h2")
      try getRoot().append(headingNode)
      try headingNode.append(createTextNode("hello world"))
    }
    #expect(try blocks(editor) == [["h2", "hello world"]])
    try update(editor) {
      let result = try headingNode.insertNewAfter()
      #expect(result is ParagraphNode)
      #expect(try result.getDirection() == headingNode.getDirection())
    }
    #expect(try blocks(editor) == [["h2", "hello world"], ["paragraph"]])
  }
}

@Suite struct BackspaceAtStartOfHeadingTests {
  private func backspace(_ build: @escaping () throws -> TextNode) throws -> Editor {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try editor.update(discrete: true) {
      try build().select(0, 0).deleteCharacter(true)
    }
    return editor
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves heading when previous block i…"
    ))
  func preservesHeadingAfterEmptyParagraph() throws {
    let editor = try backspace {
      let text = try createTextNode("Heading")
      try getRoot().clear().append(
        createParagraphNode(), createHeadingNode("h2").append(text))
      return text
    }
    #expect(try blocks(editor) == [["h2", "Heading"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves quote when previous block is …"
    ))
  func preservesQuoteAfterEmptyParagraph() throws {
    let editor = try backspace {
      let text = try createTextNode("Quote")
      try getRoot().clear().append(createParagraphNode(), createQuoteNode().append(text))
      return text
    }
    #expect(try blocks(editor) == [["quote", "Quote"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > removes only the empty paragraph betwee…"
    ))
  func removesOnlyTheEmptyParagraph() throws {
    let editor = try backspace {
      let text = try createTextNode("Heading")
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("Before")), createParagraphNode(),
        createHeadingNode("h1").append(text),
        createParagraphNode().append(createTextNode("After")))
      return text
    }
    #expect(
      try blocks(editor) == [["paragraph", "Before"], ["h1", "Heading"], ["paragraph", "After"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > legacy merge still applies when previou…"
    ))
  func legacyMergeWhenPreviousBlockIsNonEmpty() throws {
    let editor = try backspace {
      let text = try createTextNode("Heading")
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("Before")),
        createHeadingNode("h2").append(text))
      return text
    }
    try editor.read {
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      #expect(!isHeadingNode(children[0]))
      #expect(try children[0].getTextContent() == "BeforeHeading")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > HeadingNode.collapseAtStart preserves a non-empty heading"
    ))
  func collapseAtStartPreservesNonEmptyHeading() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var originalKey = ""
    try editor.update(discrete: true) {
      let heading = try createHeadingNode("h1").append(createTextNode("Heading"))
      try getRoot().clear().append(heading)
      originalKey = heading.getKey()
      try #require(try heading.getFirstChild() as? TextNode).select(0, 0)
      try heading.collapseAtStart()
    }
    #expect(try blocks(editor) == [["h1", "Heading"]])
    try editor.read { () throws in
      #expect(try getRoot().getFirstChild()?.getKey() == originalKey)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > preserves heading wrapped inside another ElementNode"
    ))
  func preservesWrappedHeading() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    var originalKey = ""
    try editor.update(discrete: true) {
      let wrapper = try createTestElementNode()
      let heading = try createHeadingNode("h1").append(createTextNode("Heading"))
      try wrapper.append(heading)
      try getRoot().clear().append(wrapper)
      originalKey = heading.getKey()
      try heading.collapseAtStart()
    }
    try editor.read {
      let wrap = try #require(try getRoot().getFirstChild() as? ElementNode)
      #expect(wrap.getType() == "test_block")
      let inner = try #require(try wrap.getFirstChild() as? HeadingNode)
      #expect(inner.getKey() == originalKey)
      #expect(try inner.getTag() == "h1")
      #expect(try inner.getTextContent() == "Heading")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Backspace at start of heading (#4359) > HeadingNode.collapseAtStart converts an empty heading to a paragraph"
    ))
  func collapseAtStartConvertsEmptyHeading() throws {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try editor.update(discrete: true) {
      let heading = try createHeadingNode("h2")
      try getRoot().clear().append(heading)
      try heading.select(0, 0)
      try heading.collapseAtStart()
    }
    #expect(try blocks(editor) == [["paragraph"]])
  }
}

@Suite struct EnterInHeadingTests {
  private func enter(_ build: @escaping () throws -> RangeSelection) throws -> [[String]] {
    let editor = try createMountedTestEditor(nodes: richTextTestNodes)
    try editor.update(discrete: true) {
      try build().insertParagraph()
    }
    return try blocks(editor)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the text after a line break sp…"
    ))
  func afterLineBreakSplits() throws {
    let result = try enter {
      let second = try createTextNode("second")
      _ = try replaceRootWithHeading(createTextNode("first"), createLineBreakNode(), second)
      return try second.select(0, 0)
    }
    #expect(result == [["h1", "first", "linebreak"], ["h1", "second"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > just after an inline decorator splits t…"
    ))
  func afterInlineDecoratorSplits() throws {
    let result = try enter {
      let text = try createTextNode(" b")
      _ = try replaceRootWithHeading(createTextNode("a "), createTestDecoratorNode(), text)
      return try text.select(0, 0)
    }
    #expect(result == [["h1", "a ", "test_decorator"], ["h1", " b"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of a heading that ends with a line …"
    ))
  func endAfterLineBreakAddsParagraph() throws {
    let result = try enter {
      try replaceRootWithHeading(createTextNode("first"), createLineBreakNode()).select(2, 2)
    }
    #expect(result == [["h1", "first", "linebreak"], ["paragraph"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of a heading that ends with an inli…"
    ))
  func endAfterInlineDecoratorAddsParagraph() throws {
    let result = try enter {
      try replaceRootWithHeading(createTextNode("a "), createTestDecoratorNode()).select(2, 2)
    }
    #expect(result == [["h1", "a ", "test_decorator"], ["paragraph"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the heading adds a paragraph a…"
    ))
  func startAddsParagraphAbove() throws {
    let result = try enter {
      let first = try createTextNode("first")
      _ = try replaceRootWithHeading(first, createLineBreakNode(), createTextNode("second"))
      return try first.select(0, 0)
    }
    #expect(result == [["paragraph"], ["h1", "first", "linebreak", "second"]])
  }

  /// `TestLinkNode` stands in for `LinkNode`, with its type and the
  /// overrides core editing consults.
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > (link, 0) in a leading link adds a para…"
    ))
  func leadingLinkAddsParagraphAbove() throws {
    let result = try enter {
      let link = try createTestLinkNode().append(createTextNode("a"))
      _ = try replaceRootWithHeading(link, createTextNode(" b"))
      let selection = createRangeSelection()
      try selection.anchor.set(link.getKey(), 0, .element)
      try selection.focus.set(link.getKey(), 0, .element)
      try setSelection(selection)
      return selection
    }
    #expect(result == [["paragraph"], ["h1", "link", " b"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > start of the heading before an inline d…"
    ))
  func beforeInlineDecoratorAddsParagraphAbove() throws {
    let result = try enter {
      try replaceRootWithHeading(createTestDecoratorNode(), createTextNode(" b")).select(0, 0)
    }
    #expect(result == [["paragraph"], ["h1", "test_decorator", " b"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > middle of a word splits the heading"
    ))
  func middleOfWordSplits() throws {
    let result = try enter {
      let second = try createTextNode("second")
      _ = try replaceRootWithHeading(createTextNode("first"), createLineBreakNode(), second)
      return try second.select(3, 3)
    }
    #expect(result == [["h1", "first", "linebreak", "sec"], ["h1", "ond"]])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/LexicalHeadingNode.test.ts::Enter in a heading after a line break or an inline decorator (#9293) > end of the text adds a paragraph"
    ))
  func endOfTextAddsParagraph() throws {
    let result = try enter {
      let second = try createTextNode("second")
      _ = try replaceRootWithHeading(createTextNode("first"), createLineBreakNode(), second)
      return try second.select(6, 6)
    }
    #expect(result == [["h1", "first", "linebreak", "second"], ["paragraph"]])
  }
}
