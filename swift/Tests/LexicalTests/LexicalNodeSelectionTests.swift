import Lexical
import LexicalTesting
import Testing

// Port of `Element-anchored selection on old parent (#6031)` in
// LexicalNode.test.ts.

/// A way to move `mover` next to (or in place of) `target`.
struct Mover: Sendable, CustomTestStringConvertible {
  let name: String
  let act: @Sendable (ElementNode, ElementNode) throws -> Void
  let actNoRestore: (@Sendable (ElementNode, ElementNode) throws -> Void)?
  var testDescription: String { name }

  static let insertBefore = Mover(
    name: "insertBefore", act: { try $0.insertBefore($1) },
    actNoRestore: { try $0.insertBefore($1, restoreSelection: false) })
  static let insertAfter = Mover(
    name: "insertAfter", act: { try $0.insertAfter($1) },
    actNoRestore: { try $0.insertAfter($1, restoreSelection: false) })
  static let replace = Mover(name: "replace", act: { try $0.replace($1) }, actNoRestore: nil)

  static let all = [insertBefore, insertAfter, replace]
  static let withRestoreFlag = [insertBefore, insertAfter]
}

private struct Refs {
  var source: ElementNode
  var target: ElementNode
  var mover: ElementNode
  var inTarget: ElementNode
}

/// The root holds `source` (mover, a2, a3) and `target` (inTarget).
private func setupTwoContainers() throws -> Refs {
  let root = try getRoot().clear()
  let refs = Refs(
    source: try createTestElementNode(), target: try createTestElementNode(),
    mover: try createParagraphNode(), inTarget: try createParagraphNode())
  try refs.source.append(refs.mover, createParagraphNode(), createParagraphNode())
  try refs.target.append(refs.inTarget)
  try root.append(refs.source, refs.target)
  return refs
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

@Suite struct ElementAnchoredSelectionOnOldParentTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore shifts offset > removed index in source parent"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter shifts offset > removed index in source parent"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace shifts offset > removed index in source parent"
    ),
    arguments: Mover.all)
  func shiftsPastRemovedIndex(_ method: Mover) throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      try refs.source.select(3, 3)
      try method.act(refs.inTarget, refs.mover)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == sourceKey)
      #expect(sel.anchor.type == .element)
      #expect(sel.anchor.offset == 2)
      #expect(sel.focus.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore does not shift offset at removed index (boundary)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter does not shift offset at removed index (boundary)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace does not shift offset at removed index (boundary)"
    ),
    arguments: Mover.all)
  func boundaryNotShifted(_ method: Mover) throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      try refs.source.select(0, 0)
      try method.act(refs.inTarget, refs.mover)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == sourceKey)
      #expect(sel.anchor.offset == 0)
      #expect(sel.focus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore shifts only the past-boundary side of a non-collapsed selection"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter shifts only the past-boundary side of a non-collapsed selection"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace shifts only the past-boundary side of a non-collapsed selection"
    ),
    arguments: Mover.all)
  func shiftsOnlyPastBoundarySide(_ method: Mover) throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      try refs.source.select(0, 3)
      try method.act(refs.inTarget, refs.mover)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == sourceKey)
      #expect(sel.anchor.offset == 0)
      #expect(sel.focus.key == sourceKey)
      #expect(sel.focus.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore leaves selection in an unrelated third parent untouched"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter leaves selection in an unrelated third parent untouched"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > replace leaves selection in an unrelated third parent untouched"
    ),
    arguments: Mover.all)
  func unrelatedParentUntouched(_ method: Mover) throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    var otherKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let source = try createTestElementNode()
      let target = try createTestElementNode()
      let other = try createTestElementNode()
      let mover = try createParagraphNode()
      let inTarget = try createParagraphNode()
      try source.append(mover, createParagraphNode(), createParagraphNode())
      try target.append(inTarget)
      try other.append(createParagraphNode())
      try root.append(source, target, other)
      sourceKey = source.__key
      otherKey = other.__key
      try other.select(1, 1)
      try method.act(inTarget, mover)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == otherKey)
      #expect(sel.anchor.offset == 1)
      #expect(sel.focus.key == otherKey)
      #expect(sel.focus.offset == 1)
      #expect(sourceKey != otherKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertBefore with restoreSelection=false skips the source-parent shift"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > cross-parent move > insertAfter with restoreSelection=false skips the source-parent shift"
    ),
    arguments: Mover.withRestoreFlag)
  func noRestoreSkipsShift(_ method: Mover) throws {
    // Upstream also expects the DOM to warn that the stale offset can't be
    // applied; there is no DOM here.
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      try refs.source.select(3, 3)
      try method.actNoRestore!(refs.inTarget, refs.mover)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == sourceKey)
      #expect(sel.anchor.offset == 3)
      #expect(sel.focus.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > within-parent move > insertBefore keeps an end-of-parent cursor in range for a no-op move"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::Element-anchored selection on old parent (#6031) > within-parent move > insertAfter keeps an end-of-parent cursor in range for a no-op move"
    ),
    arguments: Mover.withRestoreFlag)
  func withinParentNoOpMove(_ method: Mover) throws {
    let editor = try createTestEditor()
    var parentKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let parent = try createTestElementNode()
      let a = try createParagraphNode()
      let b = try createParagraphNode()
      try parent.append(a, b)
      try root.append(parent)
      parentKey = parent.__key
      try parent.select(2, 2)
      if method.name == "insertBefore" {
        try method.act(b, a)
      } else {
        try method.act(a, b)
      }
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == parentKey)
      #expect(sel.anchor.offset == 2)
      #expect(sel.focus.offset == 2)
    }
  }
}
