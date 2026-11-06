import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHashtag
import LexicalHistory
import LexicalLink
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts
// ("add, remove and query", "structural edits" and "typing at the edges of
// a mark"). The rest of the file is in MarkStateInsertedTextTests.swift and
// MarkStateOutputTests.swift; the helpers are in MarkStateTestSupport.swift.
//
// A paste event dispatched at the root element dispatches PASTE_COMMAND
// outside an update; the Swift root has no DOM events, so the "paste
// event" cases dispatch the command, outside an update, as that event does.

private func editorWith(_ withMarkNode: Bool, hashtag: Bool = true) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[root]",
      dependencies: [RichTextExtension] + (hashtag ? [HashtagExtension] : [])
        + [MarkExtension] + (withMarkNode ? [] : [MarkStateExtension])))
}

private func markSelection(_ selection: RangeSelection, _ withMarkNode: Bool) throws {
  if withMarkNode {
    try wrapSelectionInMarkNode(selection, false, "c1")
  } else {
    try addMarkID("c1", selection)
  }
}

private func setupMarked() throws -> Editor {
  let editor = try createMarkEditor()
  try update(editor) { try initDocument() }
  try mark(editor, 8, 15, "c1")
  return editor
}

/// A root that resolves every point to `caret`, as a mocked
/// `caretRangeFromPoint` does.
private final class CaretRoot: LexicalRootElement {
  var isConnected: Bool { true }
  let caret: () throws -> PointCaret?
  init(_ caret: @escaping () throws -> PointCaret?) { self.caret = caret }
  func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret? { try caret() }
}

@Suite struct MarkStateAddRemoveQueryTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > overlapping marks share TextNodes regardless of order"
    ))
  func overlappingMarks() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    let expected = ##"paragraph("Revenue " "grew"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##
    #expect(try tree(editor) == expected)

    let reversed = try createMarkEditor()
    try update(reversed) { try initDocument() }
    try mark(reversed, 12, 19, "c2")
    try mark(reversed, 8, 15, "c1")
    #expect(try tree(reversed) == expected)

    // One set-valued NodeState key, a sorted array in JSON
    let states = try paragraphChildStates(editor)
    #expect(
      states == [
        nil, markIdsState("c1"), markIdsState("c1", "c2"), markIdsState("c2"), nil,
      ])
    #expect(try paragraphChildStates(reversed) == states)
    #expect(try registryText(editor) == ["c1": [" 4%", "grew"], "c2": [" 4%", " in "]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > the selection still covers the marked text"
    ))
  func selectionStillCovers() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try update(editor) {
      try addMarkID("c1", select(8, 15))
      #expect(try getSelection()?.getTextContent() == "grew 4%")
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > $getMarkIDsAtSelection"
    ))
  func markIDsAtSelection() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    try update(editor) {
      #expect(try getMarkIDsAtSelection(select(3)) == [])
      #expect(try getMarkIDsAtSelection(select(10)) == ["c1"])
      #expect(try getMarkIDsAtSelection(select(13)) == ["c1", "c2"])
      #expect(try getMarkIDsAtSelection(select(17)) == ["c2"])
      // A caret right before a marked run (end of "Revenue ") is in it
      #expect(try getMarkIDsAtSelection(select(8)) == ["c1"])
      // Defaults to the current selection
      try select(13)
      #expect(try getMarkIDsAtSelection() == ["c1", "c2"])
      #expect(try getMarkIDsAtSelection(select(10, 17)) == ["c1", "c2"])
      #expect(try getMarkIDsAtSelection(select(10, 17), .all) == [])
      #expect(try getMarkIDsAtSelection(select(12, 15), .all) == ["c1", "c2"])
      // Leaves the selection only touches at an edge do not count
      #expect(try getMarkIDsAtSelection(select(0, 8)) == [])
      #expect(try getMarkIDsAtSelection(select(19, 22)) == [])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > the helpers return new arrays that the caller may change"
    ))
  func helpersReturnNewArrays() throws {
    // Swift arrays are values, so a caller's change never reaches the
    // stored ids; the port checks that the stored ids are unchanged.
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    try update(editor) {
      let overlap = try #require(
        try getRoot().getAllTextNodes().first { try $0.getTextContent() == " 4%" })
      let results = [
        try getNodeMarkIDs(overlap),
        try getMarkIDsAtSelection(select(13)),
        try getMarkIDsAtSelection(select(10, 17)),
        // One leaf: the result is not the stored value
        try getMarkIDsAtSelection(select(12, 15), .all),
      ]
      for var ids in results {
        #expect(ids == ["c1", "c2"])
        ids.append("x")
        ids.reverse()
      }
      #expect(try getNodeMarkIDs(overlap) == ["c1", "c2"])
      #expect(try getMarkIDsAtSelection(select(12, 15), .all) == ["c1", "c2"])
    }
    #expect(try registryText(editor) == ["c1": [" 4%", "grew"], "c2": [" 4%", " in "]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id merges the text back"
    ))
  func removingMergesBack() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    try update(editor) { try removeMarkID("c1") }
    #expect(try tree(editor) == ##"paragraph("Revenue grew" " 4% in "[c2] "Q3.")"##)
    #expect(try registryText(editor) == ["c2": [" 4% in "]])
    try update(editor) { try removeMarkID("c2") }
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3.")"##)
    #expect(try markIDNodeKeys(editor).size == 0)
    // No NodeState is left in the JSON
    #expect(!JSONValue.object(try editor.getEditorState().toJSON()).stringify().contains("\"$\""))
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id from part of its range"
    ))
  func removingFromPart() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 19, "c1")
    try update(editor) { try removeMarkID("c1", select(12, 15)) }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew"[c1] " 4%" " in "[c1] "Q3.")"##)
    #expect(try registryText(editor) == ["c1": [" in ", "grew"]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > removing an id without the active registry walks the document"
    ))
  func removingWithoutRegistry() throws {
    let editor = try createMarkEditor(MarkEditorOptions(disabled: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try mark(editor, 12, 19, "c2")
    try update(editor) { try removeMarkID("c1") }
    #expect(try tree(editor) == ##"paragraph("Revenue grew" " 4% in "[c2] "Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > add and remove in the same update"
    ))
  func addAndRemoveSameUpdate() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try update(editor) {
      try addMarkID("c1", select(8, 15))
      try removeMarkID("c1")
    }
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3.")"##)
    #expect(try markIDNodeKeys(editor).size == 0)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > add, remove and query > line breaks, inline decorators and links"
    ))
  func lineBreaksDecoratorsLinks() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("ab"), createLineBreakNode(), createTextNode("cd"), ChipNode(),
          createTextNode("ef")))
    }
    try update(editor) {
      let children = try paragraph().getChildren()
      let selection = createRangeSelection()
      try selection.anchor.set(children[0].getKey(), 1, .text)
      try selection.focus.set(children[4].getKey(), 1, .text)
      try addMarkID("c1", selection)
    }
    #expect(
      try tree(editor)
        == ##"paragraph("a" "b"[c1] br[c1] "cd"[c1] test-chip[c1] "e"[c1] "f")"##)

    let linked = try createMarkEditor()
    try update(linked) { try initDocument() }
    try update(linked) {
      try select(8, 15)
      try toggleLink("https://example.com")
    }
    try mark(linked, 8, 19, "c1")
    #expect(try tree(linked) == ##"paragraph("Revenue " link("grew 4%"[c1]) " in "[c1] "Q3.")"##)
    // A comment on part of a link marks only that part
    try mark(linked, 0, 10, "c2")
    #expect(
      try tree(linked)
        == ##"paragraph("Revenue "[c2] link("gr"[c1,c2] "ew 4%"[c1]) " in "[c1] "Q3.")"##)
    try update(linked) {
      try select(8, 15)
      try toggleLink(nil)
    }
    #expect(
      try tree(linked) == ##"paragraph("Revenue "[c2] "gr"[c1,c2] "ew 4% in "[c1] "Q3.")"##)
  }
}

