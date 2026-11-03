import Lexical
import LexicalTesting
import Testing

// Port of the `$extendCaretToRange` and `Ordering` suites of
// caret/__tests__/unit/LexicalCaret.test.ts, and of its
// `LexicalSelectionHelpers` and `$splitAtPointCaretNext` suites.
//
// Upstream uses LinkNode, MarkNode, ListNode and ListItemNode here only for
// their structure, so the core test nodes stand in for them:
// TestInlineElementNode (inline, canBeEmpty false) for LinkNode and
// MarkNode, TestElementNode for the list nodes.

private func sameCaret(_ a: PointCaret, _ b: PointCaret) -> Bool { a.isSamePointCaret(b) }

/// The document of the `Ordering` suite.
private struct OrderingDocument {
  let rootNode: RootNode
  let paragraphNode: ParagraphNode
  let paragraphText: TextNode
  let linkNode: ElementNode
  let linkText: TextNode
  let listNode: ElementNode
  let listItemText1: TextNode
  let listItemText2: TextNode
  let listItem1: ElementNode
  let listItem2: ElementNode
  let emptyParagraph: ParagraphNode

  init() throws {
    rootNode = try getRoot()
    paragraphText = try createTextNode("paragraph text")
    linkText = try createTextNode("link text")
    linkNode = try createTestInlineElementNode()
    paragraphNode = try createParagraphNode()
    listItemText1 = try createTextNode("item 1")
    listItemText2 = try createTextNode("item 2")
    listItem1 = try createTestElementNode()
    listItem2 = try createTestElementNode()
    listNode = try createTestElementNode()
    emptyParagraph = try createParagraphNode()
    try getRoot().clear().append(
      paragraphNode.append(paragraphText, linkNode.append(linkText)),
      listNode.append(listItem1.append(listItemText1), listItem2.append(listItemText2)),
      emptyParagraph)
  }
}

private func orderingEditor() throws -> (Editor, OrderingDocument) {
  let editor = try createTestEditor()
  var document: OrderingDocument!
  try update(editor) { document = try OrderingDocument() }
  return (editor, document)
}

private func expectCommonAncestor(
  _ result: CommonAncestorResult?, same node: LexicalNode
) {
  guard case .same(let commonAncestor) = result else {
    Issue.record("expected same, got \(String(describing: result))")
    return
  }
  #expect(commonAncestor === node)
}

private func expectCommonAncestor(
  _ result: CommonAncestorResult?, ancestor node: LexicalNode
) {
  guard case .ancestor(let commonAncestor) = result else {
    Issue.record("expected ancestor, got \(String(describing: result))")
    return
  }
  #expect(commonAncestor === node)
}

private func expectCommonAncestor(
  _ result: CommonAncestorResult?, descendant node: LexicalNode
) {
  guard case .descendant(let commonAncestor) = result else {
    Issue.record("expected descendant, got \(String(describing: result))")
    return
  }
  #expect(commonAncestor === node)
}

struct BranchCase: Sendable, CustomTestStringConvertible {
  let title: String
  fileprivate let nodes: @Sendable (OrderingDocument) -> (LexicalNode, LexicalNode)
  fileprivate let ancestors: @Sendable (OrderingDocument) -> (LexicalNode, LexicalNode)
  fileprivate let commonAncestor: @Sendable (OrderingDocument) -> ElementNode
  var testDescription: String { title }

  static let all: [BranchCase] = [
    BranchCase(
      title: "paragraphNode and listNode are siblings in root",
      nodes: { ($0.paragraphNode, $0.listNode) }, ancestors: { ($0.paragraphNode, $0.listNode) },
      commonAncestor: { $0.rootNode }),
    BranchCase(
      title: "paragraphText and linkNode->linkText are siblings in paragraphNode",
      nodes: { ($0.paragraphText, $0.linkText) }, ancestors: { ($0.paragraphText, $0.linkNode) },
      commonAncestor: { $0.paragraphNode }),
    BranchCase(
      title:
        "paragraphNode->linkNode->linkText and listNode->listItem2->listItemText2 are siblings in root",
      nodes: { ($0.linkText, $0.listItemText2) }, ancestors: { ($0.paragraphNode, $0.listNode) },
      commonAncestor: { $0.rootNode }),
    BranchCase(
      title: "listItem1 and listItem2->listItemText2 are siblings in listNode",
      nodes: { ($0.listItem1, $0.listItemText2) }, ancestors: { ($0.listItem1, $0.listItem2) },
      commonAncestor: { $0.listNode }),
    BranchCase(
      title: "listItem1->listItemText1 and listItem2->listItemText2 are siblings in listNode",
      nodes: { ($0.listItemText1, $0.listItemText2) }, ancestors: { ($0.listItem1, $0.listItem2) },
      commonAncestor: { $0.listNode }),
  ]
}

