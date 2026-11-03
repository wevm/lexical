import Foundation
import Lexical
import LexicalTesting
import Testing

// Port of `insertAfter selection side effects` and `selectPrevious and
// selectNext` in LexicalNode.test.ts.

private struct Refs {
  var source: ElementNode
  var target: ElementNode
  var mover: ElementNode
  var inTarget: ElementNode
}

/// The root holds `source` (mover, two paragraphs) and `target` (inTarget).
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

/// As `setupTwoContainers`, with the mover second in `source`.
private func setupMoverAtIndexOne() throws -> Refs {
  let root = try getRoot().clear()
  let refs = Refs(
    source: try createTestElementNode(), target: try createTestElementNode(),
    mover: try createParagraphNode(), inTarget: try createParagraphNode())
  try refs.source.append(createParagraphNode(), refs.mover, createParagraphNode())
  try refs.target.append(refs.inTarget)
  try root.append(refs.source, refs.target)
  return refs
}

/// An element holding two paragraphs, appended to the root.
private func appendOther() throws -> ElementNode {
  let other = try createTestElementNode()
  try other.append(createParagraphNode(), createParagraphNode())
  try getRoot().append(other)
  return other
}

private func select(
  _ anchor: (NodeKey, Int), _ focus: (NodeKey, Int)
) throws {
  let sel = createRangeSelection()
  try sel.anchor.set(anchor.0, anchor.1, .element)
  try sel.focus.set(focus.0, focus.1, .element)
  try setSelection(sel)
}

@Suite struct InsertAfterSelectionSideEffectsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors a collapsed point that sat on the moved node"
    ))
  func reanchorsCollapsedPoint() throws {
    let editor = try createTestEditor()
    var targetKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      targetKey = refs.target.__key
      try refs.source.select(1, 1)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(targetKey, 2, targetKey, 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors only the point that sat on the moved node"
    ))
  func reanchorsOnlyMovedPoint() throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    var targetKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      targetKey = refs.target.__key
      try refs.source.select(0, 1)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(sourceKey, 0, targetKey, 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move shifts a source point past the removed index (times=-1)"
    ))
  func shiftsSourcePoint() throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      sourceKey = refs.source.__key
      try refs.source.select(2, 2)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(sourceKey, 1, sourceKey, 1))
  }

  /// A parent holding `a` and another paragraph, selected at `anchor` to
  /// `focus`, then a paragraph inserted after `a`.
  private func insertIntoParent(
    _ anchor: Int, _ focus: Int, restoreSelection: Bool = true
  )
    throws -> String
  {
    let editor = try createTestEditor()
    var parentKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let parent = try createTestElementNode()
      let a = try createParagraphNode()
      try parent.append(a, createParagraphNode())
      try root.append(parent)
      parentKey = parent.__key
      try parent.select(anchor, focus)
      try a.insertAfter(createParagraphNode(), restoreSelection: restoreSelection)
    }
    return try points(editor).replacingOccurrences(of: parentKey + ":", with: "parent:")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > collapsed point on the destination parent shifts by one"
    ))
  func collapsedDestinationShifts() throws {
    #expect(try insertIntoParent(2, 2) == elementPoints("parent", 3, "parent", 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > non-collapsed selection on the destination parent shifts only the side past the insert"
    ))
  func nonCollapsedDestinationShifts() throws {
    #expect(try insertIntoParent(0, 2) == elementPoints("parent", 0, "parent", 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > restoreSelection=false leaves the destination parent offset alone"
    ))
  func noRestoreLeavesDestination() throws {
    #expect(
      try insertIntoParent(2, 2, restoreSelection: false)
        == elementPoints("parent", 2, "parent", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move leaves a point at the moved node index alone"
    ))
  func pointAtMovedIndexAlone() throws {
    let editor = try createTestEditor()
    var sourceKey = ""
    try editor.update(discrete: true) {
      let refs = try setupMoverAtIndexOne()
      sourceKey = refs.source.__key
      try refs.source.select(1, 1)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(sourceKey, 1, sourceKey, 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors onto a moved node that is not the first child"
    ))
  func reanchorsNotFirstChild() throws {
    let editor = try createTestEditor()
    var targetKey = ""
    try editor.update(discrete: true) {
      let refs = try setupMoverAtIndexOne()
      targetKey = refs.target.__key
      try refs.source.select(2, 2)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(targetKey, 2, targetKey, 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors a focus-only point that sat on the moved node"
    ))
  func reanchorsFocusOnly() throws {
    let editor = try createTestEditor()
    var otherKey = ""
    var targetKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      otherKey = try appendOther().__key
      targetKey = refs.target.__key
      try select((otherKey, 1), (refs.source.__key, 1))
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(otherKey, 1, targetKey, 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > insert shifts a focus-only point on the destination parent"
    ))
  func insertShiftsFocusOnly() throws {
    let editor = try createTestEditor()
    var otherKey = ""
    var parentKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      let parent = try createTestElementNode()
      let a = try createParagraphNode()
      try parent.append(a, createParagraphNode())
      try root.append(parent)
      otherKey = try appendOther().__key
      parentKey = parent.__key
      try select((otherKey, 1), (parentKey, 2))
      try a.insertAfter(createParagraphNode())
    }
    #expect(try points(editor) == elementPoints(otherKey, 1, parentKey, 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > cross-parent move re-anchors an anchor-only point that sat on the moved node"
    ))
  func reanchorsAnchorOnly() throws {
    let editor = try createTestEditor()
    var otherKey = ""
    var targetKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      otherKey = try appendOther().__key
      targetKey = refs.target.__key
      try select((refs.source.__key, 1), (otherKey, 1))
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(targetKey, 2, otherKey, 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertAfter selection side effects > a point in an unrelated parent survives a cross-parent move untouched"
    ))
  func unrelatedPointUntouched() throws {
    let editor = try createTestEditor()
    var otherKey = ""
    try editor.update(discrete: true) {
      let refs = try setupTwoContainers()
      let other = try appendOther()
      otherKey = other.__key
      try other.select(1, 1)
      try refs.inTarget.insertAfter(refs.mover)
    }
    #expect(try points(editor) == elementPoints(otherKey, 1, otherKey, 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::selectPrevious and selectNext > both land on the sibling index"
    ))
  func selectPreviousAndNext() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      let first = try createLineBreakNode()
      let middle = try createLineBreakNode()
      let last = try createLineBreakNode()
      try paragraph.append(first, middle, last)
      try getRoot().clear().append(paragraph)
      let sel = try last.selectPrevious()
      #expect(sel.anchor.key == paragraph.__key)
      #expect(sel.anchor.type == .element)
      #expect(sel.anchor.offset == 2)
      #expect(sel.focus.offset == 2)
      let nextSel = try first.selectNext()
      #expect(nextSel.anchor.key == paragraph.__key)
      #expect(nextSel.anchor.type == .element)
      #expect(nextSel.anchor.offset == 1)
      #expect(nextSel.focus.offset == 1)
    }
  }
}
