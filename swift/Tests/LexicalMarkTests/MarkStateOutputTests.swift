import Lexical
import LexicalExtension
import LexicalHTML
import LexicalHistory
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of the "output", "strict ids", "queries", "DOM", "isMarkIDShown",
// "HTML" and "ids" describes of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.
//
// Upstream reads the DOM of an editor mounted on a contenteditable root.
// The Swift editors are mounted on a `TestRootElement`, and the DOM of a
// leaf is what the editor's render config makes of it (`renderedLeaves`).
// A re-render is observed as the update it commits; where upstream checks
// that the DOM selection stays put, the port checks that the update is
// tagged SKIP_DOM_SELECTION_TAG and where the model caret is.

func expectInvalidMarkID(_ body: () throws -> Void) {
  do {
    try body()
    Issue.record("expected an invalid mark id error")
  } catch {
    #expect(String(describing: error).contains("invalid mark id"))
  }
}

func importHTML(_ editor: Editor, _ html: String) throws {
  try update(editor) {
    let dom = DOMParser().parseFromString(html, "text/html")
    try getRoot().clear().append(contentsOf: generateNodesFromDOMViaExtension(dom))
  }
}

func stateJSON(_ editor: Editor) throws -> String {
  JSONValue.object(try editor.getEditorState().toJSON()).stringify()
}

private func isMarkIDShownSignal(_ editor: Editor) throws -> Signal<(String) -> Bool> {
  try getExtensionDependencyFromEditor(editor, MarkStateExtension).output.isMarkIDShown
}

private func disabledSignal(_ editor: Editor) throws -> Signal<Bool> {
  try getExtensionDependencyFromEditor(editor, MarkStateExtension).output.disabled
}

@Suite struct MarkStateOutputTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > after loading JSON, and $removeMarkID(id) through it"
    ))
  func afterLoadingJSON() throws {
    let source = try createMarkEditor()
    try update(source) { try initDocument() }
    try mark(source, 8, 15, "c1")
    try mark(source, 0, 3, "c2")
    let json = try stateJSON(source)
    let editor = try createMarkEditor()
    try editor.setEditorState(editor.parseEditorState(json))
    #expect(try registryText(editor) == ["c1": ["grew 4%"], "c2": ["Rev"]])
    try update(editor) { try removeMarkID("c1") }
    #expect(try registryText(editor) == ["c2": ["Rev"]])
    #expect(try tree(editor) == ##"paragraph("Rev"[c2] "enue grew 4% in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > drops the leaves of a removed paragraph, and only those"
    ))
  func dropsRemovedParagraph() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try initDocument()
      _ = try getRoot().append(createParagraphNode().append(createTextNode("Two")))
    }
    try mark(editor, 0, 3, "c1")
    try update(editor) {
      let selection = createRangeSelection()
      let node = try paragraph(1).getFirstChildOrThrow()
      try selection.anchor.set(node.getKey(), 0, .text)
      try selection.focus.set(node.getKey(), 3, .text)
      _ = try addMarkID("c1", selection)
      _ = try addMarkID("c2", selection)
    }
    #expect(try registryText(editor) == ["c1": ["Rev", "Two"], "c2": ["Two"]])
    try update(editor) { try paragraph(1).remove() }
    #expect(try registryText(editor) == ["c1": ["Rev"]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > disabled from the start: empty, and $removeMarkID(id) walks the document"
    ))
  func disabledFromStart() throws {
    let editor = try createMarkEditor(MarkEditorOptions(disabled: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    #expect(try markIDNodeKeys(editor).size == 0)
    try update(editor) { try removeMarkID("c1") }
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > output > disabled with exportIDsToHTML: HTML export still writes the ids"
    ))
  func disabledExportsHTML() throws {
    let editor = try createMarkEditor(MarkEditorOptions(disabled: true, exportIDsToHTML: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    let html = try editor.read { try generateHtmlFromNodes(editor, nil) }
    #expect(html.contains("\(MARK_STATE_IDS_ATTRIBUTE)=\"c1\""))
  }
}

@Suite struct MarkStateStrictIDsTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > strict ids > invalid ids from JSON are dropped"
    ))
  func invalidIDsFromJSON() throws {
    let source = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(source) { try initDocument() }
    try mark(source, 8, 15, "c1")
    let sourceJSON = try stateJSON(source)
    #expect(sourceJSON.contains(#"["c1"]"#))
    let json = sourceJSON.replacingFirst(#"["c1"]"#, #"["a b","","ok","\u00a0",7]"#)
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try editor.setEditorState(editor.parseEditorState(json))
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[ok] " in Q3.")"##)
    #expect(try registryText(editor) == ["ok": ["grew 4%"]])
    let marked = try editor.read {
      try renderDOM(editor, paragraph().getChildAtIndex(1)!)
    }
    #expect(marked.getAttribute(MARK_STATE_IDS_ATTRIBUTE) == "ok")
    // Only invalid ids: the leaf carries none
    let none = try createMarkEditor()
    try none.setEditorState(
      none.parseEditorState(sourceJSON.replacingFirst(#"["c1"]"#, #"["a b"]"#)))
    #expect(try tree(none) == ##"paragraph("Revenue grew 4% in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > strict ids > invalid ids applied as NodeState or written with $setState are dropped"
    ))
  func invalidIDsAppliedAsNodeState() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try update(editor) {
      let node = try paragraph().getFirstChildOrThrow()
      // How a state from elsewhere is applied
      try getWritableNodeState(node.getWritable()).updateFromUnknown(
        MARK_STATE_KEY, .array([.string("b c"), .string("x"), .string("")]))
    }
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3."[x])"##)
    #expect(try registryText(editor) == ["x": ["Revenue grew 4% in Q3."]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > strict ids > HTML import drops invalid tokens and keeps the valid ones"
    ))
  func htmlImportDropsInvalidTokens() throws {
    let editor = try createMarkEditor()
    try importHTML(editor, "<p><span \(MARK_STATE_IDS_ATTRIBUTE)=\"  c1 \t  c2 \">x</span></p>")
    #expect(try tree(editor) == ##"paragraph("x"[c1,c2])"##)
  }
}

