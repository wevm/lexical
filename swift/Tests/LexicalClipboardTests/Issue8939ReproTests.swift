import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8939Repro.test.ts: at a
// block cursor (an element point beside a node `needsBlockCursorBeside`)
// Backspace and Delete remove that node, a shadow-root ElementNode host as
// well as a block decorator.

private func buildTestEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[8939]", dependencies: [RichTextExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(TestShadowRootNode.self)] }))
}

private func blockDecorator() throws -> LexicalNode {
  try createTestDecoratorNode().setIsInline(false)
}

private func makeShadowRoot(_ label: String) throws -> LexicalNode {
  try createTestShadowRootNode().append(createParagraphNode().append(createTextNode(label)))
}

enum BlockKind: String, Sendable {
  case decorator, shadowRoot

  fileprivate func make(_ label: String) throws -> LexicalNode {
    switch self {
    case .decorator: try blockDecorator()
    case .shadowRoot: try makeShadowRoot(label)
    }
  }
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection, "Expected RangeSelection")
}

private func deleteCharacter(_ editor: Editor, _ isBackward: Bool) throws {
  try editor.update(discrete: true) { () throws in
    try rangeSelection().deleteCharacter(isBackward)
  }
}

@Suite struct Issue8939ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent decorator (isBackward: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent decorator (isBackward: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent shadowRoot (isBackward: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deletes the adjacent shadowRoot (isBackward: false)"
    ),
    arguments: [
      (BlockKind.decorator, true), (.decorator, false), (.shadowRoot, true), (.shadowRoot, false),
    ])
  func deletesAdjacent(kind: BlockKind, isBackward: Bool) throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    var keyA = ""
    var keyB = ""
    try editor.update(discrete: true) {
      let a = try kind.make("x")
      let b = try kind.make("x")
      keyA = a.getKey()
      keyB = b.getKey()
      _ = try getRoot().clear().append(a, b)
      // The block cursor position between the two nodes.
      _ = try getRoot().select(1, 1)
    }
    try editor.read { () throws in
      // Both sides render a block cursor, so both must delete the same way.
      #expect(try needsBlockCursorBeside(getRoot().getChildAtIndex(0)))
      #expect(try needsBlockCursorBeside(getRoot().getChildAtIndex(1)))
    }
    try deleteCharacter(editor, isBackward)
    try editor.read { () throws in
      #expect(try getRoot().getChildrenSize() == 1)
      // Backspace removes the node before the cursor, Delete the one after.
      #expect(try getRoot().getChildAtIndex(0)?.getKey() == (isBackward ? keyB : keyA))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Backspace after the last shadow root deletes it"
    ))
  func backspaceAfterLastShadowRoot() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(makeShadowRoot("A"), makeShadowRoot("B"))
      _ = try getRoot().select(2, 2)
    }
    try deleteCharacter(editor, true)
    try editor.read { () throws in
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getRoot().getTextContent() == "A")
    }
  }

  // Removing the last top-level node would otherwise leave the root with no
  // children, so nowhere to put a caret.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only decorator leaves the root editable"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only shadowRoot leaves the root editable"
    ),
    arguments: [BlockKind.decorator, .shadowRoot])
  func deletingOnlyNode(kind: BlockKind) throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(kind.make("A"))
      _ = try getRoot().select(1, 1)
    }
    try deleteCharacter(editor, true)
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 1)
      let paragraph = try #require(children[0] as? ParagraphNode)
      #expect(try paragraph.isEmpty())
      // The caret is inside the restored paragraph, not on the root.
      #expect(try rangeSelection().anchor.getNode().getKey() == paragraph.getKey())
    }
    // Enter then behaves as in any document holding one empty paragraph.
    try editor.update(discrete: true) { () throws in
      let selection = try rangeSelection()
      _ = try selection.insertParagraph()
      try selection.insertText("X")
    }
    try editor.read { () throws in
      let texts = try getRoot().getChildren().map { child in
        try #require(child as? ParagraphNode).getTextContent()
      }
      #expect(texts == ["", "X"])
    }
  }

  // The same rule one level down.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > deleting the only block inside a shadow root keeps it editable"
    ))
  func deletingOnlyBlockInShadowRoot() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let shadow = try createTestShadowRootNode().append(blockDecorator())
      _ = try getRoot().clear().append(shadow)
      _ = try shadow.select(1, 1)
    }
    try deleteCharacter(editor, true)
    try editor.read { () throws in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      #expect(try shadow.getChildrenSize() == 1)
      let paragraph = try #require(try shadow.getFirstChild() as? ParagraphNode)
      #expect(try paragraph.isEmpty())
      #expect(try rangeSelection().anchor.getNode().getKey() == paragraph.getKey())
    }
  }

  // An empty block whose caret sits next to a shadow root is not a block
  // cursor: the empty block goes, the shadow root stays.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Backspace in an empty block after a shadow root removes the block, not the shadow root"
    ))
  func backspaceInEmptyBlockAfterShadowRoot() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      _ = try getRoot().clear().append(makeShadowRoot("A"), paragraph)
      _ = try paragraph.select(0, 0)
    }
    try deleteCharacter(editor, true)
    try editor.read { () throws in
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getRoot().getTextContent() == "A")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Enter over all of a shadow root splits an empty paragraph (element points)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > Enter over all of a shadow root splits an empty paragraph (text points)"
    ),
    arguments: ["element", "text"])
  func enterOverWholeShadowRoot(points: String) throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let shadow = try createTestShadowRootNode().append(
        createParagraphNode().append(createTextNode("inside")))
      _ = try getRoot().clear().append(shadow)
      if points == "element" {
        _ = try shadow.select(0, shadow.getChildrenSize())
      } else {
        let paragraph = try #require(try shadow.getFirstChild() as? ParagraphNode)
        let selection = try paragraph.select(0, 0)
        try selection.focus.set(paragraph.getFirstChildOrThrow().getKey(), 6, .text)
      }
    }
    try editor.update(discrete: true) { () throws in
      _ = try rangeSelection().insertParagraph()
    }
    try editor.read { () throws in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      let paragraphs = try shadow.getChildren().map { try #require($0 as? ParagraphNode) }
      #expect(try paragraphs.map { try $0.isEmpty() } == [true, true])
      #expect(try rangeSelection().anchor.getNode().getKey() == paragraphs[1].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8939Repro.test.ts::block cursor deletion beside an ElementNode (#8939) > a paragraph does not need a block cursor beside it"
    ))
  func paragraphNeedsNoBlockCursor() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(createParagraphNode().append(createTextNode("A")))
    }
    try editor.read { () throws in
      #expect(try !needsBlockCursorBeside(getRoot().getChildAtIndex(0)))
    }
  }
}
