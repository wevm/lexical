import Lexical
import LexicalTesting
import LexicalUtils
import Testing

// Ports of descendantsMatching.test.tsx and unwrapAndFilterDescendants.test.tsx
// in packages/lexical-utils/src/__tests__/unit.

private func setUp(_ depth: Int) throws -> Editor {
  let editor = try createHeadlessTestEditor()
  try editor.update(discrete: true) {
    try getRoot().clear().append(
      try createParagraphNode().append(contentsOf: createTextAndParagraphWithDepth(depth)))
  }
  return editor
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
}

private func allText(_ node: ElementNode) throws -> Bool {
  try node.getChildren().allSatisfy { $0 is TextNode }
}

private func descendantsMatchingAtDepth(_ depth: Int) throws {
  let editor = try setUp(depth)
  try editor.update(discrete: true) {
    let firstNode = try firstParagraph()
    #expect(try allText(firstNode) == (depth == 0))
    try firstNode.splice(
      0, firstNode.getChildrenSize(),
      descendantsMatching(firstNode.getChildren()) { $0 is TextNode })
    #expect(try allText(firstNode))
    #expect(try firstNode.getTextContent() == textContentForDepth(depth))
  }
}

private func unwrapAtDepth(_ depth: Int) throws {
  let editor = try setUp(depth)
  try editor.update(discrete: true) {
    let firstNode = try firstParagraph()
    #expect(try allText(firstNode) == (depth == 0))
    #expect(try unwrapAndFilterDescendants(firstNode) { $0 is TextNode } == (depth > 0))
    #expect(try allText(firstNode))
    #expect(try firstNode.getTextContent() == textContentForDepth(depth))
  }
}

@Suite struct DescendantsTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 0"
    ))
  func matching0() throws { try descendantsMatchingAtDepth(0) }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 1"
    ))
  func matching1() throws { try descendantsMatchingAtDepth(1) }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/descendantsMatching.test.tsx::$descendantsMatching > Can un-nest children at depth 2"
    ))
  func matching2() throws { try descendantsMatchingAtDepth(2) }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Is a no-op with valid children"
    ))
  func unwrapNoOp() throws {
    let editor = try createHeadlessTestEditor()
    try editor.update(discrete: true) { try getRoot().clear().append(try createParagraphNode()) }
    try editor.update(discrete: true) { () throws in
      #expect(try !unwrapAndFilterDescendants(getRoot()) { $0 is ParagraphNode })
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getRoot().getFirstChild() is ParagraphNode)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 0"
    ))
  func unwrap0() throws { try unwrapAtDepth(0) }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 1"
    ))
  func unwrap1() throws { try unwrapAtDepth(1) }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/unwrapAndFilterDescendants.test.tsx::$unwrapAndFilterDescendants > Can un-nest children at depth 2"
    ))
  func unwrap2() throws { try unwrapAtDepth(2) }
}
