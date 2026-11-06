import Lexical
import LexicalMark
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts.
// `toEqual` on DFS results compares depth and node; here each node is
// compared by identity with the expected latest version.

private struct DFSKeyPair: Equatable {
  let depth: Int
  let node: NodeKey
}

private func expectDFS(
  _ actual: [DFSNode], _ expected: [(Int, LexicalNode)],
  sourceLocation: SourceLocation = #_sourceLocation
) {
  #expect(actual.count == expected.count, sourceLocation: sourceLocation)
  for (a, e) in zip(actual, expected) {
    #expect(a.depth == e.0 && a.node === e.1, sourceLocation: sourceLocation)
  }
}

private func newEditor() throws -> Editor {
  try createHeadlessTestEditor(nodes: [.klass(TestElementNode.self)])
}

private func keysForNode(
  _ depth: Int, _ node: LexicalNode,
  _ children: (ElementNode) -> StepwiseIterator<SiblingCaret, LexicalNode>
) throws -> [DFSKeyPair] {
  var out = [DFSKeyPair(depth: depth, node: node.getKey())]
  if let element = node as? ElementNode {
    try children(element).forEach { out += try keysForNode(depth + 1, $0, children) }
  }
  return out
}

/// The `beforeEach` tree:
///
///                R
///         P1            P2
///      B1     B2     T4 T5 B3
///      T1   T2 T3          T6
private func dfsOrderEditor() throws -> (Editor, [DFSKeyPair], [DFSKeyPair]) {
  let editor = try newEditor()
  var expectedKeys: [DFSKeyPair] = []
  var reverseExpectedKeys: [DFSKeyPair] = []
  try update(editor) {
    let root = try getRoot()
    let paragraph1 = try createParagraphNode()
    let paragraph2 = try createParagraphNode()
    let block1 = try createTestElementNode()
    let block2 = try createTestElementNode()
    let block3 = try createTestElementNode()
    let texts = try (1...6).map { try createTextNode("text\($0)") }
    try root.append(paragraph1, paragraph2)
    try paragraph1.append(block1, block2)
    try paragraph2.append(texts[3], texts[4])
    try texts[4].toggleFormat("bold")
    try paragraph2.append(block3)
    try block1.append(texts[0])
    try block2.append(texts[1], texts[2])
    try texts[2].toggleFormat("bold")
    try block3.append(texts[5])
    expectedKeys = try keysForNode(0, root, firstToLastIterator)
    reverseExpectedKeys = try keysForNode(0, root, lastToFirstIterator)
    func pairs(_ nodes: [LexicalNode]) throws -> [DFSKeyPair] {
      try nodes.map { DFSKeyPair(depth: try $0.getParentKeys().count, node: $0.getKey()) }
    }
    #expect(
      try expectedKeys
        == pairs([
          root, paragraph1, block1, texts[0], block2, texts[1], texts[2], paragraph2, texts[3],
          texts[4], block3, texts[5],
        ]))
    #expect(
      try reverseExpectedKeys
        == pairs([
          root, paragraph2, block3, texts[5], texts[4], texts[3], paragraph1, block2, texts[2],
          texts[1], block1, texts[0],
        ]))
  }
  return (editor, expectedKeys, reverseExpectedKeys)
}

private func latestNodes(_ keys: [DFSKeyPair]) throws -> [(Int, LexicalNode)] {
  try keys.map { ($0.depth, try getNodeByKeyOrThrow($0.node).getLatest()) }
}

private func elementPair() throws -> (TestElementNode, TestElementNode) {
  let elementNode1 = try createTestElementNode().append(
    try createTextNode("word1"), try createTextNode("word2"))
  let elementNode2 = try createTestElementNode().append(try createTextNode("word3"))
  try getRoot().clear().append(elementNode1, elementNode2)
  return (elementNode1, elementNode2)
}

private final class Keys {
  var root = ""
  var paragraph = ""
  var block1 = ""
  var block2 = ""
}

