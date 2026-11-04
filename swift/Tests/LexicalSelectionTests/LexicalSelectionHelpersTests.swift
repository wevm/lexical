import Lexical
import LexicalLink
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts.
// Upstream compares the mounted root's innerHTML; `renderedHTML` renders the
// committed document the same way.

private struct NodeSpec {
  let text: String
  let key: NodeKey
  let mergeable: Bool
  init(_ key: NodeKey, _ text: String, mergeable: Bool = false) {
    self.key = key
    self.text = text
    self.mergeable = mergeable
  }
}

/// `$createParagraphWithNodes`: text nodes with fixed keys, put in the
/// pending node map by hand.
private func createParagraphWithNodes(
  _ editor: Editor, _ nodes: [NodeSpec]
) throws
  -> ParagraphNode
{
  let paragraph = try createParagraphNode()
  let nodeMap = editor._pendingEditorState!._nodeMap
  for spec in nodes {
    let textNode = try TextNode(spec.text, key: spec.key)
    nodeMap.set(spec.key, textNode)
    if !spec.mergeable {
      try textNode.toggleUnmergeable()
    }
    try paragraph.append(textNode)
  }
  return paragraph
}

private let abc = [NodeSpec("a", "a"), NodeSpec("b", "b"), NodeSpec("c", "c")]

private func keys(_ nodes: [LexicalNode]) -> [NodeKey] { nodes.map { $0.getKey() } }

/// One `setupTestCase` call: a fresh unmounted editor with a paragraph of
/// `nodes`, the selection set from `anchor` to `focus`, then `body`.
private func setupTestCase(
  _ nodes: [NodeSpec], anchor: (NodeKey?, Int, PointType), focus: (NodeKey?, Int, PointType),
  _ body: @escaping (RangeSelection, ElementNode) throws -> Void
) throws {
  let editor = try createTestEditor()
  defer { editor.dispose() }
  try editor.update { () throws in
    let element = try createParagraphWithNodes(editor, nodes)
    try getRoot().append(element)
    try setAnchorPoint(anchor.0 ?? element.getKey(), anchor.1, anchor.2)
    try setFocusPoint(focus.0 ?? element.getKey(), focus.1, focus.2)
    let selection = try #require(try getSelection() as? RangeSelection)
    try body(selection, element)
  }
}

