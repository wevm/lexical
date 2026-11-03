import Lexical
import LexicalTesting
import Testing

// Port of `insertBefore, remove and replace selection side effects` in
// LexicalNode.test.ts.

/// The root holds `source` (three paragraphs) and `target` (two).
private func setup() throws -> (source: ElementNode, target: ElementNode) {
  let root = try getRoot().clear()
  let source = try createTestElementNode()
  let target = try createTestElementNode()
  try source.append(createParagraphNode(), createParagraphNode(), createParagraphNode())
  try target.append(createParagraphNode(), createParagraphNode())
  try root.append(source, target)
  return (source, target)
}

/// An element holding two paragraphs, appended to the root.
private func appendOther() throws -> ElementNode {
  let other = try createTestElementNode()
  try other.append(createParagraphNode(), createParagraphNode())
  try getRoot().append(other)
  return other
}

private func select(_ anchor: (NodeKey, Int), _ focus: (NodeKey, Int)) throws {
  let sel = createRangeSelection()
  try sel.anchor.set(anchor.0, anchor.1, .element)
  try sel.focus.set(focus.0, focus.1, .element)
  try setSelection(sel)
}

/// Runs `body` in a discrete update and returns the resulting points with
/// the keys it names replaced by `source`, `target`, and `other`.
private func run(
  _ body: @escaping (_ source: ElementNode, _ target: ElementNode) throws -> ElementNode?
) throws -> String {
  let editor = try createTestEditor()
  var names: [(NodeKey, String)] = []
  try editor.update(discrete: true) {
    let (source, target) = try setup()
    names = [(source.__key, "source"), (target.__key, "target")]
    if let other = try body(source, target) {
      names.append((other.__key, "other"))
    }
  }
  var result = try points(editor)
  for (key, name) in names {
    result = result.split(separator: " ").map { part in
      part.hasPrefix(key + ":") ? name + part.dropFirst(key.count) : String(part)
    }.joined(separator: " ")
  }
  return result
}

private func expected(
  _ anchor: String, _ anchorOffset: Int, _ focus: String? = nil,
  _ focusOffset: Int? = nil
) -> String {
  elementPoints(anchor, anchorOffset, focus ?? anchor, focusOffset ?? anchorOffset)
}

