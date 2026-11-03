import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts.
// Upstream dispatches mousedown and click on the root element below the
// last block (jsdom measures every block as zero-sized). Here the view's
// part is assumed done: it reports the click as CLICK_COMMAND with the
// root's key as its target.

private func setUpEditor(
  disabled: Bool = false, shouldInsertAfter: ((LexicalNode) throws -> Bool)? = nil,
  withInitialChildren: Bool = true
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "click-after-last-block-test",
      dependencies: [
        RichTextExtension,
        configExtension(ClickAfterLastBlockExtension) {
          $0.disabled = disabled
          if let shouldInsertAfter { $0.shouldInsertAfter = shouldInsertAfter }
        },
      ], nodes: { [.klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          guard withInitialChildren else { return }
          // A block-level decorator at the end of the document, as Equation
          // and PageBreak are in the playground.
          let decorator = try createTestDecoratorNode().setIsInline(false)
          try getRoot().append(createParagraphNode().append(createTextNode("hello")), decorator)
        }),
      register: { editor, _, _ in
        do { try editor.setRootElement(TestRootElement()) } catch { Issue.record(error) }
        return {}
      }))
}

private func clickBelowLastBlock(_ editor: Editor) throws {
  try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click", clientY: 100, targetKey: "root"))
}

private func childrenSize(_ editor: Editor) throws -> Int {
  try editor.read { try getRoot().getChildrenSize() }
}

@Suite struct ClickAfterLastBlockTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > inserts a paragraph after the last block when the last child is a block decorator"
    ))
  func insertsParagraph() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    #expect(try childrenSize(editor) == 2)
    try clickBelowLastBlock(editor)
    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 3)
      let lastChild = try #require(try root.getLastChild() as? ParagraphNode)
      #expect(try lastChild.isEmpty())
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing when the predicate rejects the last child"
    ))
  func predicateRejects() throws {
    let editor = try setUpEditor(shouldInsertAfter: { _ in false })
    defer { editor.dispose() }
    try clickBelowLastBlock(editor)
    #expect(try childrenSize(editor) == 2)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > respects disabled config"
    ))
  func respectsDisabled() throws {
    let editor = try setUpEditor(disabled: true)
    defer { editor.dispose() }
    try clickBelowLastBlock(editor)
    #expect(try childrenSize(editor) == 2)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing in read-only mode"
    ))
  func readOnly() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.setEditable(false)
    try clickBelowLastBlock(editor)
    #expect(try childrenSize(editor) == 2)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/ClickAfterLastBlock.test.ts::ClickAfterLastBlockExtension > does nothing on an empty root with no last child"
    ))
  func emptyRoot() throws {
    let editor = try setUpEditor(withInitialChildren: false)
    defer { editor.dispose() }
    try clickBelowLastBlock(editor)
    #expect(try childrenSize(editor) == 0)
  }
}
