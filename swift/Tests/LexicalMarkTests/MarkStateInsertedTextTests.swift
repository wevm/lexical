import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHistory
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of the "inserted text" describe of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.
//
// The rule: inserted text gets the ids shared by the text right before and
// right after the place it was inserted, plus, when it replaces a
// selection, the ids all the replaced text had, plus its own ids (those of
// pasted or otherwise inserted nodes). Typed text has no ids of its own.

/// `setupWithHistory()`: mounted on a root, as the reconciler is part of
/// the upstream repro.
private func setupWithHistory() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[root]",
      dependencies: [
        RichTextExtension, configExtension(HistoryExtension) { $0.delay = 0 },
        MarkStateExtension,
      ]))
  try editor.setRootElement(TestRootElement())
  return editor
}

/// Copy a character range of the first paragraph.
func copy(_ editor: Editor, _ start: Int, _ end: Int) throws -> SerializedSelectedNodes {
  var json: SerializedSelectedNodes? = nil
  try update(editor) {
    json = try generateJSONFromSelectedNodes(editor, select(start, end))
  }
  return json!
}

func paste(
  _ editor: Editor, _ json: SerializedSelectedNodes, _ selectTarget: @escaping () throws -> Void
) throws {
  try update(editor, selectTarget)
  try update(editor) {
    try insertGeneratedNodes(
      editor, generateNodesFromSerializedNodes(json.nodes), getSelection()!)
  }
}

func firstText(_ index: Int = 0) throws -> TextNode {
  try paragraph(index).getFirstChildOrThrow() as! TextNode
}

func lastText(_ index: Int = 0) throws -> TextNode {
  try paragraph(index).getLastChildOrThrow() as! TextNode
}

func toggleSelectionFormat(_ format: String) throws {
  (try getSelection() as? RangeSelection)?.toggleFormat(format)
}

private func setupEditorWithMark() throws -> Editor {
  let editor = try createMarkEditor()
  try update(editor) { try initDocument() }
  try mark(editor, 8, 15, "c1")
  return editor
}

