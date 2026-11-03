import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts.
// Upstream mounts each editor on a jsdom `contenteditable`; Swift mounts it
// on a `TestRootElement`, and pastes with a `DataTransfer` as upstream does.
// The check that the rendered DOM's `textContent` matches the state has no
// equivalent: Swift renders no DOM.

private func mount(_ prefix: String = "b") throws -> Editor {
  let editor = try asEdited(
    buildEditorFromExtensions(
      defineExtension(
        name: "[block-id-dom]",
        dependencies: [
          RichTextExtension,
          configExtension(HistoryExtension) { $0.delay = 0 },
          configExtension(BlockIdExtension) { $0.generateId = counter(prefix) },
        ],
        // Editors that share a namespace paste each other's Lexical JSON.
        editorConfig: InitialEditorConfig(
          namespace: "block-id-dom", initialEditorState: InitialEditorStateType.none))))
  try editor.setRootElement(TestRootElement())
  return editor
}

private func copy(_ editor: Editor, _ from: Int, _ to: Int) throws -> LexicalClipboardData {
  var data = LexicalClipboardData()
  try discrete(editor) {
    try selectBlocks(from, to)
    data = try getClipboardDataFromSelection(getSelection())
  }
  return data
}

private func paste(
  _ editor: Editor, _ data: LexicalClipboardData, _ place: @escaping () throws -> Void
) throws {
  let transfer = DataTransfer()
  for (type, value) in data {
    transfer.setData(type, value)
  }
  try discrete(editor) {
    try place()
    try insertDataTransferForRichText(transfer, #require(try getSelection()), editor)
  }
}

@Suite struct BlockIdDOMTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > typing, Enter and Backspace in a mounted editor"
    ))
  func typingEnterBackspace() throws {
    let editor = try mount()
    defer { editor.dispose() }
    try seed(editor, ["hello", "world"])
    try discrete(editor) {
      try caret(0, 5)
      try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "!")
    }
    #expect(try blocks(editor) == ["paragraph:hello!:b1", "paragraph:world:b2"])
    try discrete(editor) {
      try caret(0, 2)
      try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(
      try blocks(editor) == ["paragraph:he:b1", "paragraph:llo!:b3", "paragraph:world:b2"])
    try discrete(editor) {
      try caret(2, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try blocks(editor) == ["paragraph:he:b1", "paragraph:llo!world:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > Backspace at the start of the first heading"
    ))
  func backspaceAtTheFirstHeading() throws {
    let editor = try mount()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        createHeadingNode("h2").append(createTextNode("Title")), createHeadingNode("h3"),
        paragraph("after"))
    }
    try discrete(editor) {
      try caret(0, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    // Nothing to delete before it: unchanged.
    #expect(try blocks(editor) == ["heading:Title:b1", "heading::b2", "paragraph:after:b3"])
    // An empty heading is removed; the other blocks keep their ids.
    try discrete(editor) {
      _ = try nth(1).select(0, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try blocks(editor) == ["heading:Title:b1", "paragraph:after:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > copy and paste between editors and within one editor"
    ))
  func copyAndPaste() throws {
    let source = try mount("s")
    defer { source.dispose() }
    let target = try mount("t")
    defer { target.dispose() }
    try seed(source, ["alpha", "beta", "gamma"])
    try seed(target, ["one", ""])
    let data = try copy(source, 1, 2)
    #expect(data["application/x-lexical-editor"]?.contains(#""blockId":"s2""#) == true)
    // Into another editor, replacing its empty paragraph: the ids do not
    // clash there and are kept.
    try paste(target, data) { _ = try nth(1).select() }
    #expect(
      try blocks(target) == ["paragraph:one:t1", "paragraph:beta:s2", "paragraph:gamma:s3"])
    // Pasting the same blocks again into that editor: now they clash, and
    // the earlier blocks (the ones pasted first) keep the ids. The first
    // pasted block merges into the block at the caret; the second is a copy
    // of s3 placed after the original, so it gets a fresh id.
    try paste(target, data) { _ = try getRoot().selectEnd() }
    #expect(
      try blocks(target) == [
        "paragraph:one:t1", "paragraph:beta:s2", "paragraph:gammabeta:s3", "paragraph:gamma:t3",
      ])
    // Into the source editor, below the originals: the copies get fresh ids.
    try paste(source, data) { _ = try getRoot().selectEnd() }
    #expect(
      try blocks(source) == [
        "paragraph:alpha:s1", "paragraph:beta:s2", "paragraph:gammabeta:s3", "paragraph:gamma:s4",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > HTML pasted through the legacy importer gets fresh ids"
    ))
  func legacyHTMLPasteGetsFreshIds() throws {
    let editor = try mount()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    try paste(
      editor,
      ["text/html": #"<p data-block-id="b1">pasted a</p><p data-block-id="b1">pasted b</p>"#]
    ) { _ = try getRoot().selectEnd() }
    let result = try blocks(editor)
    #expect(result.first == "paragraph:onepasted a:b1")
    #expect(Array(result.dropFirst()) == ["paragraph:pasted b:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.dom.test.ts::BlockIdExtension (DOM) > undo restores ids and the index"
    ))
  func undoRestoresIds() throws {
    let editor = try mount()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    let before = try blocks(editor)
    try discrete(editor, HISTORY_PUSH_TAG) {
      try caret(0, 1)
      try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(try blocks(editor) == ["paragraph:o:b1", "paragraph:ne:b3", "paragraph:two:b2"])
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try blocks(editor) == before)
    try editor.read { () throws in
      #expect(try getBlockById("b3") == nil)
      #expect(try getBlockById("b1")?.getTextContent() == "one")
    }
  }
}
