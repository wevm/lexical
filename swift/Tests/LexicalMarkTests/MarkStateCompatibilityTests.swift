import Lexical
import LexicalExtension
import LexicalLink
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of the "links", "migration and compatibility", "slots" and
// "markable nodes" describes, and the top-level JSON test, of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.

/// "Revenue " link("grew"[c1]) " 4% in"[c1] " Q3."
private func setupLinked() throws -> Editor {
  let editor = try createMarkEditor()
  try update(editor) { try initDocument() }
  try update(editor) {
    try select(8, 12)
    try toggleLink("https://example.com")
  }
  try mark(editor, 8, 18, "c1")
  #expect(try tree(editor) == ##"paragraph("Revenue " link("grew"[c1]) " 4% in"[c1] " Q3.")"##)
  return editor
}

private func linkedOnly() throws -> Editor {
  let editor = try createMarkEditor()
  try update(editor) { try initDocument() }
  try update(editor) {
    try select(8, 12)
    try toggleLink("https://example.com")
  }
  try mark(editor, 8, 12, "c1")
  return editor
}

private func runAfterLink() throws -> TextNode {
  try paragraph().getChildren()[2] as! TextNode
}

@Suite struct MarkStateLinksTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing at the start of a run right after a marked link"
    ))
  func typingAtStartAfterLink() throws {
    let editor = try setupLinked()
    try update(editor) { _ = try runAfterLink().select(0, 0) }
    try typeText(editor, "X")
    #expect(
      try tree(editor) == ##"paragraph("Revenue " link("grew"[c1]) "X 4% in"[c1] " Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing at the end of marked link text"
    ))
  func typingAtEndOfLinkText() throws {
    let editor = try setupLinked()
    try update(editor) { _ = try paragraph().getAllTextNodes()[1].select(4, 4) }
    try typeText(editor, "X")
    #expect(try editor.read { try nodeMarkIDsOfText("X") } == ["c1"])
    #expect(
      try editor.read { try paragraph().getAllTextNodes().map { try $0.getTextContent() }.joined() }
        == "Revenue grewX 4% in Q3.")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing with a toggled format at the link boundary"
    ))
  func toggledFormatAtLinkBoundary() throws {
    let editor = try setupLinked()
    try update(editor) {
      _ = try runAfterLink().select(0, 0)
      try toggleSelectionFormat("bold")
    }
    try typeText(editor, "X")
    #expect(try editor.read { try nodeMarkIDsOfText("X") } == ["c1"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > typing after the end of a marked link that ends the mark"
    ))
  func typingAfterLinkThatEndsMark() throws {
    let editor = try linkedOnly()
    try update(editor) { _ = try runAfterLink().select(0, 0) }
    try typeText(editor, "X")
    #expect(try editor.read { try nodeMarkIDsOfText("X") } == [])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > links > the caret after a marked link is in the mark"
    ))
  func caretBeforeLinkIsInMark() throws {
    let editor = try linkedOnly()
    try update(editor) {
      // End of "Revenue ", right before the link
      _ = try paragraph().getAllTextNodes()[0].select(8, 8)
      #expect(try getMarkIDsAtSelection() == ["c1"])
    }
  }
}

