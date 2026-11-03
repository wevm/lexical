import Lexical
import LexicalTesting
import Testing

// Port of caret/__tests__/unit/LexicalCaretMutation.test.ts. The spy on
// TextNode.prototype.insertAfter/insertBefore is a TextNode subclass that
// records its calls.

/// Checks every link of `parent`'s children: size, first, last, parents,
/// siblings and keys.
private func expectChildren(
  _ parent: ElementNode, _ children: [LexicalNode],
  sourceLocation: SourceLocation = #_sourceLocation
) throws {
  #expect(try parent.getChildrenSize() == children.count, sourceLocation: sourceLocation)
  #expect(
    try parent.getFirstChild()?.getKey() == children.first?.getKey(), sourceLocation: sourceLocation
  )
  #expect(
    try parent.getLastChild()?.getKey() == children.last?.getKey(), sourceLocation: sourceLocation)
  for (index, node) in children.enumerated() {
    #expect(try node.getParent()?.getKey() == parent.getKey(), sourceLocation: sourceLocation)
    #expect(
      try node.getPreviousSibling()?.getKey() == (index > 0 ? children[index - 1].getKey() : nil),
      sourceLocation: sourceLocation)
    #expect(
      try node.getNextSibling()?.getKey()
        == (index + 1 < children.count ? children[index + 1].getKey() : nil),
      sourceLocation: sourceLocation)
  }
  #expect(
    try parent.getChildrenKeys() == children.map { $0.getKey() }, sourceLocation: sourceLocation)
}

/// Four unmergeable text nodes "a" to "d" in a paragraph at the root.
private func createFixture() throws -> (ParagraphNode, [TextNode]) {
  let children = try ["a", "b", "c", "d"].map { try createTextNode($0).toggleUnmergeable() }
  let parent = try createParagraphNode().append(contentsOf: children)
  try getRoot().clear().append(parent)
  return (parent, children)
}

/// Records calls to `insertAfter` and `insertBefore`.
final class SpyTextNode: TextNode {
  nonisolated(unsafe) static var calls: [(String, NodeKey)] = []
  override class func config() -> NodeConfig? {
    NodeConfig(SpyTextNode.self, type: "spy-text", extends: TextNode.self)
  }
  override func insertAfter(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  ) throws
    -> LexicalNode
  {
    SpyTextNode.calls.append(("insertAfter", nodeToInsert.getKey()))
    return try super.insertAfter(nodeToInsert, restoreSelection: restoreSelection)
  }
  override func insertBefore(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  ) throws
    -> LexicalNode
  {
    SpyTextNode.calls.append(("insertBefore", nodeToInsert.getKey()))
    return try super.insertBefore(nodeToInsert, restoreSelection: restoreSelection)
  }
}

struct MoveCase: Sendable, CustomTestStringConvertible {
  let direction: CaretDirection
  let origin: Int
  let source: Int
  var testDescription: String { "sibling \(source) \(direction) from \(origin)" }
  static let all: [MoveCase] = [
    MoveCase(direction: .previous, origin: 1, source: 0),
    MoveCase(direction: .previous, origin: 1, source: 2),
    MoveCase(direction: .previous, origin: 1, source: 3),
    MoveCase(direction: .previous, origin: 2, source: 0),
    MoveCase(direction: .previous, origin: 2, source: 1),
    MoveCase(direction: .previous, origin: 2, source: 3),
    MoveCase(direction: .next, origin: 1, source: 0),
    MoveCase(direction: .next, origin: 1, source: 2),
    MoveCase(direction: .next, origin: 1, source: 3),
    MoveCase(direction: .next, origin: 2, source: 0),
    MoveCase(direction: .next, origin: 2, source: 1),
    MoveCase(direction: .next, origin: 2, source: 3),
  ]
}

struct SpliceRepairCase: Sendable, CustomTestStringConvertible {
  let start: Int
  let count: Int
  let insert: [Int]
  let expected: [Int]
  var testDescription: String { "splice(\(start), \(count), \(insert))" }
  static let all: [SpliceRepairCase] = [
    SpliceRepairCase(start: 0, count: 0, insert: [3], expected: [3, 0, 1, 2]),
    SpliceRepairCase(start: 2, count: 1, insert: [1, 3], expected: [0, 1, 3]),
    SpliceRepairCase(start: 0, count: 2, insert: [1, 0], expected: [1, 0, 2, 3]),
    SpliceRepairCase(start: 0, count: 4, insert: [], expected: []),
    SpliceRepairCase(start: 1, count: 2, insert: [2, 1], expected: [0, 2, 1, 3]),
  ]
}

