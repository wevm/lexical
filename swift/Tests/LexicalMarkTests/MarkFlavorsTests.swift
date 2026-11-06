import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalHistory
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-mark/src/__tests__/unit/MarkFlavors.test.ts.
//
// The DOM of a leaf is what the editor's render config makes of it
// (`renderDOM`), as in MarkStateTestSupport.swift. Of the "types" tests,
// the ones Swift's types can express are checked at compile time; the
// string literal rules are runtime checks in Swift (see parity/skips.json).

private let CommentMarks = try! defineMarkExtension(
  name: "@test/CommentMarks", stateKey: "commentIds", className: "comment",
  overlapClassName: "comment-overlap")

private let SuggestionMarks = try! defineMarkExtension(
  name: "@test/SuggestionMarks", stateKey: "suggestionIds", className: "suggestion")

private let TEXT = "Ship the migration on Friday"

private func createFlavorEditor(
  _ dependencies: [any AnyLexicalExtensionArgument] = [CommentMarks, SuggestionMarks],
  withRoot: Bool = false
) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[root]",
      dependencies: [
        RichTextExtension, CoreImportExtension,
        configExtension(HistoryExtension) { $0.delay = 0 },
      ] + dependencies,
      editorConfig: InitialEditorConfig(
        theme: ["mark": .string("mark"), "markOverlap": .string("mark-overlap")],
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
  if withRoot {
    try editor.setRootElement(TestRootElement())
  }
  return editor
}

private func initText(_ text: String = TEXT) throws {
  try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
}

/// A selection of [start, end) of the paragraph's text, made the editor's
/// selection inside an update.
@discardableResult
private func sel(_ start: Int, _ end: Int? = nil) throws -> RangeSelection {
  let end = end ?? start
  let texts = try getRoot().getAllTextNodes()
  func point(_ offset: Int, _ preferNext: Bool) throws -> (TextNode, Int) {
    var acc = 0
    for (i, text) in texts.enumerated() {
      let size = try text.getTextContentSize()
      if offset < acc + size || (offset == acc + size && (!preferNext || i == texts.count - 1)) {
        return (text, offset - acc)
      }
      acc += size
    }
    throw LexicalError("offset \(offset) out of range")
  }
  let (a, ao) = try point(start, true)
  let (b, bo) = try point(end, false)
  let selection = createRangeSelection()
  try selection.anchor.set(a.getKey(), ao, .text)
  try selection.focus.set(b.getKey(), bo, .text)
  if !isCurrentlyReadOnlyMode() {
    try setSelection(selection)
  }
  return selection
}

/// The text runs with their ids per flavor: `"Friday"[c:c1 s:s7]`.
private func runs(
  _ editor: Editor, _ flavors: [MarkFlavorExtension] = [CommentMarks, SuggestionMarks]
) throws -> String {
  try editor.read {
    try getRoot().getAllTextNodes().map { node in
      let ids = try flavors.compactMap { flavor -> String? in
        let flavorIDs = try getNodeMarkIDs(flavor, node)
        return flavorIDs.isEmpty ? nil : "\(flavor.stateKey):\(flavorIDs.joined(separator: ","))"
      }.joined(separator: " ")
      return quoted(try node.getTextContent()) + (ids.isEmpty ? "" : "[\(ids)]")
    }.joined(separator: " ")
  }
}

private func textNode(_ text: String) throws -> TextNode {
  guard let node = try getRoot().getAllTextNodes().first(where: { try $0.getTextContent() == text })
  else {
    throw LexicalError("no TextNode \(quoted(text))")
  }
  return node
}

private func classes(_ element: DOMElement) -> String {
  element.className.split(separator: " ").map(String.init).sorted().joined(separator: " ")
}

private func dom(_ editor: Editor, _ text: String) throws -> DOMElement {
  try editor.read { try renderDOM(editor, textNode(text)) }
}

private func typeIn(_ editor: Editor, _ text: String) throws {
  _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, text)
}

