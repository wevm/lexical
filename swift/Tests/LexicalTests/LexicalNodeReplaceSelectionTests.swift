import Lexical
import LexicalTesting
import Testing

// Port of `replace(other, includeChildren) selection mapping` and `a setter
// returns the version it wrote to` in LexicalNode.test.ts.

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

@Suite struct ReplaceIncludeChildrenSelectionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > element-anchored point maps to prevSize + originalOffset (etrepum example)"
    ))
  func elementPointMapsToPrevSize() throws {
    let editor = try createTestEditor()
    var newParentKey = ""
    var prevSize = 0
    try editor.update(discrete: true) {
      let parent = try createParagraphNode().append(
        createLineBreakNode(), createLineBreakNode(), createLineBreakNode())
      try getRoot().clear().append(parent)
      try parent.select(1, 1)
      let newParent = try createParagraphNode().append(createTextNode("before"))
      prevSize = try newParent.getChildrenSize()
      newParentKey = newParent.__key
      try parent.replace(newParent, includeChildren: true)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.isCollapsed())
      #expect(sel.anchor.key == newParentKey)
      #expect(sel.anchor.type == .element)
      #expect(sel.anchor.offset == prevSize + 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > non-collapsed element-anchored anchor and focus both shift by prevSize"
    ))
  func nonCollapsedShiftsByPrevSize() throws {
    let editor = try createTestEditor()
    var newParentKey = ""
    try editor.update(discrete: true) {
      let parent = try createParagraphNode().append(
        createLineBreakNode(), createLineBreakNode(), createLineBreakNode())
      try getRoot().clear().append(parent)
      try parent.select(1, 3)
      let newParent = try createParagraphNode().append(
        createLineBreakNode(), createLineBreakNode())
      newParentKey = newParent.__key
      try parent.replace(newParent, includeChildren: true)
    }
    #expect(try points(editor) == elementPoints(newParentKey, 3, newParentKey, 5))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > text-anchored selection inside transferred children is unaffected"
    ))
  func textPointUnaffected() throws {
    let editor = try createTestEditor()
    var textKey = ""
    try editor.update(discrete: true) {
      let parent = try createParagraphNode()
      let t1 = try createTextNode("hello")
      try parent.append(t1)
      try getRoot().clear().append(parent)
      textKey = t1.__key
      try t1.select(2, 2)
      try parent.replace(createParagraphNode().append(createLineBreakNode()), includeChildren: true)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.key == textKey)
      #expect(sel.anchor.type == .text)
      #expect(sel.anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > replace without includeChildren falls back to the legacy moveSelectionPointToEnd"
    ))
  func withoutIncludeChildrenMovesToEnd() throws {
    let editor = try createTestEditor()
    var newParentKey = ""
    var prefixKey = ""
    try editor.update(discrete: true) {
      let parent = try createParagraphNode().append(createLineBreakNode(), createLineBreakNode())
      try getRoot().clear().append(parent)
      try parent.select(1, 1)
      let prefix = try createTextNode("prefix")
      let newParent = try createParagraphNode().append(prefix)
      newParentKey = newParent.__key
      prefixKey = prefix.__key
      try parent.replace(newParent, includeChildren: false)
    }
    try editor.read { () throws in
      let sel = try rangeSelection()
      #expect(sel.anchor.type == .text)
      #expect(sel.anchor.key == prefixKey)
      #expect(try sel.anchor.getNode().getParentOrThrow().__key == newParentKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::replace(other, includeChildren) selection mapping > $setBlocksType-style pattern (prevSize=0 fresh receiver) preserves original offset"
    ))
  func freshReceiverPreservesOffset() throws {
    let editor = try createTestEditor()
    var newParentKey = ""
    try editor.update(discrete: true) {
      let parent = try createParagraphNode().append(
        createLineBreakNode(), createLineBreakNode(), createLineBreakNode())
      try getRoot().clear().append(parent)
      try parent.select(2, 2)
      let newParent = try createParagraphNode()
      newParentKey = newParent.__key
      try parent.replace(newParent, includeChildren: true)
    }
    #expect(try points(editor) == elementPoints(newParentKey, 2, newParentKey, 2))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::a setter returns the version it wrote to > setFormat/setStyle/setIndent return the writable, not the receiver"
    ))
  func setterReturnsWritable() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode())
    }
    try editor.update(discrete: true) {
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      let afterIndent = try paragraph.setIndent(2)
      #expect(afterIndent !== paragraph)
      #expect(try afterIndent === paragraph.getLatest())
      #expect(try afterIndent.setStyle("color: red") === paragraph.getLatest())
      #expect(try afterIndent.setFormat("center") === paragraph.getLatest())
    }
  }
}
