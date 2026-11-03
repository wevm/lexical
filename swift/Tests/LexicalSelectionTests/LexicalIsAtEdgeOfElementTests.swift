import Lexical
import LexicalExtension
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts.

private func pointAt(_ node: LexicalNode, _ offset: Int, _ type: PointType) throws -> Point {
  let selection = createRangeSelection()
  try selection.anchor.set(node.getKey(), offset, type)
  return selection.anchor
}

private func runInEditor(
  nodes: [LexicalNodeConfig] = [], _ fn: @escaping () throws -> Void
) throws {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "@isAtEdgeOfElement-test", nodes: { nodes }))
  defer { editor.dispose() }
  try editor.update(discrete: true, fn)
}

@Suite struct LexicalIsAtEdgeOfElementTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > text point at the start/end of a block"
    ))
  func textPointAtBlockEdges() throws {
    try runInEditor { () throws in
      let block = try createParagraphNode()
      let text = try createTextNode("hello")
      try getRoot().append(try block.append(text))
      // Start of the block.
      #expect(try isAtEdgeOfElement(pointAt(text, 0, .text), block, .previous))
      #expect(try !isAtEdgeOfElement(pointAt(text, 0, .text), block, .next))
      // End of the block.
      #expect(try isAtEdgeOfElement(pointAt(text, 5, .text), block, .next))
      #expect(try !isAtEdgeOfElement(pointAt(text, 5, .text), block, .previous))
      // Middle of the text is at neither edge.
      #expect(try !isAtEdgeOfElement(pointAt(text, 2, .text), block, .previous))
      #expect(try !isAtEdgeOfElement(pointAt(text, 2, .text), block, .next))
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > a sibling between the point and the edge disqualifies it"
    ))
  func siblingDisqualifies() throws {
    try runInEditor { () throws in
      let block = try createParagraphNode()
      let a = try createTextNode("a")
      let b = try createTextNode("b")
      try getRoot().append(try block.append(a, b))
      // offset 0 of b is not the start (a precedes it).
      #expect(try !isAtEdgeOfElement(pointAt(b, 0, .text), block, .previous))
      // end of b is the end of the block.
      #expect(try isAtEdgeOfElement(pointAt(b, 1, .text), block, .next))
      // end of a is not the end (b follows it).
      #expect(try !isAtEdgeOfElement(pointAt(a, 1, .text), block, .next))
      // start of a is the start of the block.
      #expect(try isAtEdgeOfElement(pointAt(a, 0, .text), block, .previous))
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > an empty element is at both of its edges"
    ))
  func emptyElementAtBothEdges() throws {
    try runInEditor { () throws in
      let block = try createParagraphNode()
      try getRoot().append(block)
      #expect(try isAtEdgeOfElement(pointAt(block, 0, .element), block, .previous))
      #expect(try isAtEdgeOfElement(pointAt(block, 0, .element), block, .next))
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > element points at the leading/trailing child edge"
    ))
  func elementPointsAtChildEdges() throws {
    try runInEditor { () throws in
      let block = try createParagraphNode()
      let a = try createTextNode("a")
      let b = try createTextNode("b")
      try getRoot().append(try block.append(a, b))
      // Before the first child / after the last child.
      #expect(try isAtEdgeOfElement(pointAt(block, 0, .element), block, .previous))
      #expect(try isAtEdgeOfElement(pointAt(block, 2, .element), block, .next))
      // Between the two children is at neither edge.
      #expect(try !isAtEdgeOfElement(pointAt(block, 1, .element), block, .previous))
      #expect(try !isAtEdgeOfElement(pointAt(block, 1, .element), block, .next))
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > a point outside the element is never at its edge"
    ))
  func pointOutsideElement() throws {
    try runInEditor { () throws in
      let block1 = try createParagraphNode()
      let block2 = try createParagraphNode()
      let text1 = try createTextNode("one")
      let text2 = try createTextNode("two")
      try getRoot().append(try block1.append(text1), try block2.append(text2))
      #expect(try !isAtEdgeOfElement(pointAt(text1, 0, .text), block2, .previous))
      #expect(try !isAtEdgeOfElement(pointAt(text1, 3, .text), block2, .next))
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > resolves against a named slot value (slot boundary)"
    ))
  func namedSlotValue() throws {
    try runInEditor(nodes: [.klass(TestShadowRootNode.self)]) { () throws in
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      let slotPara = try createParagraphNode()
      let slotText = try createTextNode("slotted")
      try slotPara.append(slotText)
      try slot.append(slotPara)
      try host.append(try createTextNode("body"))
      try getRoot().append(host)
      try setSlot(host, "title", slot)
      // Start / end of the slot's content, against the slot value itself.
      #expect(try isAtEdgeOfElement(pointAt(slotText, 0, .text), slot, .previous))
      #expect(try isAtEdgeOfElement(pointAt(slotText, 7, .text), slot, .next))
      // Not at the opposite edges.
      #expect(try !isAtEdgeOfElement(pointAt(slotText, 0, .text), slot, .next))
      #expect(try !isAtEdgeOfElement(pointAt(slotText, 7, .text), slot, .previous))
      // Mid-text is at neither edge.
      #expect(try !isAtEdgeOfElement(pointAt(slotText, 3, .text), slot, .previous))
      #expect(try !isAtEdgeOfElement(pointAt(slotText, 3, .text), slot, .next))
    }
  }
}