private func expectError(_ message: String, contains: Bool = false, _ body: () throws -> Void) {
  do {
    try body()
    Issue.record("expected an error: \(message)")
  } catch {
    let description = String(describing: error)
    #expect(contains ? description.contains(message) : description == message)
  }
}

@Suite struct MarkFlavorsTwoFlavorsTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > each flavor sees only its own ids"
    ))
  func eachFlavorSeesOwnIDs() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    // "migration on Friday" commented, "Friday" with a suggestion
    try update(editor) { try addMarkID(CommentMarks, "c1", sel(9, 28)) }
    try update(editor) { try addMarkID(SuggestionMarks, "s7", sel(22, 28)) }
    #expect(
      try runs(editor)
        == ##""Ship the " "migration on "[commentIds:c1] "Friday"[commentIds:c1 suggestionIds:s7]"##
    )
    try editor.read {
      let friday = try textNode("Friday")
      #expect(try getNodeMarkIDs(CommentMarks, friday) == ["c1"])
      #expect(try getNodeMarkIDs(SuggestionMarks, friday) == ["s7"])
      #expect(try getMarkIDsAtSelection(CommentMarks, sel(10, 24)) == ["c1"])
      #expect(try getMarkIDsAtSelection(SuggestionMarks, sel(10, 24), .any) == ["s7"])
      #expect(try getMarkIDsAtSelection(SuggestionMarks, sel(10, 24), .all) == [])
      #expect(try getMarkIDsByFlavor(friday) == ["commentIds": ["c1"], "suggestionIds": ["s7"]])
    }
    #expect(try registryText(editor, CommentMarks) == ["c1": ["Friday", "migration on "]])
    #expect(try registryText(editor, SuggestionMarks) == ["s7": ["Friday"]])

    // Removing everywhere touches only its own flavor; equal neighbors merge
    try update(editor) { try removeMarkID(SuggestionMarks, "s7") }
    #expect(try runs(editor) == ##""Ship the " "migration on Friday"[commentIds:c1]"##)
    #expect(try registryText(editor, SuggestionMarks) == [:])
    // The same id in two flavors is two different marks
    try update(editor) { try addMarkID(SuggestionMarks, "c1", sel(0, 4)) }
    try update(editor) { try removeMarkID(CommentMarks, "c1") }
    #expect(try runs(editor) == ##""Ship"[suggestionIds:c1] " the migration on Friday""##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > each flavor renders its own classes and attribute"
    ))
  func eachFlavorRendersOwnClasses() throws {
    let editor = try createFlavorEditor(
      [CommentMarks, SuggestionMarks, MarkStateExtension], withRoot: true)
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(9, 28))
      try addMarkID(CommentMarks, "c2", sel(22, 28))
      try addMarkID(SuggestionMarks, "s7", sel(22, 28))
    }
    try update(editor) { try addMarkID("m1", sel(0, 4)) }
    let friday = try dom(editor, "Friday")
    #expect(classes(friday) == "comment comment-overlap suggestion")
    #expect(friday.getAttribute("data-lexical-comment-ids") == "c1 c2")
    #expect(friday.getAttribute("data-lexical-suggestion-ids") == "s7")
    #expect(!friday.hasAttribute(MARK_STATE_IDS_ATTRIBUTE))
    let migration = try dom(editor, "migration on ")
    #expect(classes(migration) == "comment")
    #expect(!migration.hasAttribute("data-lexical-suggestion-ids"))
    // The default flavor keeps the theme classes
    let ship = try dom(editor, "Ship")
    #expect(classes(ship) == "mark")
    #expect(ship.getAttribute(MARK_STATE_IDS_ATTRIBUTE) == "m1")

    try update(editor) { try removeMarkID(CommentMarks, "c2") }
    #expect(classes(try dom(editor, "Friday")) == "comment suggestion")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > flavors that share a class do not remove it from each other"
    ))
  func sharedClass() throws {
    let comments = try defineMarkExtension(
      name: "@test/SharedComments", stateKey: "commentIds", className: "annotation comment")
    let suggestions = try defineMarkExtension(
      name: "@test/SharedSuggestions", stateKey: "suggestionIds", className: "annotation mark")
    let editor = try createFlavorEditor(
      [MarkStateExtension, comments, suggestions], withRoot: true)
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(comments, "c1", sel(0, 4))
      try addMarkID("m1", sel(5, 8))
    }
    // Suggestions has no id on either leaf, and keeps its hands off
    // `annotation` (Comments) and `mark` (MarkStateExtension's theme class)
    #expect(classes(try dom(editor, "Ship")) == "annotation comment")
    #expect(classes(try dom(editor, "the")) == "mark")
    try update(editor) { try removeMarkID(comments, "c1") }
    #expect(classes(try dom(editor, "Ship ")) == "")
  }

  struct TypingCase: Sendable, CustomTestStringConvertible {
    let label: String
    let at: Int
    let expected: String
    var testDescription: String { label }
    static let all = [
      TypingCase(
        label: "at the end of both marks", at: 28,
        expected: ##""Ship the migration on " "Friday"[commentIds:c1 suggestionIds:s7] "!""##),
      TypingCase(
        label: "inside both marks", at: 25,
        expected: ##""Ship the migration on " "Fri!day"[commentIds:c1 suggestionIds:s7]"##),
      TypingCase(
        label: "at the start of both marks", at: 22,
        expected: ##""Ship the migration on !" "Friday"[commentIds:c1 suggestionIds:s7]"##),
    ]
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing at the end of both marks gives the text the ids of both flavors by the same rule"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing inside both marks gives the text the ids of both flavors by the same rule"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing at the start of both marks gives the text the ids of both flavors by the same rule"
    ),
    arguments: TypingCase.all)
  func typing(_ c: TypingCase) throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(22, 28))
      try addMarkID(SuggestionMarks, "s7", sel(22, 28))
    }
    try update(editor) {
      try sel(c.at)
      try typeIn(editor, "!")
    }
    #expect(try runs(editor) == c.expected)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > text typed at the edge of one flavor joins the other where it is inside"
    ))
  func edgeOfOneFlavor() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(9, 28))
      try addMarkID(SuggestionMarks, "s7", sel(22, 28))
    }
    try update(editor) {
      try sel(22)
      try typeIn(editor, "next ")
    }
    #expect(
      try runs(editor)
        == ##""Ship the " "migration on next "[commentIds:c1] "Friday"[commentIds:c1 suggestionIds:s7]"##
    )
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > text replacing a selection gets the ids all of it had, per flavor"
    ))
  func replacingPerFlavor() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(22, 28))
      try addMarkID(SuggestionMarks, "s7", sel(9, 28))
    }
    try update(editor) {
      try sel(22, 28)
      try typeIn(editor, "Monday")
    }
    #expect(
      try runs(editor)
        == ##""Ship the " "migration on "[suggestionIds:s7] "Monday"[commentIds:c1 suggestionIds:s7]"##
    )
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > pasted text keeps the ids of every flavor"
    ))
  func pastedKeepsEveryFlavor() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(22, 28))
      try addMarkID(SuggestionMarks, "s7", sel(25, 28))
    }
    try update(editor) {
      let json = try generateJSONFromSelectedNodes(editor, sel(22, 28))
      try sel(0)
      guard let selection = try getSelection() as? RangeSelection else {
        throw LexicalError("no selection")
      }
      try insertGeneratedNodes(editor, generateNodesFromSerializedNodes(json.nodes), selection)
    }
    #expect(
      try runs(editor)
        == ##""Fri"[commentIds:c1] "day"[commentIds:c1 suggestionIds:s7] "Ship the migration on " "Fri"[commentIds:c1] "day"[commentIds:c1 suggestionIds:s7]"##
    )
  }
}

