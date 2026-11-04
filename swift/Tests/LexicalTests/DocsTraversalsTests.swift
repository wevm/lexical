import LexicalTesting
import Testing

@testable import Lexical

// Port of caret/__tests__/unit/docs-traversals.test.ts (the examples of
// docs/concepts/traversals.md). LinkNode is used for structure only, so
// TestLinkNode stands in for it. Upstream compares carets with `toEqual`;
// here they are compared with isSamePointCaret.

private struct Doc {
  let paragraphA: ParagraphNode
  let textA1: TextNode
  let linkA2: TestLinkNode
  let textA3: TextNode
  let textA4: TextNode
  let paragraphB: ParagraphNode
  let textB1: TextNode
  let paragraphC: ParagraphNode
}

/// Root > [Paragraph A > [Text A1, Link A2 > [Text A3], Text A4],
/// Paragraph B > [Text B1], Paragraph C].
private func withDoc(_ body: @escaping (Doc) throws -> Void) throws {
  let editor = try createTestEditor()
  try editor.update(discrete: true) {
    let doc = Doc(
      paragraphA: try createParagraphNode(), textA1: try createTextNode("Text A1"),
      linkA2: try createTestLinkNode(), textA3: try createTextNode("Text A3"),
      textA4: try createTextNode("Text A4"), paragraphB: try createParagraphNode(),
      textB1: try createTextNode("Text B1"), paragraphC: try createParagraphNode())
    try getRoot().clear().append(
      try doc.paragraphA.append(doc.textA1, try doc.linkA2.append(doc.textA3), doc.textA4),
      try doc.paragraphB.append(doc.textB1), doc.paragraphC)
    try body(doc)
  }
}

private func same(_ a: [PointCaret], _ b: [PointCaret]) -> Bool {
  a.count == b.count && zip(a, b).allSatisfy { $0.isSamePointCaret($1) }
}

private func depthFirst(_ d: Doc) -> [PointCaret] {
  [
    getChildCaret(d.paragraphA, .next), getSiblingCaret(d.textA1, .next),
    getChildCaret(d.linkA2, .next), getSiblingCaret(d.textA3, .next),
    getSiblingCaret(d.linkA2, .next), getSiblingCaret(d.textA4, .next),
    getSiblingCaret(d.paragraphA, .next), getChildCaret(d.paragraphB, .next),
    getSiblingCaret(d.textB1, .next), getSiblingCaret(d.paragraphB, .next),
    getChildCaret(d.paragraphC, .next), getSiblingCaret(d.paragraphC, .next),
  ]
}

/// `$iterSiblings`.
private func iterSiblings(_ startCaret: PointCaret) throws -> [PointCaret] {
  var result: [PointCaret] = []
  var caret = try startCaret.getAdjacentCaret()
  while let current = caret {
    result.append(current)
    caret = try current.getAdjacentCaret()
  }
  return result
}