@Suite struct MarkStateQueriesTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > $addMarkID returns the marked nodes, and a collapsed selection marks nothing"
    ))
  func addMarkIDReturnsNodes() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try update(editor) {
      #expect(try addMarkID("c1", select(3)).isEmpty)
      let nodes = try addMarkID("c1", select(8, 15))
      #expect(try nodes.map { try $0.getTextContent() } == ["grew 4%"])
      // A collapsed selection removes nothing either
      try removeMarkID("c1", select(10))
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > 'all' with nothing shared, and a backward selection"
    ))
  func allWithNothingShared() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 0, 7, "c1")
    try mark(editor, 8, 15, "c2")
    try editor.read {
      let first = try paragraph().getChildren()[0]
      let last = try paragraph().getLastChildOrThrow()
      let selection = createRangeSelection()
      try selection.anchor.set(last.getKey(), 0, .text)
      try selection.focus.set(first.getKey(), 0, .text)
      #expect(try selection.isBackward())
      #expect(try getMarkIDsAtSelection(selection, .all) == [])
      #expect(try getMarkIDsAtSelection(selection, .any) == ["c1", "c2"])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > queries > two editors with different markableNodes"
    ))
  func twoEditorsDifferentMarkableNodes() throws {
    let lineBreaksOnly = try createMarkEditor(
      MarkEditorOptions(markableNodes: [.klass(LineBreakNode.self)]))
    let defaults = try createMarkEditor()
    try update(lineBreaksOnly) { try initDocument() }
    try update(defaults) { try initDocument() }
    try mark(lineBreaksOnly, 8, 15, "c1")
    try mark(defaults, 8, 15, "c1")
    #expect(try tree(lineBreaksOnly) == ##"paragraph("Revenue grew 4% in Q3.")"##)
    #expect(try tree(defaults) == ##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##)
  }
}

