import LexicalTesting
import Testing

@testable import Lexical

// Port of packages/lexical/src/__tests__/unit/LexicalEditorState.test.ts,
// whose editor `initializeUnitTest` mounts (createMountedTestEditor).
@Suite struct LexicalEditorStateTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > isEmpty"))
  func isEmpty() throws {
    let editor = try createMountedTestEditor()
    try editor.update { () throws in
      let root = try createRootNode()
      #expect(EditorState(NodeMap([("root", root)])).isEmpty())
      // A state with nothing in it at all is emptier still.
      #expect(EditorState(NodeMap()).isEmpty())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor()
    try editor.update { () throws in
      let root = try createRootNode()
      let nodeMap = NodeMap([("root", root)])
      let editorState = EditorState(nodeMap)
      #expect(editorState._nodeMap === nodeMap)
      #expect(editorState._selection == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > read('pending' | 'latest' | 'force-commit')"
    ))
  func readModes() throws {
    let editor = try createMountedTestEditor()
    let textContent = { try getRoot().getTextContent() }
    // A regular update is processed synchronously but committed
    // asynchronously.
    try editor.update { () throws in
      try getRoot().append(createParagraphNode().append(createTextNode("foo")))
    }
    #expect(try editor.getEditorState().read(textContent) == "")
    // read(.pending) observes the pending state without flushing it.
    #expect(try editor.read(.pending, textContent) == "foo")
    // read(.latest) reads the committed state without flushing.
    #expect(try editor.read(.latest, textContent) == "")
    #expect(try editor.getEditorState().read(textContent) == "")
    // read() defaults to force-commit, which flushes the pending state.
    #expect(try editor.read(textContent) == "foo")
    #expect(try editor.getEditorState().read(textContent) == "foo")
    // Without a pending state, every mode reads the committed state.
    #expect(try editor.read(.pending, textContent) == "foo")
    #expect(try editor.read(.latest, textContent) == "foo")
    #expect(try editor.read(.forceCommit, textContent) == "foo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > read()"))
  func read() throws {
    let editor = try createMountedTestEditor()
    try update(editor) { () throws in
      let paragraph = try createParagraphNode()
      let text = try createTextNode("foo")
      try paragraph.append(text)
      try getRoot().append(paragraph)
    }
    var root: RootNode!
    var paragraph: ParagraphNode!
    var text: TextNode!
    try editor.getEditorState().read { () throws in
      root = try getRoot()
      paragraph = try #require(try root.getFirstChild() as? ParagraphNode)
      text = try #require(try paragraph.getFirstChild() as? TextNode)
    }
    #expect(root.__cachedText == "foo")
    #expect(root.__dir == nil && root.__format == 0 && root.__indent == 0)
    #expect(root.__first == "1" && root.__last == "1" && root.__size == 1)
    #expect(root.__key == "root" && root.__parent == nil)
    #expect(root.__prev == nil && root.__next == nil)
    #expect(root.__slotHost == nil && root.__slots == nil)
    #expect(root.__style == "" && root.__textFormat == 0 && root.__textStyle == "")
    #expect(root.__type == "root")

    #expect(paragraph.__dir == nil && paragraph.__format == 0 && paragraph.__indent == 0)
    #expect(paragraph.__first == "2" && paragraph.__last == "2" && paragraph.__size == 1)
    #expect(paragraph.__key == "1" && paragraph.__parent == "root")
    #expect(paragraph.__prev == nil && paragraph.__next == nil)
    #expect(paragraph.__slotHost == nil && paragraph.__slots == nil)
    #expect(paragraph.__style == "" && paragraph.__textFormat == 0)
    #expect(paragraph.__textStyle == "" && paragraph.__type == "paragraph")

    #expect(text.__detail == 0 && text.__format == 0 && text.__mode == 0)
    #expect(text.__key == "2" && text.__parent == "1")
    #expect(text.__prev == nil && text.__next == nil)
    #expect(text.__style == "" && text.__text == "foo" && text.__type == "text")

    #expect {
      try editor.getEditorState().read { _ = try getEditor() }
    } throws: { error in
      "\(error)".contains("Unable to find an active editor")
    }
    #expect(try editor.getEditorState().read(editor: editor) { try getEditor() } === editor)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > toJSON()"))
  func toJSON() throws {
    let editor = try createMountedTestEditor()
    try update(editor) { () throws in
      let paragraph = try createParagraphNode()
      let text = try createTextNode("Hello world")
      try text.select(6, 11)
      try paragraph.append(text)
      try getRoot().append(paragraph)
    }
    // Compared as a structure, as upstream does: key order is not part of
    // this test (the vectors pin the bytes).
    let expected = try JSONValue.parse(
      #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hello world","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
    )
    #expect(JSONValue.object(try editor.getEditorState().toJSON()) == expected)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorState.test.ts::LexicalEditorState tests > ensure garbage collection works as expected"
    ))
  func garbageCollection() throws {
    let editor = try createMountedTestEditor()
    try update(editor) { () throws in
      let paragraph = try createParagraphNode()
      try paragraph.append(createTextNode("foo"))
      try getRoot().append(paragraph)
    }
    // Removing the only paragraph collects everything under it.
    try update(editor) { () throws in
      try getRoot().getFirstChild()!.remove()
    }
    let nodeMap = editor.getEditorState()._nodeMap
    #expect(nodeMap.keysArray == ["root"])
    let root = try #require(nodeMap.get("root") as? RootNode)
    #expect(root.__cachedText == "")
    #expect(root.__first == nil && root.__last == nil && root.__size == 0)
    #expect(root.__key == "root" && root.__parent == nil && root.__type == "root")
  }
}
