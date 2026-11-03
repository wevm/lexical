import Lexical
import LexicalTesting
import Testing

// Port of the `LexicalSelection tests` suite of
// __tests__/unit/LexicalSelection.test.ts. TestLinkNode stands in for
// LinkNode, and the reconciled HTML is compared as `shape()`.

enum InlinePosition: String, CaseIterable, Sendable {
  case startOfParagraph = "start-of-paragraph"
  case midParagraph = "mid-paragraph"
  case endOfParagraph = "end-of-paragraph"

  var initialShape: String {
    switch self {
    case .startOfParagraph: "<paragraph><link>[a]</link>[b]</paragraph>"
    case .midParagraph: "<paragraph>[a]<link>[b]</link>[c]</paragraph>"
    case .endOfParagraph: "<paragraph>[a]<link>[b]</link></paragraph>"
    }
  }
}

enum InsertMethod: String, CaseIterable, Sendable {
  case insertText, insertNodes
}

struct InlineInsertCase: Sendable, CustomTestStringConvertible {
  let before: Bool
  let position: InlinePosition
  let method: InsertMethod
  var testDescription: String { "\(before ? "before" : "after") \(position) \(method)" }

  static let all: [InlineInsertCase] = [true, false].flatMap { before in
    InlinePosition.allCases.flatMap { position in
      InsertMethod.allCases.map { InlineInsertCase(before: before, position: position, method: $0) }
    }
  }

  var expectedShape: String {
    switch (before, position) {
    case (true, .startOfParagraph): "<paragraph>[x]<link>[a]</link>[b]</paragraph>"
    case (true, .midParagraph): "<paragraph>[ax]<link>[b]</link>[c]</paragraph>"
    case (true, .endOfParagraph): "<paragraph>[ax]<link>[b]</link></paragraph>"
    case (false, .startOfParagraph): "<paragraph><link>[a]</link>[xb]</paragraph>"
    case (false, .midParagraph): "<paragraph>[a]<link>[b]</link>[xc]</paragraph>"
    case (false, .endOfParagraph): "<paragraph>[a]<link>[b]</link>[x]</paragraph>"
    }
  }
}

/// A token or segmented text node beside a plain one, in the `removeText`
/// suites.
struct RemoveTextCase: Sendable, CustomTestStringConvertible {
  /// The two texts and which one has `mode`.
  let leading: String
  let trailing: String
  let mode: TextModeType
  let modeOnLeading: Bool
  let title: String
  /// Anchor and focus: node index (0 leading, 1 trailing) and offset, `-1`
  /// for the node's end.
  let anchor: (Int, Int)
  let focus: (Int, Int)
  let leadingAttached: Bool
  let trailingAttached: Bool
  let remainingTexts: [String]?
  /// Where the collapsed selection ends: `nil` for the paragraph, else a
  /// node index (2 for the first remaining text node), and the offset.
  let anchorNode: Int?
  let anchorOffset: Int
  var testDescription: String { "\(mode) \(modeOnLeading ? "leading" : "trailing"): \(title)" }

  var upstreamDescribe: String {
    switch (mode, modeOnLeading) {
    case ("token", false): "with a leading TextNode and a trailing token TextNode"
    case ("token", true): "with a leading token TextNode and a trailing TextNode"
    default: "with a leading TextNode and a trailing segmented TextNode"
    }
  }