@Suite struct MarkStateDOMTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > DOM > marked leaves get the theme classes and the ids attribute"
    ))
  func themeClassesAndAttribute() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    #expect(
      try renderedWithAttribute(editor) == [
        ["grew", "c1", "mark"], [" 4%", "c1 c2", "mark mark-overlap"], [" in ", "c2", "mark"],
      ])
    try update(editor) { try removeMarkID("c2") }
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew 4%", "c1", "mark"], [" in Q3.", nil, ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > DOM > toggling disabled removes and restores the decoration"
    ))
  func togglingDisabled() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    let disabled = try disabledSignal(editor)
    disabled.value = true
    try update(editor) {}
    #expect(try renderedWithAttribute(editor) == [])
    #expect(try renderedLeaves(editor).allSatisfy { $0[2] == "" })
    #expect(try markIDNodeKeys(editor).size == 0)
    disabled.value = false
    try update(editor) {}
    #expect(try renderedWithAttribute(editor).map { $0[1] } == ["c1"])
    #expect(try registryText(editor) == ["c1": ["grew 4%"]])
  }
}

@Suite struct MarkStateIsMarkIDShownTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > the default renders every id"
    ))
  func defaultRendersEveryID() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    #expect(try isMarkIDShownSignal(editor).value("anything"))
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", "c1", "mark"], [" 4%", "c1 c2", "mark mark-overlap"],
        [" in ", "c2", "mark"], ["Q3.", nil, ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > hidden ids are not rendered but stay in the document"
    ))
  func hiddenIDsStayInDocument() throws {
    let editor = try createMarkEditor(
      MarkEditorOptions(exportIDsToHTML: true, isMarkIDShown: { $0 != "c2" }, withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", "c1", "mark"], [" 4%", "c1", "mark"], [" in ", nil, ""],
        ["Q3.", nil, ""],
      ])
    // Only rendering is filtered
    #expect(try registryText(editor) == ["c1": [" 4%", "grew"], "c2": [" 4%", " in "]])
    var atSelection: [String] = []
    try update(editor) { atSelection = try getMarkIDsAtSelection(select(12, 19), .any) }
    #expect(atSelection == ["c1", "c2"])
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html.contains("\(MARK_STATE_IDS_ATTRIBUTE)=\"c1 c2\""))
    #expect(html.contains("\(MARK_STATE_IDS_ATTRIBUTE)=\"c2\""))
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > newly marked text follows the filter"
    ))
  func newlyMarkedFollowsFilter() throws {
    let editor = try createMarkEditor(
      MarkEditorOptions(isMarkIDShown: { $0 != "c2" }, withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 12, "c2")
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", nil, ""], [" 4% in Q3.", nil, ""],
      ])
    try mark(editor, 8, 12, "c1")
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", "c1", "mark"], [" 4% in Q3.", nil, ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > assigning a new filter at runtime re-renders the affected leaves"
    ))
  func newFilterAtRuntime() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    let isMarkIDShown = try isMarkIDShownSignal(editor)
    var resolved = Set<String>()
    var updates = 0
    let unregister = editor.registerUpdateListener { _ in updates += 1 }
    defer { unregister() }
    let decorated = try renderedLeaves(editor)
    let editorState = try stateJSON(editor)

    // Resolve c2
    resolved.insert("c2")
    let snapshot1 = resolved
    isMarkIDShown.value = { !snapshot1.contains($0) }
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", "c1", "mark"], [" 4%", "c1", "mark"], [" in ", nil, ""],
        ["Q3.", nil, ""],
      ])
    // A re-render only: the document and the output are unchanged
    #expect(try stateJSON(editor) == editorState)
    #expect(try registryText(editor) == ["c1": [" 4%", "grew"], "c2": [" 4%", " in "]])

    // Resolve c1 too: nothing is decorated
    resolved.insert("c1")
    let snapshot2 = resolved
    isMarkIDShown.value = { !snapshot2.contains($0) }
    #expect(try renderedWithAttribute(editor) == [])
    #expect(try renderedLeaves(editor).allSatisfy { $0[2] == "" })

    // Reopen both
    resolved.removeAll()
    isMarkIDShown.value = { _ in true }
    #expect(try renderedLeaves(editor) == decorated)
    #expect(updates == 3)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > assigning a new filter changes no node, also where an element's stored text format is not its first text's"
    ))
  func newFilterChangesNoNode() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]",
        dependencies: [
          RichTextExtension, configExtension(HistoryExtension) { $0.delay = 0 },
          MarkStateExtension,
        ]))
    // The first paragraph's textFormat (0) is not its bold text's format, as
    // a client that never rendered it (headless, an agent) writes it
    try update(editor) {
      let bold = try createTextNode("bold")
      _ = try bold.toggleFormat("bold")
      try getRoot().clear().append(
        createParagraphNode().append(bold),
        createParagraphNode().append(createTextNode("t4z t5z")))
    }
    try editor.setRootElement(TestRootElement())
    let textFormat = { () throws -> Int in
      try (getRoot().getFirstChild() as? ElementNode)?.getTextFormat() ?? -1
    }
    #expect(try editor.read(textFormat) == 0)
    // A comment in the other paragraph, so that a filter re-renders
    try update(editor) { _ = try addMarkID("c1", select(0, 3, paragraph(1))) }
    // The last edit, which an undo takes back
    try update(editor) { try paragraph(1).selectEnd().insertText(" t6z") }
    let editorState = try stateJSON(editor)
    try isMarkIDShownSignal(editor).value = { $0 != "c1" }
    drainMicrotasks()
    #expect(try editor.read(textFormat) == 0)
    #expect(try stateJSON(editor) == editorState)
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "bold\n\nt4z t5z")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a new filter re-renders nothing in a document without marks"
    ))
  func newFilterWithoutMarks() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    var updates = 0
    let unregister = editor.registerUpdateListener { _ in updates += 1 }
    defer { unregister() }
    try isMarkIDShownSignal(editor).value = { _ in false }
    #expect(updates == 0)
    try mark(editor, 8, 15, "c1")
    #expect(updates == 1)
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew 4%", nil, ""], [" in Q3.", nil, ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a filter assigned inside an edit re-renders after it, without changing it"
    ))
  func filterAssignedInsideEdit() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try update(editor) { try select(15) }
    let isMarkIDShown = try isMarkIDShownSignal(editor)
    try update(editor) {
      isMarkIDShown.value = { _ in false }
      _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "X")
    }
    // Text typed at the end of the run is still not marked
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##)
    drainMicrotasks()
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew 4%", nil, ""], ["X in Q3.", nil, ""],
      ])
  }

  /// The two "filter assigned in an update listener" tests: the listener
  /// queues an edit and assigns a filter, in either order.
  private func filterInListener(editFirst: Bool) throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try update(editor) { try select(10) }
    let isMarkIDShown = try isMarkIDShownSignal(editor)
    var armed = true
    var tags: [Set<String>] = []
    let unregister = editor.registerUpdateListener { payload in
      tags.append(Set(payload.tags.array))
      if !armed {
        return
      }
      armed = false
      // A listener (an app's thread store reacting to the document)
      // resolves the thread and queues an edit of its own
      if !editFirst {
        isMarkIDShown.value = { _ in false }
      }
      try editor.update {
        _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "X")
      }
      if editFirst {
        isMarkIDShown.value = { _ in false }
      }
    }
    defer { unregister() }
    // Moving the caret commits an update that runs the listener
    try update(editor) { try select(15) }
    drainMicrotasks()
    // Text typed at the end of the run is still not marked
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##)
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew 4%", nil, ""], ["X in Q3.", nil, ""],
      ])
    // The edit moved the caret past the inserted text, and the re-render
    // (the last update) does not move the DOM selection
    try editor.read {
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try selection.anchor.getNode().getTextContent() == "X in Q3.")
      #expect(selection.anchor.offset == 1)
    }
    #expect(tags.last?.contains(SKIP_DOM_SELECTION_TAG) == true)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a filter assigned in an update listener with an edit re-renders after that edit, which keeps its caret update"
    ))
  func filterInListenerWithEdit() throws {
    try filterInListener(editFirst: false)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a filter assigned in an update listener after its edit re-renders after that edit, which keeps its caret update"
    ))
  func filterInListenerAfterEdit() throws {
    try filterInListener(editFirst: true)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a new filter does not move the DOM selection out of another editable"
    ))
  func newFilterKeepsDOMSelection() throws {
    // The Swift root has no DOM selection: the re-render is an update
    // tagged SKIP_DOM_SELECTION_TAG, which is what leaves it alone
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try update(editor) { try select(10) }
    var tags: [Set<String>] = []
    let unregister = editor.registerUpdateListener { tags.append(Set($0.tags.array)) }
    defer { unregister() }
    try isMarkIDShownSignal(editor).value = { _ in false }
    #expect(try renderedLeaves(editor).allSatisfy { $0[2] == "" })
    #expect(tags.count == 1)
    #expect(tags.first?.contains(SKIP_DOM_SELECTION_TAG) == true)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a filter changed while disabled applies when re-enabled"
    ))
  func filterChangedWhileDisabled() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    let disabled = try disabledSignal(editor)
    disabled.value = true
    try update(editor) {}
    try isMarkIDShownSignal(editor).value = { $0 != "c1" }
    #expect(try renderedWithAttribute(editor) == [])
    disabled.value = false
    try update(editor) {}
    #expect(
      try renderedLeaves(editor) == [
        ["Revenue ", nil, ""], ["grew", nil, ""], [" 4%", "c2", "mark"], [" in ", "c2", "mark"],
        ["Q3.", nil, ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > isMarkIDShown > a filter set before the root element is attached applies on mount"
    ))
  func filterBeforeMount() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try isMarkIDShownSignal(editor).value = { _ in false }
    try editor.setRootElement(TestRootElement())
    #expect(try renderedWithAttribute(editor) == [])
    #expect(try renderedLeaves(editor).allSatisfy { $0[2] == "" })
  }
}