@Suite struct DocsTraversalsTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > $iterSiblings"
    ))
  func iterSiblingsExample() throws {
    try withDoc { d throws in
      let start = getChildCaret(try getRoot(), .next)
      #expect(
        same(
          try iterSiblings(start),
          [
            getSiblingCaret(d.paragraphA, .next), getSiblingCaret(d.paragraphB, .next),
            getSiblingCaret(d.paragraphC, .next),
          ]))
      #expect(same(try iterSiblings(start), try start.siblings()))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > root has no siblings"
    ))
  func rootHasNoSiblings() throws {
    try withDoc { _ throws in
      #expect(try getSiblingCaret(try getRoot(), .next).siblings().isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > root has paragraph children"
    ))
  func rootHasParagraphChildren() throws {
    try withDoc { d throws in
      #expect(
        same(
          try getChildCaret(try getRoot(), .next).siblings(),
          [
            getSiblingCaret(d.paragraphA, .next), getSiblingCaret(d.paragraphB, .next),
            getSiblingCaret(d.paragraphC, .next),
          ]))
      #expect(
        same(
          try getChildCaret(try getRoot(), .previous).siblings(),
          [
            getSiblingCaret(d.paragraphC, .previous), getSiblingCaret(d.paragraphB, .previous),
            getSiblingCaret(d.paragraphA, .previous),
          ]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Adjacent Caret Traversals > iteration does not include the origin"
    ))
  func iterationExcludesOrigin() throws {
    try withDoc { d throws in
      #expect(
        same(
          try getSiblingCaret(d.paragraphB, .next).siblings(),
          [getSiblingCaret(d.paragraphC, .next)]))
      #expect(
        same(
          try getSiblingCaret(d.paragraphB, .previous).siblings(),
          [getSiblingCaret(d.paragraphA, .previous)]))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterCaretsDepthFirst > via generator"
    ))
  func depthFirstViaGenerator() throws {
    try withDoc { d throws in
      func step(_ prevCaret: PointCaret) throws -> PointCaret? {
        let nextCaret = try prevCaret.getAdjacentCaret()
        return try nextCaret?.getChildCaret() ?? nextCaret ?? prevCaret.getParentCaret(.root)
      }
      var carets: [PointCaret] = []
      var caret = try step(getChildCaret(try getRoot(), .next))
      while let current = caret {
        carets.append(current)
        caret = try step(current)
      }
      #expect(same(carets, depthFirst(d)))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterCaretsDepthFirst > via CaretRange"
    ))
  func depthFirstViaCaretRange() throws {
    try withDoc { d throws in
      let start = getChildCaret(try getRoot(), .next)
      let carets = try getCaretRange(start, getSiblingCaret(try getRoot(), start.direction))
        .iterNodeCarets().toArray()
      #expect(same(carets, depthFirst(d)))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > $iterNodesDepthFirst > includes only wholly included nodes"
    ))
  func nodesDepthFirst() throws {
    try withDoc { d throws in
      var seen = Set<NodeKey>()
      var nodes: [LexicalNode] = []
      for caret in try getCaretRange(
        getChildCaret(d.paragraphA, .next), getChildCaret(d.paragraphC, .next)
      ).iterNodeCarets().toArray() {
        if caret.isChild {
          seen.insert(caret.origin.getKey())
        } else if !(caret.origin is ElementNode) || seen.contains(caret.origin.getKey()) {
          nodes.append(caret.origin)
        }
      }
      let expected: [LexicalNode] = [
        d.textA1, d.textA3, d.linkA2, d.textA4, d.textB1, d.paragraphB,
      ]
      #expect(nodes.map(ObjectIdentifier.init) == expected.map(ObjectIdentifier.init))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/docs-traversals.test.ts::traversals.md > Traversal Strategies > Depth First Caret Traversals > full traversal"
    ))
  func fullTraversal() throws {
    try withDoc { d throws in
      let carets = try getCaretRange(
        getChildCaret(try getRoot(), .next), getSiblingCaret(try getRoot(), .next)
      ).iterNodeCarets().toArray()
      #expect(same(carets, depthFirst(d)))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal stops at a nested ancestor endpoint (reverse=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal stops at a nested ancestor endpoint (reverse=true)"
    ), arguments: [false, true])
  func nodesBetweenNestedAncestor(reverse: Bool) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let text = try createTextNode("inside")
      let link = try createTestLinkNode().append(text)
      let paragraph = try createParagraphNode().append(link)
      try getRoot().clear().append(
        paragraph, try createParagraphNode().append(try createTextNode("outside")))
      let nodes = try reverse ? paragraph.getNodesBetween(text) : text.getNodesBetween(paragraph)
      let expected: [LexicalNode] = [text, link, paragraph]
      #expect(nodes.map(ObjectIdentifier.init) == expected.map(ObjectIdentifier.init))
    }
  }

  // Mis-skipped as needing @lexical/link: it uses only core nodes.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeBetween.test.ts::node traversal recognizes a stale target reference"
    ))
  func nodesBetweenStaleTarget() throws {
    let editor = try createTestEditor()
    var start: TextNode!
    var end: TextNode!
    try editor.update(discrete: true) {
      start = try createTextNode("start").toggleUnmergeable()
      end = try createTextNode("end").toggleUnmergeable()
      try getRoot().clear().append(
        try createParagraphNode().append(
          start, end, try createTextNode("outside").toggleUnmergeable()))
    }
    try editor.update(discrete: true) {
      let currentEnd = try end.setFormat("italic")
      #expect(currentEnd !== end)
      let nodes = try start.getNodesBetween(end)
      #expect(nodes.count == 2 && nodes[0] === start && nodes[1] === currentEnd)
    }
  }
}
