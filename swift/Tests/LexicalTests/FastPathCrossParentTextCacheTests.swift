import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/FastPathCrossParentTextCache.test.ts. The
// Swift reconciler has no children fast path, so these check that the
// general walk keeps the cached text in sync. TestLinkNode stands in for
// LinkNode, which is used for structure only.

private final class FlipInlineNode: ElementNode {
  var __inline = false
  override class func config() -> NodeConfig? {
    NodeConfig(FlipInlineNode.self, type: "flip-inline", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __inline = (prevNode as! FlipInlineNode).__inline
  }
  override func isInline() throws -> Bool { try getLatest().__inline }
  func setInline(_ inline: Bool) throws { try getWritable().__inline = inline }
  override func canBeEmpty() throws -> Bool { false }
}

/// The root's (cached) text and the text of its children joined fresh.
private func cachedAndFresh(_ editor: Editor) throws -> (String, String) {
  try editor.read(.latest) {
    let root = try getRoot()
    return (
      try root.getTextContent(),
      try root.getChildren().map { try $0.getTextContent() }.joined(separator: "\n\n")
    )
  }
}

@Suite struct FastPathCrossParentTextCacheTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/FastPathCrossParentTextCache.test.ts::children fast path: cross-parent move and sibling text cache > getTextContent matches the live tree after moving + merging a trailing link"
    ))
  func movingTrailingLink() throws {
    var errors: [any Error] = []
    let editor = try createTestEditor()
    editor._onError = { errors.append($0) }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      let leadingLink = try createTestLinkNode().append(try createTextNode("P0TEXT"))
      let p0 = try createParagraphNode().append(leadingLink, try createTextNode("after"))
      let trailingLink = try createTestLinkNode().append(try createLineBreakNode())
      let p1 = try createParagraphNode().append(
        try createTextNode("x"), try createLineBreakNode(), try createTextNode("y"), trailingLink)
      try getRoot().clear().append(p0, p1)
    }
    try editor.update(discrete: true) {
      let root = try getRoot()
      let p0 = try #require(try root.getFirstChildOrThrow() as? ElementNode)
      let p1 = try #require(try root.getLastChildOrThrow() as? ElementNode)
      try p0.getFirstChildOrThrow().insertBefore(try p1.getLastChildOrThrow())
    }
    #expect(errors.isEmpty)
    let (cached, fresh) = try cachedAndFresh(editor)
    #expect(cached == fresh)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/FastPathCrossParentTextCache.test.ts::children fast path: cross-parent move and sibling text cache > flipping a non-last suffix element block<->inline keeps the cache in sync"
    ))
  func flippingInline() throws {
    var errors: [any Error] = []
    let editor = try createTestEditor(nodes: [.klass(FlipInlineNode.self)])
    editor._onError = { errors.append($0) }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      let e = try FlipInlineNode().append(try createTextNode("e"))
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("a"), try createLineBreakNode(), e, try createTextNode("z")))
    }
    try editor.update(discrete: true) {
      let p = try #require(try getRoot().getLastChildOrThrow() as? ElementNode)
      try #require(try p.getChildren()[2] as? FlipInlineNode).setInline(true)
      try #require(try p.getLastChildOrThrow() as? TextNode).toggleFormat("bold")
    }
    #expect(errors.isEmpty)
    let (cached, fresh) = try cachedAndFresh(editor)
    #expect(cached == fresh)
  }
}