@Suite struct MarkStateHTMLTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > HTML > ids are not exported by default"
    ))
  func notExportedByDefault() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    let html = try editor.read { try generateHtmlFromNodes(editor, nil) }
    #expect(html.contains("grew 4%"))
    #expect(!html.contains(MARK_STATE_IDS_ATTRIBUTE))
    #expect(!html.contains("c1"))
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > HTML > export and import round trip the ids"
    ))
  func exportImportRoundTrip() throws {
    let editor = try createMarkEditor(MarkEditorOptions(exportIDsToHTML: true))
    try update(editor) { try initDocument() }
    try update(editor) { try select(10, 13).formatText("bold") }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    let before = try tree(editor)
    let html = try editor.read { try generateHtmlFromNodes(editor, nil) }
    #expect(html.contains("\(MARK_STATE_IDS_ATTRIBUTE)=\"c1 c2\""))
    let target = try createMarkEditor()
    try importHTML(target, html)
    #expect(try tree(target) == before)
    #expect(try registryText(target) == registryText(editor))
  }
}

@Suite struct MarkStateIDsTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids > ids must be non-empty and free of whitespace"
    ))
  func idsMustBeValid() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    for id in ["", "a b", "a\tb", "a\nb", " a"] {
      expectInvalidMarkID {
        try update(editor) { _ = try addMarkID(id, select(8, 15)) }
      }
    }
    // Anything else is fine, and round trips through HTML
    let ids = ["a\"b", "a,b", "<x>", "__proto__", "é-1"]
    let exporting = try createMarkEditor(MarkEditorOptions(exportIDsToHTML: true))
    try update(exporting) { try initDocument() }
    for id in ids {
      try mark(exporting, 8, 15, id)
    }
    let html = try exporting.read { try generateHtmlFromNodes(exporting, nil) }
    let target = try createMarkEditor()
    try importHTML(target, html)
    #expect(try target.read { try nodeMarkIDsOfText("grew") } == jsSort(ids))
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids > migrating a MarkNode with an invalid id throws"
    ))
  func migratingInvalidIDThrows() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(createMarkNode(["a b"]).append(createTextNode("x"))))
    }
    expectInvalidMarkID {
      try update(editor) { _ = try migrateMarkNodesToMarkState() }
    }
  }
}
