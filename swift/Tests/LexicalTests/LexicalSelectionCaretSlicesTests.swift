import Lexical
import LexicalTesting
import Testing

// Port of src/__tests__/unit/LexicalSelectionCaretSlices.test.ts. The two
// styling tests use $patchStyleText from @lexical/selection and wait for that
// package.

private func formatOrExtract(_ selection: RangeSelection, _ operation: String) throws {
  if operation == "format" {
    try selection.formatText("bold")
  } else {
    _ = try selection.extract()
  }
}

/// `[start, end]`: the anchor and focus, or the focus and anchor when
/// backward.
private func ends(_ selection: RangeSelection, backward: Bool) -> (Point, Point) {
  backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
}

private func isSameSelection(_ a: (any BaseSelection)?, _ b: RangeSelection) -> Bool {
  (a as? RangeSelection) === b
}

struct ElementStartCase: Sendable, CustomTestStringConvertible {
  let backward: Bool
  let active: Bool
  let preceding: Bool
  let endOffset: Int
  var testDescription: String {
    "backward=\(backward), active=\(active), preceding=\(preceding), end=\(endOffset)"
  }
  static let all: [ElementStartCase] = [
    ElementStartCase(backward: false, active: false, preceding: false, endOffset: 1),
    ElementStartCase(backward: false, active: false, preceding: false, endOffset: 3),
    ElementStartCase(backward: false, active: false, preceding: true, endOffset: 1),
    ElementStartCase(backward: false, active: false, preceding: true, endOffset: 3),
    ElementStartCase(backward: false, active: true, preceding: false, endOffset: 1),
    ElementStartCase(backward: false, active: true, preceding: false, endOffset: 3),
    ElementStartCase(backward: false, active: true, preceding: true, endOffset: 1),
    ElementStartCase(backward: false, active: true, preceding: true, endOffset: 3),
    ElementStartCase(backward: true, active: false, preceding: false, endOffset: 1),
    ElementStartCase(backward: true, active: false, preceding: false, endOffset: 3),
    ElementStartCase(backward: true, active: false, preceding: true, endOffset: 1),
    ElementStartCase(backward: true, active: false, preceding: true, endOffset: 3),
    ElementStartCase(backward: true, active: true, preceding: false, endOffset: 1),
    ElementStartCase(backward: true, active: true, preceding: false, endOffset: 3),
    ElementStartCase(backward: true, active: true, preceding: true, endOffset: 1),
    ElementStartCase(backward: true, active: true, preceding: true, endOffset: 3),
  ]
}

@Suite struct LexicalSelectionCaretSlicesTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > format of a detached text range repairs the active element point"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extract of a detached text range repairs the active element point"
    ),
    arguments: ["format", "extract"])
  func detachedTextRangeRepairsActivePoint(operation: String) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("abcde")
      let following = try createTextNode("after").setFormat("italic")
      let paragraph = try createParagraphNode().append(text, following)
      try getRoot().clear().append(paragraph)
      let active = try paragraph.select(2, 2)
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 1, .text)
      try selection.focus.set(text.getKey(), 4, .text)
      try formatOrExtract(selection, operation)
      #expect(try selection.getTextContent() == "bcd")
      #expect(isSameSelection(try getSelection(), active))
      #expect(active.isCollapsed())
      #expect(active.anchor.key == following.getKey())
      #expect(active.anchor.type == .text)
      #expect(active.anchor.offset == 5)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > format retains detached element endpoints after a split"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extract retains detached element endpoints after a split"
    ),
    arguments: ["format", "extract"])
  func detachedElementEndpointsAfterSplit(operation: String) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("abc")
      let second = try createTextNode("def").setFormat("italic")
      let paragraph = try createParagraphNode().append(first, second)
      let other = try createTextNode("other")
      try getRoot().clear().append(paragraph, try createParagraphNode().append(other))
      let active = try other.select(2, 2)
      let selection = createRangeSelection()
      try selection.anchor.set(first.getKey(), 1, .text)
      try selection.focus.set(paragraph.getKey(), 2, .element)
      try formatOrExtract(selection, operation)
      #expect(try selection.getTextContent() == "bcdef")
      let focusNode = try selection.focus.getNode()
      let focusSize =
        selection.focus.type == .element
        ? try (focusNode as! ElementNode).getChildrenSize() : try focusNode.getTextContentSize()
      #expect(selection.focus.offset <= focusSize)
      #expect(isSameSelection(try getSelection(), active))
      #expect(active.anchor.key == other.getKey())
      #expect(active.anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extracts through an element endpoint between children (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > extracts through an element endpoint between children (backward=true)"
    ),
    arguments: [false, true])
  func extractsThroughElementEndpoint(backward: Bool) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("abcd")
      let second = try createTextNode("efgh").setFormat("italic")
      let third = try createTextNode("ijkl").setFormat("bold")
      let paragraph = try createParagraphNode().append(first, second, third)
      try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      let (start, end) = ends(selection, backward: backward)
      try start.set(first.getKey(), 1, .text)
      try end.set(paragraph.getKey(), 2, .element)
      try setSelection(selection)
      #expect(
        try selection.extract().compactMap { $0 as? TextNode }.map { try $0.getTextContent() } == [
          "bcd", "efgh",
        ])
      #expect(try paragraph.getTextContent() == "abcdefghijkl")
      #expect(try selection.isBackward() == backward)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > does not extract text from an empty mixed-point range (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > does not extract text from an empty mixed-point range (backward=true)"
    ),
    arguments: [false, true])
  func emptyMixedPointRangeExtractsNothing(backward: Bool) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("abcd")
      let following = try createTextNode("efgh").setFormat("italic")
      let paragraph = try createParagraphNode().append(text, following)
      try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      let (elementPoint, textPoint) = ends(selection, backward: backward)
      try elementPoint.set(paragraph.getKey(), 1, .element)
      try textPoint.set(text.getKey(), 4, .text)
      try setSelection(selection)
      #expect(try selection.extract().isEmpty)
      #expect(
        try paragraph.getChildren().map(ObjectIdentifier.init)
          == [text, following].map(ObjectIdentifier.init))
      #expect(try text.getTextContent() == "abcd")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > pending format excludes a final node selected at offset zero"
    ))
  func pendingFormatExcludesFinalNodeAtZero() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("abcd").setFormat(IS_BOLD)
      let second = try createTextNode("efgh").setFormat(IS_ITALIC)
      let paragraph = try createParagraphNode().append(first, second)
      try getRoot().clear().append(paragraph)
      let selection = createRangeSelection()
      try selection.anchor.set(first.getKey(), 0, .text)
      try selection.focus.set(second.getKey(), 0, .text)
      selection.format = IS_BOLD
      try setSelection(selection)
      try selection.formatText("bold")
      #expect(try first.getFormat() == 0)
      #expect(try second.getFormat() == IS_ITALIC)
      #expect(selection.format == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting a detached selection retains its partial text range"
    ))
  func detachedSelectionRetainsPartialRange() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("abcde")
      let other = try createTextNode("other")
      let paragraph = try createParagraphNode().append(text)
      try getRoot().clear().append(paragraph, try createParagraphNode().append(other))
      let active = try other.select(2, 2)
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 4, .text)
      try selection.focus.set(text.getKey(), 1, .text)
      try selection.formatText("bold")
      let children = try paragraph.getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["a", "bcd", "e"])
      #expect(try children.map { try ($0 as? TextNode)?.getFormat() } == [0, IS_BOLD, 0])
      #expect(try selection.anchor.getNode().getTextContent() == "bcd")
      #expect(selection.anchor.offset == 3)
      #expect(selection.focus.offset == 0)
      #expect(isSameSelection(try getSelection(), active))
      #expect(active.anchor.key == other.getKey())
      #expect(active.anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=false, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=false, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=true, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=false, preceding=true, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=false, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=false, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=true, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=false, active=true, preceding=true, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=false, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=false, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=true, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=false, preceding=true, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=false, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=false, end=3)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=true, end=1)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionCaretSlices.test.ts::selection text slices > formatting retains an element start (backward=true, active=true, preceding=true, end=3)"
    ),
    arguments: ElementStartCase.all)
  func formattingRetainsElementStart(_ testCase: ElementStartCase) throws {
    let (backward, active, preceding, endOffset) = (
      testCase.backward, testCase.active, testCase.preceding, testCase.endOffset
    )
    let editor = try createTestEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("abc")
      let paragraph = try createParagraphNode()
      if preceding {
        try paragraph.append(try createTextNode("before").setFormat("italic"))
      }
      try paragraph.append(text, try createTextNode("def"))
      let other = try createTextNode("other")
      try getRoot().clear().append(paragraph, try createParagraphNode().append(other))
      let unrelated = try other.select(2, 2)
      let selection = createRangeSelection()
      let (start, end) = ends(selection, backward: backward)
      try start.set(paragraph.getKey(), preceding ? 1 : 0, .element)
      try end.set(text.getKey(), endOffset, .text)
      if active {
        try setSelection(selection)
      }
      let prefix = String("abc".prefix(endOffset))
      for expectedFormat in [IS_BOLD, 0] {
        try selection.formatText("bold")
        let formatted = try paragraph.getChildAtIndex(preceding ? 1 : 0)
        #expect(try (formatted as? TextNode)?.getFormat() == expectedFormat)
        #expect(try formatted?.getTextContent() == prefix)
        #expect(try selection.getTextContent() == prefix)
        #expect(start.key == (endOffset == 3 ? paragraph.getKey() : formatted?.getKey()))
        #expect(start.type == (endOffset == 3 ? .element : .text))
        #expect(start.offset == (endOffset == 3 && preceding ? 1 : 0))
        #expect(end.key == formatted?.getKey())
        #expect(end.offset == endOffset)
        #expect(try selection.isBackward() == backward)
        if !active {
          #expect(isSameSelection(try getSelection(), unrelated))
          #expect(unrelated.anchor.key == other.getKey())
          #expect(unrelated.anchor.offset == 2)
        }
      }
    }
  }
}