/// Records the changes a flavor's listener reports.
private final class Recorder {
  var calls: [String] = []
  var unregister: () -> Void = {}

  init(_ editor: Editor, _ flavor: MarkFlavorExtension) throws {
    unregister = try registerMarkIDsListener(editor, flavor) { [unowned self] change in
      func list(_ set: Set<String>) -> String { jsSort(Array(set)).joined(separator: ",") }
      let tags = [HISTORIC_TAG, COLLABORATION_TAG].filter { change.tags.contains($0) }
      self.calls.append(
        "+[\(list(change.added))] -[\(list(change.removed))] =[\(list(change.ids))]"
          + (tags.isEmpty ? "" : " \(tags.joined(separator: ","))"))
    }
  }
}

@Suite struct MarkFlavorsListenerTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > reports added, removed (orphaned) and restored ids across undo and paste"
    ))
  func reportsAcrossUndoAndPaste() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
    let comments = try Recorder(editor, CommentMarks)
    let suggestions = try Recorder(editor, SuggestionMarks)
    #expect(comments.calls == [])

    try update(editor) { try addMarkID(CommentMarks, "c1", sel(22, 28)) }
    try update(editor) { try addMarkID(CommentMarks, "c2", sel(0, 4)) }
    #expect(comments.calls == ["+[c1] -[] =[c1]", "+[c2] -[] =[c1,c2]"])

    // Typing inside a mark changes no ids: nothing to report
    try update(editor) {
      try sel(25)
      try typeIn(editor, "x")
    }
    #expect(comments.calls.count == 2)

    // Copy the commented text, then delete all of it: c1 is orphaned
    var copied: SerializedSelectedNodes? = nil
    try update(editor) { copied = try generateJSONFromSelectedNodes(editor, sel(22, 29)) }
    try update(editor) { try sel(21, 29).removeText() }
    #expect(comments.calls.last == "+[] -[c1] =[c2]")

    // Undo brings it back
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    // The undo commits at the next microtask; a read commits it now
    try editor.read {}
    #expect(comments.calls.last == "+[c1] -[] =[c1,c2] \(HISTORIC_TAG)")
    #expect(try runs(editor).contains(##""Frixday"[commentIds:c1]"##))

    // Deleted again, then pasted back
    try update(editor) { try sel(21, 29).removeText() }
    #expect(comments.calls.last == "+[] -[c1] =[c2]")
    try update(editor) {
      let selection = try sel(21)
      try insertGeneratedNodes(editor, generateNodesFromSerializedNodes(copied!.nodes), selection)
    }
    #expect(comments.calls.last == "+[c1] -[] =[c1,c2]")

    // The other flavor's listener saw nothing
    #expect(suggestions.calls == [])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a collaborator update is tagged"
    ))
  func collaboratorTagged() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    let recorder = try Recorder(editor, CommentMarks)
    try update(editor) {
      try addUpdateTag(COLLABORATION_TAG)
      try addMarkID(CommentMarks, "c1", sel(0, 4))
    }
    #expect(recorder.calls == ["+[c1] -[] =[c1] \(COLLABORATION_TAG)"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > the first call lists the ids already in the document, and unregistering stops it"
    ))
  func firstCallAndUnregister() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(0, 4))
      try addMarkID(CommentMarks, "c2", sel(5, 8))
    }
    let recorder = try Recorder(editor, CommentMarks)
    #expect(recorder.calls == ["+[c1,c2] -[] =[c1,c2]"])
    recorder.unregister()
    try update(editor) { try removeMarkID(CommentMarks, "c1") }
    #expect(recorder.calls.count == 1)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > disabling reports nothing, enabling reports the net change"
    ))
  func disablingReportsNetChange() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) { try addMarkID(CommentMarks, "c1", sel(0, 4)) }
    let recorder = try Recorder(editor, CommentMarks)
    let disabled = try getExtensionDependencyFromEditor(editor, CommentMarks).output.disabled
    disabled.value = true
    try update(editor) {
      try removeMarkID(CommentMarks, "c1")
      try addMarkID(CommentMarks, "c3", sel(5, 8))
    }
    #expect(recorder.calls == ["+[c1] -[] =[c1]"])
    disabled.value = false
    #expect(recorder.calls == ["+[c1] -[] =[c1]", "+[c3] -[c1] =[c3]"])
    // Disabled, and back with no net change: nothing
    disabled.value = true
    disabled.value = false
    #expect(recorder.calls.count == 2)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a listener registered while disabled gets every id when enabled"
    ))
  func registeredWhileDisabled() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) { try addMarkID(CommentMarks, "c1", sel(0, 4)) }
    let early = try Recorder(editor, CommentMarks)
    let disabled = try getExtensionDependencyFromEditor(editor, CommentMarks).output.disabled
    disabled.value = true
    let late = try Recorder(editor, CommentMarks)
    #expect(late.calls == [])
    disabled.value = false
    #expect(late.calls == ["+[c1] -[] =[c1]"])
    // The listener registered before has nothing new to hear
    #expect(early.calls == ["+[c1] -[] =[c1]"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a listener that throws on its first call is not registered"
    ))
  func throwingListenerNotRegistered() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) { try addMarkID(CommentMarks, "c1", sel(0, 4)) }
    var calls = 0
    expectError("listener failed") {
      _ = try registerMarkIDsListener(editor, CommentMarks) { _ in
        calls += 1
        throw LexicalError("listener failed")
      }
    }
    try update(editor) { try removeMarkID(CommentMarks, "c1") }
    #expect(calls == 1)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a flavor disabled by another flavor listener in the same commit reports nothing"
    ))
  func disabledByAnotherListener() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    let output = try getExtensionDependencyFromEditor(editor, SuggestionMarks).output
    _ = try registerMarkIDsListener(editor, CommentMarks) { _ in output.disabled.value = true }
    let suggestions = try Recorder(editor, SuggestionMarks)
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(0, 4))
      try addMarkID(SuggestionMarks, "s1", sel(0, 4))
    }
    #expect(output.disabled.value)
    #expect(suggestions.calls == [])
    #expect(output.markIDNodeKeys.value.size == 0)
    output.disabled.value = false
    #expect(suggestions.calls == ["+[s1] -[] =[s1]"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > an editor without the flavor throws, naming the fix"
    ))
  func editorWithoutFlavorThrows() throws {
    let editor = try createFlavorEditor([CommentMarks])
    expectError(
      "registerMarkIDsListener: the editor was not built with @test/SuggestionMarks. Add it to the dependencies of the extension the editor is built from."
    ) {
      _ = try registerMarkIDsListener(editor, SuggestionMarks) { _ in }
    }
  }
}

