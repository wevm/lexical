import Lexical
import LexicalTesting
import Testing

// Port of the `$removeTextFromCaretRange` suite of
// caret/__tests__/unit/LexicalCaret.test.ts.

private let texts = ["first", "second", "third"]

/// `beforeEach` of `single block`: the texts, styled apart so that they do
/// not merge, in one paragraph.
private func singleBlockEditor() throws -> Editor {
  let editor = try createTestEditor()
  try update(editor) {
    let textNodes = try texts.map { try createTextNode($0).setStyle("color: --color-\($0)") }
    try getRoot().clear().append(createParagraphNode().append(contentsOf: textNodes))
  }
  return editor
}

/// `beforeEach` of `multiple blocks`: each text in its own paragraph.
private func multipleBlocksEditor() throws -> Editor {
  let editor = try createTestEditor()
  try update(editor) {
    let textNodes = try texts.map { try createTextNode($0).setStyle("color: --color-\($0)") }
    try getRoot().clear().append(contentsOf: textNodes.map { try createParagraphNode().append($0) })
  }
  return editor
}

/// Whether `caret` is a text caret in `origin` at `offset`, facing
/// `direction`.
private func isTextCaret(
  _ caret: PointCaret, _ direction: CaretDirection?, _ offset: Int, _ origin: LexicalNode
) -> Bool {
  caret.isText && (direction == nil || caret.direction == direction) && caret.offset == offset
    && caret.origin.is(origin)
}

private func pointIs(_ point: Point, _ key: NodeKey, _ offset: Int, _ type: PointType) -> Bool {
  point.key == key && point.offset == offset && point.type == type
}

private func sameNodes(_ a: [LexicalNode], _ b: [LexicalNode]) -> Bool {
  a.count == b.count && zip(a, b).allSatisfy { $0 === $1 }
}

/// `startOfNode`, `insideNode` and `endOfNode`.
enum EdgeOffset: String, CaseIterable, Sendable {
  case startOfNode, insideNode, endOfNode

  func offset(_ size: Int) -> Int {
    switch self {
    case .startOfNode: 0
    case .insideNode: 1
    case .endOfNode: size
    }
  }
}

struct BiasCase: Sendable, CustomTestStringConvertible {
  let direction: CaretDirection
  let anchorOutside: Bool
  let focusOutside: Bool
  let index: Int
  var testDescription: String {
    "\(texts[index]) node (\(direction) \(anchorOutside ? "outside" : "inside") \(focusOutside ? "outside" : "inside"))"
  }

  static let all: [BiasCase] = caretDirections.flatMap { direction in
    [(false, true), (true, false), (true, true)].flatMap { anchor, focus in
      texts.indices.map {
        BiasCase(direction: direction, anchorOutside: anchor, focusOutside: focus, index: $0)
      }
    }
  }
}

struct MultipleTextNodeCase: Sendable, CustomTestStringConvertible {
  let direction: CaretDirection
  let nodeIndexStart: Int
  let nodeIndexEnd: Int
  let start: EdgeOffset
  let end: EdgeOffset
  var testDescription: String {
    "\(direction) \(texts[nodeIndexStart]) \(start) \(texts[nodeIndexEnd]) \(end)"
  }

  static let all: [MultipleTextNodeCase] = caretDirections.flatMap { direction in
    [(0, 1), (0, 2), (1, 2)].flatMap { nodeIndexStart, nodeIndexEnd in
      EdgeOffset.allCases.flatMap { start in
        EdgeOffset.allCases.map { end in
          MultipleTextNodeCase(
            direction: direction, nodeIndexStart: nodeIndexStart, nodeIndexEnd: nodeIndexEnd,
            start: start, end: end)
        }
      }
    }
  }
}

