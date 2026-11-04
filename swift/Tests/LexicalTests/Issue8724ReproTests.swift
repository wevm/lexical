import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8724Repro.test.ts.

private let ext = defineExtension(
  name: "[8724]", dependencies: [RichTextExtension],
  nodes: {
    [.klass(TestDecoratorNode.self), .klass(TestElementNode.self), .klass(TestShadowRootNode.self)]
  })

private let listExt = defineExtension(
  name: "[8724-list]", dependencies: [RichTextExtension, ListExtension],
  nodes: { [.klass(TestDecoratorNode.self)] })

/// A block DecoratorNode must never become the direct child of a
/// ListItemNode.
private func assertNoBlockInsideListItem(_ node: LexicalNode) throws {
  if let item = node as? ListItemNode {
    for child in try item.getChildren() {
      let isBlock = try (child is ElementNode || child is DecoratorNode) && !child.isInline()
      #expect(
        isBlock == (child is ListNode),
        "ListItemNode should not directly hold non-inline \(child.getType())")
    }
  }
  if let element = node as? ElementNode {
    for child in try element.getChildren() {
      try assertNoBlockInsideListItem(child)
    }
  }
}

/// Runs `setup`, then inserts `nodes()` at the selection it leaves; returns
/// the editor and the key of the first inserted node.
private func paste(
  _ extension_: LexicalExtension<EmptyConfig, Void, Void>, _ setup: @escaping () throws -> Void,
  _ nodes: @escaping () throws -> [LexicalNode]
) throws -> (Editor, NodeKey) {
  let editor = try buildEditorFromExtensions(extension_)
  try editor.update(discrete: true, setup)
  var insertedKey = ""
  try editor.update(discrete: true) {
    let selection = try #require(try getSelection() as? RangeSelection, "Expected RangeSelection")
    let inserted = try nodes()
    insertedKey = inserted[0].getKey()
    try selection.insertNodes(inserted)
  }
  return (editor, insertedKey)
}

private func blockDecorator() throws -> [LexicalNode] {
  [try createTestDecoratorNode().setIsInline(false)]
}

/// A TestElementNode holding a paragraph with "abc", the caret at its end.
private func elementHoldingParagraph() throws {
  let text = try createTextNode("abc")
  try getRoot().clear().append(
    createTestElementNode().append(createParagraphNode().append(text)))
  _ = try text.select(3, 3)
}

@Suite struct Issue8724ReproTests {
  // A non-inline ElementNode may only contain inline children (#8724). When a
  // block cursor sits on such an element that, in a malformed document, holds
  // a block-level child directly, pasting another block node used to throw
  // (#8713). It must instead split up to the nearest root/shadow root so the
  // pasted block never becomes the child of an inline-only element.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > block node at a block cursor inside an inline-only element splits up to the shadow root"
    ))
  func splitsUpToTheShadowRoot() throws {
    let (editor, key) = try paste(
      ext,
      {
        let element = try createTestElementNode()
        // A pre-existing (malformed) block child so the block cursor lands
        // on an element point with no block ancestor.
        try element.append(createTestDecoratorNode().setIsInline(false))
        try getRoot().clear().append(createTestShadowRootNode().append(element))
        _ = try element.select(1, 1)
      }, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let inserted = try #require(try getNodeByKey(key), "Expected the inserted node to exist")
      // The pasted block landed in the nearest valid container (the shadow
      // root), not nested inside the inline-only element.
      #expect(try inserted.getParent() is TestShadowRootNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > block node at a block cursor inside an inline-only element splits up to the root"
    ))
  func splitsUpToTheRoot() throws {
    let (editor, key) = try paste(
      ext,
      {
        let element = try createTestElementNode()
        try element.append(createTestDecoratorNode().setIsInline(false))
        try getRoot().clear().append(element)
        _ = try element.select(1, 1)
      }, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let inserted = try #require(try getNodeByKey(key), "Expected the inserted node to exist")
      // The pasted block landed directly in the root.
      #expect(try inserted.getParent() is RootNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization of non-inline nodes (#8713, #8724) > still inserts a block node directly at a block cursor on a shadow root (#8708)"
    ))
  func insertsDirectlyOnAShadowRoot() throws {
    let (editor, _) = try paste(
      ext,
      {
        let shadow = try createTestShadowRootNode()
        try shadow.append(createTestDecoratorNode().setIsInline(false))
        try getRoot().clear().append(shadow)
        _ = try shadow.select(1, 1)
      }, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let shadow = try #require(try getRoot().getFirstChild() as? TestShadowRootNode)
      // Both block decorators are direct children of the shadow root.
      #expect(try shadow.getChildrenSize() == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization does not regress lists (#8724) > block node pasted in a list item text splits the list (unchanged CASE 3)"
    ))
  func blockInListItemTextSplitsTheList() throws {
    let (editor, key) = try paste(
      listExt,
      {
        let text = try createTextNode("hello")
        try getRoot().clear().append(
          createListNode("bullet").append(createListItemNode().append(text)))
        _ = try text.select(2, 2)
      }, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let inserted = try #require(try getNodeByKey(key), "Expected the inserted node to exist")
      // The block landed at the root (between the split lists), never inside
      // a ListItemNode.
      #expect(!(try inserted.getParent() is ListItemNode))
      try assertNoBlockInsideListItem(getRoot())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste normalization does not regress lists (#8724) > block node at an element point on a list item holding a nested list does not crash"
    ))
  func blockOnItemHoldingNestedList() throws {
    let (editor, key) = try paste(
      listExt,
      {
        let nestingItem = try createListItemNode().append(
          createListNode("bullet").append(createListItemNode().append(createTextNode("b"))))
        try getRoot().clear().append(
          createListNode("bullet").append(
            createListItemNode().append(createTextNode("a")), nestingItem))
        // Element point on the listitem that holds the nested list: this is
        // an INTERNAL_$isBlock=false node with no block ancestor.
        _ = try nestingItem.select(0, 0)
      }, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let inserted = try #require(try getNodeByKey(key), "Expected the inserted node to exist")
      // The block is not nested inside a ListItemNode, and the surrounding
      // list structure remains valid.
      #expect(!(try inserted.getParent() is ListItemNode))
      try assertNoBlockInsideListItem(getRoot())
    }
  }

  // When the target block lives inside a non-inline element that is not a
  // root or shadow root (so it may only contain inline children, e.g. the
  // ParagraphNode inside a CollapsibleTitleNode), a pasted block cannot be
  // placed as its sibling without nesting it in the inline-only element. The
  // paste is flattened to its inline content; a block with no inline form is
  // dropped.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste flattens non-inline nodes inside an inline-only element (#8724) > drops a block-only paste with no inline form"
    ))
  func dropsABlockOnlyPaste() throws {
    let (editor, key) = try paste(ext, elementHoldingParagraph, blockDecorator)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      // The block decorator was dropped entirely; the document is unchanged.
      #expect(try getNodeByKey(key) == nil)
      let element = try #require(try getRoot().getFirstChild() as? ElementNode)
      #expect(try element.getTextContent() == "abc")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8724Repro.test.ts::Paste flattens non-inline nodes inside an inline-only element (#8724) > keeps the inline content of a block paste"
    ))
  func keepsInlineContentOfABlockPaste() throws {
    // The paragraph's inline content is kept; the block wrapper is dropped.
    let (editor, _) = try paste(ext, elementHoldingParagraph) {
      [try createParagraphNode().append(createTextNode("def"))]
    }
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let element = try #require(try getRoot().getFirstChild() as? ElementNode)
      #expect(try element.getTextContent() == "abcdef")
    }
  }
}