/// Upstream's mounted `createTestEditor` with a paragraph of `nodes` and the
/// selection set, then the committed selection's points.
private func committedPoints(
  _ nodes: [NodeSpec], anchor: (NodeKey?, Int, PointType), focus: (NodeKey?, Int, PointType)
) throws -> (String, String, NodeKey) {
  let editor = try createMountedTestEditor()
  defer { editor.dispose() }
  var elementKey: NodeKey = ""
  try editor.update { () throws in
    let element = try createParagraphWithNodes(editor, nodes)
    try getRoot().append(element)
    elementKey = element.getKey()
    try setAnchorPoint(anchor.0 ?? element.getKey(), anchor.1, anchor.2)
    try setFocusPoint(focus.0 ?? element.getKey(), focus.1, focus.2)
  }
  drainMicrotasks()
  let selection = try editor.read(.latest) { try #require(try getSelection() as? RangeSelection) }
  return (pointString(selection.anchor), pointString(selection.focus), elementKey)
}

/// A mounted test editor with HeadingNode and LinkNode, the update, then the
/// rendered HTML.
private func renderAfterUpdate(_ fn: @escaping () throws -> Void) throws -> String {
  let editor = try createMountedTestEditor(nodes: [.klass(HeadingNode.self), .klass(LinkNode.self)])
  defer { editor.dispose() }
  try update(editor, fn)
  return try renderedHTML(editor)
}

/// The paragraph-with-selection setup of `can insert ... correctly`.
private func selectEmptyParagraph() throws -> RangeSelection {
  let paragraph = try createParagraphNode()
  try getRoot().append(paragraph)
  try setAnchorPoint(paragraph.getKey(), 0, .element)
  try setFocusPoint(paragraph.getKey(), 0, .element)
  return try #require(try getSelection() as? RangeSelection)
}

private func selectText(_ text: TextNode, _ anchor: Int, _ focus: Int) throws -> RangeSelection {
  try setAnchorPoint(text.getKey(), anchor, .text)
  try setFocusPoint(text.getKey(), focus, .text)
  return try #require(try getSelection() as? RangeSelection)
}

@Suite struct LexicalSelectionHelpersTests {
  // MARK: Collapsed

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle a text point"
    ))
  func collapsedTextPoint() throws {
    let a: (NodeKey?, Int, PointType) = ("a", 0, .text)
    // getNodes
    try setupTestCase(abc, anchor: a, focus: a) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["a"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: a, focus: a) { selection, _ throws in
      #expect(try selection.getTextContent() == "")
    }
    // insertText
    try setupTestCase(abc, anchor: a, focus: a) { selection, _ throws in
      try selection.insertText("Test")
      #expect(try getNodeByKey("a")?.getTextContent() == "Testa")
      #expect(pointString(selection.anchor) == "a:4:text")
      #expect(pointString(selection.focus) == "a:4:text")
    }
    // insertNodes
    try setupTestCase(abc, anchor: a, focus: a) { selection, element throws in
      try selection.insertNodes([try createTextNode("foo")])
      let first = try #require(try element.getFirstChild()).getKey()
      #expect(pointString(selection.anchor) == "\(first):3:text")
      #expect(pointString(selection.focus) == "\(first):3:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: a, focus: a) { selection, _ throws in
      try selection.insertParagraph()
      #expect(pointString(selection.anchor) == "a:0:text")
      #expect(pointString(selection.focus) == "a:0:text")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: a, focus: a) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase(abc, anchor: a, focus: a) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
      #expect(try first.getNextSibling()?.getTextContent() == "a")
    }
    // Extract selection
    try setupTestCase(abc, anchor: a, focus: a) { selection, _ throws in
      #expect(try keys(selection.extract()) == ["a"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal after merge"
    ))
  func textPointAfterMerge() throws {
    let nodes = [
      NodeSpec("a", "a", mergeable: true), NodeSpec("bb", "bb", mergeable: true),
      NodeSpec("empty", "", mergeable: true), NodeSpec("cc", "cc", mergeable: true),
      NodeSpec("d", "d", mergeable: true),
    ]
    let (anchor, focus, _) = try committedPoints(
      nodes, anchor: ("bb", 1, .text), focus: ("cc", 1, .text))
    #expect(anchor == "a:2:text")
    #expect(focus == "a:4:text")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal after merge (2)"
    ))
  func textPointAfterMerge2() throws {
    let nodes = [
      NodeSpec("a", "a", mergeable: true), NodeSpec("empty", "", mergeable: true),
      NodeSpec("b", "b", mergeable: true), NodeSpec("c", "c", mergeable: true),
    ]
    let (anchor, focus, _) = try committedPoints(
      nodes, anchor: ("a", 0, .text), focus: ("c", 1, .text))
    #expect(anchor == "a:0:text")
    #expect(focus == "a:3:text")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point adjust to element point after removal of a single empty text node"
    ))
  func textPointToElementPointAfterEmptyRemoval() throws {
    let (anchor, focus, element) = try committedPoints(
      [NodeSpec("a", "", mergeable: true)], anchor: ("a", 0, .text), focus: ("a", 0, .text))
    #expect(anchor == "\(element):0:element")
    #expect(focus == "\(element):0:element")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after removal of an empty text node in a group #1"
    ))
  func elementPointAfterEmptyRemovalInGroup1() throws {
    let nodes = [NodeSpec("a", "", mergeable: true), NodeSpec("b", "b")]
    let (anchor, focus, _) = try committedPoints(
      nodes, anchor: (nil, 2, .element), focus: (nil, 2, .element))
    #expect(anchor == "b:1:text")
    #expect(focus == "b:1:text")
  }

  private static let group = [
    NodeSpec("a", "", mergeable: true), NodeSpec("b", "b"), NodeSpec("c", "c", mergeable: true),
    NodeSpec("d", "d", mergeable: true),
  ]

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after removal of an empty text node in a group #2"
    ))
  func elementPointAfterEmptyRemovalInGroup2() throws {
    let (anchor, focus, _) = try committedPoints(
      Self.group, anchor: (nil, 4, .element), focus: (nil, 4, .element))
    #expect(anchor == "c:2:text")
    #expect(focus == "c:2:text")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal of an empty text node in a group #3"
    ))
  func textPointAfterEmptyRemovalInGroup3() throws {
    let (anchor, focus, _) = try committedPoints(
      Self.group, anchor: ("d", 1, .text), focus: ("d", 1, .text))
    #expect(anchor == "c:2:text")
    #expect(focus == "c:2:text")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle an element point on empty element"
    ))
  func elementPointOnEmptyElement() throws {
    let point: (NodeKey?, Int, PointType) = (nil, 0, .element)
    // getNodes
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      #expect(try keys(selection.getNodes()) == [element.getKey()])
    }
    // getTextContent
    try setupTestCase([], anchor: point, focus: point) { selection, _ throws in
      #expect(try selection.getTextContent() == "")
    }
    // insertText
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // insertParagraph
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      try selection.insertParagraph()
      let next = try #require(try element.getNextSibling()).getKey()
      #expect(pointString(selection.anchor) == "\(next):0:element")
      #expect(pointString(selection.focus) == "\(next):0:element")
    }
    // insertLineBreak
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase([], anchor: point, focus: point) { selection, element throws in
      #expect(try keys(selection.extract()) == [element.getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle a start element point"
    ))
  func startElementPoint() throws {
    let point: (NodeKey?, Int, PointType) = (nil, 0, .element)
    // getNodes
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["a"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try selection.getTextContent() == "")
    }
    // insertText
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      try selection.insertParagraph()
      #expect(pointString(selection.anchor) == "a:0:text")
      #expect(pointString(selection.focus) == "a:0:text")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try keys(selection.extract()) == ["a"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle an end element point"
    ))
  func endElementPoint() throws {
    let point: (NodeKey?, Int, PointType) = (nil, 3, .element)
    // getNodes
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["c"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try selection.getTextContent() == "")
    }
    // insertText
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.insertText("Test")
      let last = try #require(try element.getLastChild())
      #expect(try last.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(last.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(last.getKey()):4:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.insertParagraph()
      let next = try #require(try element.getNextSibling()).getKey()
      #expect(pointString(selection.anchor) == "\(next):0:element")
      #expect(pointString(selection.focus) == "\(next):0:element")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.insertLineBreak()
      #expect(pointString(selection.anchor) == "\(element.getKey()):4:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):4:element")
    }
    // Format text
    try setupTestCase(abc, anchor: point, focus: point) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let last = try #require(try element.getLastChild())
      #expect(try last.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(last.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(last.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase(abc, anchor: point, focus: point) { selection, _ throws in
      #expect(try keys(selection.extract()) == ["c"])
    }
  }

  private static let mergeableABC = [
    NodeSpec("a", "a", mergeable: true), NodeSpec("b", "b", mergeable: true),
    NodeSpec("c", "c", mergeable: true),
  ]

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after merge from middle"
    ))
  func elementPointAfterMergeFromMiddle() throws {
    let (anchor, focus, _) = try committedPoints(
      Self.mergeableABC, anchor: (nil, 2, .element), focus: (nil, 2, .element))
    #expect(anchor == "a:2:text")
    #expect(focus == "a:2:text")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after merge from end"
    ))
  func elementPointAfterMergeFromEnd() throws {
    let (anchor, focus, _) = try committedPoints(
      Self.mergeableABC, anchor: (nil, 3, .element), focus: (nil, 3, .element))
    #expect(anchor == "a:3:text")
    #expect(focus == "a:3:text")
  }

  // MARK: Simple range

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle multiple text points"
    ))
  func multipleTextPoints() throws {
    let anchor: (NodeKey?, Int, PointType) = ("a", 0, .text)
    let focus: (NodeKey?, Int, PointType) = ("b", 0, .text)
    // getNodes
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["a", "b"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try selection.getTextContent() == "a")
    }
    // insertText
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // insertNodes
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertNodes([try createTextNode("foo")])
      let first = try #require(try element.getFirstChild()).getKey()
      #expect(pointString(selection.anchor) == "\(first):3:text")
      #expect(pointString(selection.focus) == "\(first):3:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      try selection.insertParagraph()
      #expect(pointString(selection.anchor) == "b:0:text")
      #expect(pointString(selection.focus) == "b:0:text")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try keys(selection.extract()) == ["a"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle multiple element points"
    ))
  func multipleElementPoints() throws {
    let anchor: (NodeKey?, Int, PointType) = (nil, 0, .element)
    let focus: (NodeKey?, Int, PointType) = (nil, 1, .element)
    // getNodes
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["a"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try selection.getTextContent() == "a")
    }
    // insertText
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      try selection.insertParagraph()
      #expect(pointString(selection.anchor) == "b:0:text")
      #expect(pointString(selection.focus) == "b:0:text")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      #expect(try keys(selection.extract()) == [try #require(try element.getFirstChild()).getKey()])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle a mix of text and element points"
    ))
  func mixOfTextAndElementPoints() throws {
    let anchor: (NodeKey?, Int, PointType) = (nil, 0, .element)
    let focus: (NodeKey?, Int, PointType) = ("c", 1, .text)
    // isBefore
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try selection.anchor.isBefore(selection.focus))
    }
    // getNodes
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try keys(selection.getNodes()) == ["a", "b", "c"])
    }
    // getTextContent
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try selection.getTextContent() == "abc")
    }
    // insertText
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // insertParagraph
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertParagraph()
      let next = try #require(try element.getNextSibling()).getKey()
      #expect(pointString(selection.anchor) == "\(next):0:element")
      #expect(pointString(selection.focus) == "\(next):0:element")
    }
    // insertLineBreak
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.insertLineBreak(true)
      #expect(pointString(selection.anchor) == "\(element.getKey()):0:element")
      #expect(pointString(selection.focus) == "\(element.getKey()):0:element")
    }
    // Format text
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, element throws in
      try selection.formatText("bold")
      try selection.insertText("Test")
      let first = try #require(try element.getFirstChild())
      #expect(try first.getTextContent() == "Test")
      #expect(pointString(selection.anchor) == "\(first.getKey()):4:text")
      #expect(pointString(selection.focus) == "\(first.getKey()):4:text")
    }
    // Extract selection
    try setupTestCase(abc, anchor: anchor, focus: focus) { selection, _ throws in
      #expect(try keys(selection.extract()) == ["a", "b", "c"])
    }
  }

  // MARK: can insert non-element nodes correctly

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > a single text node"
    ))
  func emptyParagraphSingleText() throws {
    let html = try renderAfterUpdate { () throws in
      try selectEmptyParagraph().insertNodes([try createTextNode("foo")])
    }
    #expect(html == "<p dir=\"auto\"><span data-lexical-text=\"true\">foo</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > two text nodes"
    ))
  func emptyParagraphTwoTexts() throws {
    let html = try renderAfterUpdate { () throws in
      try selectEmptyParagraph().insertNodes([try createTextNode("foo"), try createTextNode("bar")])
    }
    #expect(html == "<p dir=\"auto\"><span data-lexical-text=\"true\">foobar</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > link insertion without parent element"
    ))
  func emptyParagraphLinkWithoutParent() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try selectEmptyParagraph()
      let link = try createLinkNode("https://")
      try link.append(try createTextNode("ello worl"))
      try selection.insertNodes([try createTextNode("h"), link, try createTextNode("d")])
    }
    #expect(
      html
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">h</span><a href=\"https://\"><span data-lexical-text=\"true\">ello worl</span></a><span data-lexical-text=\"true\">d</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > a single heading node with a child text node"
    ))
  func emptyParagraphHeading() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try selectEmptyParagraph()
      let heading = try createHeadingNode("h1")
      try heading.append(try createTextNode("foo"))
      try selection.insertNodes([heading])
    }
    #expect(html == "<h1 dir=\"auto\"><span data-lexical-text=\"true\">foo</span></h1>")
  }

  private static func existingText() throws -> RangeSelection {
    let paragraph = try createParagraphNode()
    let text = try createTextNode("Existing text...")
    try paragraph.append(text)
    try getRoot().append(paragraph)
    return try selectText(text, 16, 16)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a single text node"
    ))
  func existingTextSingleText() throws {
    let html = try renderAfterUpdate { () throws in
      try Self.existingText().insertNodes([try createTextNode("foo")])
    }
    #expect(
      html == "<p dir=\"auto\"><span data-lexical-text=\"true\">Existing text...foo</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > two text nodes"
    ))
  func existingTextTwoTexts() throws {
    let html = try renderAfterUpdate { () throws in
      try Self.existingText().insertNodes([try createTextNode("foo"), try createTextNode("bar")])
    }
    #expect(
      html == "<p dir=\"auto\"><span data-lexical-text=\"true\">Existing text...foobar</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a single heading node with a child text node"
    ))
  func existingTextHeading() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try Self.existingText()
      let heading = try createHeadingNode("h1")
      try heading.append(try createTextNode("foo"))
      try selection.insertNodes([heading])
    }
    #expect(
      html == "<p dir=\"auto\"><span data-lexical-text=\"true\">Existing text...foo</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a paragraph with a child text and a child italic text and a child text"
    ))
  func existingTextParagraphWithItalic() throws {
    let html = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      let text = try createTextNode("AE")
      try paragraph.append(text)
      try getRoot().append(paragraph)
      let selection = try selectText(text, 1, 1)
      let insertedParagraph = try createParagraphNode()
      let insertedTextC = try createTextNode("C")
      try insertedTextC.toggleFormat("italic")
      try insertedParagraph.append(try createTextNode("B"), insertedTextC, try createTextNode("D"))
      try selection.insertNodes([insertedParagraph])
      let expected = try #require(
        try paragraph.getChildAtIndex(paragraph.getChildrenSize() - 2)
      ).getKey()
      #expect(pointString(selection.anchor) == "\(expected):1:text")
      #expect(pointString(selection.focus) == "\(expected):1:text")
    }
    #expect(
      html
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">AB</span><em data-lexical-text=\"true\">C</em><span data-lexical-text=\"true\">DE</span></p>"
    )
  }

  private static func fullySelectedText() throws -> RangeSelection {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    let text = try createTextNode("Existing text...")
    try paragraph.append(text)
    return try selectText(text, 0, "Existing text...".utf16.count)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node > a single text node"
    ))
  func fullySelectedSingleText() throws {
    let html = try renderAfterUpdate { () throws in
      try Self.fullySelectedText().insertNodes([try createTextNode("foo")])
    }
    #expect(html == "<p dir=\"auto\"><span data-lexical-text=\"true\">foo</span></p>")
  }

  /// The fully selected "Existing text..." with a link holding "link" after
  /// it, or before it when `linkFirst`.
  private static func fullySelectedTextNextToLink(linkFirst: Bool) throws -> RangeSelection {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    let text = try createTextNode("Existing text...")
    let link = try createLinkNode("https://")
    try link.append(try createTextNode("link"))
    if linkFirst {
      try paragraph.append(link, text)
    } else {
      try paragraph.append(text, link)
    }
    return try selectText(text, 0, "Existing text...".utf16.count)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node followed by an inline element > a single text node"
    ))
  func fullySelectedBeforeLinkSingleText() throws {
    let html = try renderAfterUpdate { () throws in
      try Self.fullySelectedTextNextToLink(linkFirst: false).insertNodes([
        try createTextNode("foo")
      ])
    }
    #expect(
      html
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">foo</span><a href=\"https://\"><span data-lexical-text=\"true\">link</span></a></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node preceded by an inline element > a single text node"
    ))
  func fullySelectedAfterLinkSingleText() throws {
    let html = try renderAfterUpdate { () throws in
      try Self.fullySelectedTextNextToLink(linkFirst: true).insertNodes([
        try createTextNode("foo")
      ])
    }
    #expect(
      html
        == "<p dir=\"auto\"><a href=\"https://\"><span data-lexical-text=\"true\">link</span></a><span data-lexical-text=\"true\">foo</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > can insert a linebreak node before an inline element node"
    ))
  func lineBreakBeforeInlineElement() throws {
    let html = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let link = try createLinkNode("https://lexical.dev/")
      try paragraph.append(link)
      let text = try createTextNode("Lexical")
      try link.append(text)
      _ = try text.select(0, 0)
      try insertNodes([try createLineBreakNode()])
    }
    // TODO #5109 ElementNode should have a way to control when other nodes can be inserted inside
    #expect(
      html
        == "<p dir=\"auto\"><br><a href=\"https://lexical.dev/\"><span data-lexical-text=\"true\">Lexical</span></a></p>"
    )
  }

  // MARK: can insert block element nodes correctly

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node followed by an inline element > a paragraph node"
    ))
  func fullySelectedBeforeLinkParagraph() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try Self.fullySelectedTextNextToLink(linkFirst: false)
      let paragraphToInsert = try createParagraphNode()
      try paragraphToInsert.append(try createTextNode("foo"))
      try selection.insertNodes([paragraphToInsert])
    }
    #expect(
      html
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">foo</span><a href=\"https://\"><span data-lexical-text=\"true\">link</span></a></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node preceded by an inline element > a paragraph node"
    ))
  func fullySelectedAfterLinkParagraph() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try Self.fullySelectedTextNextToLink(linkFirst: true)
      let paragraphToInsert = try createParagraphNode()
      try paragraphToInsert.append(try createTextNode("foo"))
      try selection.insertNodes([paragraphToInsert])
    }
    #expect(
      html
        == "<p dir=\"auto\"><a href=\"https://\"><span data-lexical-text=\"true\">link</span></a><span data-lexical-text=\"true\">foo</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert link into empty paragraph"
    ))
  func linkIntoEmptyParagraph() throws {
    let html = try renderAfterUpdate { () throws in
      try getRoot().append(try createParagraphNode())
      let linkNode = try createLinkNode("https://lexical.dev")
      try linkNode.append(try createTextNode("Lexical"))
      try insertNodes([linkNode])
    }
    #expect(
      html
        == "<p dir=\"auto\"><a href=\"https://lexical.dev\"><span data-lexical-text=\"true\">Lexical</span></a></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert link into empty paragraph (2)"
    ))
  func linkAndTextIntoEmptyParagraph() throws {
    let html = try renderAfterUpdate { () throws in
      try getRoot().append(try createParagraphNode())
      let linkNode = try createLinkNode("https://lexical.dev")
      try linkNode.append(try createTextNode("Lexical"))
      try insertNodes([linkNode, try createTextNode("...")])
    }
    #expect(
      html
        == "<p dir=\"auto\"><a href=\"https://lexical.dev\"><span data-lexical-text=\"true\">Lexical</span></a><span data-lexical-text=\"true\">...</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node > a paragraph node"
    ))
  func fullySelectedParagraph() throws {
    let html = try renderAfterUpdate { () throws in
      let selection = try Self.fullySelectedText()
      let paragraphToInsert = try createParagraphNode()
      try paragraphToInsert.append(try createTextNode("foo"))
      try selection.insertNodes([paragraphToInsert])
    }
    #expect(html == "<p dir=\"auto\"><span data-lexical-text=\"true\">foo</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert an ElementNode after ShadowRoot"
    ))
  func insertElementAfterShadowRoot() throws {
    let html = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectStart()
      try insertNodes([try createTestShadowRootNode(), try createTestElementNode()])
    }
    #expect(
      [
        "<div dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></div><div dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></div>",
        "<div dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></div><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>",
      ].contains(html))
  }

  // MARK: extract

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::extract > Should return the selected node when collapsed on a TextNode"
    ))
  func extractCollapsedOnText() throws {
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      let text = try createTextNode("Existing text...")
      try paragraph.append(text)
      try getRoot().append(paragraph)
      let selection = try selectText(text, 16, 16)
      #expect(try keys(selection.extract()) == [text.getKey()])
    }
  }

  // MARK: insertNodes

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert element next to top level decorator node"
    ))
  func insertNextToTopLevelDecorator() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    // Upstream mocks TestDecoratorNode.isInline to return false; the Swift
    // TestDecoratorNode is made block with setIsInline(false), which also
    // makes its createDOM a <div> rather than a <span>.
    try update(editor) { () throws in
      try getRoot().append(
        try createParagraphNode(), try createTestDecoratorNode().setIsInline(false),
        try createParagraphNode().append(try createTextNode("Text after")))
    }
    try update(editor) { () throws in
      let selectionNode = try #require(try getRoot().getFirstChild() as? ElementNode)
      let selection = try selectionNode.select()
      try selection.insertNodes([
        try createParagraphNode().append(try createTextNode("Text before"))
      ])
    }
    #expect(
      try renderedHTML(editor)
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">Text before</span></p>"
        + "<div data-lexical-decorator=\"true\" contenteditable=\"false\"></div>"
        + "<p dir=\"auto\"><span data-lexical-text=\"true\">Text after</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert when previous selection was null"
    ))
  func insertWhenPreviousSelectionNull() throws {
    let editor = try createTestEditor(headless: true)
    defer { editor.dispose() }
    try update(editor) { () throws in
      let selection = createRangeSelection()
      try selection.anchor.set("root", 0, .element)
      try selection.focus.set("root", 0, .element)
      try selection.insertNodes([try createParagraphNode().append(try createTextNode("Text"))])
      #expect(try getRoot().getTextContent() == "Text")
      try setSelection(nil)
    }
    try update(editor) { () throws in
      let selection = createRangeSelection()
      let text = try #require(try getRoot().getLastDescendant())
      try selection.anchor.set(text.getKey(), 0, .text)
      try selection.focus.set(text.getKey(), 0, .text)
      try selection.insertNodes([try createParagraphNode().append(try createTextNode("Before "))])
      #expect(try getRoot().getTextContent() == "Before Text")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert when before empty text node"
    ))
  func insertBeforeEmptyText() throws {
    _ = try renderAfterUpdate { () throws in
      // Empty text node to test empty text split
      let emptyTextNode = try createTextNode("")
      try getRoot().append(
        try createParagraphNode().append(emptyTextNode, try createTextNode("text")))
      try emptyTextNode.select(0, 0)
      let selection = try #require(try getSelection() as? RangeSelection)
      try selection.insertNodes([try createTextNode("foo")])
      #expect(try getRoot().getTextContent() == "footext")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > last node is LineBreakNode"
    ))
  func lastNodeIsLineBreak() throws {
    let editor = try createMountedTestEditor(nodes: [.klass(HeadingNode.self)])
    defer { editor.dispose() }
    try update(editor) { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let selection = try paragraph.select()
      let newHeading = try createHeadingNode("h1").append(try createTextNode("heading"))
      try selection.insertNodes([newHeading, try createLineBreakNode()])
    }
    // the lone trailing LineBreakNode collapses to an empty paragraph,
    // rendered with only the managed linebreak
    #expect(
      try renderedHTML(editor)
        == "<h1 dir=\"auto\"><span data-lexical-text=\"true\">heading</span></h1><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>"
    )
    try editor.read(.latest) { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      let selectedNode = try selection.anchor.getNode()
      #expect(selectedNode is ParagraphNode)
      #expect(try selectedNode.getPreviousSibling() is HeadingNode)
    }
  }

  // MARK: $patchStyleText

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection anchored to the end of a TextNode before an inline element"
    ))
  func patchAtEndBeforeInlineElement() throws {
    let editor = try createMountedTestEditor(nodes: [.klass(LinkNode.self)])
    defer { editor.dispose() }
    try update(editor) { () throws in
      let paragraph = try createParagraphWithNodes(
        editor, [NodeSpec("a", "a"), NodeSpec("b", "b")])
      try getRoot().append(paragraph)
      let link = try createLinkNode("https://")
      try link.append(try createTextNode("link"))
      try #require(try getNodeByKey("a")).insertAfter(link)
      try setAnchorPoint("a", 1, .text)
      try setFocusPoint("b", 1, .text)
      let selection = try #require(try getSelection() as? RangeSelection)
      try patchStyleText(selection, ["text-emphasis": "filled"])
    }
    #expect(
      try renderedHTML(editor)
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">a</span>"
        + "<a href=\"https://\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">link</span>"
        + "</a>"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">b</span></p>")
  }

  /// "a" then a link holding "link", the selection from `anchor` to
  /// `focus` (`nil` keys are the link), patched with text-emphasis.
  private func patchNextToLink(
    anchor: (NodeKey?, Int, PointType), focus: (NodeKey?, Int, PointType), withA: Bool = true
  ) throws -> String {
    let editor = try createMountedTestEditor(nodes: [.klass(LinkNode.self)])
    defer { editor.dispose() }
    try update(editor) { () throws in
      let link = try createLinkNode("https://")
      try link.append(try createTextNode("link"))
      if withA {
        try getRoot().append(try createParagraphWithNodes(editor, [NodeSpec("a", "a")]))
        try #require(try getNodeByKey("a")).insertAfter(link)
      } else {
        let paragraph = try createParagraphNode()
        try getRoot().append(paragraph)
        try paragraph.append(link)
      }
      try setAnchorPoint(anchor.0 ?? link.getKey(), anchor.1, anchor.2)
      try setFocusPoint(focus.0 ?? link.getKey(), focus.1, focus.2)
      let selection = try #require(try getSelection() as? RangeSelection)
      try patchStyleText(selection, ["text-emphasis": "filled"])
    }
    return try renderedHTML(editor)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection that ends on an element"
    ))
  func patchEndingOnElement() throws {
    // Select to end of the link _element_
    #expect(
      try patchNextToLink(anchor: ("a", 0, .text), focus: (nil, 1, .element))
        == "<p dir=\"auto\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">a</span>"
        + "<a href=\"https://\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">link</span>"
        + "</a>"
        + "</p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a reversed selection that ends on an element"
    ))
  func patchReversedEndingOnElement() throws {
    // Select from the end of the link _element_
    #expect(
      try patchNextToLink(anchor: (nil, 1, .element), focus: ("a", 0, .text))
        == "<p dir=\"auto\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">a</span>"
        + "<a href=\"https://\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">link</span>"
        + "</a>"
        + "</p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection that starts and ends on an element"
    ))
  func patchStartingAndEndingOnElement() throws {
    #expect(
      try patchNextToLink(anchor: (nil, 0, .element), focus: (nil, 1, .element), withA: false)
        == "<p dir=\"auto\">"
        + "<a href=\"https://\">"
        + "<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">link</span>"
        + "</a>"
        + "</p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection anchored to the end of a TextNode at the end of a paragraph"
    ))
  func patchAtEndOfParagraph() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try update(editor) { () throws in
      let paragraph1 = try createParagraphWithNodes(editor, [NodeSpec("a", "a")])
      let paragraph2 = try createParagraphWithNodes(editor, [NodeSpec("b", "b")])
      try getRoot().append(paragraph1)
      try getRoot().append(paragraph2)
      try setAnchorPoint("a", 1, .text)
      try setFocusPoint("b", 1, .text)
      let selection = try #require(try getSelection() as? RangeSelection)
      try patchStyleText(selection, ["text-emphasis": "filled"])
    }
    #expect(
      try renderedHTML(editor)
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">a</span></p>"
        + "<p dir=\"auto\"><span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">b</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can clear a style"
    ))
  func clearStyle() throws {
    let html = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("text")
      try paragraph.append(text)
      let selection = try selectText(text, 0, text.getTextContentSize())
      try patchStyleText(selection, ["text-emphasis": "filled"])
      try patchStyleText(selection, ["text-emphasis": nil])
    }
    #expect(html == "<p dir=\"auto\"><span data-lexical-text=\"true\">text</span></p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can toggle a style on a collapsed selection"
    ))
  func toggleStyleOnCollapsedSelection() throws {
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("text")
      try paragraph.append(text)
      let selection = try selectText(text, 0, 0)
      try patchStyleText(selection, ["text-emphasis": "filled"])
      #expect(try getSelectionStyleValueForProperty(selection, "text-emphasis", "") == "filled")
      try patchStyleText(selection, ["text-emphasis": nil])
      #expect(try getSelectionStyleValueForProperty(selection, "text-emphasis", "") == "")
      try patchStyleText(selection, ["text-emphasis": "filled"])
      #expect(try getSelectionStyleValueForProperty(selection, "text-emphasis", "") == "filled")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > updates cached styles when setting on a collapsed selection"
    ))
  func updatesCachedStyles() throws {
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("text")
      try paragraph.append(text)
      // First fetch the initial style -- this will cause the CSS cache to be
      // populated with an empty string pointing to an empty style object.
      let selection = try selectText(text, 0, 0)
      _ = try getSelectionStyleValueForProperty(selection, "color", "")
      // Now when we set the style, we should _not_ touch the previously
      // created empty style object, but create a new one instead.
      try patchStyleText(selection, ["color": "red"])
      // We can check that result by clearing the style and re-querying it.
      let current = try #require(try getSelection() as? RangeSelection)
      current.setStyle("")
      #expect(try getSelectionStyleValueForProperty(current, "color", "") == "")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > $getSelectionStyleValueForProperty returns consistent value regardless of selection direction"
    ))
  func styleValueRegardlessOfDirection() throws {
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let unstyled = try createTextNode("plain")
      let styled = try createTextNode("colored")
      try styled.setStyle("color: red")
      try paragraph.append(unstyled)
      try paragraph.append(styled)
      // Forward selection: unstyled -> styled
      try setAnchorPoint(unstyled.getKey(), 0, .text)
      try setFocusPoint(styled.getKey(), "colored".utf16.count, .text)
      let forwardValue = try getSelectionStyleValueForProperty(
        #require(try getSelection() as? RangeSelection), "color", "")
      // Backward selection: styled -> unstyled
      try setAnchorPoint(styled.getKey(), "colored".utf16.count, .text)
      try setFocusPoint(unstyled.getKey(), 0, .text)
      let backwardValue = try getSelectionStyleValueForProperty(
        #require(try getSelection() as? RangeSelection), "color", "")
      #expect(forwardValue == "")
      #expect(backwardValue == "")
      #expect(forwardValue == backwardValue)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > $getSelectionStyleValueForProperty ignores nodes with zero characters selected at boundaries"
    ))
  func styleValueIgnoresZeroWidthBoundaries() throws {
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let styledA = try createTextNode("aaa").setStyle("color: red")
      let styledB = try createTextNode("bbb").setStyle("color: red")
      let different = try createTextNode("ccc").setStyle("color: blue")
      try paragraph.append(styledA)
      try paragraph.append(styledB)
      try paragraph.append(different)
      // Select from end of styledA to start of different: styledA has 0 chars
      // selected (offset at end), different has 0 chars (offset 0), only
      // styledB is fully selected.
      try setAnchorPoint(styledA.getKey(), 3, .text)
      try setFocusPoint(different.getKey(), 0, .text)
      let value = try getSelectionStyleValueForProperty(
        #require(try getSelection() as? RangeSelection), "color", "")
      #expect(value == "red")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can update style of text node that is in token mode"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can update style of text node that is in segmented mode"
    ),
    arguments: ["token", "segmented"])
  func updateStyleOfModedText(_ mode: TextModeType) throws {
    let html = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let text = try createTextNode("first").setFormat("bold")
      try paragraph.append(text)
      let textInMode = try createTextNode("second").setMode(mode)
      try paragraph.append(textInMode)
      try setAnchorPoint(text.getKey(), 3, .text)
      try setFocusPoint(textInMode.getKey(), 3, .text)
      try patchStyleText(#require(try getSelection()), ["font-size": "15px"])
    }
    #expect(
      html
        == "<p dir=\"auto\">"
        + "<strong data-lexical-text=\"true\">fir</strong>"
        + "<strong style=\"font-size: 15px;\" data-lexical-text=\"true\">st</strong>"
        + "<span style=\"font-size: 15px;\" data-lexical-text=\"true\">second</span>" + "</p>")
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > preserve backward selection when changing style of 2 different text nodes"
    ))
  func preserveBackwardSelection() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let firstText = try createTextNode("first ").setFormat("bold")
      try paragraph.append(firstText)
      let secondText = try createTextNode("second").setFormat("italic")
      try paragraph.append(secondText)
      try setAnchorPoint(secondText.getKey(), 3, .text)
      try setFocusPoint(firstText.getKey(), 3, .text)
      let selection = try #require(try getSelection())
      try patchStyleText(selection, ["font-size": "11px"])
      let (newAnchor, newFocus) = try #require(selection.getStartEndPoints())
      #expect(try newAnchor.getNode().getTextContent() == "sec")
      #expect(newAnchor.offset == 3)
      #expect(try newFocus.getNode().getTextContent() == "st ")
      #expect(newFocus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > applies a function-valued patch exactly once to an empty element"
    ))
  func functionPatchOnceOnEmptyElement() throws {
    var paragraphStyle = ""
    var selectionStyle = ""
    _ = try renderAfterUpdate { () throws in
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectStart()
      let selection = try #require(try getSelection())
      // Mirrors the playground font-size +/- buttons, which patch with a
      // function of the previous value rather than a constant.
      try patchStyleText(
        selection,
        [
          "font-size": .compute { currentValue, _ in
            let size = Int(String((currentValue ?? "").prefix(while: \.isNumber)))
            return "\((size ?? 16) + 2)px"
          }
        ])
      selectionStyle = try #require(try getSelection() as? RangeSelection).style
      paragraphStyle = try paragraph.getTextStyle()
    }
    #expect(selectionStyle == "font-size: 18px;")
    #expect(paragraphStyle == "font-size: 18px;")
  }
}