private func getLatestEditor() throws -> (Editor, Keys) {
  let editor = try newEditor()
  let keys = Keys()
  try update(editor) {
    let root = try getRoot()
    let paragraph = try createParagraphNode()
    let block1 = try createTestElementNode()
    let block2 = try createTestElementNode()
    keys.root = root.getKey()
    keys.paragraph = paragraph.getKey()
    keys.block1 = block1.getKey()
    keys.block2 = block2.getKey()
    try root.append(paragraph)
    try paragraph.append(block1, block2)
  }
  return (editor, keys)
}

private func emptyParagraphEditor() throws -> (Editor, NodeKey) {
  let editor = try newEditor()
  var paragraphKey = ""
  try update(editor) {
    let paragraph = try createParagraphNode()
    let paragraph2 = try createParagraphNode()
    paragraphKey = paragraph.getKey()
    try paragraph2.append(try createTextNode("test"))
    try getRoot().append(paragraph, paragraph2)
  }
  return (editor, paragraphKey)
}

@Suite struct LexicalNodeHelpersTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS node order"
    ))
  func dfsNodeOrder() throws {
    let (editor, expectedKeys, _) = try dfsOrderEditor()
    try editor.read(.latest) {
      let expected = try latestNodes(expectedKeys)
      expectDFS(try dfs(expected.first!.1, expected.last!.1), expected)
      expectDFS(
        try dfs(expected[1].1, expected[expected.count - 2].1),
        Array(expected[1..<(expected.count - 1)]))
      expectDFS(try dfs(), expected)
      expectDFS(try dfs(getRoot()), expected)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > Reverse DFS node order"
    ))
  func reverseDfsNodeOrder() throws {
    let (editor, _, reverseExpectedKeys) = try dfsOrderEditor()
    try editor.read(.latest) {
      let expected = try latestNodes(reverseExpectedKeys)
      expectDFS(try reverseDfs(expected.first!.1, expected.last!.1), expected)
      expectDFS(
        try reverseDfs(expected[1].1, expected[expected.count - 2].1),
        Array(expected[1..<(expected.count - 1)]))
      expectDFS(try reverseDfs(), expected)
      expectDFS(try reverseDfs(getRoot()), expected)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS from middle leaf node should only include that node"
    ))
  func dfsFromMiddleLeaf() throws {
    let (editor, _, _) = try dfsOrderEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      try root.clear().append(
        try createParagraphNode().append(
          try createTextNode("Hello"), try createTextNode("world").toggleFormat("bold"),
          try createTextNode("!")))
      let paragraph = try #require(try root.getFirstChild() as? ElementNode)
      let children = try paragraph.getChildren()
      expectDFS(try dfs(children[1]), [(2, children[1])])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS starting at last child of element node should not include parent's siblings"
    ))
  func dfsFromLastChildElement() throws {
    let editor = try createHeadlessTestEditor(
      nodes: [.klass(TestElementNode.self), .klass(MarkNode.self)])
    try editor.update {
      let root = try getRoot()
      let p1 = try createParagraphNode().append(
        try createTextNode("Hello"),
        try createMarkNode().append(try createTextNode("world").toggleFormat("bold")))
      let p2 = try createParagraphNode().append(try createTextNode("!!"))

      try root.clear().append(p1, p2)

      let markNode = try #require(try p1.getChildAtIndex(1) as? MarkNode)
      let textNodes = try markNode.getChildren()
      expectDFS(try dfs(markNode), [(2, markNode), (3, textNodes[0])])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > dfs order > DFS with endNode as last descendant should not include parent siblings"
    ))
  func dfsEndNodeLastDescendant() throws {
    let (editor, _, _) = try dfsOrderEditor()
    try editor.update {
      let (elementNode1, _) = try elementPair()
      let results = try dfs(elementNode1, elementNode1.getLastDescendant()!)
      expectDFS(
        results,
        [
          (1, elementNode1), (2, try elementNode1.getFirstDescendant()!),
          (2, try elementNode1.getLastDescendant()!),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS with endNode as ElementNode should stop at the ElementNode, not include children"
    ))
  func dfsEndNodeElement() throws {
    let editor = try newEditor()
    try editor.update {
      let (elementNode1, elementNode2) = try elementPair()
      let first = try elementNode1.getFirstDescendant()!
      let last = try elementNode1.getLastDescendant()!
      expectDFS(
        try dfs(nil, elementNode2),
        [(0, try getRoot()), (1, elementNode1), (2, first), (2, last), (1, elementNode2)])
      expectDFS(
        try dfs(elementNode1, elementNode2),
        [(1, elementNode1), (2, first), (2, last), (1, elementNode2)])
      expectDFS(try dfs(elementNode1, elementNode1), [(1, elementNode1)])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS triggers getLatest()"
    ))
  func dfsGetLatest() throws {
    let (editor, keys) = try getLatestEditor()
    try update(editor) {
      let root = try getNodeByKeyOrThrow(keys.root)
      let paragraph = try getNodeByKeyOrThrow(keys.paragraph)
      let block1 = try #require(try getNodeByKey(keys.block1) as? ElementNode)
      let block2 = try getNodeByKeyOrThrow(keys.block2)
      let block3 = try createTestElementNode()
      try block1.append(block3)
      expectDFS(
        try dfs(root),
        [
          (0, try root.getLatest()), (1, try paragraph.getLatest()), (2, try block1.getLatest()),
          (3, try block3.getLatest()), (2, try block2.getLatest()),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS triggers getLatest()"
    ))
  func reverseDfsGetLatest() throws {
    let (editor, keys) = try getLatestEditor()
    try update(editor) {
      let root = try getNodeByKeyOrThrow(keys.root)
      let paragraph = try getNodeByKeyOrThrow(keys.paragraph)
      let block1 = try #require(try getNodeByKey(keys.block1) as? ElementNode)
      let block2 = try getNodeByKeyOrThrow(keys.block2)
      let block3 = try createTestElementNode()
      try block1.append(block3)
      expectDFS(
        try reverseDfs(),
        [
          (0, try root.getLatest()), (1, try paragraph.getLatest()), (2, try block2.getLatest()),
          (2, try block1.getLatest()), (3, try block3.getLatest()),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > DFS of empty ParagraphNode returns only itself"
    ))
  func dfsEmptyParagraph() throws {
    let (editor, paragraphKey) = try emptyParagraphEditor()
    try update(editor) {
      let paragraph = try getNodeByKeyOrThrow(paragraphKey)
      expectDFS(try dfs(paragraph), [(1, try paragraph.getLatest())])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS of empty ParagraphNode returns only itself"
    ))
  func reverseDfsEmptyParagraph() throws {
    let (editor, paragraphKey) = try emptyParagraphEditor()
    try update(editor) {
      let paragraph = try getNodeByKeyOrThrow(paragraphKey)
      expectDFS(try reverseDfs(paragraph, paragraph), [(1, try paragraph.getLatest())])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > reverse DFS of empty RootNode returns only itself"
    ))
  func reverseDfsEmptyRoot() throws {
    let editor = try newEditor()
    try update(editor) { expectDFS(try reverseDfs(), [(0, try getRoot().getLatest())]) }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalNodeHelpers.test.ts::LexicalNodeHelpers tests > $getNextSiblingOrParentSibling"
    ))
  func nextSiblingOrParentSibling() throws {
    let editor = try newEditor()
    try update(editor) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      let text1 = try createTextNode("text1")
      let text2 = try createTextNode("text2").toggleUnmergeable()
      try paragraph.append(text1, text2)
      try root.append(paragraph, paragraph2)
      let a = try getNextSiblingOrParentSibling(paragraph)
      #expect(a?.0 === paragraph2 && a?.1 == 0)
      let b = try getNextSiblingOrParentSibling(text1)
      #expect(b?.0 === text2 && b?.1 == 0)
      let c = try getNextSiblingOrParentSibling(text2)
      #expect(c?.0 === paragraph2 && c?.1 == -1)
      #expect(try getNextSiblingOrParentSibling(paragraph2) == nil)
    }
  }
}