@Suite struct MarkFlavorsIsMarkIDShownTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > isMarkIDShown per flavor > a flavor filter hides only that flavor"
    ))
  func flavorFilterHidesOnlyThatFlavor() throws {
    var resolved: Set<String> = ["c1"]
    let editor = try createFlavorEditor(
      [
        configExtension(CommentMarks) { $0.isMarkIDShown = { !resolved.contains($0) } },
        SuggestionMarks,
      ], withRoot: true)
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(22, 28))
      try addMarkID(SuggestionMarks, "c1", sel(22, 28))
    }
    #expect(classes(try dom(editor, "Friday")) == "suggestion")
    #expect(!(try dom(editor, "Friday")).hasAttribute("data-lexical-comment-ids"))
    #expect(try dom(editor, "Friday").getAttribute("data-lexical-suggestion-ids") == "c1")
    // The ids stay in the document
    #expect(try runs(editor).contains(##""Friday"[commentIds:c1 suggestionIds:c1]"##))

    // Reopen the thread
    resolved.remove("c1")
    try getExtensionDependencyFromEditor(editor, CommentMarks).output.isMarkIDShown.value = {
      !resolved.contains($0)
    }
    #expect(classes(try dom(editor, "Friday")) == "comment suggestion")
    #expect(try dom(editor, "Friday").getAttribute("data-lexical-comment-ids") == "c1")
  }
}