  static let all: [RemoveTextCase] = {
    func trailingToken(
      _ title: String, _ anchor: (Int, Int), _ focus: (Int, Int), _ attached: (Bool, Bool),
      _ texts: [String]?, _ node: Int?, _ offset: Int
    ) -> RemoveTextCase {
      RemoveTextCase(
        leading: "leading text", trailing: "token text", mode: "token", modeOnLeading: false,
        title: title, anchor: anchor, focus: focus, leadingAttached: attached.0,
        trailingAttached: attached.1, remainingTexts: texts, anchorNode: node,
        anchorOffset: offset)
    }
    func leadingToken(
      _ title: String, _ anchor: (Int, Int), _ focus: (Int, Int), _ attached: (Bool, Bool),
      _ texts: [String]?, _ node: Int?, _ offset: Int
    ) -> RemoveTextCase {
      RemoveTextCase(
        leading: "token text", trailing: "trailing text", mode: "token", modeOnLeading: true,
        title: title, anchor: anchor, focus: focus, leadingAttached: attached.0,
        trailingAttached: attached.1, remainingTexts: texts, anchorNode: node,
        anchorOffset: offset)
    }
    func trailingSegmented(
      _ title: String, _ anchor: (Int, Int), _ focus: (Int, Int), _ attached: (Bool, Bool),
      _ texts: [String]?, _ node: Int?, _ offset: Int
    ) -> RemoveTextCase {
      RemoveTextCase(
        leading: "leading text", trailing: "segmented text", mode: "segmented",
        modeOnLeading: false, title: title, anchor: anchor, focus: focus,
        leadingAttached: attached.0, trailingAttached: attached.1, remainingTexts: texts,
        anchorNode: node, anchorOffset: offset)
    }
    return [
      trailingToken("remove all text", (0, 0), (1, -1), (false, false), [], nil, 0),
      trailingToken("remove initial TextNode", (0, 0), (0, -1), (false, true), nil, 1, 0),
      trailingToken("remove trailing token TextNode", (1, 0), (1, -1), (true, false), nil, 0, -1),
      trailingToken(
        "remove initial TextNode and partial token TextNode", (0, 0), (1, 6), (false, false), [],
        nil, 0),
      trailingToken(
        "remove partial initial TextNode and partial token TextNode", (0, 4), (1, 6),
        (true, false), ["lead"], 0, 4),
      leadingToken("remove all text", (0, 0), (1, -1), (false, false), [], nil, 0),
      leadingToken("remove trailing TextNode", (1, 0), (1, -1), (true, false), nil, 0, -1),
      leadingToken("remove leading token TextNode", (0, 0), (0, -1), (false, true), nil, 1, 0),
      leadingToken(
        "remove partial leading token TextNode and trailing TextNode", (0, 6), (1, -1),
        (false, false), [], nil, 0),
      leadingToken(
        "remove partial token TextNode and partial trailing TextNode", (0, 6), (1, 5),
        (false, true), ["ing text"], 1, 0),
      trailingSegmented("remove all text", (0, 0), (1, -1), (false, false), [], nil, 0),
      trailingSegmented("remove initial TextNode", (0, 0), (0, -1), (false, true), nil, 1, 0),
      trailingSegmented(
        "remove trailing segmented TextNode", (1, 0), (1, -1), (true, false), nil, 0, -1),
      trailingSegmented(
        "remove initial TextNode and partial segmented TextNode", (0, 0), (1, 10),
        (false, false), ["text"], 2, 0),
      trailingSegmented(
        "remove partial initial TextNode and partial segmented TextNode", (0, 4), (1, 10),
        (true, false), ["lead", "text"], 0, 4),
    ]
  }()
}

