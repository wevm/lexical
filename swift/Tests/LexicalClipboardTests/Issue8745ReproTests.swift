import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8745Repro.test.ts.

private func buildTestEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[8745]", dependencies: [RichTextExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(TestShadowRootNode.self)] }))
}

private func paragraph(_ text: String) throws -> ParagraphNode {
  try createParagraphNode().append(createTextNode(text))
}

private func shadowRoot(_ children: LexicalNode...) throws -> TestShadowRootNode {
  try createTestShadowRootNode().append(contentsOf: children)
}

/// Builds the root from `children`, runs `action` in a second update, and
/// returns the root's size and its first child's text.
private func run(
  _ children: @escaping () throws -> [LexicalNode], _ action: @escaping () throws -> Void
) throws -> (size: Int, firstText: String?) {
  let editor = try buildTestEditor()
  defer { editor.dispose() }
  try editor.update(discrete: true) { _ = try getRoot().clear().append(contentsOf: children()) }
  try editor.update(discrete: true) { try action() }
  return try editor.read { () throws in
    let root = try getRoot()
    return (try root.getChildrenSize(), try root.getFirstChild()?.getTextContent())
  }
}

private func removeAll() throws { try selectAll().removeText() }
private func deleteAll(_ isBackward: Bool) -> () throws -> Void {
  { try selectAll().deleteCharacter(isBackward) }
}

private func helloThenShadow(_ text: String = "shadow content") -> () throws -> [LexicalNode] {
  { [try paragraph("hello"), try shadowRoot(paragraph(text))] }
}

private func onlyShadow() throws -> [LexicalNode] {
  [try shadowRoot(paragraph("only shadow"))]
}

@Suite struct Issue8745ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph when document ends with a shadow root"
    ))
  func trailingShadowRoot() throws {
    let result = try run(helloThenShadow(), removeAll)
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph with nested shadow roots"
    ))
  func nestedShadowRoots() throws {
    let result = try run(
      { [try paragraph("hello"), try shadowRoot(shadowRoot(paragraph("nested content")))] },
      removeAll)
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > leaves one empty paragraph with multi-child shadow root (columns)"
    ))
  func columns() throws {
    let result = try run(
      {
        [
          try paragraph("hello"),
          try shadowRoot(shadowRoot(paragraph("column 1")), shadowRoot(paragraph("column 2"))),
        ]
      }, removeAll)
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > preserves content after shadow root when shadow root is in the middle"
    ))
  func shadowRootInMiddle() throws {
    let result = try run(
      { [try paragraph("before"), try shadowRoot(paragraph("shadow")), try paragraph("after")] },
      removeAll)
    // Anchor and focus are both direct children of the root, so the merge
    // works normally.
    #expect(result.size == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > deleteCharacter with select-all cleans up trailing shadow root"
    ))
  func deleteCharacterCleansUp() throws {
    let result = try run(helloThenShadow(), deleteAll(false))
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > heading first child + backward deleteCharacter: reproduces issue author scenario"
    ))
  func headingFirstChild() throws {
    let result = try run(
      {
        [
          try createHeadingNode("h1").append(createTextNode("Welcome")),
          try paragraph("some text"), try shadowRoot(paragraph("shadow content")),
        ]
      }, deleteAll(true))
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > paragraph first child + backward deleteCharacter: simple case"
    ))
  func paragraphFirstChild() throws {
    let result = try run(helloThenShadow(), deleteAll(true))
    #expect(result.size == 1)
    #expect(result.firstText == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > HR + collapsible only: select-all delete leaves at least one child"
    ))
  func hrAndCollapsible() throws {
    let result = try run(
      {
        [
          try createTestDecoratorNode().setIsInline(false),
          try shadowRoot(shadowRoot(paragraph("Title")), shadowRoot(paragraph("Content"))),
        ]
      },
      {
        // What the browser does: an element selection over every root child.
        let root = try getRoot()
        try root.select(0, root.getChildrenSize()).deleteCharacter(true)
      })
    #expect(result.size >= 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > select-all + forward deleteCharacter leaves at least one child"
    ))
  func forwardDeleteCharacter() throws {
    #expect(try run(helloThenShadow("shadow"), deleteAll(false)).size >= 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > select-all + removeText leaves at least one child"
    ))
  func removeTextLeavesChild() throws {
    #expect(try run(helloThenShadow("shadow"), removeAll).size >= 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > NodeSelection.deleteNodes leaves at least one root child"
    ))
  func nodeSelectionDeleteNodes() throws {
    let result = try run(
      { [try createTestDecoratorNode().setIsInline(false), try shadowRoot(paragraph("shadow"))] },
      {
        let selection = createNodeSelection()
        for child in try getRoot().getChildren() {
          selection.add(child.getKey())
        }
        try setSelection(selection)
        try selection.deleteNodes()
      })
    #expect(result.size >= 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > partial deletion within shadow root preserves structure"
    ))
  func partialDeletionInShadowRoot() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    var keys: (title: NodeKey, content: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let titleText = try createTextNode("title text")
      let contentText = try createTextNode("content text")
      let container = try shadowRoot(
        createParagraphNode().append(titleText),
        shadowRoot(createParagraphNode().append(contentText)))
      _ = try getRoot().clear().append(paragraph("before"), container)
      keys = (titleText.getKey(), contentText.getKey())
    }
    try editor.update(discrete: true) { () throws in
      let titleText = try #require(try getNodeByKey(keys.title) as? TextNode)
      let contentText = try #require(try getNodeByKey(keys.content) as? TextNode)
      let selection = try titleText.select(5, 5)
      try selection.setTextNodeRange(titleText, 5, contentText, 7)
      try selection.removeText()
    }
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let container = try #require(try root.getLastChild() as? ElementNode)
      #expect(try container.getChildrenSize() == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > shadow root only (no paragraph): select-all + removeText leaves at least one child"
    ))
  func onlyShadowRemoveText() throws {
    #expect(try run(onlyShadow, removeAll).size >= 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8745Repro.test.ts::Select-all + delete with trailing shadow root (#8745) > shadow root only (no paragraph): select-all + backward deleteCharacter leaves at least one child"
    ))
  func onlyShadowDeleteCharacter() throws {
    #expect(try run(onlyShadow, deleteAll(true)).size >= 1)
  }
}
