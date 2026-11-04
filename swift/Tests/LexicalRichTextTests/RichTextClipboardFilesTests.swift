import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Ports of packages/lexical-rich-text/src/__tests__/unit
// ShouldHandlePasteAsFiles.test.ts and RichTextDropFiles.test.ts.

private let fakeImage = DataTransferFile(
  name: "photo.png", type: "image/png", data: Array("fake-bytes".utf8))

private func dataTransfer(
  _ text: [(String, String)], _ files: [DataTransferFile]
)
  -> DataTransfer
{
  let dataTransfer = DataTransfer()
  for (type, value) in text {
    dataTransfer.setData(type, value)
  }
  dataTransfer.files = files
  return dataTransfer
}

/// Runs `dispatch` with a critical DRAG_DROP_PASTE handler that records the
/// files it receives.
private func captureFiles(
  _ editor: Editor, _ dispatch: () throws -> Void
) throws
  -> [DataTransferFile]?
{
  var dispatched: [DataTransferFile]?
  let dispose = editor.registerCommand(
    DRAG_DROP_PASTE,
    { files, _ in
      dispatched = files
      return true
    }, COMMAND_PRIORITY_CRITICAL)
  defer { dispose() }
  try dispatch()
  return dispatched
}

private func paste(_ editor: Editor, _ dataTransfer: DataTransfer) throws -> [DataTransferFile]? {
  try captureFiles(editor) {
    _ = try editor.dispatchCommand(
      PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: dataTransfer)))
  }
}

nonisolated(unsafe) private let filesAlwaysWin: ShouldHandlePasteAsFiles = { files, _ in
  !files.isEmpty
}

private func richTextEditor(
  _ shouldHandlePasteAsFiles: ShouldHandlePasteAsFiles? = nil,
  _ initial: InitialEditorStateType = .none
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        configExtension(RichTextExtension) { config in
          if let shouldHandlePasteAsFiles {
            config.shouldHandlePasteAsFiles = shouldHandlePasteAsFiles
          }
        }
      ], editorConfig: InitialEditorConfig(initialEditorState: initial)))
}

@Suite struct ShouldHandlePasteAsFilesTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > default: a file pasted alongside text/html (browser copy-image) does NOT dispatch DRAG_DROP_PASTE"
    ))
  func defaultBlocksFiles() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    #expect(
      try paste(editor, dataTransfer([("text/html", #"<img src="blob:fake">"#)], [fakeImage]))
        == nil)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override: treating only text/plain as content lets the file win over an incidental text/html entry"
    ))
  func overrideLetsFileWin() throws {
    let editor = try richTextEditor(filesAlwaysWin)
    defer { editor.dispose() }
    #expect(
      try paste(editor, dataTransfer([("text/html", #"<img src="blob:fake">"#)], [fakeImage]))
        == [fakeImage])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override falls back to HTML when DRAG_DROP_PASTE is not handled"
    ))
  func fallsBackToHTML() throws {
    let editor = try richTextEditor(
      filesAlwaysWin, .update { _ in try getRoot().append(try createParagraphNode()).selectEnd() })
    defer { editor.dispose() }
    try update(editor) { try getRoot().getFirstChildOrThrow().selectEnd() }
    _ = try editor.dispatchCommand(
      PASTE_COMMAND,
      .clipboard(
        ClipboardEvent(
          "paste",
          clipboardData: dataTransfer([("text/html", "<p>HTML fallback</p>")], [fakeImage]))))
    #expect(try editor.read(.forceCommit) { try getRoot().getTextContent() } == "HTML fallback")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > override does not hijack a plain text/html paste with no file"
    ))
  func noFileNoHijack() throws {
    let editor = try richTextEditor(filesAlwaysWin, .update { _ in _ = try getRoot() })
    defer { editor.dispose() }
    #expect(try paste(editor, dataTransfer([("text/html", "<b>hello</b>")], [])) == nil)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > a file with no text content at all still dispatches DRAG_DROP_PASTE under the default config"
    ))
  func fileOnly() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    #expect(try paste(editor, dataTransfer([], [fakeImage])) == [fakeImage])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/ShouldHandlePasteAsFiles.test.ts::RichTextExtension shouldHandlePasteAsFiles > shouldHandlePasteAsFiles can be modified as a signal after the editor is built"
    ))
  func signal() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    let dep = try getExtensionDependencyFromEditor(editor, RichTextExtension)
    dep.output.shouldHandlePasteAsFiles.value = filesAlwaysWin
    #expect(
      try paste(editor, dataTransfer([("text/html", #"<img src="blob:fake">"#)], [fakeImage]))
        != nil)
  }
}

private func dropEvent(_ files: [DataTransferFile]) -> DragEvent {
  DragEvent("drop", dataTransfer: dataTransfer([], files), clientX: 10, clientY: 10)
}

/// A root that resolves every point to `caret`, as a mocked
/// `caretRangeFromPoint` does.
private final class CaretRoot: LexicalRootElement {
  var isConnected: Bool { true }
  let caret: () throws -> PointCaret?
  init(_ caret: @escaping () throws -> PointCaret?) { self.caret = caret }
  func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret? { try caret() }
}

@Suite struct RichTextDropFilesTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > unhandled desktop text files are still consumed"
    ))
  func unhandledConsumed() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    let event = dropEvent([DataTransferFile(name: "document.txt", type: "text/plain")])
    #expect(try editor.dispatchCommand(DROP_COMMAND, event))
    #expect(event.defaultPrevented)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > dropped files are forwarded even when the drop point resolves to no caret"
    ))
  func forwardedWithoutCaret() throws {
    let editor = try richTextEditor()
    defer { editor.dispose() }
    let files = try captureFiles(editor) {
      _ = try editor.dispatchCommand(DROP_COMMAND, dropEvent([fakeImage]))
    }
    #expect(files == [fakeImage])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextDropFiles.test.ts::RichTextExtension DROP_COMMAND file handling > dropped files are forwarded when the drop point does resolve to a caret"
    ))
  func forwardedWithCaret() throws {
    let editor = try richTextEditor(
      nil,
      .update { _ in
        try getRoot().append(try createParagraphNode().append(try createTextNode("hello")))
      })
    defer { editor.dispose() }
    let root = CaretRoot {
      let text = try #require(try getRoot().getFirstDescendant() as? TextNode)
      return try getTextPointCaret(text, .next, 1)
    }
    try editor.setRootElement(root)
    try editor.update(discrete: true) {}
    let files = try captureFiles(editor) {
      _ = try editor.dispatchCommand(DROP_COMMAND, dropEvent([fakeImage]))
    }
    #expect(files == [fakeImage])
    try editor.setRootElement(nil)
  }
}

