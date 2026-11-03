import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalBulkSplice.test.ts.

private func expectLinkedChildren(_ parent: ElementNode) throws {
  let children = try parent.getChildren()
  #expect(children.count == (try parent.getChildrenSize()))
  for (i, child) in children.enumerated() {
    #expect(try child.getPreviousSibling()?.getKey() == (i > 0 ? children[i - 1].getKey() : nil))
    #expect(
      try child.getNextSibling()?.getKey()
        == (i + 1 < children.count ? children[i + 1].getKey() : nil))
    #expect(try child.getParent()?.getKey() == parent.getKey())
  }
}

private final class ObservedTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ObservedTextNode.self, type: "observed-text", extends: TextNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    if let parent = try prevNode.getParent() {
      try expectLinkedChildren(parent)
    }
  }
}

@Suite struct LexicalBulkSpliceTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalBulkSplice.test.ts::bulk removal preserves clone-hook traversal, detached links, and snapshots"
    ))
  func bulkRemoval() throws {
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "bulk-splice", nodes: [.klass(ObservedTextNode.self)], onError: { throw $0 }))
    var parent: ElementNode!
    var nodes: [TextNode] = []
    try editor.update(discrete: true) {
      nodes = try "abcde".map {
        try create(ObservedTextNode.self).setTextContent(String($0)).toggleUnmergeable()
      }
      parent = try createParagraphNode().append(contentsOf: nodes)
      try getRoot().clear().append(parent)
    }
    let snapshot = editor.getEditorState()
    try editor.update(discrete: true) { () throws in
      _ = try parent.splice(1, 3, [])
      try expectLinkedChildren(parent)
      #expect(try parent.getTextContent() == "ae")
      for node in nodes[1..<4] {
        #expect(try node.getParent() == nil)
        #expect(try node.getPreviousSibling() == nil)
        #expect(try node.getNextSibling() == nil)
      }
      _ = try parent.splice(1, 0, Array(nodes[1..<4]))
      try expectLinkedChildren(parent)
    }
    try snapshot.read { () throws in
      try expectLinkedChildren(parent)
      #expect(try getRoot().getTextContent() == "abcde")
    }
    #expect(try editor.read { try parent.getTextContent() } == "abcde")
  }
}
