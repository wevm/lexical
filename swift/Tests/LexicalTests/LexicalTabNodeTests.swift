import Lexical
import LexicalTesting
import Testing

// Port of nodes/__tests__/unit/LexicalTabNode.test.tsx. The paste tests need
// @lexical/clipboard and the indentation tests need @lexical/rich-text,
// @lexical/list and the tab indentation plugin, so they wait for those
// packages.

/// `beforeEach`: an empty, selected paragraph.
private func tabNodeEditor() throws -> Editor {
  let editor = try createTestEditor()
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
}