@Suite struct MarkFlavorsDefaultFlavorTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > MarkStateExtension is the flavor on markIds"
    ))
  func markStateIsDefaultFlavor() {
    #expect(MarkStateExtension.stateKey == MARK_STATE_KEY)
    #expect(MarkStateExtension.attribute == MARK_STATE_IDS_ATTRIBUTE)
    #expect(MarkStateExtension.name == "@lexical/mark/MarkState")
    #expect(CommentMarks.attribute == "data-lexical-comment-ids")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > the functions without a flavor act on MarkStateExtension, beside other flavors"
    ))
  func functionsWithoutFlavor() throws {
    let editor = try createFlavorEditor([MarkStateExtension, CommentMarks])
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID("m1", sel(0, 4))
      try addMarkID(MarkStateExtension, "m2", sel(0, 4))
      try addMarkID(CommentMarks, "c1", sel(0, 4))
    }
    try editor.read {
      let ship = try textNode("Ship")
      #expect(try getNodeMarkIDs(ship) == ["m1", "m2"])
      #expect(try getNodeMarkIDs(MarkStateExtension, ship) == ["m1", "m2"])
      #expect(try getMarkIDsAtSelection(sel(1)) == ["m1", "m2"])
      #expect(try getMarkIDsAtSelection(CommentMarks, sel(1)) == ["c1"])
      let state = try #require(try ship.exportJSON()["$"]?.objectValue)
      #expect(state.keys.count == 2)
      #expect(state[MARK_STATE_KEY] == .array([.string("m1"), .string("m2")]))
      #expect(state["commentIds"] == .array([.string("c1")]))
    }
    try update(editor) { try removeMarkID("m1") }
    #expect(
      try runs(editor, [MarkStateExtension, CommentMarks])
        == ##""Ship"[markIds:m2 commentIds:c1] " the migration on Friday""##)
    let flavors = try getExtensionDependencyFromEditor(editor, MarkBaseExtension).output.flavors
    #expect(flavors.keysArray == ["markIds", "commentIds"])
    #expect(flavors.get("commentIds") === CommentMarks)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > $migrateMarkNodesToMarkState can target a flavor"
    ))
  func migrateToFlavor() throws {
    let editor = try createFlavorEditor([MarkExtension, CommentMarks])
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("Ship the "), createMarkNode(["t1"]).append(createTextNode("migration"))))
    }
    var count = 0
    try update(editor) { count = try migrateMarkNodesToMarkState(CommentMarks) }
    #expect(count == 1)
    #expect(
      try runs(editor, [MarkStateExtension, CommentMarks])
        == ##""Ship the " "migration"[commentIds:t1]"##)
  }
}

@Suite struct MarkFlavorsSerializationTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > JSON round trips the ids of every flavor"
    ))
  func jsonRoundTrip() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(9, 28))
      try addMarkID(SuggestionMarks, "s7", sel(22, 28))
    }
    let json = try editor.getEditorState().toJSON()
    let friday = json["root"]?.objectValue?["children"]?.arrayValue?.first?.objectValue?[
      "children"]?.arrayValue?[2].objectValue
    let state = try #require(friday?["$"]?.objectValue)
    #expect(state.keys.count == 2)
    #expect(state["commentIds"] == .array([.string("c1")]))
    #expect(state["suggestionIds"] == .array([.string("s7")]))

    let target = try createFlavorEditor()
    try target.setEditorState(target.parseEditorState(JSONValue.object(json).stringify()))
    #expect(try runs(target) == runs(editor))
    #expect(try registryText(target, CommentMarks) == registryText(editor, CommentMarks))
    #expect(try stateJSON(target) == JSONValue.object(json).stringify())
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > ids of a flavor the editor lacks are kept as unknown state"
    ))
  func unknownFlavorKept() throws {
    let editor = try createFlavorEditor()
    try update(editor) { try initText() }
    try update(editor) { try addMarkID(SuggestionMarks, "s7", sel(22, 28)) }
    let json = try stateJSON(editor)
    let target = try createFlavorEditor([CommentMarks])
    try target.setEditorState(target.parseEditorState(json))
    #expect(try stateJSON(target) == json)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > HTML round trips each flavor through its own attribute"
    ))
  func htmlRoundTrip() throws {
    let editor = try createFlavorEditor([
      configExtension(CommentMarks) { $0.exportIDsToHTML = true }, SuggestionMarks,
    ])
    try update(editor) { try initText() }
    try update(editor) {
      try addMarkID(CommentMarks, "c1", sel(22, 28))
      try addMarkID(SuggestionMarks, "s7", sel(22, 28))
    }
    let html = try editor.read { try generateHtmlFromNodes(editor, nil) }
    #expect(html.contains(#"data-lexical-comment-ids="c1""#))
    // SuggestionMarks does not export its ids
    #expect(!html.contains("s7"))

    let target = try createFlavorEditor()
    try importHTML(
      target,
      html.replacingFirst(
        #"data-lexical-comment-ids="c1""#,
        #"data-lexical-comment-ids="c1" data-lexical-suggestion-ids="s9""#))
    #expect(
      try runs(target) == ##""Ship the migration on " "Friday"[commentIds:c1 suggestionIds:s9]"##)
  }
}

@Suite struct MarkFlavorsErrorsTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > two flavors with the same stateKey fail when the editor is built"
    ))
  func sameStateKey() throws {
    let notes = try defineMarkExtension(name: "@test/Notes", stateKey: "commentIds")
    expectError(
      #"MarkBaseExtension: @test/CommentMarks and @test/Notes both use stateKey "commentIds". Give each flavor its own stateKey."#
    ) { _ = try createFlavorEditor([CommentMarks, notes]) }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > two flavors with the same attribute fail when the editor is built"
    ))
  func sameAttribute() throws {
    let notes = try defineMarkExtension(
      name: "@test/Notes", stateKey: "noteIds", attribute: "data-lexical-comment-ids")
    expectError(
      "MarkBaseExtension: @test/CommentMarks and @test/Notes both render the attribute data-lexical-comment-ids. Give each flavor its own attribute."
    ) { _ = try createFlavorEditor([CommentMarks, notes]) }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > attributes that differ only in case clash"
    ))
  func attributesDifferingInCase() throws {
    let notes = try defineMarkExtension(
      name: "@test/Notes", stateKey: "noteIds", attribute: "DATA-LEXICAL-COMMENT-IDS")
    #expect(notes.attribute == "data-lexical-comment-ids")
    expectError("both render the attribute data-lexical-comment-ids", contains: true) {
      _ = try createFlavorEditor([CommentMarks, notes])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey \"\" is rejected with the fix in the message"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey \"a b\" is rejected with the fix in the message"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey \"markIds\" is rejected with the fix in the message"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey \"__proto__\" is rejected with the fix in the message"
    ),
    arguments: ["", "a b", MARK_STATE_KEY, "__proto__"])
  func invalidStateKey(_ stateKey: String) {
    expectError(
      "defineMarkExtension: invalid stateKey \(quoted(stateKey)) for @test/Bad. Use a non-empty name without whitespace, other than \"markIds\" (the key of MarkStateExtension) and \"__proto__\", such as \"commentIds\"."
    ) { _ = try defineMarkExtension(name: "@test/Bad", stateKey: stateKey) }
  }
}

@Suite struct MarkFlavorsTypesTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > types > the definition types the flavor"
    ))
  func definitionTypesTheFlavor() {
    // Swift has no string literal types: a flavor is a MarkFlavorExtension
    // whose stateKey and name are its values
    let flavor: MarkFlavorExtension = CommentMarks
    #expect(flavor.stateKey == "commentIds")
    #expect(flavor.name == "@test/CommentMarks")
    #expect(MarkStateExtension.stateKey == "markIds")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > types > the original signatures stay the last overloads"
    ))
  func originalSignatures() {
    // The functions without a flavor keep the signatures they had before
    // flavors (checked when this compiles)
    let add: (String, any BaseSelection) throws -> [LexicalNode] = addMarkID
    let remove: (String, (any BaseSelection)?) throws -> Void = removeMarkID
    let atSelection: ((any BaseSelection)?, MarkIDsMode) throws -> [String] =
      getMarkIDsAtSelection
    let nodeIDs: (LexicalNode) throws -> [String] = getNodeMarkIDs
    let migrate: (LexicalNode?) throws -> Int = migrateMarkNodesToMarkState
    _ = (add, remove, atSelection, nodeIDs, migrate)
  }
}
