import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of nodes/__tests__/unit/LexicalTabNode.test.tsx. The paste tests,
// which need @lexical/clipboard, are in LexicalClipboardTests. Upstream
// compares the reconciled HTML; the indentation tests compare the shape and
// each block's indent, which is what the HTML's padding shows.

/// `beforeEach`: an empty, selected paragraph, in an editor that also
/// registers upstream's default heading, quote and list nodes.
private func tabNodeEditor() throws -> Editor {
  let editor = try createTestEditor(nodes: [
    .klass(HeadingNode.self), .klass(QuoteNode.self), .klass(ListNode.self),
    .klass(ListItemNode.self),
  ])
  try update(editor) {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    try paragraph.select()
  }
  return editor
}

struct TabBoundaryCase: Sendable, CustomTestStringConvertible {
  let direction: CaretDirection
  let start: Int
  let output: String
  var testDescription: String { "\"x\\tx\" to \"\(output)\" at \(start) (\(direction))" }

  static let all: [TabBoundaryCase] = [CaretDirection.next, .previous].flatMap { direction in
    [
      TabBoundaryCase(direction: direction, start: 0, output: "yx"),
      TabBoundaryCase(direction: direction, start: 1, output: "xy"),
    ]
  }
}

@Suite struct LexicalTabNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can type between two (leaf nodes) canInsertBeforeAfter false"
    ))
  func typeBetweenTwoTabs() throws {
    let editor = try tabNodeEditor()
    try update(editor) {
      let tab1 = try createTabNode()
      let tab2 = try createTabNode()
      try insertNodes([tab1, tab2])
      try tab1.select(1, 1)
      try #require(try getSelection()).insertText("f")
    }
    #expect(try shape(editor) == "<paragraph>[\t][f][\t]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > can be serialized and deserialized"
    ))
  func serializeDeserialize() throws {
    let editor = try tabNodeEditor()
    try update(editor) {
      try getRoot().clear().append(createParagraphNode().append(createTabNode()))
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 1)
      #expect(isTabNode(textNodes.first))
    }
    let json = try editor.getEditorState().toJSON()
    try update(editor) { try getRoot().clear().append(createParagraphNode()) }
    try editor.read { () throws in
      #expect(try getRoot().getAllTextNodes().isEmpty)
    }
    try editor.setEditorState(editor.parseEditorState(json))
    drainMicrotasks()
    try editor.read {
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 1)
      #expect(isTabNode(textNodes.first))
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode \"x\\tx\" to \"yx\" at 0 (next)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode \"x\\tx\" to \"xy\" at 1 (next)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode \"x\\tx\" to \"yx\" at 0 (previous)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > TabNode at selection boundaries with normal TextNode sibling (#7602) > TabNode \"x\\tx\" to \"xy\" at 1 (previous)"
    ),
    arguments: TabBoundaryCase.all)
  func tabAtSelectionBoundaries(_ testCase: TabBoundaryCase) throws {
    let editor = try tabNodeEditor()
    try editor.update(discrete: true) {
      try selectAll().insertRawText("x\tx")
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.map(isTabNode) == [false, true, false])
      let caretRange = try getCaretRange(
        getTextPointCaret(textNodes[testCase.start], .next, 0),
        getTextPointCaret(textNodes[testCase.start + 1], .next, .direction(.next)))
      let range = try setSelectionFromCaretRange(
        getCaretRangeInDirection(caretRange, testCase.direction))
      try range.insertRawText("y")
    }
    try editor.read {
      let nodes = try getRoot().getAllTextNodes()
      #expect(try nodes.map { try $0.getTextContent() } == [testCase.output])
      #expect(nodes.map(isTabNode) == [false])
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > setTextContent normalizes back to \\t without throwing (#8596)"
    ))
  func setTextContentNormalizes() throws {
    let editor = try tabNodeEditor()
    try editor.update(discrete: true) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ElementNode)
      let tabNode = try createTabNode()
      try paragraph.append(tabNode)
      for text in ["\tㅁ", "arbitrary", ""] {
        #expect(throws: Never.self) { try tabNode.setTextContent(text) }
        #expect(try tabNode.getTextContent() == "\t")
      }
    }
  }

  /// `registerRichText`, `registerTabIndentation`, `setup`, then Tab.
  private func tab(_ setup: @escaping () throws -> Void) throws -> Editor {
    let editor = try tabNodeEditor()
    _ = registerRichText(editor)
    _ = registerTabIndentation(editor)
    try update(editor, setup)
    try editor.dispatchCommand(KEY_TAB_COMMAND, KeyboardEvent("keydown"))
    drainMicrotasks()
    return editor
  }

  private func indents(_ editor: Editor) throws -> [Int] {
    try editor.read(.latest) {
      try getRoot().getChildren().map { try ($0 as! ElementNode).getIndent() }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element indents when selection at the start of the block"
    ))
  func indentsAtTheStart() throws {
    let editor = try tab {
      try #require(try getSelection()).insertText("foo")
      _ = try getRoot().selectStart()
    }
    #expect(try shape(editor) == "<paragraph>[foo]</paragraph>")
    #expect(try indents(editor) == [1])
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > elements indent when selection spans across multiple blocks"
    ))
  func indentsAcrossBlocks() throws {
    let editor = try tab {
      let root = try getRoot()
      let paragraph = try #require(try root.getFirstChild() as? ElementNode)
      let paragraphText = try createTextNode("foo")
      let headingText = try createTextNode("bar")
      let listItemText = try createTextNode("xyz")
      try root.append(
        createHeadingNode("h1").append(headingText),
        createListNode("number").append(createListItemNode().append(listItemText)))
      try paragraph.append(paragraphText)
      let selection = createRangeSelection()
      try selection.focus.set(paragraphText.getKey(), 1, .text)
      try selection.anchor.set(listItemText.getKey(), 1, .text)
      try setSelection(selection)
    }
    #expect(
      try shape(editor)
        == "<paragraph>[foo]</paragraph><heading>[bar]</heading>"
        + "<list><listitem><list><listitem>[xyz]</listitem></list></listitem></list>")
    // The list item is indented by nesting it, which leaves the list at 0.
    #expect(try indents(editor) == [1, 1, 0])
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (1)"
    ))
  func tabsAtTheEnd() throws {
    let editor = try tab { try #require(try getSelection()).insertText("foo") }
    #expect(try shape(editor) == "<paragraph>[foo][\t]</paragraph>")
    #expect(try indents(editor) == [0])
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (2)"
    ))
  func tabsInTheMiddle() throws {
    let editor = try tab {
      try #require(try getSelection()).insertText("foo")
      _ = try #require(try getRoot().getLastDescendant() as? TextNode).select(1, 1)
    }
    #expect(try shape(editor) == "<paragraph>[f][\t][oo]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > element tabs when selection is not at the start (3)"
    ))
  func tabReplacesARange() throws {
    let editor = try tab {
      try #require(try getSelection()).insertText("foo")
      _ = try #require(try getRoot().getLastDescendant() as? TextNode).select(1, 2)
    }
    #expect(try shape(editor) == "<paragraph>[f][\t][o]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTabNode.test.tsx::LexicalTabNode tests > elements tabs when selection is not at the start and overlaps another tab"
    ))
  func tabOverAnotherTab() throws {
    let editor = try tab {
      try #require(try getSelection()).insertRawText("hello\tworld")
      let root = try getRoot()
      let selection = createRangeSelection()
      try selection.anchor.set(try #require(try root.getFirstDescendant()).getKey(), 4, .text)
      try selection.focus.set(try #require(try root.getLastDescendant()).getKey(), 2, .text)
      try setSelection(selection)
    }
    #expect(try shape(editor) == "<paragraph>[hell][\t][rld]</paragraph>")
  }
}
