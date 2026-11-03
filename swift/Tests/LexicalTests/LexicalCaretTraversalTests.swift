import Lexical
import LexicalTesting
import Testing

// Port of caret/__tests__/unit/LexicalCaretTraversal.test.ts. JS takes any
// number as an index; the Swift API takes an Int, so -Infinity and Infinity
// are Int.min and Int.max (which clamp the same way), and the 0.5, 1.5 and
// NaN cases are skipped as deviations.

/// Four unmergeable text nodes "a" to "d" in a paragraph at the root.
private func fourTexts() throws -> (ParagraphNode, [TextNode]) {
  let children = try ["a", "b", "c", "d"].map { try createTextNode($0).toggleUnmergeable() }
  let parent = try createParagraphNode().append(contentsOf: children)
  try getRoot().clear().append(parent)
  return (parent, children)
}

private func ids(_ nodes: [LexicalNode]) -> [ObjectIdentifier] { nodes.map(ObjectIdentifier.init) }

@Suite struct LexicalCaretTraversalTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at -Infinity preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at -1 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 0 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 1 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 2 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 4 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at 5 preserves clamping and direction"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret at Infinity preserves clamping and direction"
    ),
    arguments: [Int.min, -1, 0, 1, 2, 4, 5, Int.max])
  func childCaretPreservesClampingAndDirection(_ index: Int) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (parent, children) = try fourTexts()
      let offset = index > 0 ? min(4, index) : 0
      let next = try getChildCaretAtIndex(parent, index, .next)
      let previous = try getChildCaretAtIndex(parent, index, .previous)
      #expect(try next.getNodeAtCaret() === (offset < 4 ? children[offset] : nil))
      #expect(try previous.getNodeAtCaret() === (offset > 0 ? children[offset - 1] : nil))
      #expect(try next.getFlipped().isSameNodeCaret(previous))
      #expect(try next.getParentAtCaret() === parent)
      #expect(try previous.getParentAtCaret() === parent)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at -Infinity stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at -1 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 0 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 1 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 2 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 4 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at 5 stays at its boundary"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::child caret in an empty element at Infinity stays at its boundary"
    ),
    arguments: [Int.min, -1, 0, 1, 2, 4, 5, Int.max])
  func childCaretInEmptyElementStaysAtBoundary(_ index: Int) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let parent = try createParagraphNode()
      try getRoot().clear().append(parent)
      for direction in [CaretDirection.next, .previous] {
        let boundary: ChildCaret = getChildCaret(parent as ElementNode, direction)
        #expect(try getChildCaretAtIndex(parent, index, direction).isSameNodeCaret(boundary))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::node collectors and retained carets follow moved siblings in document order"
    ))
  func collectorsFollowMovedSiblings() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (parent, children) = try fourTexts()
      let (a, b, c, d) = (children[0], children[1], children[2], children[3])
      let start: ChildCaret = getChildCaret(parent as ElementNode, .next)
      let end: ChildCaret = getChildCaret(parent as ElementNode, .previous)
      try b.insertBefore(d)
      #expect(try ids(parent.getChildren()) == ids([a, d, b, c]))
      #expect(try ids(b.getPreviousSiblings()) == ids([a, d]))
      #expect(try ids(d.getNextSiblings()) == ids([b, c]))
      #expect(try ids(getAdjacentNodes(start)) == ids([a, d, b, c]))
      #expect(try ids(getAdjacentNodes(end)) == ids([c, b, d, a]))
      #expect(try ids(getAdjacentNodes(getSiblingCaret(b, .previous))) == ids([d, a]))
      try b.remove()
      #expect(try b.getPreviousSiblings().isEmpty)
      #expect(try b.getNextSiblings().isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretTraversal.test.ts::indexed descendants distinguish child starts, document end and empty children"
    ))
  func indexedDescendants() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let a = try createTextNode("a").toggleUnmergeable()
      let b = try createTextNode("b").toggleUnmergeable()
      let empty = try createParagraphNode()
      let root = try getRoot().clear().append(createParagraphNode().append(a, b), empty)
      #expect(try root.getDescendantByIndex(0) === a)
      #expect(try root.getDescendantByIndex(1) === empty)
      #expect(try root.getDescendantByIndex(2) === empty)
      try empty.remove()
      #expect(try root.getDescendantByIndex(1) === b)
      #expect(try root.getDescendantByIndex(Int.max) === b)
      // JS also returns null for 0.5 and NaN, which an Int can't be.
      #expect(try root.getDescendantByIndex(-1) == nil)
      try root.clear()
      #expect(try root.getDescendantByIndex(0) == nil)
    }
  }
}