/// p1 [hello] and p2 [world] in the root.
private func helloWorld() throws -> (TextNode, TextNode, ParagraphNode, ParagraphNode) {
  let t1 = try createTextNode("hello")
  let t2 = try createTextNode("world")
  let p1 = try createParagraphNode().append(t1)
  let p2 = try createParagraphNode().append(t2)
  try getRoot().clear().append(p1, p2)
  return (t1, t2, p1, p2)
}

@Suite struct LexicalCaretOrderingTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > next direction: focus can be flipped and used to delete to end of document (regression for #8927)"
    ))
  func extendNextAndDeleteToEnd() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (t1, _, p1, p2) = try helloWorld()
      let range = try extendCaretToRange(getTextPointCaret(t1, .next, 2))
      #expect(sameCaret(range.focus, getSiblingCaret(p2, .next)))
      #expect(try sameCaret(range.focus.getFlipped(), getChildCaret(getRoot(), .previous)))
      #expect(sameCaret(try range.focus.getFlipped().getFlipped(), range.focus))
      _ = try removeTextFromCaretRange(range)
      #expect(try getRoot().getChildren().map(\.__key) == [p1.__key])
      #expect(try getRoot().getAllTextNodes().map(\.__key) == [t1.__key])
      #expect(try getRoot().getTextContent() == "he")
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > previous direction: focus can be flipped and used to delete to start of document"
    ))
  func extendPreviousAndDeleteToStart() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let (_, t2, p1, p2) = try helloWorld()
      let range = try extendCaretToRange(getTextPointCaret(t2, .previous, 2))
      #expect(sameCaret(range.focus, getSiblingCaret(p1, .previous)))
      #expect(try sameCaret(range.focus.getFlipped(), getChildCaret(getRoot(), .next)))
      #expect(sameCaret(try range.focus.getFlipped().getFlipped(), range.focus))
      _ = try removeTextFromCaretRange(range)
      #expect(try getRoot().getChildren().map(\.__key) == [p2.__key])
      #expect(try getRoot().getAllTextNodes().map(\.__key) == [t2.__key])
      #expect(try getRoot().getTextContent() == "rld")
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > doesn't change which nodes plain iteration visits"
    ))
  func extendIteration() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let (t1, t2, p1, p2) = try helloWorld()
      let visited = try extendCaretToRange(getTextPointCaret(t1, .next, 2)).iterNodeCarets()
      let expected = [
        getSiblingCaret(p1, .next), getChildCaret(p2, .next), getSiblingCaret(t2, .next),
        getSiblingCaret(p2, .next),
      ]
      #expect(visited.count == expected.count)
      #expect(zip(visited, expected).allSatisfy(sameCaret))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > $extendCaretToRange > empty document: focus can still be flipped without throwing"
    ))
  func extendEmptyDocument() throws {
    let editor = try createTestEditor()
    try update(editor) {
      try getRoot().clear()
      let range = try extendCaretToRange(getChildCaret(getRoot(), .next))
      #expect(throws: Never.self) { _ = try range.focus.getFlipped() }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > trivial caret checks"
    ))
  func compareTrivialCarets() throws {
    let (editor, _) = try orderingEditor()
    try editor.update(discrete: true) {
      var seenCarets: [NodeCaret] = []
      let range = try getCaretRange(
        getChildCaret(getRoot(), .next), getSiblingCaret(getRoot(), .next))
      #expect(try comparePointCaretNext(range.anchor, range.focus) == -1)
      #expect(try comparePointCaretNext(range.anchor, range.anchor) == 0)
      #expect(try comparePointCaretNext(range.focus, range.focus) == 0)
      #expect(try comparePointCaretNext(range.focus, range.anchor) == 1)
      for caret in try range.iterNodeCarets() {
        #expect(try comparePointCaretNext(caret, caret) == 0)
        for seenCaret in seenCarets {
          #expect(try comparePointCaretNext(caret, seenCaret) == 1)
          #expect(try comparePointCaretNext(seenCaret, caret) == -1)
        }
        seenCarets.append(caret)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > TextPointCaret checks single origin"
    ))
  func compareTextCaretsSingleOrigin() throws {
    let (editor, doc) = try orderingEditor()
    try editor.update(discrete: true) {
      let text = doc.paragraphText
      let size = try text.getTextContentSize()
      func at(_ offset: Int) throws -> PointCaret { try getTextPointCaret(text, .next, offset) }
      let sibling = getSiblingCaret(text, .next)
      #expect(try comparePointCaretNext(at(0), at(0)) == 0)
      #expect(try comparePointCaretNext(at(0), at(2)) == -1)
      #expect(try comparePointCaretNext(at(2), at(0)) == 1)
      #expect(try comparePointCaretNext(at(2), sibling) == -1)
      #expect(try comparePointCaretNext(sibling, at(2)) == 1)
      #expect(try comparePointCaretNext(at(size), sibling) == -1)
      #expect(try comparePointCaretNext(sibling, at(size)) == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $comparePointCaretNext > TextPointCaret multiple origin"
    ))
  func compareTextCaretsMultipleOrigin() throws {
    let (editor, doc) = try orderingEditor()
    try editor.update(discrete: true) { () throws in
      #expect(
        try comparePointCaretNext(
          getTextPointCaret(doc.paragraphText, .next, 0), getTextPointCaret(doc.linkText, .next, 0))
          == -1)
      #expect(
        try comparePointCaretNext(
          getTextPointCaret(doc.paragraphText, .next, 0), getSiblingCaret(doc.linkText, .next))
          == -1)
      #expect(
        try comparePointCaretNext(
          getTextPointCaret(doc.linkText, .next, 0), getTextPointCaret(doc.paragraphText, .next, 0))
          == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > trivial node checks"
    ))
  func commonAncestorTrivial() throws {
    let (editor, _) = try orderingEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      expectCommonAncestor(try getCommonAncestor(root, root), same: root)
      for caret in try getCaretRange(getChildCaret(root, .next), getSiblingCaret(root, .next))
        .iterNodeCarets() where caret.isSibling
      {
        expectCommonAncestor(try getCommonAncestor(caret.origin, caret.origin), same: caret.origin)
        expectCommonAncestor(try getCommonAncestor(root, caret.origin), ancestor: root)
        expectCommonAncestor(try getCommonAncestor(caret.origin, root), descendant: root)
        let parent = try #require(try caret.getParentAtCaret())
        expectCommonAncestor(try getCommonAncestor(parent, caret.origin), ancestor: parent)
        expectCommonAncestor(try getCommonAncestor(caret.origin, parent), descendant: parent)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphNode and listNode are siblings in root"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphText and linkNode->linkText are siblings in paragraphNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > paragraphNode->linkNode->linkText and listNode->listItem2->listItemText2 are siblings in root"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > listItem1 and listItem2->listItemText2 are siblings in listNode"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalCaret > Ordering > $getCommonAncestor > listItem1->listItemText1 and listItem2->listItemText2 are siblings in listNode"
    ),
    arguments: BranchCase.all)
  func commonAncestorBranch(_ c: BranchCase) throws {
    let (editor, doc) = try orderingEditor()
    try editor.update(discrete: true) {
      let (n0, n1) = c.nodes(doc)
      let (a0, a1) = c.ancestors(doc)
      let nodes = [try n0.getLatest(), try n1.getLatest()]
      let ancestors = [try a0.getLatest(), try a1.getLatest()]
      let commonAncestor = try c.commonAncestor(doc).getLatest()
      for (x, y) in [(0, 1), (1, 0)] {
        guard case .branch(let common, let a, let b) = try getCommonAncestor(nodes[x], nodes[y])
        else {
          Issue.record("expected a branch")
          continue
        }
        #expect(common === commonAncestor)
        #expect(a === ancestors[x])
        #expect(b === ancestors[y])
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > with a fully-selected text node preceded by an inline element > a single text node"
    ))
  func fullySelectedTextAfterInlineElement() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      try root.append(paragraph)
      let link = try createTestInlineElementNode()
      try link.append(createTextNode("link"))
      try paragraph.append(link)
      let text = try createTextNode("Existing text...")
      try paragraph.append(text)
      let range = try getCaretRange(
        getTextPointCaret(text, .next, 0), getTextPointCaret(text, .next, .direction(.next)))
      let newRange = try removeTextFromCaretRange(range)
      #expect(newRange.anchor.direction == .next)
      #expect(try newRange.anchor.origin === link.getLatest())
      #expect(newRange.anchor.kind == .sibling)
      try newRange.focus.insert(createTextNode("foo"))
    }
    #expect(
      try shape(editor)
        == "<paragraph><test_inline_block>[link]</test_inline_block>[foo]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > MarkNode is removed when all children are deleted via element-type range"
    ))
  func markRemovedWhenEmptied() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let p = try createParagraphNode()
      let before = try createTextNode("before ")
      let mark = try createTestInlineElementNode()
      let after = try createTextNode(" after")
      try mark.append(createTextNode("aaa"), createTextNode("bbb"))
      try p.append(before, mark, after)
      try root.append(p)
      let range = try getCaretRange(
        getChildCaretAtIndex(mark, 0, .next), getChildCaretAtIndex(mark, 2, .next))
      _ = try removeTextFromCaretRange(range)
      #expect(try mark.isAttached() == false)
      #expect(try before.isAttached())
      #expect(try after.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > LinkNode is removed when all children are deleted via element-type range"
    ))
  func linkRemovedWhenEmptied() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let p = try createParagraphNode()
      let before = try createTextNode("before ")
      let link = try createTestInlineElementNode()
      let after = try createTextNode(" after")
      try link.append(createTextNode("click"))
      try p.append(before, link, after)
      try root.append(p)
      let range = try getCaretRange(
        getChildCaretAtIndex(link, 0, .next), getChildCaretAtIndex(link, 1, .next))
      _ = try removeTextFromCaretRange(range)
      #expect(try link.isAttached() == false)
      #expect(try before.isAttached())
      #expect(try after.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > canBeEmpty()=false parent survives when only some children are removed"
    ))
  func partiallyEmptiedParentSurvives() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let p = try createParagraphNode()
      let mark = try createTestInlineElementNode()
      let t2 = try createTextNode("bbb")
      try mark.append(createTextNode("aaa"), t2, createTextNode("ccc"))
      try p.append(mark)
      try root.append(p)
      let range = try getCaretRange(
        getChildCaretAtIndex(mark, 0, .next), getSiblingCaret(t2, .next))
      _ = try removeTextFromCaretRange(range)
      #expect(try mark.isAttached())
      #expect(try mark.getChildrenSize() > 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > canBeEmpty()=false parent cleanup > multiple canBeEmpty()=false parents in same range are cleaned up"
    ))
  func multipleEmptiedParentsRemoved() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let p = try createParagraphNode()
      let mark1 = try createTestInlineElementNode()
      let mark2 = try createTestInlineElementNode()
      try mark1.append(createTextNode("aaa"))
      try mark2.append(createTextNode("bbb"))
      try p.append(mark1, createTextNode(" "), mark2)
      try root.append(p)
      let range = try getCaretRange(
        getChildCaretAtIndex(p, 0, .next), getChildCaretAtIndex(p, 3, .next))
      _ = try removeTextFromCaretRange(range)
      #expect(try mark1.isAttached() == false)
      #expect(try mark2.isAttached() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Does not split a TextNode at the beginning"
    ))
  func splitAtTextStart() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let textNode = try createTextNode("test")
      let paragraphNode = try createParagraphNode()
      try getRoot().clear().append(paragraphNode.append(textNode))
      let after = try splitAtPointCaretNext(getTextPointCaret(textNode, .next, 0))
      #expect(try textNode.getTextContent() == "test")
      #expect(getChildCaret(paragraphNode, .next).isSamePointCaret(after))
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Splits a TextNode in the middle"
    ))
  func splitTextInMiddle() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let textNode = try createTextNode("test")
      let paragraphNode = try createParagraphNode()
      try getRoot().clear().append(paragraphNode.append(textNode))
      let after = try splitAtPointCaretNext(getTextPointCaret(textNode, .next, 2))
      #expect(try textNode.getTextContent() == "te")
      let nextCaret = getSiblingCaret(textNode, .next)
      #expect(nextCaret.isSamePointCaret(after))
      #expect(try (nextCaret.getNodeAtCaret() as? TextNode)?.getTextContent() == "st")
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::$splitAtPointCaretNext > Splits a ParagraphNode"
    ))
  func splitParagraph() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let beforeTextNode = try createTextNode("before")
      let afterTextNode = try createTextNode("after")
      let paragraphNode = try createParagraphNode()
      try getRoot().clear().append(paragraphNode.append(beforeTextNode, afterTextNode))
      let after = try splitAtPointCaretNext(getSiblingCaret(beforeTextNode, .next))
      #expect(try paragraphNode.getAllTextNodes().map(\.__key) == [beforeTextNode.__key])
      let nextCaret = getSiblingCaret(paragraphNode, .next)
      #expect(nextCaret.isSamePointCaret(after))
      #expect(
        try (nextCaret.getNodeAtCaret() as? ParagraphNode)?.getAllTextNodes().map(\.__key)
          == [afterTextNode.__key])
    }
  }

  /// A block decorator, then `last`, in the root; then select all and
  /// remove the text.
  private func selectAllDeleteAfterDecorator(
    _ last: @escaping () throws -> LexicalNode
  ) throws
    -> Editor
  {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(createTestDecoratorNode().setIsInline(false), last())
    }
    try editor.update(discrete: true) { try selectAll().removeText() }
    return editor
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > a container that may be empty is kept"
    ))
  func selectAllDeleteKeepsContainer() throws {
    let editor = try selectAllDeleteAfterDecorator {
      try createTestElementNode().append(createParagraphNode().append(createTextNode("inner")))
    }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getFirstChild() is TestElementNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaret.test.ts::LexicalSelectionHelpers > the root's last child after select all delete > a paragraph is kept"
    ))
  func selectAllDeleteKeepsParagraph() throws {
    let editor = try selectAllDeleteAfterDecorator {
      try createParagraphNode().append(createTextNode("text"))
    }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getFirstChild() is ParagraphNode)
      #expect(try root.getTextContent() == "")
    }
  }
}
