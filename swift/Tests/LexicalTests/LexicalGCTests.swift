import Lexical
import LexicalTesting
import Testing

// Port of nodes/__tests__/unit/LexicalGC.test.tsx.

private func nodeMapSize(_ editor: Editor) -> Int { editor.getEditorState()._nodeMap.size }

/// `generatePermutations(['1', …, '6'], 2)`: every ordered choice of up to
/// two keys, in upstream's order.
private let complexTreeRemovals: [[NodeKey]] = [
  [], ["1"], ["1", "2"], ["1", "3"], ["1", "4"], ["1", "5"], ["1", "6"], ["2"], ["2", "1"],
  ["2", "3"], ["2", "4"], ["2", "5"], ["2", "6"], ["3"], ["3", "1"], ["3", "2"], ["3", "4"],
  ["3", "5"], ["3", "6"], ["4"], ["4", "1"], ["4", "2"], ["4", "3"], ["4", "5"], ["4", "6"], ["5"],
  ["5", "1"], ["5", "2"], ["5", "3"], ["5", "4"], ["5", "6"], ["6"], ["6", "1"], ["6", "2"],
  ["6", "3"], ["6", "4"], ["6", "5"],
]

@Suite struct LexicalGCTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and subchild"
    ))
  func clearChildAndSubchild() throws {
    let editor = try createTestEditor()
    try update(editor) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("foo")))
    }
    #expect(nodeMapSize(editor) == 3)
    try update(editor) { _ = try getRoot().clear() }
    #expect(nodeMapSize(editor) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > collects newly created dirty subtrees, parent created first: false"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > collects newly created dirty subtrees, parent created first: true"
    ),
    arguments: [false, true])
  func collectsNewDirtySubtrees(parentFirst: Bool) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let first = try createTestElementNode()
      let second = try createTestElementNode()
      let parent = parentFirst ? first : second
      let child = parentFirst ? second : first
      try getRoot().append(
        try parent.append(
          try child.append(try createParagraphNode().append(try createTextNode("temporary")))))
      try parent.remove()
    }
    #expect(editor.getEditorState()._nodeMap.keysArray == ["root"])
  }

  /// The paragraph `foo`, `bar`, `zzz` (keys 1–3, paragraph 4).
  private func threeSubchildren() throws -> Editor {
    let editor = try createTestEditor()
    try update(editor) {
      let text1 = try createTextNode("foo")
      let text2 = try createTextNode("bar").toggleUnmergeable()
      let text3 = try createTextNode("zzz").toggleUnmergeable()
      let paragraph = try createParagraphNode()
      try paragraph.append(text1, text2, text3)
      try getRoot().append(paragraph)
    }
    #expect(nodeMapSize(editor) == 5)
    return editor
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren"
    ))
  func clearThreeSubchildren() throws {
    let editor = try threeSubchildren()
    try update(editor) { _ = try getRoot().clear() }
    #expect(nodeMapSize(editor) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 0 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a child and three subchildren, subchild 2 removed first"
    ),
    arguments: 0..<3)
  func clearAfterRemovingSubchild(index: Int) throws {
    let editor = try threeSubchildren()
    try update(editor) { () throws in
      let root = try getRoot()
      let firstChild = try #require(try root.getFirstChild() as? ElementNode)
      let subchild = try #require(try firstChild.getChildAtIndex(index))
      #expect(try subchild.getTextContent() == ["foo", "bar", "zzz"][index])
      try subchild.remove()
      _ = try root.clear()
    }
    #expect(nodeMapSize(editor) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes  removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,5 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 1,6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,5 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 2,6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,5 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 3,6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,5 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 4,6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 5,6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,1 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,2 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,3 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,4 removed first"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalGC.test.tsx::LexicalGC tests > RootNode.clear() with a complex tree, nodes 6,5 removed first"
    ),
    arguments: complexTreeRemovals)
  func clearComplexTree(removeKeys: [NodeKey]) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let testElement = try createTestElementNode()
      let testElementText1 = try createTextNode("te1").toggleUnmergeable()
      let testElementText2 = try createTextNode("te2").toggleUnmergeable()
      let text1 = try createTextNode("a").toggleUnmergeable()
      let text2 = try createTextNode("b").toggleUnmergeable()
      let paragraph = try createParagraphNode()
      try testElement.append(testElementText1, testElementText2)
      try paragraph.append(text1, testElement, text2)
      try getRoot().append(paragraph)
    }
    #expect(nodeMapSize(editor) == 7)
    try update(editor) {
      for key in removeKeys {
        try getNodeByKey(key)!.remove()
      }
      _ = try getRoot().clear()
    }
    #expect(nodeMapSize(editor) == 1)
  }
}