@Suite struct MarkStateMigrationTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > a MarkNode document migrates to the same tree"
    ))
  func markNodeDocumentMigrates() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    // Nested MarkNodes (no nested element resolver registered)
    try update(editor) { try wrapSelectionInMarkNode(select(8, 15), false, "c1") }
    try update(editor) { try wrapSelectionInMarkNode(select(12, 19), false, "c2") }
    #expect(
      try tree(editor)
        == ##"paragraph("Revenue " mark[c1]("grew" mark[c2](" 4%")) mark[c2](" in ") "Q3.")"##)
    var count = 0
    try update(editor) { count = try migrateMarkNodesToMarkState() }
    #expect(count == 3)

    let direct = try createMarkEditor()
    try update(direct) { try initDocument() }
    try mark(direct, 8, 15, "c1")
    try mark(direct, 12, 19, "c2")
    #expect(try tree(editor) == tree(direct))
    #expect(try registryText(editor) == registryText(direct))
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > a MarkNode with several ids migrates"
    ))
  func markNodeWithSeveralIDs() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("a"), createMarkNode(["x", "y"]).append(createTextNode("b"))))
    }
    try update(editor) { _ = try migrateMarkNodesToMarkState() }
    #expect(try tree(editor) == ##"paragraph("a" "b"[x,y])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > migration and compatibility > an editor without the extension keeps the ids"
    ))
  func editorWithoutExtensionKeepsIDs() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    let json = try stateJSON(editor)

    let plain = try buildEditorFromExtensions(
      defineExtension(name: "[root]", dependencies: [RichTextExtension]))
    try plain.setEditorState(plain.parseEditorState(json))
    try update(plain) {
      _ = try paragraph().selectStart()
      try getSelection()!.insertText("Q4 ")
    }
    let roundTrip = try createMarkEditor()
    try roundTrip.setEditorState(roundTrip.parseEditorState(stateJSON(plain)))
    #expect(
      try tree(roundTrip)
        == ##"paragraph("Q4 Revenue " "grew"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##)
    // The output is rebuilt from a loaded editor state
    #expect(try registryText(roundTrip) == ["c1": [" 4%", "grew"], "c2": [" 4%", " in "]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > ids in JSON are a set: unordered input is saved sorted and without duplicates"
    ))
  func idsInJSONAreASet() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    let json = try stateJSON(editor)
    let text = #""text":"Revenue grew 4% in Q3.""#
    #expect(json.contains(text))
    let edited = json.replacingFirst(text, text + #","$":{"markIds":["c2","c1","c2",7]}"#)
    let loaded = try createMarkEditor()
    try loaded.setEditorState(loaded.parseEditorState(edited))
    #expect(try tree(loaded) == ##"paragraph("Revenue grew 4% in Q3."[c1,c2])"##)
    #expect(
      try registryText(loaded) == [
        "c1": ["Revenue grew 4% in Q3."], "c2": ["Revenue grew 4% in Q3."],
      ])
    let saved = try stateJSON(loaded)
    #expect(try paragraphChildStates(loaded) == [markIdsState("c1", "c2")])
    // Stable across another round trip
    let again = try createMarkEditor()
    try again.setEditorState(again.parseEditorState(saved))
    #expect(try stateJSON(again) == saved)
  }
}

/// paragraph(P) / figure{caption paragraph} / paragraph("After it.")
private func initFigure() throws {
  try initDocument()
  let figure = try FigureNode()
  _ = try getRoot().append(figure, createParagraphNode().append(createTextNode("After it.")))
  _ = try setSlot(figure, "caption", createParagraphNode().append(createTextNode("A caption")))
}

private func selectAcrossFigure() throws -> RangeSelection {
  let (first, _) = try locate(paragraph(0), 8)
  let (last, _) = try locate(paragraph(2), 5)
  let selection = createRangeSelection()
  try selection.anchor.set(first.getKey(), 8, .text)
  try selection.focus.set(last.getKey(), 5, .text)
  return selection
}

private let MARKED =
  ##"paragraph("Revenue " "grew 4% in Q3."[c1]) / "##
  + ##"test-figure{paragraph("A caption"[c1])} / "## + ##"paragraph("After"[c1] " it.")"##

@Suite struct MarkStateSlotsTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > marking a range marks the content of the slots in it"
    ))
  func markingRangeMarksSlots() throws {
    let editor = try createMarkEditor()
    try update(editor, initFigure)
    try update(editor) { _ = try addMarkID("c1", selectAcrossFigure()) }
    #expect(try tree(editor) == MARKED)
    #expect(try registryText(editor) == ["c1": ["A caption", "After", "grew 4% in Q3."]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > a NodeSelection of a slot host marks the content of its slots"
    ))
  func nodeSelectionOfSlotHost() throws {
    let editor = try createMarkEditor()
    try update(editor, initFigure)
    try update(editor) {
      let selection = createNodeSelection()
      selection.add(try getRoot().getChildAtIndex(1)!.getKey())
      _ = try addMarkID("c1", selection)
    }
    #expect(
      try tree(editor)
        == ##"paragraph("Revenue grew 4% in Q3.") / "##
        + ##"test-figure{paragraph("A caption"[c1])} / paragraph("After it.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > removing an id removes it from the slots too"
    ))
  func removingFromSlots() throws {
    for withExtension in [true, false] {
      let editor = try createMarkEditor()
      try update(editor, initFigure)
      try update(editor) { _ = try addMarkID("c1", selectAcrossFigure()) }
      let json = try stateJSON(editor)
      // Without the extension, removeMarkID walks the document
      let target =
        withExtension
        ? editor
        : try buildEditorFromExtensions(
          defineExtension(
            name: "[root]", dependencies: [RichTextExtension],
            nodes: { [.klass(FigureNode.self)] }))
      try target.setEditorState(target.parseEditorState(json))
      try update(target) { try removeMarkID("c1") }
      #expect(
        try tree(target)
          == ##"paragraph("Revenue grew 4% in Q3.") / "##
          + ##"test-figure{paragraph("A caption")} / paragraph("After it.")"##)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > the output is rebuilt from the slots of a loaded state"
    ))
  func outputRebuiltFromSlots() throws {
    let editor = try createMarkEditor()
    try update(editor, initFigure)
    try update(editor) { _ = try addMarkID("c1", selectAcrossFigure()) }
    let loaded = try createMarkEditor()
    try loaded.setEditorState(loaded.parseEditorState(stateJSON(editor)))
    #expect(try tree(loaded) == MARKED)
    #expect(try registryText(loaded) == ["c1": ["A caption", "After", "grew 4% in Q3."]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > slots > a MarkNode inside a slot migrates"
    ))
  func markNodeInsideSlotMigrates() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try initFigure()
      let caption = try getSlot(getRoot().getChildAtIndex(1)!, "caption") as! ElementNode
      let text = try caption.getFirstChildOrThrow() as! TextNode
      let rest = try text.splitText(2)[1]
      let markNode = try createMarkNode(["c2"])
      _ = try rest.replace(markNode)
      _ = try markNode.append(rest)
    }
    var count = 0
    try update(editor) { count = try migrateMarkNodesToMarkState() }
    #expect(count == 1)
    #expect(
      try tree(editor)
        == ##"paragraph("Revenue grew 4% in Q3.") / "##
        + ##"test-figure{paragraph("A " "caption"[c2])} / paragraph("After it.")"##)
  }
}