@Suite struct InsertBeforeRemoveReplaceSelectionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a point past the removed index in the old parent"
    ))
  func insertBeforeShiftsOldParent() throws {
    let result = try run { source, target in
      try source.select(2, 2)
      try target.getLastChildOrThrow().insertBefore(source.getChildAtIndex(1)!)
      return nil
    }
    #expect(result == expected("source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point at the removed index in the old parent alone"
    ))
  func insertBeforeLeavesRemovedIndex() throws {
    let result = try run { source, target in
      try source.select(1, 1)
      try target.getLastChildOrThrow().insertBefore(source.getChildAtIndex(1)!)
      return nil
    }
    #expect(result == expected("source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a point at or past the insert index in the new parent"
    ))
  func insertBeforeShiftsNewParent() throws {
    let result = try run { _, target in
      try target.select(2, 2)
      try target.getLastChildOrThrow().insertBefore(createParagraphNode())
      return nil
    }
    #expect(result == expected("target", 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point before the insert index alone"
    ))
  func insertBeforeLeavesEarlierPoint() throws {
    let result = try run { _, target in
      try target.select(0, 2)
      try target.getLastChildOrThrow().insertBefore(createParagraphNode())
      return nil
    }
    #expect(result == expected("target", 0, "target", 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore with restoreSelection=false leaves both parents alone"
    ))
  func insertBeforeNoRestore() throws {
    let result = try run { _, target in
      try target.select(2, 2)
      try target.getLastChildOrThrow().insertBefore(
        createParagraphNode(), restoreSelection: false)
      return nil
    }
    #expect(result == expected("target", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore leaves a point in an unrelated parent untouched"
    ))
  func insertBeforeUnrelated() throws {
    let result = try run { source, target in
      try source.select(1, 1)
      try target.getLastChildOrThrow().insertBefore(createParagraphNode())
      return nil
    }
    #expect(result == expected("source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts a point past the removed index"
    ))
  func removeShifts() throws {
    let result = try run { source, _ in
      try source.select(3, 3)
      try source.getChildAtIndex(1)!.remove()
      return nil
    }
    #expect(result == expected("source", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove leaves a point at or before the removed index alone"
    ))
  func removeLeavesEarlierPoint() throws {
    let result = try run { source, _ in
      try source.select(1, 3)
      try source.getChildAtIndex(1)!.remove()
      return nil
    }
    #expect(result == expected("source", 1, "source", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove leaves a point in an unrelated parent untouched"
    ))
  func removeUnrelated() throws {
    let result = try run { source, target in
      try target.select(2, 2)
      try source.getChildAtIndex(1)!.remove()
      return nil
    }
    #expect(result == expected("target", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts a point past the replacement's index in its old parent"
    ))
  func replaceShiftsOldParent() throws {
    let result = try run { source, target in
      try source.select(2, 2)
      try target.getFirstChildOrThrow().replace(source.getChildAtIndex(1)!)
      return nil
    }
    #expect(result == expected("source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace leaves a point at the replacement's index in its old parent alone"
    ))
  func replaceLeavesIndex() throws {
    let result = try run { source, target in
      try source.select(1, 1)
      try target.getFirstChildOrThrow().replace(source.getChildAtIndex(1)!)
      return nil
    }
    #expect(result == expected("source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace leaves a point in an unrelated parent untouched"
    ))
  func replaceUnrelated() throws {
    let result = try run { source, target in
      try target.select(2, 2)
      try target.getFirstChildOrThrow().replace(source.getChildAtIndex(1)!)
      return nil
    }
    #expect(result == expected("target", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a focus-only point in the old parent"
    ))
  func insertBeforeFocusOldParent() throws {
    let result = try run { source, target in
      let other = try appendOther()
      try select((other.__key, 1), (source.__key, 2))
      try target.getLastChildOrThrow().insertBefore(source.getChildAtIndex(1)!)
      return other
    }
    #expect(result == expected("other", 1, "source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts a focus-only point in the new parent"
    ))
  func insertBeforeFocusNewParent() throws {
    let result = try run { _, target in
      let other = try appendOther()
      try select((other.__key, 1), (target.__key, 2))
      try target.getLastChildOrThrow().insertBefore(createParagraphNode())
      return other
    }
    #expect(result == expected("other", 1, "target", 3))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts a focus-only point past the removed index"
    ))
  func removeFocusOnly() throws {
    let result = try run { source, _ in
      let other = try appendOther()
      try select((other.__key, 1), (source.__key, 3))
      try source.getChildAtIndex(1)!.remove()
      return other
    }
    #expect(result == expected("other", 1, "source", 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts a focus-only point in the replacement's old parent"
    ))
  func replaceFocusOnly() throws {
    let result = try run { source, target in
      let other = try appendOther()
      try select((other.__key, 1), (source.__key, 2))
      try target.getFirstChildOrThrow().replace(source.getChildAtIndex(1)!)
      return other
    }
    #expect(result == expected("other", 1, "source", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > insertBefore shifts an anchor-only point in the new parent"
    ))
  func insertBeforeAnchorOnly() throws {
    let result = try run { _, target in
      let other = try appendOther()
      try select((target.__key, 2), (other.__key, 1))
      try target.getLastChildOrThrow().insertBefore(createParagraphNode())
      return other
    }
    #expect(result == expected("target", 3, "other", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > remove shifts an anchor-only point past the removed index"
    ))
  func removeAnchorOnly() throws {
    let result = try run { source, _ in
      let other = try appendOther()
      try select((source.__key, 3), (other.__key, 1))
      try source.getChildAtIndex(1)!.remove()
      return other
    }
    #expect(result == expected("source", 2, "other", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace shifts an anchor-only point in the replacement's old parent"
    ))
  func replaceAnchorOnly() throws {
    let result = try run { source, target in
      let other = try appendOther()
      try select((source.__key, 2), (other.__key, 1))
      try target.getFirstChildOrThrow().replace(source.getChildAtIndex(1)!)
      return other
    }
    #expect(result == expected("source", 1, "other", 1))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::insertBefore, remove and replace selection side effects > replace with a fresh node leaves the parent offsets alone"
    ))
  func replaceWithFreshNode() throws {
    let result = try run { _, target in
      try target.select(2, 2)
      try target.getFirstChildOrThrow().replace(createParagraphNode())
      return nil
    }
    #expect(result == expected("target", 2))
  }
}