@Suite struct MarkStateStructuralEditTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > Enter splits a marked run and Backspace joins it again"
    ))
  func enterSplitsBackspaceJoins() throws {
    let editor = try setupMarked()
    try update(editor) {
      try select(12)
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(
      try tree(editor) == ##"paragraph("Revenue " "grew"[c1]) / paragraph(" 4%"[c1] " in Q3.")"##)
    #expect(try registryText(editor) == ["c1": [" 4%", "grew"]])
    try update(editor) {
      _ = try paragraph(1).selectStart()
      _ = try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##)
    #expect(try registryText(editor) == ["c1": ["grew 4%"]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > Enter at the edge of a run keeps it whole"
    ))
  func enterAtEdge() throws {
    let editor = try setupMarked()
    try update(editor) {
      try select(15)
      _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1]) / paragraph(" in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > deleting across the boundaries of a run"
    ))
  func deletingAcrossBoundaries() throws {
    let editor = try setupMarked()
    // Delete "ue gr" across the start of the run
    try update(editor) { try select(5, 10).removeText() }
    #expect(try tree(editor) == ##"paragraph("Reven" "ew 4%"[c1] " in Q3.")"##)
    // Delete the last character of the run
    try update(editor) { try select(9, 10).removeText() }
    #expect(try tree(editor) == ##"paragraph("Reven" "ew 4"[c1] " in Q3.")"##)
    // Deleting the whole run removes the id
    try update(editor) { try select(4, 11).removeText() }
    #expect(try tree(editor) == ##"paragraph("Reven Q3.")"##)
    #expect(try markIDNodeKeys(editor).size == 0)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > structural edits > marked and unmarked text do not merge, equal ids do"
    ))
  func equalIDsMerge() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("aabbcc") }
    try mark(editor, 0, 2, "x")
    try mark(editor, 2, 4, "y")
    #expect(try tree(editor) == ##"paragraph("aa"[x] "bb"[y] "cc")"##)
    try mark(editor, 4, 6, "y")
    #expect(try tree(editor) == ##"paragraph("aa"[x] "bbcc"[y])"##)
  }
}

@Suite struct MarkStateTypingAtEdgesTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed at the end of a run is not marked"
    ))
  func typedAtEnd() throws {
    let editor = try setupMarked()
    try update(editor) { try select(15) }
    try typeText(editor, "X")
    try typeText(editor, "Y")
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "XY in Q3.")"##)
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try String(selection.anchor.getNode().getTextContent().prefix(2)) == "XY")
      #expect(selection.anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed at the start of a run is not marked"
    ))
  func typedAtStart() throws {
    let editor = try setupMarked()
    // The caret at the start of the marked node (not the end of "Revenue ")
    try update(editor) {
      _ = try (paragraph().getChildren()[1] as! TextNode).select(0, 0)
    }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue X" "grew 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a run at the start of a paragraph"
    ))
  func runAtParagraphStart() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 0, 7, "c1")
    try update(editor) { _ = try paragraph().selectStart() }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("X" "Revenue"[c1] " grew 4% in Q3.")"##)
    // and at the end of a paragraph
    try mark(editor, 20, 23, "c2")
    try update(editor) { _ = try paragraph().selectEnd() }
    try typeText(editor, "Y")
    #expect(
      try tree(editor) == ##"paragraph("X" "Revenue"[c1] " grew 4% in " "Q3."[c2] "Y")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > the class that replaces TextNode is split like TextNode, like MarkNode"
    ))
  func replacementClassSplits() throws {
    // With or without withKlass
    for withKlass in [true, false] {
      for withMarkNode in [false, true] {
        let editor = try buildEditorFromExtensions(
          defineExtension(
            name: "[root]",
            dependencies: [RichTextExtension, MarkExtension]
              + (withMarkNode ? [] : [MarkStateExtension]),
            nodes: {
              [
                .klass(ReplacementTextNode.self),
                .replacement(
                  LexicalNodeReplacement(
                    replace: TextNode.self,
                    with: { node in
                      try create(ReplacementTextNode.self).setTextContent(
                        (node as! TextNode).getTextContent())
                    },
                    withKlass: withKlass ? ReplacementTextNode.self : nil)),
              ]
            }))
        try update(editor) { try initDocument("abcdef") }
        try update(editor) { try markSelection(select(0, 3), withMarkNode) }
        try update(editor) { try select(3) }
        try typeText(editor, "X")
        try update(editor) { _ = try paragraph().getAllTextNodes()[0].select(0, 0) }
        try typeText(editor, "Y")
        // The core does not merge TextNodes of another type
        #expect(
          try tree(editor)
            == (withMarkNode
              ? ##"paragraph("Y" mark[c1]("abc") "X" "def")"##
              : ##"paragraph("Y" "abc"[c1] "X" "def")"##))
        #expect(
          try editor.read {
            try paragraph().getAllTextNodes().allSatisfy { $0 is ReplacementTextNode }
          })
      }
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text inserted after a marked hashtag is not marked, like MarkNode"
    ))
  func insertedAfterMarkedHashtag() throws {
    // The hashtag ends where the inserted text starts, and its transform
    // splits that text off with the ids of the hashtag: those are not the
    // inserted text's own, however it was inserted
    let cases: [(String, String, String, String)] = [
      (
        "typed", " ", ##"paragraph("see " "#foo"[c1] "  bar")"##,
        ##"paragraph("see " mark[c1]("#foo") "  bar")"##
      ),
      (
        "pasted", " tail", ##"paragraph("see " "#foo"[c1] " tail bar")"##,
        ##"paragraph("see " mark[c1]("#foo") " tail bar")"##
      ),
      (
        "paste event", ", x", ##"paragraph("see " "#foo"[c1] ", x bar")"##,
        ##"paragraph("see " mark[c1]("#foo") ", x bar")"##
      ),
      (
        "code", ", x #bar y", ##"paragraph("see " "#foo"[c1] ", x " "#bar" " y bar")"##,
        ##"paragraph("see " mark[c1]("#foo") ", x " "#bar" " y bar")"##
      ),
    ]
    for (how, text, expected, expectedWithMarkNode) in cases {
      for withMarkNode in [false, true] {
        let editor = try editorWith(withMarkNode)
        if how == "paste event" {
          try editor.setRootElement(TestRootElement())
        }
        try update(editor) { try initDocument("see #foo bar") }
        try update(editor) { try markSelection(select(4, 8), withMarkNode) }
        try update(editor) { try select(8) }
        switch how {
        case "typed":
          try typeText(editor, text)
        case "pasted", "paste event":
          _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(text))
        default:
          try update(editor) { try select(8).insertText(text) }
        }
        #expect(
          [how, try tree(editor)] == [how, withMarkNode ? expectedWithMarkNode : expected])
      }
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a hashtag inserted at the edge of a marked run is not marked, like MarkNode"
    ))
  func hashtagInsertedAtEdge() throws {
    // The inserted text lands in the marked node, and the hashtag transform
    // splits the hashtag off with a copy of that node's ids (which the
    // hashtag keeps): those are not the inserted text's own
    let cases: [(String, Int, String, String, String)] = [
      (
        "pasted after", 3, " #tag", ##"paragraph("see"[c1] " " "#tag" " more")"##,
        ##"paragraph(mark[c1]("see") " " "#tag" " more")"##
      ),
      (
        "code after", 3, " #tag", ##"paragraph("see"[c1] " " "#tag" " more")"##,
        ##"paragraph(mark[c1]("see") " " "#tag" " more")"##
      ),
      (
        "code before", 0, "#tag ",
        // As on main: the space lands in the text with "see"
        ##"paragraph("#tag" " see"[c1] " more")"##,
        ##"paragraph("#tag" " " mark[c1]("see") " more")"##
      ),
    ]
    for (how, at, text, expected, expectedWithMarkNode) in cases {
      for withMarkNode in [false, true] {
        let editor = try editorWith(withMarkNode)
        try update(editor) { try initDocument("see more") }
        try update(editor) { try markSelection(select(0, 3), withMarkNode) }
        try update(editor) { try select(at) }
        if how.hasPrefix("pasted") {
          _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(text))
        } else {
          try update(editor) { try select(at).insertText(text) }
        }
        #expect(
          [how, "\(withMarkNode)", try tree(editor)]
            == [how, "\(withMarkNode)", withMarkNode ? expectedWithMarkNode : expected])
      }
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasted Lexical data keeps its ids after a marked hashtag"
    ))
  func pastedLexicalDataAfterHashtag() throws {
    // Pasted or inserted content with the same ids as the hashtag is not
    // taken for text split off it: its ids are a value of its own
    for how in ["paste", "code"] {
      let editor = try editorWith(false)
      try update(editor) { try initDocument("see #foo bar") }
      try update(editor) { try addMarkID("c1", select(4, 12)) }
      var json: SerializedSelectedNodes? = nil
      try update(editor) {
        json = try generateJSONFromSelectedNodes(editor, select(8, 12))
        try removeMarkID("c1", select(8, 12))
      }
      try update(editor) { try select(8) }
      if how == "paste" {
        _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(" bar", clipboardJSON(json!)))
      } else {
        try update(editor) {
          try insertGeneratedNodes(
            editor, generateNodesFromSerializedNodes(json!.nodes), getSelection()!)
        }
      }
      #expect(
        [how, try tree(editor)] == [how, ##"paragraph("see " "#foo"[c1] " bar"[c1] " bar")"##])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a plain TextNode given the ids array of a marked hashtag is taken for a piece split off it"
    ))
  func copiedStateTakenForSplitPiece() throws {
    // The documented limitation: NodeState copied without a round trip
    // through a string shares the array. Ids added with addMarkID are a
    // value of the node's own.
    for how in ["copied state", "$addMarkID"] {
      let editor = try editorWith(false)
      try update(editor) { try initDocument("see #foo bar") }
      try mark(editor, 4, 8, "c1")
      try update(editor) {
        let hashtag = try paragraph().getAllTextNodes()[1]
        let node = try createTextNode(" more")
        if how == "copied state" {
          var json = try node.exportJSON()
          json.set("$", try hashtag.exportJSON()["$"])
          _ = try node.updateFromJSON(json)
        }
        _ = try hashtag.select(4, 4)
        try insertNodes([node])
        if how == "$addMarkID" {
          let selection = createRangeSelection()
          try selection.anchor.set(node.getKey(), 0, .text)
          try selection.focus.set(node.getKey(), 5, .text)
          try addMarkID("c1", selection)
          _ = try node.select(5, 5)
        }
      }
      #expect(
        [how, try tree(editor)]
          == [
            how,
            how == "copied state"
              ? ##"paragraph("see " "#foo"[c1] " more bar")"##
              : ##"paragraph("see " "#foo"[c1] " more"[c1] " bar")"##,
          ])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text dropped from another editor after a marked hashtag keeps its ids, like MarkNode"
    ))
  func droppedFromAnotherEditor() throws {
    // The drop point is resolved by the root, as the mocked
    // `caretRangeFromPoint` resolves it to the end of the hashtag's text
    for withMarkNode in [false, true] {
      let editor = try editorWith(withMarkNode)
      try editor.setRootElement(
        CaretRoot {
          let hashtag = try paragraph().getAllTextNodes().first {
            try $0.getTextContent() == "#foo"
          }!
          return try getTextPointCaret(hashtag, .next, 4)
        })
      // " more"[c1] in another editor, as Lexical data
      try update(editor) { try initDocument(" more") }
      try update(editor) { try markSelection(select(0, 5), withMarkNode) }
      var json = ""
      try update(editor) {
        json = clipboardJSON(try generateJSONFromSelectedNodes(editor, select(0, 5)))
      }
      try update(editor) { try initDocument("see #foo bar") }
      try update(editor) { try markSelection(select(4, 8), withMarkNode) }
      // Dropped at the end of the hashtag: a drop runs no paste command
      let event = DragEvent(
        "drop",
        dataTransfer: dataTransfer([
          // Dragged from an editor that is not on the page
          ("application/x-lexical-drag", #"{"editorKey":"another-editor"}"#),
          ("application/x-lexical-editor", json),
          ("text/plain", " more"),
        ]), clientX: 1, clientY: 1)
      #expect(try editor.dispatchCommand(DROP_COMMAND, event))
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph("see " mark[c1]("#foo") mark[c1](" more") " bar")"##
            : ##"paragraph("see " "#foo"[c1] " more"[c1] " bar")"##))
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text that code inserts after a hashtag and marks in the same update keeps its ids"
    ))
  func codeInsertsAndMarks() throws {
    // Marked together with the hashtag, like MarkNode
    for withMarkNode in [false, true] {
      let editor = try editorWith(withMarkNode)
      try update(editor) { try initDocument("see #foo bar") }
      try update(editor) {
        try select(8)
        try insertNodes([createTextNode(" Q")])
        try markSelection(select(4, 10), withMarkNode)
        try select(10)
      }
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph("see " mark[c1]("#foo" " Q") " bar")"##
            : ##"paragraph("see " "#foo"[c1] " Q"[c1] " bar")"##))
    }
    // A run of nodes inserted after a marked hashtag, each marked by code
    let editor = try editorWith(false)
    try update(editor) { try initDocument("see #foo") }
    try mark(editor, 4, 8, "c1")
    try update(editor) {
      let nodes: [TextNode] = [
        try createTextNode(" "), try createHashtagNode("#bar"), try createTextNode(" baz"),
      ]
      try select(8)
      try insertNodes(nodes)
      for node in nodes {
        let selection = createRangeSelection()
        try selection.anchor.set(node.getKey(), 0, .text)
        try selection.focus.set(node.getKey(), node.getTextContentSize(), .text)
        try addMarkID("c1", selection)
      }
    }
    #expect(
      try tree(editor) == ##"paragraph("see " "#foo"[c1] " "[c1] "#bar"[c1] " baz"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasted text whose HTML is the same text is not marked after a marked hashtag, like MarkNode"
    ))
  func pastedSameHTML() throws {
    // The HTML import skips such HTML (iOS sends it with autocorrect), so
    // the text goes into the hashtag as plain text. The `beforeinput` of
    // an autocorrection reaches CONTROLLED_TEXT_INSERTION_COMMAND through
    // BEFORE_INPUT_COMMAND in Swift, whose payload is the text.
    for how in ["paste", "beforeinput"] {
      for withMarkNode in [false, true] {
        let editor = try editorWith(withMarkNode)
        try update(editor) { try initDocument("see #foo bar") }
        try update(editor) { try markSelection(select(4, 8), withMarkNode) }
        try update(editor) { try select(8) }
        if how == "paste" {
          _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(", x", nil, ", x"))
        } else {
          let event = InputEvent(
            inputType: "insertReplacementText",
            dataTransfer: dataTransfer([("text/html", ", x"), ("text/plain", ", x")]))
          _ = try editor.dispatchCommand(BEFORE_INPUT_COMMAND, event)
        }
        #expect(
          [how, try tree(editor)]
            == [
              how,
              withMarkNode
                ? ##"paragraph("see " mark[c1]("#foo") ", x bar")"##
                : ##"paragraph("see " "#foo"[c1] ", x bar")"##,
            ])
      }
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed inside a run is marked"
    ))
  func typedInsideRun() throws {
    let editor = try setupMarked()
    try update(editor) { try select(11) }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue " "greXw 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text replacing a selection inside a run is marked"
    ))
  func replacingSelectionInsideRun() throws {
    let editor = try setupMarked()
    try update(editor) { try select(9, 11) }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue " "gXw 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > ids shared with the neighbor continue across the edge"
    ))
  func sharedIDsContinue() throws {
    let editor = try setupMarked()
    try mark(editor, 12, 19, "c2")
    // Caret at the end of "grew"[c1], before " 4%"[c1,c2]
    try update(editor) { _ = try (paragraph().getChildren()[1] as! TextNode).select(4, 4) }
    try typeText(editor, "X")
    #expect(
      try tree(editor) == ##"paragraph("Revenue " "grewX"[c1] " 4%"[c1,c2] " in "[c2] "Q3.")"##)
    // Caret at the end of " 4%"[c1,c2], before " in "[c2]: c1 ends here
    try update(editor) { _ = try (paragraph().getChildren()[2] as! TextNode).select(3, 3) }
    try typeText(editor, "Y")
    #expect(
      try tree(editor)
        == ##"paragraph("Revenue " "grewX"[c1] " 4%"[c1,c2] "Y in "[c2] "Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text inserted natively into the DOM text"
    ))
  func insertedNatively() throws {
    // What the native beforeinput path ends up doing: the text of the
    // marked node changes, with the caret after it
    let editor = try setupMarked()
    try update(editor) { try select(15) }
    try update(editor) {
      let marked = try paragraph().getChildren()[1] as! TextNode
      _ = try marked.setTextContent("grew 4%X")
      _ = try marked.select(8, 8)
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > composition at the end of a run"
    ))
  func compositionAtEnd() throws {
    let editor = try setupMarked()
    try update(editor) { try select(15) }
    try update(editor) { try dispatchCompositionStart(editor) }
    #expect(editor.isComposing())
    // The IME updates the text of the composing node
    for composed in ["に", "日本"] {
      try update(editor) {
        let marked = try paragraph().getChildren()[1] as! TextNode
        #expect(try marked.isComposing())
        _ = try marked.setTextContent("grew 4%" + composed)
        let end = 7 + composed.utf16.count
        _ = try marked.select(end, end)
      }
      // While composing the text stays in the node the IME writes to
      #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%\##(composed)"[c1] " in Q3.")"##)
    }
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "日本 in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > composition inside a run"
    ))
  func compositionInside() throws {
    let editor = try setupMarked()
    try update(editor) { try select(11) }
    try update(editor) { try dispatchCompositionStart(editor) }
    try update(editor) {
      let marked = try paragraph().getChildren()[1] as! TextNode
      _ = try marked.setTextContent("gre日w 4%")
      _ = try marked.select(4, 4)
    }
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == ##"paragraph("Revenue " "gre日w 4%"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > replayed and historic updates are left alone"
    ))
  func replayedAndHistoric() throws {
    for tag in [COLLABORATION_TAG, HISTORIC_TAG] {
      let editor = try setupMarked()
      try update(editor) { try select(15) }
      try update(editor) {
        try addUpdateTag(tag)
        let marked = try paragraph().getChildren()[1] as! TextNode
        _ = try marked.setTextContent("grew 4%X")
        _ = try marked.select(8, 8)
      }
      #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > a caret that was not at the edge does not split"
    ))
  func caretNotAtEdge() throws {
    let editor = try setupMarked()
    try update(editor) { try select(3) }
    // e.g. a programmatic append while the caret is elsewhere
    try update(editor) {
      _ = try (paragraph().getChildren()[1] as! TextNode).setTextContent("grew 4%X")
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > pasting at the end of a run"
    ))
  func pastingAtEnd() throws {
    let editor = try setupMarked()
    try update(editor) {
      try select(15)
      try insertNodes([createTextNode("pasted")])
    }
    #expect(try editor.read { try nodeMarkIDsOfText("pasted") } == [])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text typed with a different format inside a run is marked"
    ))
  func typedWithFormatInside() throws {
    let editor = try setupMarked()
    try update(editor) {
      try select(11)
      (try getSelection() as? RangeSelection)?.toggleFormat("bold")
    }
    try typeText(editor, "X")
    #expect(
      try tree(editor) == ##"paragraph("Revenue " "gre"[c1] "X"[c1] "w 4%"[c1] " in Q3.")"##)
    #expect(try editor.read { try getRoot().getAllTextNodes()[2].hasFormat("bold") })
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > text pasted inside a run is marked"
    ))
  func pastedInsideRun() throws {
    let editor = try setupMarked()
    try update(editor) {
      try select(11)
      try insertNodes([createTextNode("pasted").toggleFormat("italic")])
    }
    #expect(try editor.read { try nodeMarkIDsOfText("pasted") } == ["c1"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > undo restores the marks and the output"
    ))
  func undoRestores() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]",
        dependencies: [
          RichTextExtension, configExtension(HistoryExtension) { $0.delay = 0 },
          MarkStateExtension,
        ]))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    #expect(try registryText(editor) == ["c1": ["grew 4%"]])
    try update(editor) { try select(15) }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] "X in Q3.")"##)
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%"[c1] " in Q3.")"##)
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try tree(editor) == ##"paragraph("Revenue grew 4% in Q3.")"##)
    #expect(try markIDNodeKeys(editor).size == 0)
    _ = try editor.dispatchCommand(REDO_COMMAND)
    #expect(try registryText(editor) == ["c1": ["grew 4%"]])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > typing at the edges of a mark > disabled: typed text joins the run"
    ))
  func disabledJoins() throws {
    let editor = try createMarkEditor(MarkEditorOptions(disabled: true))
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try update(editor) { try select(15) }
    try typeText(editor, "X")
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%X"[c1] " in Q3.")"##)
    #expect(try markIDNodeKeys(editor).size == 0)
  }
}