// Port of packages/lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts.
// Cutting the whole document wipes it as deleting does (#5835).

private func cutEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "[5835-cut]", dependencies: [RichTextExtension, ListExtension]))
  try editor.setRootElement(TestRootElement())
  return editor
}

/// The document as nested `[type, children]` pairs, with text spelled out.
private indirect enum Outline: Equatable {
  case element(String, [Outline])
  case leaf(String)
}

private func outline(_ node: LexicalNode) throws -> Outline {
  if let element = node as? ElementNode {
    return .element(node.getType(), try element.getChildren().map(outline))
  }
  return .leaf("\(node.getType()):\(try node.getTextContent())")
}

private func readOutline(_ editor: Editor) throws -> [Outline] {
  try editor.read { try getRoot().getChildren().map(outline) }
}

private func cut(_ editor: Editor, _ data: DataTransfer = DataTransfer()) throws {
  _ = try editor.dispatchCommand(
    CUT_COMMAND, .clipboard(ClipboardEvent("cut", clipboardData: data)))
  drainMicrotasks()
}

@Suite struct RichTextCutSelectAllTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a lone heading leaves an empty paragraph"
    ))
  func loneHeading() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(try createHeadingNode("h1").append(try createTextNode("hi")))
    }
    try editor.update(discrete: true) { _ = try selectAll() }
    try cut(editor)
    #expect(try readOutline(editor) == [.element("paragraph", [])])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > cutting a lone heading and pasting restores it"
    ))
  func cutAndPasteHeading() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(try createHeadingNode("h1").append(try createTextNode("hi")))
    }
    let before = try readOutline(editor)
    try editor.update(discrete: true) { _ = try selectAll() }
    let cutData = DataTransfer()
    try cut(editor, cutData)
    #expect(try readOutline(editor) == [.element("paragraph", [])])
    _ = try editor.dispatchCommand(
      PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: cutData)))
    drainMicrotasks()
    #expect(try readOutline(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a collapsed caret in an empty heading cuts nothing"
    ))
  func collapsedInEmptyHeading() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let block = try createHeadingNode("h1")
      try getRoot().clear().append(block)
      try block.selectStart()
    }
    try cut(editor)
    #expect(try readOutline(editor) == [.element("heading", [])])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a lone list leaves an empty paragraph"
    ))
  func loneList() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createListNode("bullet").append(createListItemNode().append(createTextNode("one"))))
    }
    try editor.update(discrete: true) { _ = try selectAll() }
    try cut(editor)
    #expect(try readOutline(editor) == [.element("paragraph", [])])
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > cutting a lone list and pasting restores it"
    ))
  func cutAndPasteList() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("one")),
          createListItemNode().append(createTextNode("two"))))
    }
    let before = try readOutline(editor)
    try editor.update(discrete: true) { _ = try selectAll() }
    let cutData = DataTransfer()
    try cut(editor, cutData)
    #expect(try readOutline(editor) == [.element("paragraph", [])])
    _ = try editor.dispatchCommand(
      PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: cutData)))
    drainMicrotasks()
    #expect(try readOutline(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextCutSelectAll.test.ts::select-all + cut (#5835) > a collapsed caret in an empty list cuts nothing"
    ))
  func collapsedInEmptyList() throws {
    let editor = try cutEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let block = try createListNode("bullet").append(createListItemNode())
      try getRoot().clear().append(block)
      try block.selectStart()
    }
    try cut(editor)
    #expect(try readOutline(editor) == [.element("list", [.element("listitem", [])])])
  }
}