@Suite struct LexicalSelectionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Start-of-paragraph inline elements > Can insert text before a start-of-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Start-of-paragraph inline elements > Can insert text before a start-of-paragraph inline element, using insertNodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Mid-paragraph inline elements > Can insert text before a mid-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > Mid-paragraph inline elements > Can insert text before a mid-paragraph inline element, using insertNodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > End-of-paragraph inline elements > Can insert text before an end-of-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text before inline elements > End-of-paragraph inline elements > Can insert text before an end-of-paragraph inline element, using insertNodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Start-of-paragraph inline elements > Can insert text after a start-of-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Start-of-paragraph inline elements > Can insert text after a start-of-paragraph inline element, using insertNodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Mid-paragraph inline elements > Can insert text after a mid-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > Mid-paragraph inline elements > Can insert text after a mid-paragraph inline element, using insertNodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > End-of-paragraph inline elements > Can insert text after an end-of-paragraph inline element, using insertText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > Inserting text either side of inline elements > Inserting text after inline elements > End-of-paragraph inline elements > Can insert text after an end-of-paragraph inline element, using insertNodes"
    ),
    arguments: InlineInsertCase.all)
  func insertTextBesideInlineElement(_ c: InlineInsertCase) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let root = try getRoot().clear()
      let paragraph = try createParagraphNode()
      switch c.position {
      case .startOfParagraph:
        try paragraph.append(
          createTestLinkNode().append(createTextNode("a")), createTextNode("b"))
      case .midParagraph:
        try paragraph.append(
          createTextNode("a"), createTestLinkNode().append(createTextNode("b")),
          createTextNode("c"))
      case .endOfParagraph:
        try paragraph.append(createTextNode("a"), createTestLinkNode().append(createTextNode("b")))
      }
      try root.append(paragraph)
    }
    #expect(try shape(editor) == c.position.initialShape)
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let selection: RangeSelection
      switch (c.before, c.position) {
      case (true, .startOfParagraph):
        selection = try #require(try paragraph.getFirstChild() as? TestLinkNode).select(0, 0)
      case (true, _):
        selection = try #require(try paragraph.getFirstChild() as? TextNode).select(1, 1)
      case (false, .endOfParagraph):
        selection = try #require(try paragraph.getLastChild() as? TestLinkNode).select(1, 1)
      case (false, _):
        selection = try #require(try paragraph.getLastChild() as? TextNode).select(0, 0)
      }
      if c.method == .insertText {
        try selection.insertText("x")
      } else {
        try selection.insertNodes([createParagraphNode().append(createTextNode("x"))])
      }
    }
    #expect(try shape(editor) == c.expectedShape)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > insertText() > inserts into existing paragraph node when selection is on parent of paragraph"
    ))
  func insertTextAtRootElementPoint() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      try root.clear().append(createParagraphNode())
      let selection = createRangeSelection()
      try selection.anchor.set("root", 0, .element)
      try selection.focus.set("root", 0, .element)
      try setSelection(selection)
      try selection.insertText("text")
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getTextContent() == "text")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove all text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove initial TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove trailing token TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove initial TextNode and partial token TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing token TextNode > remove partial initial TextNode and partial token TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove all text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove trailing TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove leading token TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove partial leading token TextNode and trailing TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading token TextNode and a trailing TextNode > remove partial token TextNode and partial trailing TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove all text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove initial TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove trailing segmented TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove initial TextNode and partial segmented TextNode"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelection.test.ts::LexicalSelection tests > removeText > with a leading TextNode and a trailing segmented TextNode > remove partial initial TextNode and partial segmented TextNode"
    ),
    arguments: RemoveTextCase.all)
  func removeText(_ c: RemoveTextCase) throws {
    let editor = try createTestEditor()
    var nodes: [TextNode] = []
    var paragraph: ParagraphNode!
    try editor.update(discrete: true) {
      nodes = [try createTextNode(c.leading), try createTextNode(c.trailing)]
      try nodes[c.modeOnLeading ? 0 : 1].setMode(c.mode)
      paragraph = try createParagraphNode().append(contentsOf: nodes)
      try getRoot().clear().append(paragraph)
    }
    try editor.update(discrete: true) {
      func offset(_ point: (Int, Int)) throws -> Int {
        try point.1 == -1 ? nodes[point.0].getTextContentSize() : point.1
      }
      let sel = try nodes[0].select(0, 0).setTextNodeRange(
        nodes[c.anchor.0], offset(c.anchor), nodes[c.focus.0], offset(c.focus))
      #expect(try getSelection() === sel)
      try sel.removeText()
      #expect(try getSelection() === sel)
      #expect(try nodes[0].isAttached() == c.leadingAttached)
      #expect(try nodes[1].isAttached() == c.trailingAttached)
      let allTextNodes = try getRoot().getAllTextNodes()
      if let texts = c.remainingTexts {
        #expect(try allTextNodes.map { try $0.getTextContent() } == texts)
      } else {
        #expect(allTextNodes.count == 1)
      }
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.isCollapsed())
      let expectedKey: NodeKey
      var expectedOffset = c.anchorOffset
      switch c.anchorNode {
      case nil: expectedKey = paragraph.getKey()
      case 2?: expectedKey = allTextNodes[0].getKey()
      case let index?:
        expectedKey = nodes[index].getKey()
        if expectedOffset == -1 {
          expectedOffset = try nodes[index].getTextContentSize()
        }
      }
      #expect(selection.anchor.key == expectedKey)
      #expect(selection.anchor.offset == expectedOffset)
    }
    if c.mode == "segmented" && c.leadingAttached && c.remainingTexts?.count == 2 {
      try editor.read(.latest) { () throws in
        #expect(try getRoot().getAllTextNodes().map { try $0.getTextContent() } == ["leadtext"])
        #expect(try nodes[0].isAttached())
      }
    }
  }
}
