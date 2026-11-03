import Lexical
import LexicalTesting
import Testing

// Port of the `$getChildCaret`, `$getSiblingCaret` and
// `$caretRangeFromSelection` suites of caret/__tests__/unit/LexicalCaret.test.ts.

extension CaretDirection: CustomTestStringConvertible {
  public var testDescription: String { rawValue }
}

let caretDirections: [CaretDirection] = [.next, .previous]

/// The texts of `paragraph`'s latest children.
func childTexts(_ paragraph: ElementNode) throws -> [String] {
  try paragraph.getLatest().getChildren().map { try $0.getTextContent() }
}

/// Three token-mode text nodes, `first`, `second` and `third`, in one
/// paragraph that replaces the root's children.
func tokenTextNodes() throws -> [TextNode] {
  let textNodes = try ["first", "second", "third"].map { try createTextNode($0).setMode("token") }
  try getRoot().clear().append(createParagraphNode().append(contentsOf: textNodes))
  return textNodes
}

/// Whether `slice` is in `origin` at `offset`, facing `direction`, over
/// `distance`.
func sliceMatches(
  _ slice: TextPointCaretSlice?, _ direction: CaretDirection, _ offset: Int,
  _ origin: LexicalNode, _ distance: Int
) -> Bool {
  guard let slice else { return false }
  return slice.caret.isText && slice.caret.direction == direction && slice.caret.offset == offset
    && slice.caret.origin.is(origin) && slice.distance == distance
}

func nonNilSlices(_ range: CaretRange) throws -> [TextPointCaretSlice] {
  let (a, b) = try range.getTextSlices()
  return [a, b].compactMap { $0 }
}