/// The shared setup of the `multiple text node selection` tests: the range
/// from `startCaret` to `endCaret`, checked as upstream checks it.
private func multipleTextNodeRange(
  _ c: MultipleTextNodeCase, _ originalNodes: [TextNode]
) throws -> (startCaret: TextPointCaret, endCaret: TextPointCaret, range: CaretRange) {
  let startNode = originalNodes[c.nodeIndexStart]
  let endNode = originalNodes[c.nodeIndexEnd]
  #expect(startNode !== endNode)
  #expect(try startNode.isBefore(endNode))
  let startCaret = try getTextPointCaret(
    startNode, c.direction, c.start.offset(startNode.getTextContentSize()))
  let endCaret = try getTextPointCaret(
    endNode, c.direction, c.end.offset(endNode.getTextContentSize()))
  let (anchor, focus) = c.direction == .next ? (startCaret, endCaret) : (endCaret, startCaret)
  let range = try getCaretRange(anchor, focus)
  let slices = try nonNilSlices(range)
  #expect(slices.count == 2)
  let startText = try startCaret.origin.getTextContent().jsSlice(startCaret.offset)
  let endText = try endCaret.origin.getTextContent().jsSlice(0, endCaret.offset)
  #expect(
    try slices.map { try $0.getTextContent() }
      == (c.direction == .next ? [startText, endText] : [endText, startText]))
  return (startCaret, endCaret, range)
}

/// The shared ending of the `multiple text node selection` tests: which of
/// `originalNodes` survive, with what text.
private func expectRemainingNodes(
  _ c: MultipleTextNodeCase, _ originalNodes: [TextNode], _ startCaret: TextPointCaret,
  _ endCaret: TextPointCaret
) throws {
  let remainingNodes = try getRoot().getAllTextNodes()
  var newIndex = 0
  for (originalIndex, originalNode) in originalNodes.enumerated() {
    let originalText = texts[originalIndex]
    let deleted: Bool
    if originalIndex == c.nodeIndexStart {
      deleted = startCaret.offset == 0
      if !deleted {
        #expect(try originalNode.getTextContent() == originalText.jsSlice(0, startCaret.offset))
      }
    } else if originalIndex > c.nodeIndexStart && originalIndex < c.nodeIndexEnd {
      deleted = true
    } else if originalIndex == c.nodeIndexEnd {
      deleted = endCaret.offset == originalText.utf16.count
      if !deleted {
        #expect(try originalNode.getTextContent() == originalText.jsSlice(endCaret.offset))
      }
    } else {
      deleted = false
      #expect(try originalNode.getTextContent() == originalText)
    }
    #expect(try originalNode.isAttached() == !deleted)
    if !deleted {
      #expect(newIndex < remainingNodes.count && originalNode.is(remainingNodes[newIndex]))
    }
    newIndex += deleted ? 0 : 1
  }
  #expect(remainingNodes.count == newIndex)
}

@Suite struct LexicalCaretRemoveTextTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > ported LexicalSelection tests > remove partial initial TextNode and partial segmented TextNode"
    ))
  func removePartialInitialAndPartialSegmented() throws {
    let editor = try createTestEditor()
    var leadingText: TextNode!
    var trailingSegmentedText: TextNode!
    try editor.update(discrete: true) {
      let sel = createRangeSelection()
      leadingText = try createTextNode("leading text")
      trailingSegmentedText = try createTextNode("segmented text").setMode("segmented")
      try getRoot().clear().append(
        createParagraphNode().append(leadingText, trailingSegmentedText))
      try sel.anchor.set(leadingText.getKey(), "lead".utf16.count, .text)
      try sel.focus.set(trailingSegmentedText.getKey(), "segmented ".utf16.count, .text)
      try setSelection(sel)
      let resultRange = try removeTextFromCaretRange(caretRangeFromSelection(sel))
      try setSelectionFromCaretRange(resultRange)
      #expect(try isTextCaret(resultRange.anchor, nil, 4, leadingText.getLatest()))
      #expect(resultRange.direction == .next)
      #expect(try leadingText.isAttached())
      #expect(try trailingSegmentedText.isAttached() == false)
      #expect(try getRoot().getAllTextNodes().map { try $0.getTextContent() } == ["lead", "text"])
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.isCollapsed())
      #expect(selection.anchor.key == leadingText.getKey())
      #expect(selection.anchor.offset == 4)
    }
    try editor.read(.latest) { () throws in
      #expect(try getRoot().getAllTextNodes().map { try $0.getTextContent() } == ["leadtext"])
      #expect(try leadingText.isAttached())
      #expect(try trailingSegmentedText.isAttached() == false)
    }
  }

  // TestInlineElementNode (inline, canBeEmpty false) stands in for LinkNode.
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > remove second TextNode when wrapped in a LinkNode that will become empty"
    ))
  func removeSecondTextNodeInEmptiedLink() throws {
    let editor = try singleBlockEditor()
    try editor.update(discrete: true) {
      let sel = createRangeSelection()
      let originalNodes = try getRoot().getAllTextNodes()
      let leadingText = originalNodes[0]
      let trailingLinkText = originalNodes[1]
      let linkWrapper = try createTestInlineElementNode()
      try trailingLinkText.replace(linkWrapper)
      try linkWrapper.append(trailingLinkText)
      try sel.anchor.set(trailingLinkText.getKey(), 0, .text)
      try sel.focus.set(trailingLinkText.getKey(), trailingLinkText.getTextContentSize(), .text)
      let direction = CaretDirection.next
      let range = try caretRangeFromSelection(sel)
      try setSelection(sel)
      #expect(try isTextCaret(range.anchor, direction, 0, trailingLinkText.getLatest()))
      #expect(
        try isTextCaret(
          range.focus, direction, trailingLinkText.getTextContentSize(),
          trailingLinkText.getLatest()))
      let resultRange = try removeTextFromCaretRange(range)
      try setSelectionFromCaretRange(resultRange)
      #expect(try leadingText.isAttached())
      #expect(try trailingLinkText.isAttached() == false)
      #expect(try getRoot().getAllTextNodes().count == 2)
      #expect(resultRange.isCollapsed())
      #expect(sel.isCollapsed())
      #expect(try sel.anchor.getNode() === leadingText.getLatest())
      #expect(sel.anchor.key == leadingText.getKey())
      #expect(try sel.anchor.offset == leadingText.getTextContentSize())
      #expect(
        try isTextCaret(
          resultRange.anchor, direction, leadingText.getTextContentSize(), leadingText.getLatest()))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > remove first TextNode with second in token mode"
    ))
  func removeFirstTextNodeWithSecondInTokenMode() throws {
    let editor = try singleBlockEditor()
    try editor.update(discrete: true) {
      let sel = createRangeSelection()
      let originalNodes = try getRoot().getAllTextNodes()
      let leadingText = originalNodes[0]
      let trailingTokenText = originalNodes[1]
      try trailingTokenText.setMode("token")
      try sel.anchor.set(leadingText.getKey(), 0, .text)
      try sel.focus.set(leadingText.getKey(), leadingText.getTextContentSize(), .text)
      let direction = CaretDirection.next
      let range = try caretRangeFromSelection(sel)
      try setSelection(sel)
      #expect(isTextCaret(range.anchor, direction, 0, leadingText))
      #expect(
        try isTextCaret(range.focus, direction, leadingText.getTextContentSize(), leadingText))
      let resultRange = try removeTextFromCaretRange(range)
      try setSelectionFromCaretRange(resultRange)
      #expect(try leadingText.isAttached() == false)
      #expect(try trailingTokenText.isAttached())
      #expect(try getRoot().getAllTextNodes().count == 2)
      #expect(resultRange.isCollapsed())
      #expect(sel.isCollapsed())
      #expect(sel.anchor.key == trailingTokenText.getKey())
      #expect(sel.anchor.offset == 0)
      #expect(try isTextCaret(resultRange.anchor, direction, 0, trailingTokenText.getLatest()))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > collapsed text point selection"
    ))
  func collapsedTextPointSelection() throws {
    let editor = try singleBlockEditor()
    try update(editor) {
      let textNodes = try getRoot().getAllTextNodes()
      let originalText = try getRoot().getTextContent()
      let node = textNodes[1]
      let cases: [(() throws -> RangeSelection, Int)] = [
        ({ try node.selectStart() }, 0),
        ({ try node.selectEnd() }, try node.getTextContentSize()),
        ({ try node.select(3, 3) }, 3),
      ]
      for (makeSelection, offset) in cases {
        let key = node.getKey()
        let selection = try makeSelection()
        #expect(pointIs(selection.anchor, key, offset, .text))
        #expect(pointIs(selection.focus, key, offset, .text))
        let range = try caretRangeFromSelection(selection)
        #expect(range.isCollapsed())
        #expect(range.anchor.isText && range.focus.isText)
        func matchesOriginal(_ range: CaretRange) -> Bool {
          range.anchor.direction == .next && range.anchor.offset == offset
            && range.focus.direction == .next && range.focus.offset == offset
        }
        #expect(matchesOriginal(range))
        let slices = try nonNilSlices(range)
        #expect(slices.count == 1)
        #expect(sliceMatches(slices.first, .next, offset, node, 0))
        #expect(try range.iterNodeCarets(.root).toArray().isEmpty)
        #expect(matchesOriginal(try removeTextFromCaretRange(range)))
        #expect(try getRoot().getTextContent() == originalText)
        try setSelection(nil)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > first node (next)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > second node (next)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > third node (next)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > first node (previous)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > second node (previous)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node internal selection > third node (previous)"
    ),
    arguments: caretDirections, texts.indices)
  func fullTextNodeInternalSelection(_ direction: CaretDirection, _ i: Int) throws {
    let editor = try singleBlockEditor()
    try update(editor) {
      var originalNodes = try getRoot().getAllTextNodes()
      let node = originalNodes.remove(at: i)
      let key = node.getKey()
      let size = try node.getTextContentSize()
      let (anchorOffset, focusOffset) = direction == .next ? (0, size) : (size, 0)
      let selection = try node.select(anchorOffset, focusOffset)
      #expect(pointIs(selection.anchor, key, anchorOffset, .text))
      #expect(pointIs(selection.focus, key, focusOffset, .text))
      let range = try caretRangeFromSelection(selection)
      #expect(isTextCaret(range.anchor, nil, anchorOffset, node))
      #expect(isTextCaret(range.focus, nil, focusOffset, node))
      #expect(range.direction == direction)
      let slices = try nonNilSlices(range)
      #expect(slices.count == 1)
      #expect(
        slices.first.map {
          $0.caret.offset == (direction == .next ? 0 : size) && $0.caret.origin.is(node)
            && $0.distance == (direction == .next ? 1 : -1) * size
        } == true)
      #expect(try range.iterNodeCarets(.root).toArray().isEmpty)
      #expect(range.isCollapsed() == false)
      let resultRange = try removeTextFromCaretRange(range)
      try setSelection(nil)
      let remainingNodes = try getRoot().getAllTextNodes()
      #expect(sameNodes(remainingNodes, try originalNodes.map { try $0.getLatest() }))
      #expect(
        try remainingNodes.map { try $0.getTextContent() }
          == texts.enumerated().filter { $0.offset != i }.map(\.element))
      #expect(resultRange.isCollapsed())
      let newOrigin = remainingNodes[max(0, i - 1)]
      let offset = try i == 0 ? 0 : newOrigin.getTextContentSize()
      #expect(isTextCaret(resultRange.anchor, direction, offset, newOrigin))
      #expect(isTextCaret(resultRange.focus, direction, offset, newOrigin))
      #expect(resultRange.direction == direction)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (next outside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (next outside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (next outside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous inside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous outside inside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > first node (previous outside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > second node (previous outside outside)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > full text node biased selection > third node (previous outside outside)"
    ),
    arguments: BiasCase.all)
  func fullTextNodeBiasedSelection(_ c: BiasCase) throws {
    let editor = try singleBlockEditor()
    let direction = c.direction
    let i = c.index
    try update(editor) {
      var originalNodes = try getRoot().getAllTextNodes()
      let node = originalNodes.remove(at: i)
      let size = try node.getTextContentSize()
      let (anchorOffset, focusOffset) = direction == .next ? (0, size) : (size, 0)
      let selection = try node.select(anchorOffset, focusOffset)
      let nodeCaret = getSiblingCaret(node, direction)
      let parent = try node.getParentOrThrow()
      func expectStartOfNode(_ point: Point) throws {
        if i == 0 {
          #expect(pointIs(point, parent.getKey(), 0, .element))
        } else {
          let adj = originalNodes[i - 1]
          #expect(pointIs(point, adj.getKey(), try adj.getTextContentSize(), .text))
        }
      }
      func expectEndOfNode(_ point: Point) throws {
        if i == texts.count - 1 {
          #expect(pointIs(point, parent.getKey(), try parent.getChildrenSize(), .element))
        } else {
          #expect(pointIs(point, originalNodes[i].getKey(), 0, .text))
        }
      }
      if c.anchorOutside {
        try setPointFromCaret(selection.anchor, rewindSiblingCaret(nodeCaret))
        try direction == .next
          ? expectStartOfNode(selection.anchor) : expectEndOfNode(selection.anchor)
      }
      if c.focusOutside {
        try setPointFromCaret(selection.focus, getSiblingCaret(node, direction).getFlipped())
        try direction == .next
          ? expectEndOfNode(selection.focus) : expectStartOfNode(selection.focus)
      }
      let range = try caretRangeFromSelection(selection)
      #expect(range.isCollapsed() == false)
      let bothOutside = c.anchorOutside && c.focusOutside
      #expect(try range.iterNodeCarets(.root).toArray().count == (bothOutside ? 1 : 0))
      let slices = try nonNilSlices(range).filter { $0.distance != 0 }
      if bothOutside {
        #expect(slices.isEmpty)
      } else if (!c.anchorOutside) == (direction == .next) {
        #expect(slices.count == 1 && slices[0].caret.offset == 0 && slices[0].distance == size)
      } else {
        #expect(
          slices.count == 1 && slices[0].caret.offset == size && slices[0].distance == -size)
      }
      let resultRange = try removeTextFromCaretRange(range)
      try setSelection(nil)
      let remainingNodes = try getRoot().getAllTextNodes()
      #expect(
        try remainingNodes.map { try $0.getTextContent() }
          == texts.enumerated().filter { $0.offset != i }.map(\.element))
      #expect(sameNodes(remainingNodes, try originalNodes.map { try $0.getLatest() }))
      #expect(resultRange.isCollapsed())
      let newOrigin = remainingNodes[max(0, i - 1)]
      let offset = try i == 0 ? 0 : newOrigin.getTextContentSize()
      #expect(isTextCaret(resultRange.anchor, direction, offset, newOrigin))
      #expect(isTextCaret(resultRange.focus, direction, offset, newOrigin))
      #expect(resultRange.direction == direction)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 0:-1"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 1:-1"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > next 1:0"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 0:-1"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 1:-1"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > single text node non-empty partial selection > previous 1:0"
    ),
    arguments: caretDirections, [(0, 1), (1, 1), (1, 0)])
  func singleTextNodePartialSelection(
    _ direction: CaretDirection, _ edges: (Int, Int)
  ) throws {
    let (anchorEdgeOffset, focusEdgeOffset) = edges
    let editor = try singleBlockEditor()
    try update(editor) {
      let originalNodes = try getRoot().getAllTextNodes()
      let i = 0
      let text = texts[i]
      let node = originalNodes[i]
      let size = try node.getTextContentSize()
      let anchor = try getTextPointCaret(
        node, direction, direction == .next ? anchorEdgeOffset : size - anchorEdgeOffset)
      let focus = try getTextPointCaret(
        node, direction, direction == .next ? size - focusEdgeOffset : focusEdgeOffset)
      let offsetStart = min(anchor.offset, focus.offset)
      let offsetEnd = max(anchor.offset, focus.offset)
      let range = try getCaretRange(anchor, focus)
      let slices = try nonNilSlices(range).filter { $0.distance != 0 }
      #expect(try range.iterNodeCarets(.root).toArray().isEmpty)
      try #require(slices.count == 1)
      let slice = slices[0]
      #expect(
        slice.distance == (direction == .next ? 1 : -1)
          * (size - anchorEdgeOffset - focusEdgeOffset))
      #expect(try slice.getTextContent() == text.jsSlice(offsetStart, offsetEnd))
      let resultRange = try removeTextFromCaretRange(range)
      try setSelection(nil)
      #expect(resultRange.isCollapsed())
      #expect(try isTextCaret(resultRange.anchor, direction, offsetStart, node.getLatest()))
      let remainingNodes = try getRoot().getAllTextNodes()
      #expect(remainingNodes.count == texts.count)
      #expect(
        try remainingNodes.map { try $0.getTextContent() }
          == texts.enumerated().map {
            i == $0.offset
              ? $0.element.jsSlice(0, offsetStart) + $0.element.jsSlice(offsetEnd) : $0.element
          })
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next first endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > next second endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous first endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > single block > multiple text node selection > previous second endOfNode third endOfNode"
    ),
    arguments: MultipleTextNodeCase.all)
  func singleBlockMultipleTextNodeSelection(_ c: MultipleTextNodeCase) throws {
    let editor = try singleBlockEditor()
    try update(editor) {
      let originalNodes = try getRoot().getAllTextNodes()
      let (startCaret, endCaret, range) = try multipleTextNodeRange(c, originalNodes)
      #expect(
        try range.iterNodeCarets(.root).toArray().count
          == max(0, c.nodeIndexEnd - c.nodeIndexStart - 1))
      let resultRange = try removeTextFromCaretRange(range)
      #expect(resultRange.anchor.direction == c.direction)
      #expect(resultRange.direction == c.direction)
      #expect(resultRange.focus.direction == c.direction)
      if startCaret.offset != 0 {
        #expect(
          try isTextCaret(resultRange.anchor, nil, startCaret.offset, startCaret.origin.getLatest())
        )
      } else if c.nodeIndexStart > 0 {
        let prevNode = try originalNodes[c.nodeIndexStart - 1].getLatest()
        #expect(try isTextCaret(resultRange.anchor, nil, prevNode.getTextContentSize(), prevNode))
      } else if endCaret.offset != texts[c.nodeIndexEnd].utf16.count {
        #expect(
          try isTextCaret(resultRange.anchor, nil, 0, originalNodes[c.nodeIndexEnd].getLatest()))
      } else if c.nodeIndexEnd != texts.count - 1 {
        #expect(
          try isTextCaret(resultRange.anchor, nil, 0, originalNodes[c.nodeIndexEnd + 1].getLatest())
        )
      } else {
        #expect(resultRange.anchor.isChild)
        #expect(resultRange.anchor.origin.is(try getRoot().getFirstChild()))
      }
      try expectRemainingNodes(c, originalNodes, startCaret, endCaret)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next first endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > next second endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode second endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous first endOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second startOfNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second insideNode third endOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third startOfNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third insideNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $removeTextFromCaretRange > multiple blocks > multiple text node selection > previous second endOfNode third endOfNode"
    ),
    arguments: MultipleTextNodeCase.all)
  func multipleBlocksMultipleTextNodeSelection(_ c: MultipleTextNodeCase) throws {
    let editor = try multipleBlocksEditor()
    try update(editor) {
      let originalNodes = try getRoot().getAllTextNodes()
      let (startCaret, endCaret, range) = try multipleTextNodeRange(c, originalNodes)
      let (anchor, focus) = c.direction == .next ? (startCaret, endCaret) : (endCaret, startCaret)
      let originalStartParent = try #require(try startCaret.getParentAtCaret())
      let originalEndParent = try #require(try endCaret.getParentAtCaret())
      let resultRange = try removeTextFromCaretRange(range)
      #expect(resultRange.direction == c.direction)
      if c.direction == .next {
        if anchor.offset != 0 {
          #expect(
            try isTextCaret(
              resultRange.anchor, c.direction, anchor.offset, anchor.origin.getLatest()))
        } else if focus.offset != texts[c.nodeIndexEnd].utf16.count {
          #expect(
            try isTextCaret(
              resultRange.anchor, c.direction, 0, originalNodes[c.nodeIndexEnd].getLatest()))
        } else {
          #expect(resultRange.anchor.isChild && resultRange.anchor.direction == c.direction)
          #expect(resultRange.anchor.origin.is(try originalStartParent.getLatest()))
        }
      }
      #expect(
        try resultRange.anchor.getParentAtCaret()?.getLatest() === originalStartParent.getLatest())
      #expect(try originalEndParent.isAttached() == false)
      #expect(try !focus.origin.isAttached() || originalStartParent.is(focus.origin.getParent()))
      try expectRemainingNodes(c, originalNodes, startCaret, endCaret)
    }
  }
}