@Suite struct MarkStateInsertedTextTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > typing next to a gap in a mark does not spread the mark over the gap"
    ))
  func typingNextToGap() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("kjisk") }
    try mark(editor, 0, 5, "c4")
    try update(editor) { try removeMarkID("c4", select(1, 3)) }
    #expect(try tree(editor) == ##"paragraph("k"[c4] "ji" "sk"[c4])"##)
    try update(editor) { _ = try firstText().select(1, 1) }
    try typeText(editor, "C")
    #expect(try tree(editor) == ##"paragraph("k"[c4] "Cji" "sk"[c4])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > typing with a toggled format next to a gap in a mark"
    ))
  func typingWithToggledFormatNextToGap() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("kjisk") }
    try mark(editor, 0, 5, "c4")
    try update(editor) { try removeMarkID("c4", select(1, 3)) }
    try update(editor) {
      try select(3)
      try toggleSelectionFormat("bold")
    }
    try typeText(editor, "C")
    #expect(try tree(editor) == ##"paragraph("k"[c4] "ji" "C" "sk"[c4])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > a caret set programmatically at the start of a run"
    ))
  func caretSetAtStartOfRun() throws {
    // The core moves a caret at offset 0 of a TextNode to the end of the
    // previous one before inserting: the text must still stay out of the
    // marks that start there. With a root element, as the core normalizes
    // the caret as it does for a caret read from the DOM
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument("aek") }
    try mark(editor, 0, 1, "c2")
    try mark(editor, 0, 1, "c3")
    try mark(editor, 1, 3, "c2")
    try mark(editor, 1, 3, "c5")
    #expect(try tree(editor) == ##"paragraph("a"[c2,c3] "ek"[c2,c5])"##)
    let places: [() throws -> Void] = [
      { _ = try lastText().select(0, 0) },
      { _ = try paragraph().getLastChildOrThrow().selectStart() },
    ]
    for place in places {
      let copyEditor = try createMarkEditor(MarkEditorOptions(withRoot: true))
      try copyEditor.setEditorState(
        copyEditor.parseEditorState(editor.getEditorState().toJSON()))
      try update(copyEditor, place)
      try typeText(copyEditor, "B")
      #expect(try tree(copyEditor) == ##"paragraph("a"[c2,c3] "B"[c2] "ek"[c2,c5])"##)
    }
    // And at the end of the first run
    try update(editor) { _ = try firstText().select(1, 1) }
    try typeText(editor, "B")
    #expect(try tree(editor) == ##"paragraph("a"[c2,c3] "B"[c2] "ek"[c2,c5])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text inserted in the same update that moves the caret to an edge"
    ))
  func insertedInSameUpdateThatMovesCaret() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("abcdef") }
    try mark(editor, 0, 3, "c1")
    try update(editor) { try select(5) }
    try update(editor) {
      _ = try firstText().select(3, 3)
      try (getSelection() as? RangeSelection)?.insertText("X")
    }
    #expect(try tree(editor) == ##"paragraph("abc"[c1] "Xdef")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text replacing a whole marked run keeps its ids, text replacing across its edge does not"
    ))
  func replacingWholeRun() throws {
    let editor = try setupEditorWithMark()
    try update(editor) { try select(8, 15) }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue " "X"[c1] " in Q3.")"##)
    let across = try setupEditorWithMark()
    try update(across) { try select(5, 11) }
    try typeText(across, "X")
    #expect(try tree(across) == ##"paragraph("RevenX" "w 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > composition inside a run, in a new TextNode for a toggled format"
    ))
  func compositionInToggledFormat() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("abcdef") }
    try mark(editor, 0, 6, "c1")
    try update(editor) {
      try select(3)
      try toggleSelectionFormat("bold")
    }
    try update(editor) { try dispatchCompositionStart(editor) }
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      let node = try selection.anchor.getNode() as! TextNode
      #expect(try node.isComposing())
      _ = try node.setTextContent("日")
      _ = try node.select(1, 1)
    }
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == ##"paragraph("abc"[c1] "日"[c1] "def"[c1])"##)
    #expect(try editor.read { try getRoot().getAllTextNodes()[1].hasFormat("bold") })
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasting several nodes inside a run: all of them join it"
    ))
  func pastingSeveralNodesInsideRun() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try initDocument("aaaa")
      _ = try paragraph().append(createTextNode("bbbb"))
    }
    try mark(editor, 1, 3, "x")
    try mark(editor, 4, 8, "c1")
    #expect(try tree(editor) == ##"paragraph("a" "aa"[x] "a" "bbbb"[c1])"##)
    let json = try copy(editor, 1, 4)
    try paste(editor, json) { try select(6) }
    #expect(
      try tree(editor) == ##"paragraph("a" "aa"[x] "a" "bb"[c1] "aa"[c1,x] "abb"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasted text keeps its own ids at the edge of a run"
    ))
  func pastedKeepsOwnIDsAtEdge() throws {
    // The same ids as the run it is pasted at the end of
    let editor = try createMarkEditor()
    try update(editor) {
      try initDocument("abcd")
      _ = try paragraph().append(createTextNode("zz").toggleFormat("bold"))
    }
    try mark(editor, 0, 4, "c1")
    try paste(editor, copy(editor, 2, 4)) { _ = try firstText().select(4, 4) }
    #expect(try tree(editor) == ##"paragraph("abcdcd"[c1] "zz")"##)
    // Other ids, at the start of a run
    let other = try createMarkEditor()
    try update(other) { try initDocument("xy abcd") }
    try mark(other, 0, 2, "c2")
    try mark(other, 3, 7, "c1")
    try paste(other, copy(other, 0, 2)) { _ = try lastText().select(0, 0) }
    #expect(try tree(other) == ##"paragraph("xy"[c2] " " "xy"[c2] "abcd"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > pasted marked text inside a run keeps its ids and joins the run"
    ))
  func pastedMarkedTextInsideRun() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("xy abcd") }
    try mark(editor, 0, 2, "c2")
    try mark(editor, 3, 7, "c1")
    try paste(editor, copy(editor, 0, 2)) { try select(5) }
    #expect(
      try tree(editor) == ##"paragraph("xy"[c2] " " "ab"[c1] "xy"[c1,c2] "cd"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > text inserted with insertNodes keeps its own ids"
    ))
  func insertNodesKeepsOwnIDs() throws {
    let editor = try setupEditorWithMark()
    try update(editor) {
      try select(15)
      let node = try createTextNode("new")
      try insertNodes([node])
      try addMarkID("c1", node.select(0, 3))
    }
    // Equal ids at the edge: merged into the run, and kept
    try update(editor) {
      try select(18)
      let node = try createTextNode("!").toggleFormat("bold")
      try insertNodes([node])
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%new"[c1] "!" " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > undo after removing marked text restores the output"
    ))
  func undoAfterRemovingMarkedText() throws {
    // A history setEditorState that other transforms also dirty: the output
    // must not depend on the shape of its dirty sets
    for bold in [false, true] {
      let editor = try setupWithHistory()
      try update(editor) {
        let node = try createTextNode("xqCm")
        try getRoot().clear().append(createParagraphNode().append(node))
        if bold {
          _ = try node.toggleFormat("bold")
        }
        try addMarkID("c2", node.select(0, 4))
      }
      _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
      try update(editor) { _ = try firstText().select(3, 3) }
      try update(editor) { _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND) }
      try update(editor) { _ = try firstText(1).select(0, 1) }
      try update(editor) { _ = try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true) }
      _ = try editor.dispatchCommand(UNDO_COMMAND)
      #expect(try tree(editor) == ##"paragraph("xqC"[c2]) / paragraph("m"[c2])"##)
      #expect(try registryText(editor) == ["c2": ["m", "xqC"]])
      try update(editor) { try removeMarkID("c2") }
      #expect(try tree(editor) == ##"paragraph("xqC") / paragraph("m")"##)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > inserted text > a setEditorState of a state from elsewhere rescans the output"
    ))
  func setEditorStateRescans() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    let marked = editor.getEditorState()
    try update(editor) { try removeMarkID("c1") }
    #expect(try markIDNodeKeys(editor).size == 0)
    // A transform that dirties another node during the setEditorState
    let unregister = try editor.registerNodeTransform(RootNode.self) { root in
      for node in try root.getAllTextNodes() where try node.getTextContent() == " in Q3." {
        _ = try node.setTextContent(" in Q4.")
      }
    }
    try editor.setEditorState(marked)
    unregister()
    #expect(try registryText(editor) == ["c1": ["grew 4%"]])
  }
}