@Suite struct LexicalCaretTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getChildCaret > direction next"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getChildCaret > direction previous"
    ),
    arguments: caretDirections)
  func getChildCaretDirection(_ direction: CaretDirection) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let root = try getRoot()
      try root.clear().append(paragraph)
      let caret = getChildCaret(root, direction)
      #expect(root.is(caret.origin))
      #expect(caret.direction == direction)
      #expect(caret.kind == .child)
      #expect(paragraph.is(try caret.getNodeAtCaret()))
      #expect(root.is(try caret.getParentAtCaret()))
      let flipped = try caret.getFlipped()
      #expect(flipped.isSamePointCaret(caret) == false)
      #expect(try flipped.getFlipped().isSameNodeCaret(caret))
      #expect(flipped.direction != direction)
      #expect(flipped.kind == .sibling)
      #expect(try flipped.getNodeAtCaret() == nil)
      #expect(try flipped.getAdjacentCaret() == nil)
      for mode in [RootMode.root, .shadowRoot] {
        #expect(try caret.getParentCaret(mode) == nil)
        #expect(try flipped.getParentCaret(mode) == nil)
      }
      let adjacent = try #require(
        try caret.getAdjacentCaret(),
        "depth caret of a non-empty element must always have an adjacent caret")
      #expect(paragraph.is(adjacent.origin))
      #expect(adjacent.kind == .sibling)
      #expect(try adjacent.getAdjacentCaret() == nil)
      #expect(try root.getChildrenSize() == 1)
      try caret.remove()
      #expect(try root.isEmpty())
      try caret.replaceOrInsert(paragraph)
      #expect(try root.getChildrenSize() == 1)
      try caret.remove()
      try caret.insert(paragraph)
      #expect(try root.getChildrenSize() == 1)
      let secondParagraph = try createParagraphNode()
      try caret.insert(secondParagraph)
      #expect(try root.getChildrenSize() == 2)
      let paragraphKeys = [paragraph.getKey(), secondParagraph.getKey()]
      let reversed = Array(paragraphKeys.reversed())
      #expect(try root.getChildrenKeys() == (direction == .next ? reversed : paragraphKeys))
      try caret.splice(2, [])
      #expect(try root.getChildrenSize() == 0)
      try caret.splice(0, [paragraph, secondParagraph])
      #expect(try root.getChildrenKeys() == paragraphKeys)
      try caret.splice(0, [secondParagraph, paragraph])
      #expect(try root.getChildrenKeys() == reversed)
      try caret.splice(0, [paragraph, secondParagraph])
      #expect(try root.getChildrenKeys() == paragraphKeys)
      try caret.splice(2, [secondParagraph, paragraph])
      #expect(try root.getChildrenKeys() == reversed)
      try caret.splice(2, [paragraph, secondParagraph])
      #expect(try root.getChildrenKeys() == paragraphKeys)
      try caret.splice(20, [paragraph])
      #expect(try root.getChildrenKeys() == [paragraph.getKey()])
      try caret.splice(-1, [secondParagraph])
      #expect(try root.getChildrenKeys() == (direction == .next ? reversed : paragraphKeys))
      try caret.splice(Int.max, [paragraph, secondParagraph], direction)
      #expect(try root.getChildrenKeys() == (direction == .next ? paragraphKeys : reversed))
      let keys = try root.getChildrenKeys()
      #expect(
        try caret.siblings().map { $0.origin.getKey() }
          == (direction == .next ? keys : keys.reversed()))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getSiblingCaret > direction next"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $getSiblingCaret > direction previous"
    ),
    arguments: caretDirections)
  func getSiblingCaretDirection(_ direction: CaretDirection) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let tokens = try ["-2", "-1", "0", "1", "2"].map { try createTextNode($0).setMode("token") }
      let root = try getRoot()
      try root.clear().append(paragraph.append(contentsOf: tokens))
      let zeroIndex = 2
      let zToken = tokens[zeroIndex]
      let nextToken = try #require(
        try direction == .next ? zToken.getNextSibling() : zToken.getPreviousSibling())
      let caret = getSiblingCaret(zToken, direction)
      let sign = direction == .next ? 1 : -1
      func signedTexts(_ caret: SiblingCaret) throws -> [Int] {
        try caret.siblings().map { sign * Int(try $0.origin.getTextContent())! }
      }
      #expect(zToken.is(caret.origin))
      #expect(caret.direction == direction)
      #expect(caret.kind == .sibling)
      #expect(nextToken.is(try caret.getNodeAtCaret()))
      #expect(paragraph.is(try caret.getParentAtCaret()))
      #expect(try signedTexts(caret) == [1, 2])
      let flipped = try caret.getFlipped()
      #expect(flipped.isSamePointCaret(caret) == false)
      #expect(flipped.origin.is(try caret.getNodeAtCaret()))
      #expect(flipped.direction != direction)
      #expect(flipped.kind == .sibling)
      #expect(zToken.is(try flipped.getNodeAtCaret()))
      let flippedAdjacent = try #require(
        try flipped.getAdjacentCaret(),
        "A flipped SiblingCaret always has an adjacent caret because it points back to the origin")
      #expect(flippedAdjacent.origin.is(caret.origin))
      for mode in [RootMode.root, .shadowRoot] {
        #expect(
          getSiblingCaret(paragraph, caret.direction).isSameNodeCaret(
            try caret.getParentCaret(mode)))
        #expect(
          getSiblingCaret(paragraph, flipped.direction).isSameNodeCaret(
            try flipped.getParentCaret(mode)))
      }
      let adjacent = try #require(try caret.getAdjacentCaret(), "expecting adjacent caret")
      let offset = sign
      let offsetToken = tokens[zeroIndex + offset]
      #expect(offsetToken.is(adjacent.origin))
      #expect(adjacent.kind == .sibling)
      #expect(try adjacent.origin.getTextContent() == String(offset))
      #expect(try offsetToken.isAttached())
      #expect(offsetToken.is(try caret.getNodeAtCaret()))
      #expect(try paragraph.getChildrenSize() == tokens.count)
      try caret.remove()
      #expect(try paragraph.getChildrenSize() == tokens.count - 1)
      #expect(try offsetToken.isAttached() == false)
      #expect(tokens[zeroIndex + 2 * offset].is(try caret.getNodeAtCaret()))
      #expect(try signedTexts(caret) == [2])
      let tokenTexts = try tokens.map { try $0.getTextContent() }
      #expect(try childTexts(paragraph) == tokenTexts.filter { $0 != String(offset) })
      try caret.insert(offsetToken)
      #expect(try childTexts(paragraph) == tokenTexts)
      try caret.replaceOrInsert(offsetToken)
      #expect(try childTexts(paragraph) == tokenTexts)
      try caret.replaceOrInsert(createTextNode("replaced!"))
      #expect(
        try childTexts(paragraph)
          == tokenTexts.enumerated().map {
            $0.offset == zeroIndex + offset ? "replaced!" : $0.element
          })
      try caret.replaceOrInsert(offsetToken)
      let abNodes = try ["a", "b"].map { try createTextNode($0) }
      func around(_ zero: [String], dropping drop: Int? = nil) -> [String] {
        tokenTexts.enumerated().flatMap { i, text -> [String] in
          if i == drop { return [] }
          return i == zeroIndex ? zero : [text]
        }
      }
      try caret.splice(0, abNodes)
      #expect(
        try childTexts(paragraph)
          == around(direction == .next ? ["0", "a", "b"] : ["a", "b", "0"]))
      for node in abNodes { try node.remove() }
      try caret.splice(0, abNodes, .previous)
      #expect(
        try childTexts(paragraph)
          == around(direction == .next ? ["0", "b", "a"] : ["b", "a", "0"]))
      for node in abNodes { try node.remove() }
      try caret.splice(1, abNodes)
      #expect(
        try childTexts(paragraph)
          == around(
            direction == .next ? ["0", "a", "b"] : ["a", "b", "0"], dropping: zeroIndex + offset))
      try paragraph.clear().append(contentsOf: tokens)
      try caret.splice(1, Array(abNodes.prefix(1)))
      #expect(
        try childTexts(paragraph)
          == tokenTexts.enumerated().map { $0.offset == zeroIndex + offset ? "a" : $0.element })
      try paragraph.clear().append(contentsOf: tokens)
      try caret.splice(2, Array(abNodes.prefix(1)))
      #expect(
        try childTexts(paragraph)
          == (direction == .next ? ["-2", "-1", "0", "a"] : ["a", "0", "1", "2"]))
      try paragraph.clear().append(contentsOf: tokens)
      try caret.splice(Int.max, abNodes)
      #expect(
        try childTexts(paragraph)
          == (direction == .next ? ["-2", "-1", "0", "a", "b"] : ["a", "b", "0", "1", "2"]))
      try paragraph.clear().append(contentsOf: tokens)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > collapsed text point selection"
    ))
  func caretRangeFromCollapsedSelection() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let node = try tokenTextNodes()[1]
      let cases: [(() throws -> RangeSelection, Int)] = [
        ({ try node.selectStart() }, 0),
        ({ try node.selectEnd() }, try node.getTextContentSize()),
        ({ try node.select(3, 3) }, 3),
      ]
      for (makeSelection, offset) in cases {
        let key = node.getKey()
        let selection = try makeSelection()
        for point in [selection.anchor, selection.focus] {
          #expect(point.key == key && point.offset == offset && point.type == .text)
        }
        let range = try caretRangeFromSelection(selection)
        #expect(range.isCollapsed())
        #expect(range.anchor.isText && range.focus.isText)
        #expect(range.anchor.direction == .next && range.anchor.offset == offset)
        #expect(range.focus.direction == .next && range.focus.offset == offset)
        let slices = try nonNilSlices(range)
        #expect(slices.count == 1)
        #expect(sliceMatches(slices.first, .next, offset, node, 0))
        #expect(try range.iterNodeCarets(.root).isEmpty)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > full text node selection (next)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > full text node selection (previous)"
    ),
    arguments: caretDirections)
  func caretRangeFromFullTextNodeSelection(_ direction: CaretDirection) throws {
    let editor = try createTestEditor()
    try update(editor) {
      for node in try tokenTextNodes() {
        let key = node.getKey()
        let textSize = try node.getTextContentSize()
        let (anchorOffset, focusOffset) = direction == .next ? (0, textSize) : (textSize, 0)
        let selection = try node.select(anchorOffset, focusOffset)
        #expect(selection.anchor.key == key && selection.anchor.offset == anchorOffset)
        #expect(selection.focus.key == key && selection.focus.offset == focusOffset)
        let range = try caretRangeFromSelection(selection)
        #expect(range.anchor.isText && range.focus.isText)
        #expect(range.direction == direction)
        #expect(range.anchor.direction == direction && range.anchor.offset == anchorOffset)
        #expect(range.anchor.origin.is(node) && range.focus.origin.is(node))
        #expect(range.focus.direction == direction && range.focus.offset == focusOffset)
        let slices = try nonNilSlices(range)
        #expect(slices.count == 1)
        #expect(
          sliceMatches(slices.first, direction, anchorOffset, node, focusOffset - anchorOffset))
        #expect(try range.iterNodeCarets(.root).isEmpty)
        #expect(range.isCollapsed() == false)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > single text node non-empty selection"
    ))
  func caretRangeFromSingleTextNodeSelection() throws {
    let editor = try createTestEditor()
    try update(editor) {
      for node in try tokenTextNodes() {
        let textSize = try node.getTextContentSize()
        for indexStart in 0..<textSize {
          for indexEnd in (indexStart + 1)...textSize {
            for direction in caretDirections {
              let (offset, size) =
                direction == .next
                ? (indexStart, indexEnd - indexStart) : (indexEnd, indexStart - indexEnd)
              let selection =
                try direction == .next
                ? node.select(indexStart, indexEnd) : node.select(indexEnd, indexStart)
              let range = try caretRangeFromSelection(selection)
              #expect(range.anchor.isText && range.focus.isText)
              #expect(range.direction == direction)
              let slices = try nonNilSlices(range)
              #expect(slices.count == 1)
              #expect(sliceMatches(slices.first, direction, offset, node, size))
              #expect(try range.iterNodeCarets(.root).isEmpty)
            }
          }
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > multiple text node non-empty selection (next)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $caretRangeFromSelection > multiple text node non-empty selection (previous)"
    ),
    arguments: caretDirections)
  func caretRangeFromMultipleTextNodeSelection(_ direction: CaretDirection) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let textNodes = try tokenTextNodes()
      let selection = try getRoot().select()
      let nodeCount = textNodes.count
      for indexNodeStart in 0..<nodeCount {
        for indexNodeEnd in (indexNodeStart + 1)..<max(nodeCount, indexNodeStart + 1) {
          let startNode = textNodes[indexNodeStart]
          let endNode = textNodes[indexNodeEnd]
          for indexStart in [0, 1, try startNode.getTextContentSize()] {
            for indexEnd in [0, 1, try endNode.getTextContentSize()] {
              let (anchorNode, anchorOffset, focusNode, focusOffset) =
                direction == .next
                ? (startNode, indexStart, endNode, indexEnd)
                : (endNode, indexEnd, startNode, indexStart)
              try selection.setTextNodeRange(anchorNode, anchorOffset, focusNode, focusOffset)
              let range = try caretRangeFromSelection(selection)
              #expect(range.anchor.isText && range.focus.isText)
              #expect(range.direction == direction)
              let slices = try nonNilSlices(range)
              try #require(slices.count == 2)
              #expect(
                sliceMatches(
                  slices[0], direction, anchorOffset, anchorNode,
                  direction == .next
                    ? try anchorNode.getTextContentSize() - anchorOffset : 0 - anchorOffset))
              #expect(
                sliceMatches(
                  slices[1], direction, focusOffset, focusNode,
                  direction == .next
                    ? 0 - focusOffset : try focusNode.getTextContentSize() - focusOffset))
              let carets = try range.iterNodeCarets(.root)
              let expected = Array(textNodes[(indexNodeStart + 1)..<indexNodeEnd])
              #expect(carets.count == expected.count)
              for (caret, origin) in zip(carets, expected) {
                #expect(caret.direction == direction && caret.origin.is(origin))
                #expect(caret.kind == .sibling)
              }
            }
          }
        }
      }
    }
  }
}