private func initMixed() throws {
  try getRoot().clear().append(
    createParagraphNode().append(
      createTextNode("ab"), try TestDecoratorTextNode(), createLineBreakNode(), try ChipNode(),
      createTextNode("cd")))
}

private func selectAll() throws -> RangeSelection {
  let paragraph = try paragraph()
  let selection = createRangeSelection()
  try selection.anchor.set(paragraph.getFirstChildOrThrow().getKey(), 0, .text)
  try selection.focus.set(paragraph.getLastChildOrThrow().getKey(), 2, .text)
  return selection
}

@Suite struct MarkStateMarkableNodesTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > by default TextNode, LineBreakNode and inline decorators, including DecoratorTextNode"
    ))
  func defaultMarkableNodes() throws {
    let editor = try createMarkEditor(MarkEditorOptions(withRoot: true))
    try update(editor, initMixed)
    try update(editor) { _ = try addMarkID("c1", selectAll()) }
    #expect(
      try tree(editor)
        == ##"paragraph("ab"[c1] test-decorator-text[c1] br[c1] test-chip[c1] "cd"[c1])"##)
    #expect(try registryText(editor) == ["c1": ["\n", "#", "@", "ab", "cd"]])
    let decoratorText = try editor.read {
      try renderDOM(editor, paragraph().getChildAtIndex(1)!)
    }
    #expect(decoratorText.getAttribute(MARK_STATE_IDS_ATTRIBUTE) == "c1")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > a class list"
    ))
  func classList() throws {
    let editor = try createMarkEditor(MarkEditorOptions(markableNodes: [.klass(TextNode.self)]))
    try update(editor, initMixed)
    try update(editor) { _ = try addMarkID("c1", selectAll()) }
    #expect(
      try tree(editor) == ##"paragraph("ab"[c1] test-decorator-text br test-chip "cd"[c1])"##)
    #expect(try registryText(editor) == ["c1": ["ab", "cd"]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > a guard function"
    ))
  func guardFunction() throws {
    let editor = try createMarkEditor(
      MarkEditorOptions(markableNodes: [
        .predicate { node in
          guard let text = node as? TextNode else { return false }
          return !(try text.getTextContent().contains("c"))
        },
        .klass(TestDecoratorTextNode.self),
      ]))
    try update(editor, initMixed)
    try update(editor) { _ = try addMarkID("c1", selectAll()) }
    #expect(
      try tree(editor) == ##"paragraph("ab"[c1] test-decorator-text[c1] br test-chip "cd")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > markable nodes > extending the default with a block decorator"
    ))
  func extendingDefaultWithBlockDecorator() throws {
    let editor = try createMarkEditor(
      MarkEditorOptions(markableNodes: DEFAULT_MARKABLE_NODES + [.klass(FigureNode.self)]))
    try update(editor) {
      try initDocument()
      _ = try getRoot().append(try FigureNode())
    }
    try update(editor) {
      let selection = createNodeSelection()
      selection.add(try getRoot().getLastChildOrThrow().getKey())
      _ = try addMarkID("c1", selection)
    }
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3.") / test-figure[c1]{}"##)
    #expect(try registryText(editor) == ["c1": [""]])
  }
}
