import Lexical
import LexicalExtension
import LexicalHistory
import LexicalTesting
import Testing

// Port of packages/lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts.

// Empty inline elements change the adjacent TextNode's sibling pointer,
// without adding another dirty leaf or changing the text's serialized value.
final class InlineEmptyElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineEmptyElementNode.self, type: "inline-empty", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("span")
  }
  override func isInline() throws -> Bool { true }
}

private func getParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
}

private func readChildren(_ editor: Editor) throws -> [[String]] {
  try editor.read {
    try getParagraph().getChildren().map { [$0.getType(), try $0.getTextContent()] }
  }
}

private func createPushTagEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        // Keep every update within the merge window without relying on wall time.
        configExtension(HistoryExtension) {
          $0.delay = 1000
          $0.disabled = false
          $0.now = { 0 }
        }
      ], nodes: { [.klass(InlineEmptyElementNode.self)] }))
  try editor.update(discrete: true) { try getRoot().append(createParagraphNode()) }
  try editor.update(discrete: true) {
    let paragraph = try getParagraph()
    try paragraph.append(createTextNode("ab"))
    try paragraph.selectEnd()
  }
  return editor
}

private func getHistoryState(_ editor: Editor) throws -> HistoryState {
  try getExtensionDependencyFromEditor(editor, HistoryExtension).output.historyState.peek()
}

private let textChildren = [["text", "ab"]]
private let insertedChildren = textChildren + [["inline-empty", ""]]

@Suite struct LexicalHistoryPushTagTests {
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-push' ]"
    ),
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-push', 'history-merge' ]"
    ),
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-merge', 'history-push' ]"
    ),
    arguments: [
      [HISTORY_PUSH_TAG], [HISTORY_PUSH_TAG, HISTORY_MERGE_TAG],
      [HISTORY_MERGE_TAG, HISTORY_PUSH_TAG],
    ])
  func isolatesElementInsertion(tag: [UpdateTag]) throws {
    let editor = try createPushTagEditor()
    defer { editor.dispose() }
    var calls: [UpdateListenerPayload] = []
    let unregister = editor.registerUpdateListener { calls.append($0) }
    try editor.update(
      { try getRoot().getAllTextNodes()[0].insertAfter(create(InlineEmptyElementNode.self)) },
      EditorUpdateOptions(tag: tag, discrete: true))
    unregister()

    // Verify the regression's dirty-leaf shape so the fixture cannot silently
    // turn into a different insertion that already worked without the fix.
    try #require(calls.count == 1)
    let payload = calls[0]
    let textKey = try editor.read { try getRoot().getAllTextNodes()[0].getKey() }
    #expect(payload.dirtyLeaves.array == [textKey])
    #expect(
      try payload.editorState.read { try getRoot().getAllTextNodes()[0].exportJSON() }
        == payload.prevEditorState.read { try getRoot().getAllTextNodes()[0].exportJSON() })
    #expect(try readChildren(editor) == insertedChildren)

    // Undo the insertion and earlier text separately, then redo both. Check
    // content and structure: a stack-length assertion alone is insufficient.
    try editor.update(discrete: true) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(try readChildren(editor) == textChildren)
    try editor.update(discrete: true) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(try readChildren(editor) == [])
    try editor.update(discrete: true) { try editor.dispatchCommand(REDO_COMMAND) }
    #expect(try readChildren(editor) == textChildren)
    try editor.update(discrete: true) { try editor.dispatchCommand(REDO_COMMAND) }
    #expect(try readChildren(editor) == insertedChildren)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > still merges an untagged TextNode marked dirty without a change"
    ))
  func mergesUntaggedUnchangedTextNode() throws {
    let editor = try createPushTagEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try getRoot().getAllTextNodes()[0].markDirty() }
    #expect(try getHistoryState(editor).undoStack.count == 1)
    try editor.update(discrete: true) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(try readChildren(editor) == [])
  }
}