enum EndpointOperation: String, Sendable { case insertBefore, insertAfter, replace }

struct EndpointCase: Sendable, CustomTestStringConvertible {
  let operation: EndpointOperation
  let backward: Bool
  let offset: Int
  var testDescription: String { "\(operation), backward=\(backward), offset=\(offset)" }
  static let all: [EndpointCase] = [
    EndpointCase(operation: .insertBefore, backward: false, offset: 0),
    EndpointCase(operation: .insertBefore, backward: false, offset: 2),
    EndpointCase(operation: .insertBefore, backward: true, offset: 0),
    EndpointCase(operation: .insertBefore, backward: true, offset: 2),
    EndpointCase(operation: .insertAfter, backward: false, offset: 0),
    EndpointCase(operation: .insertAfter, backward: false, offset: 2),
    EndpointCase(operation: .insertAfter, backward: true, offset: 0),
    EndpointCase(operation: .insertAfter, backward: true, offset: 2),
    EndpointCase(operation: .replace, backward: false, offset: 0),
    EndpointCase(operation: .replace, backward: false, offset: 2),
    EndpointCase(operation: .replace, backward: true, offset: 0),
    EndpointCase(operation: .replace, backward: true, offset: 2),
  ]
}

@Suite(.serialized) struct LexicalCaretMutationTests {
  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::replacement with an existing previous sibling preserves both links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::replacement with an existing next sibling preserves both links"
    ),
    arguments: [CaretDirection.previous, .next])
  func replacementWithExistingSibling(_ direction: CaretDirection) throws {
    let editor = try createTestEditor()
    var fixture: (ParagraphNode, [TextNode])?
    try editor.update(discrete: true) { fixture = try createFixture() }
    try editor.update(discrete: true) {
      let (parent, children) = try #require(fixture)
      let (a, b, c, d) = (children[0], children[1], children[2], children[3])
      try b.replace(direction == .previous ? a : c)
      try expectChildren(parent, [a, c, d])
      #expect(try b.isAttached() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 previous from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 2 previous from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 previous from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 previous from 2 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 1 previous from 2 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 previous from 2 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 next from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 2 next from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 next from 1 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 0 next from 2 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 1 next from 2 preserves committed links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving sibling 3 next from 2 preserves committed links"
    ),
    arguments: MoveCase.all)
  func movingSiblingPreservesCommittedLinks(_ testCase: MoveCase) throws {
    let editor = try createTestEditor()
    var fixture: (ParagraphNode, [TextNode])?
    try editor.update(discrete: true) { fixture = try createFixture() }
    let previousState = editor.getEditorState()
    try editor.update(discrete: true) {
      let (parent, children) = try #require(fixture)
      let source = children[testCase.source]
      let origin = children[testCase.origin]
      var expected = children.filter { $0 !== source }
      let at = expected.firstIndex { $0 === origin }! + (testCase.direction == .next ? 1 : 0)
      expected.insert(source, at: at)
      _ = try getSiblingCaret(origin, testCase.direction).insert(source)
      try expectChildren(parent, expected)
    }
    try previousState.read {
      let (parent, children) = try #require(fixture)
      try expectChildren(parent, children)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 0, [ 3 ]) repairs both boundary links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(2, 1, [ 1, 3 ]) repairs both boundary links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 2, [ 1, 0 ]) repairs both boundary links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(0, 4, []) repairs both boundary links"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::splice(1, 2, [ 2, 1 ]) repairs both boundary links"
    ),
    arguments: SpliceRepairCase.all)
  func spliceRepairsBoundaryLinks(_ testCase: SpliceRepairCase) throws {
    let editor = try createTestEditor()
    var fixture: (ParagraphNode, [TextNode])?
    try editor.update(discrete: true) { fixture = try createFixture() }
    let previousState = editor.getEditorState()
    try editor.update(discrete: true) {
      let (parent, children) = try #require(fixture)
      try parent.splice(testCase.start, testCase.count, testCase.insert.map { children[$0] })
      try expectChildren(parent, testCase.expected.map { children[$0] })
      for (index, child) in children.enumerated() where !testCase.expected.contains(index) {
        #expect(try child.getParent() == nil)
        #expect(try child.getPreviousSibling() == nil)
        #expect(try child.getNextSibling() == nil)
      }
    }
    try previousState.read {
      let (parent, children) = try #require(fixture)
      try expectChildren(parent, children)
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::caret insertion dispatches to the previous node method"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::caret insertion dispatches to the next node method"
    ),
    arguments: [CaretDirection.previous, .next])
  func caretInsertionDispatchesToNodeMethod(_ direction: CaretDirection) throws {
    let editor = try createTestEditor(nodes: [.klass(SpyTextNode.self)])
    try editor.update(discrete: true) {
      let children = try ["a", "b", "c", "d"].map { text -> TextNode in
        let node = try SpyTextNode()
        try node.setTextContent(text)
        return try node.toggleUnmergeable()
      }
      try getRoot().clear().append(createParagraphNode().append(contentsOf: children))
      SpyTextNode.calls = []
      _ = try getSiblingCaret(children[1], direction).insert(children[3])
      let method = direction == .next ? "insertAfter" : "insertBefore"
      #expect(SpyTextNode.calls.count == 1)
      #expect(SpyTextNode.calls.first?.0 == method)
      #expect(SpyTextNode.calls.first?.1 == children[3].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=false, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=false, offset=2)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=true, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertBefore, backward=true, offset=2)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=false, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=false, offset=2)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=true, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (insertAfter, backward=true, offset=2)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=false, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=false, offset=2)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=true, offset=0)"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::moving a text endpoint beside its parent endpoint (replace, backward=true, offset=2)"
    ),
    arguments: EndpointCase.all)
  func movingTextEndpointBesideParentEndpoint(_ testCase: EndpointCase) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let first = try createTextNode("first").toggleUnmergeable()
      let moved = try createTextNode("second").toggleUnmergeable()
      let parent = try createParagraphNode().append(first, moved)
      try getRoot().append(parent)
      let selection = createRangeSelection()
      let (textPoint, elementPoint) =
        testCase.backward
        ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try textPoint.set(moved.getKey(), 1, .text)
      try elementPoint.set(parent.getKey(), testCase.offset, .element)
      try setSelection(selection)
      switch testCase.operation {
      case .insertBefore:
        _ = try first.insertBefore(moved)
        try expectChildren(parent, [moved, first])
      case .insertAfter:
        _ = try first.insertAfter(moved)
        try expectChildren(parent, [first, moved])
      case .replace:
        _ = try first.replace(moved)
        try expectChildren(parent, [moved])
      }
      let after = try #require(try getSelection() as? RangeSelection)
      let retained = testCase.backward ? after.focus : after.anchor
      #expect(retained.key == moved.getKey())
      #expect(retained.type == .text)
      #expect(retained.offset == 1)
      #expect(try after.anchor.getNode().isAttached())
      #expect(try after.focus.getNode().isAttached())
      #expect(try after.isBackward() == after.cloneRange().isBackward())
    }
  }

  @Test(
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::invalidates text-only selection direction after insertBefore reorders siblings"
    ),
    .upstream(
      "lexical/src/caret/__tests__/unit/LexicalCaretMutation.test.ts::invalidates text-only selection direction after insertAfter reorders siblings"
    ),
    arguments: ["insertBefore", "insertAfter"])
  func reorderInvalidatesSelectionDirection(_ operation: String) throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let first = try createTextNode("first").toggleUnmergeable()
      let second = try createTextNode("second").toggleUnmergeable()
      let parent = try createParagraphNode().append(first, second)
      try getRoot().append(parent)
      let selection = createRangeSelection()
      try selection.anchor.set(first.getKey(), 1, .text)
      try selection.focus.set(second.getKey(), 1, .text)
      try setSelection(selection)
      #expect(try selection.isBackward() == false)
      if operation == "insertAfter" {
        _ = try second.insertAfter(first)
      } else {
        _ = try first.insertBefore(second)
      }
      try expectChildren(parent, [second, first])
      #expect(try selection.isBackward())
      #expect(try selection.isBackward() == selection.cloneRange().isBackward())
    }
  }
}
