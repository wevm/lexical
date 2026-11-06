import Lexical
import LexicalExtension
import LexicalHashtag
import LexicalHistory
import LexicalMark
import LexicalMarkdown
import LexicalTesting
import LexicalText
import Testing

// Port of
// packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts.
// "a hashtag inside a MarkNode stays inside it when it becomes text" needs
// @lexical/mark, which is not ported yet.

nonisolated(unsafe) private let getHashtagMatch = HashtagConfig().getHashtagMatch

// MARK: - The test fixtures

// Ad-hoc NodeState, as an app keeps comment ids or block ids on text.
nonisolated(unsafe) private let note = createState("note", parse: { $0?.stringValue ?? "" })

/// A reference-typed value: the carried value is the same object (JS
/// arrays are objects; Swift arrays are values).
private final class IDList {
  let values: [String]
  init(_ values: [String]) { self.values = values }
}

nonisolated(unsafe) private let ids = createState(
  "ids",
  parse: { IDList($0?.arrayValue?.compactMap { $0.stringValue } ?? []) },
  unparse: { .array($0.values.map { .string($0) }) },
  isEqual: { $0.values == $1.values })

// A keyword-style entity (normal mode) and a token entity (mention-like).
private final class KeywordNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(KeywordNode.self, type: "test-keyword", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private func createKeywordNode(_ text: String) throws -> KeywordNode {
  try applyNodeReplacement(KeywordNode(text))
}

private let keywordRegex = JSRegExp("(^|[^A-Za-z])(cat)(?![A-Za-z])")

private func getKeywordMatch(_ text: String) -> EntityMatch? {
  guard let m = keywordRegex.exec(text) else { return nil }
  let start = m.index + (m[1]?.utf16.count ?? 0)
  return EntityMatch(start: start, end: start + 3)
}

private final class MentionNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(MentionNode.self, type: "test-mention", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private func createMentionNode(_ text: String) throws -> MentionNode {
  try applyNodeReplacement(MentionNode(text)).setMode("token")
}

private let getMentionMatch = regexMatch("@[a-z]+")

// An entity whose own $config declares a state (nested and flat).
nonisolated(unsafe) private let entityOwn = createState(
  "entityOwn", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let entityFlat = createState(
  "entityFlat", parse: { $0?.stringValue ?? "" })

private final class StatefulEntityNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      StatefulEntityNode.self, type: "test-stateful-entity", extends: TextNode.self,
      stateConfigs: [
        RequiredNodeStateConfig(entityFlat, flat: true), RequiredNodeStateConfig(entityOwn),
      ])
  }
  override func isTextEntity() -> Bool { true }
}

private let getHashMatch = regexMatch(#"#\w+"#)

/// `createEditor({nodes, onError})` with a root element.
private func createMountedEditor(_ nodes: [LexicalNodeConfig]) throws -> Editor {
  let editor = try createEditor(CreateEditorArgs(nodes: nodes, onError: { throw $0 }))
  try editor.setRootElement(TestRootElement())
  return editor
}

private func createHashtagEditor(_ nodes: [LexicalNodeConfig] = []) throws -> Editor {
  let editor = try createMountedEditor([.klass(HashtagNode.self)] + nodes)
  _ = try registerLexicalTextEntity(editor, getHashtagMatch, HashtagNode.self) {
    try createHashtagNode($0.getTextContent())
  }
  return editor
}

/// [type, text, note] of every text node.
private func read(_ editor: Editor) throws -> [[String]] {
  try editor.read { () throws in
    try getRoot().getAllTextNodes().map {
      [$0.getType(), try $0.getTextContent(), try getState($0, note)]
    }
  }
}

private func update(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(discrete: true) {
    try fn()
    try setSelection(nil)
  }
}

private func paragraph(_ children: LexicalNode...) throws -> ParagraphNode {
  try createParagraphNode().append(contentsOf: children)
}

private func text(_ i: Int) throws -> TextNode { try getRoot().getAllTextNodes()[i] }

/// The entries of `object`, compared without regard to key order, as
/// `toEqual` compares.
private func unordered(_ object: JSONObject?) -> [String: JSONValue]? {
  object.map { Dictionary(uniqueKeysWithValues: $0.map { ($0.key, $0.value) }) }
}

@Suite struct RegisterLexicalTextEntityNodeStateTests {
  // MARK: registerLexicalTextEntity keeps NodeState

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > #9703: a hashtag that stops matching keeps its NodeState when it becomes text (typing before it)"
    ))
  func typingBeforeHashtag() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("one "), setState(createHashtagNode("#tag"), note, "c1"),
          createTextNode(" more")))
    }
    try update(editor) { try text(1).select(0, 0).insertText("x") }
    #expect(
      try read(editor) == [["text", "one x", ""], ["text", "#tag", "c1"], ["text", " more", ""]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > deleting the # of a hashtag keeps its NodeState (reverse transform, no match)"
    ))
  func deletingHash() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("one "), setState(createHashtagNode("#tag"), note, "c1")))
    }
    try update(editor) { try text(1).select(0, 1).removeText() }
    #expect(try read(editor) == [["text", "one ", ""], ["text", "tag", "c1"]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag that becomes text merges into neighbors with the same NodeState"
    ))
  func mergesWithSameNodeState() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          setState(createTextNode("one "), note, "c1"),
          setState(createHashtagNode("#tag"), note, "c1"),
          setState(createTextNode(" more"), note, "c1")))
    }
    try update(editor) { try text(1).select(0, 0).insertText("x") }
    #expect(try read(editor) == [["text", "one x#tag more", "c1"]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag with no NodeState becomes text and merges as before"
    ))
  func noNodeStateMerges() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("one "), createHashtagNode("#tag"), createTextNode(" more")))
    }
    try update(editor) { try text(1).select(0, 0).insertText("x") }
    #expect(try read(editor) == [["text", "one x#tag more", ""]])
    // No NodeState was created on the merged text
    try editor.getEditorState().read { () throws in
      #expect(try text(0).exportJSON()[NODE_STATE_KEY] == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > plain text with NodeState that becomes a hashtag keeps it"
    ))
  func plainTextBecomesHashtag() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(paragraph(setState(createTextNode("a #"), note, "c1")))
    }
    try update(editor) {
      // The "#" was there: the hashtag is not only inserted text
      try text(0).select(3, 3).insertText("tag")
    }
    #expect(try read(editor) == [["text", "a ", "c1"], ["hashtag", "#tag", "c1"]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag made only of text inserted into a node does not take its NodeState"
    ))
  func insertedHashtagDoesNotTakeNodeState() throws {
    // The inserted characters land in the node and get a copy of its
    // NodeState, which is not theirs (as on main; MarkStateExtension does not
    // add text inserted at a comment's edge to the comment either)
    let cases: [(Int, String, [[String]])] = [
      (1, " #tag", [["text", "a ", "c1"], ["hashtag", "#tag", ""]]),
      (0, "#tag ", [["hashtag", "#tag", ""], ["text", " a", "c1"]]),
    ]
    for (at, inserted, expected) in cases {
      let editor = try createHashtagEditor()
      try update(editor) {
        try getRoot().append(paragraph(setState(createTextNode("a"), note, "c1")))
      }
      try update(editor) { try text(0).select(at, at).insertText(inserted) }
      #expect(try read(editor) == expected)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > state that createNode gives the entity wins over the text it replaces"
    ))
  func createNodeStateWins() throws {
    let editor = try createMountedEditor([.klass(HashtagNode.self)])
    _ = try registerLexicalTextEntity(editor, getHashtagMatch, HashtagNode.self) {
      try setState(createHashtagNode($0.getTextContent()), note, "entity")
    }
    try update(editor) {
      try getRoot().append(
        paragraph(setState(setState(createTextNode("a #tag"), note, "text"), ids, IDList(["c1"]))))
    }
    try editor.getEditorState().read { () throws in
      let nodes = try getRoot().getAllTextNodes()
      #expect(nodes.count == 2)
      let text = nodes[0]
      let hashtag = nodes[1]
      #expect(try getState(text, note) == "text")
      #expect(hashtag.getType() == "hashtag")
      #expect(try getState(hashtag, note) == "entity")
      #expect(try getState(hashtag, ids).values == ["c1"])
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > text inserted after a hashtag that becomes a hashtag does not take the NodeState copied from it"
    ))
  func textInsertedAfterHashtag() throws {
    // The hashtag's transform splits the inserted text off with a copy of
    // its NodeState (as before); a hashtag made from that text does not
    // take that copy, which is not the inserted text's own.
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("see "), setState(createHashtagNode("#foo"), note, "c1")))
    }
    try update(editor) { try text(1).select(4, 4).insertText(", x #bar y") }
    let rows = try read(editor)
    #expect(rows[1] == ["hashtag", "#foo", "c1"])
    #expect(rows.first { $0[1] == "#bar" } == ["hashtag", "#bar", ""])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a reference-typed state value is carried as the same object"
    ))
  func referenceValueCarried() throws {
    let editor = try createHashtagEditor()
    let value = IDList(["c1", "c2"])
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("one "), setState(createHashtagNode("#tag"), ids, value)))
    }
    try update(editor) { try text(1).select(0, 1).removeText() }
    try editor.getEditorState().read { () throws in
      let node = try text(1)
      #expect(node.getType() == "text")
      #expect(try getState(node, ids) === value)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > NodeState read from JSON (not parsed yet) is carried and exported"
    ))
  func unparsedNodeStateCarried() throws {
    let editor = try createHashtagEditor()
    try editor.setEditorState(
      editor.parseEditorState(
        #"""
        {"root":{"children":[{"children":[
        {"detail":0,"format":0,"mode":"normal","style":"","text":"one ","type":"text","version":1},
        {"$":{"comment":["c1"],"note":"n1"},"detail":0,"format":0,"mode":"normal","style":"",
        "text":"#tag","type":"hashtag","version":1}],
        "direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph",
        "version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}
        """#))
    try update(editor) { try text(1).select(0, 0).insertText("x") }
    try editor.getEditorState().read { () throws in
      let json = try text(1).exportJSON()
      #expect(json["type"] == "text")
      #expect(json["text"] == "#tag")
      #expect(
        unordered(json[NODE_STATE_KEY]?.objectValue)
          == ["comment": .array([.string("c1")]), "note": .string("n1")])
      #expect(try getState(text(1), note) == "n1")
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > NodeState declared in the entity $config (nested and flat) is carried to the text"
    ))
  func declaredNodeStateCarried() throws {
    let editor = try createMountedEditor([.klass(StatefulEntityNode.self)])
    _ = try registerLexicalTextEntity(editor, getHashMatch, StatefulEntityNode.self) {
      try applyNodeReplacement(StatefulEntityNode($0.getTextContent()))
    }
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("one "),
          setState(
            setState(setState(StatefulEntityNode("#tag"), entityOwn, "own"), entityFlat, "flat"),
            note, "c1")))
    }
    try update(editor) { try text(1).select(0, 1).removeText() }
    try editor.getEditorState().read { () throws in
      let node = try text(1)
      #expect(node.getType() == "text")
      #expect(try getState(node, note) == "c1")
      #expect(try getState(node, entityOwn) == "own")
      #expect(try getState(node, entityFlat) == "flat")
      // TextNode does not declare `entityFlat` flat: it serializes nested
      let state = try node.exportJSON()[NODE_STATE_KEY]?.objectValue
      #expect(state?["entityFlat"] == "flat")
      #expect(state?["entityOwn"] == "own")
      #expect(state?["note"] == "c1")
    }
    // Exported and imported again, the text keeps it
    let json = try editor.getEditorState().toJSON()
    try editor.setEditorState(editor.parseEditorState(json))
    try editor.getEditorState().read { () throws in
      let node = try text(1)
      #expect(try getState(node, entityOwn) == "own")
      #expect(try getState(node, entityFlat) == "flat")
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a key the plain text type declares with another config is parsed with that config"
    ))
  func otherConfigForKey() throws {
    try OtherConfigForKey.run()
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag inside a MarkNode stays inside it when it becomes text"
    ))
  func hashtagInMarkStaysInIt() throws {
    let editor = try createHashtagEditor([.klass(MarkNode.self)])
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("one "),
          createMarkNode(["c1"]).append(setState(createHashtagNode("#tag"), note, "n1")),
          createTextNode(" more")))
    }
    try update(editor) {
      let hashtag = try getRoot().getAllTextNodes()[1]
      try hashtag.select(0, 1).removeText()
    }
    try editor.getEditorState().read {
      let node = try getRoot().getAllTextNodes()[1]
      #expect(node.getType() == "text")
      #expect(try node.getTextContent() == "tag")
      #expect(try getState(node, note) == "n1")
      let mark = try #require(try node.getParent() as? MarkNode)
      #expect(try mark.getIDs() == ["c1"])
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > undo and redo of a demotion restore the entity and the text with their NodeState"
    ))
  func undoRedoDemotion() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test", dependencies: [HashtagExtension, HistoryExtension],
        editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
    try editor.setRootElement(TestRootElement())
    try editor.update(tag: HISTORY_PUSH_TAG, discrete: true) {
      try getRoot().clear().append(
        paragraph(createTextNode("one "), setState(createHashtagNode("#tag"), note, "c1")))
      try text(0).select(0, 0)
    }
    let before = try read(editor)
    try editor.update(tag: HISTORY_PUSH_TAG, discrete: true) {
      try text(1).select(0, 0).insertText("x")
    }
    let after = try read(editor)
    #expect(after == [["text", "one x", ""], ["text", "#tag", "c1"]])
    _ = try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try read(editor) == before)
    _ = try editor.dispatchCommand(REDO_COMMAND)
    #expect(try read(editor) == after)
    editor.dispose()
  }

  // MARK: every path that turns an entity into text keeps NodeState

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: the entity before it stops matching with its text (prevMatch null)"
    ))
  func entityBeforeStopsMatching() throws {
    // A keyword followed by a letter in another node: "cat" + "s" is no
    // keyword, so the text transform of "s" turns the keyword into text.
    let editor = try createMountedEditor([.klass(KeywordNode.self)])
    _ = try registerLexicalTextEntity(editor, getKeywordMatch, KeywordNode.self) {
      try createKeywordNode($0.getTextContent())
    }
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("a "), setState(createKeywordNode("cat"), note, "k1"),
          createTextNode(" b")))
    }
    try update(editor) { try text(2).select(0, 0).insertText("s") }
    #expect(
      try read(editor) == [["text", "a ", ""], ["text", "cat", "k1"], ["text", "s b", ""]])
  }

  // MARK: the text transform: a token entity (mention-like) that text follows

  private func createMentionEditor() throws -> Editor {
    let editor = try createMountedEditor([.klass(MentionNode.self)])
    _ = try registerLexicalTextEntity(editor, getMentionMatch, MentionNode.self) {
      try createMentionNode($0.getTextContent())
    }
    return editor
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with the same NodeState as the text: it keeps it and takes the text"
    ))
  func tokenSameNodeState() throws {
    let editor = try createMentionEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("hi "), setState(createMentionNode("@bob"), note, "m1"),
          setState(createTextNode("x"), note, "m1")))
    }
    // Before this change, this looped forever: the entity became text
    // without its NodeState, which did not merge with "x".
    #expect(try read(editor) == [["text", "hi ", ""], ["test-mention", "@bobx", "m1"]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with other NodeState than the text: kept with its NodeState, the text stays apart"
    ))
  func tokenOtherNodeState() throws {
    let editor = try createMentionEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("hi "), setState(createMentionNode("@bob"), note, "m1"),
          createTextNode("x")))
    }
    // It becomes text with its NodeState, which does not merge with "x",
    // and is made again; the third time it is kept, and "x" (which has
    // none) stays text rather than take "m1". Before, it became text
    // without its NodeState, merged and was made again without it.
    #expect(
      try read(editor) == [
        ["text", "hi ", ""], ["test-mention", "@bob", "m1"], ["text", "x", ""],
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > typed text after it: the selection stays after the typed text"
    ))
  func tokenTypedTextSelection() throws {
    let editor = try createMentionEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("hi "), setState(createMentionNode("@bob"), note, "m1"),
          createTextNode("!")))
    }
    try editor.update(discrete: true) {
      // Paste "x yz" right after the mention
      try text(1).select(4, 4).insertText("x yz")
    }
    #expect(
      try read(editor) == [
        ["text", "hi ", ""], ["test-mention", "@bob", "m1"], ["text", "x yz!", ""],
      ])
    try editor.read { () throws in
      let selection = try getSelection()
      #expect(isRangeSelection(selection))
      let range = try #require(selection as? RangeSelection)
      let anchor = range.anchor
      #expect(try anchor.getNode().getTextContent() == "x yz!")
      #expect(anchor.offset == 4)
      #expect(range.focus.is(anchor))
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with other NodeState than text that its match extends into, which keeps it apart: kept, each keeps its own"
    ))
  func tokenOtherNodeStateInMatch() throws {
    // Text with NodeState of its own never merges with it (before this
    // change, this looped forever). The third time it is kept, and the
    // text stays apart with its own NodeState.
    let editor = try createMentionEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("hi "), setState(createMentionNode("@bob"), note, "m1"),
          setState(createTextNode("x"), note, "x1")))
    }
    #expect(
      try read(editor) == [
        ["text", "hi ", ""], ["test-mention", "@bob", "m1"], ["text", "x", "x1"],
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with a value object distinct from the text after it, equal as JSON: each keeps its own"
    ))
  func tokenDistinctValueObjects() throws {
    // Two value objects are two values unless their `isEqual` says
    // otherwise, as when text nodes merge: the token does not take "x".
    final class Tag {
      let id: String
      init(id: String) { self.id = id }
    }
    let tag = createState(
      "tag",
      parse: { (v: JSONValue?) -> Tag in
        Tag(id: v?.objectValue?["id"]?.stringValue ?? "")
      },
      unparse: { .object(JSONObject([("id", .string($0.id))])) })
    let a = Tag(id: "c1")
    let b = Tag(id: "c1")
    let editor = try createMentionEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("hi "), setState(createMentionNode("@bob"), tag, a),
          setState(createTextNode("x"), tag, b)))
    }
    try editor.read { () throws in
      let nodes = try getRoot().getAllTextNodes()
      #expect(
        try nodes.map { [$0.getType(), try $0.getTextContent()] } == [
          ["text", "hi "], ["test-mention", "@bob"], ["text", "x"],
        ])
      #expect(try getState(nodes[1], tag) === a)
      #expect(try getState(nodes[2], tag) === b)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: the entity after it stops matching (nextMatch null)"
    ))
  func entityAfterStopsMatching() throws {
    // "x" + "#tag": "x#tag" holds no hashtag, so the text transform of "x"
    // turns the hashtag after it into text.
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("x").setFormat("bold"), setState(createHashtagNode("#tag"), note, "c1")))
    }
    try update(editor) {
      // Mark both dirty
      try text(0).markDirty()
      try text(1).markDirty()
    }
    let rows = try read(editor)
    // On main's rules the hashtag either stays or becomes text; either way
    // its characters keep their NodeState.
    #expect(rows.first { $0[1] == "#tag" }?[2] == "c1")
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the reverse transform: adjacent entities both become text"
    ))
  func adjacentEntities() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          setState(createHashtagNode("#a"), note, "c1"),
          setState(createHashtagNode("#b"), note, "c2")))
    }
    // Both become text, then the first is a hashtag again: each keeps its
    // NodeState (before, both lost it)
    #expect(try read(editor) == [["hashtag", "#a", "c1"], ["text", "#b", "c2"]])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the reverse transform: an entity followed by an entity"
    ))
  func entityFollowedByEntity() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("one "), setState(createHashtagNode("#a"), note, "c1")))
    }
    try update(editor) {
      try text(1).insertAfter(setState(createHashtagNode("#b"), note, "c2"))
    }
    let rows = try read(editor)
    #expect(rows.first { $0[1] == "#a" }?[2] == "c1")
    #expect(rows.first { $0[1] == "#b" }?[2] == "c2")
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > joining two blocks across text of another format keeps the NodeState of every node"
    ))
  func joiningBlocks() throws {
    let editor = try createHashtagEditor()
    try update(editor) {
      try getRoot().append(
        paragraph(
          createTextNode("one "), setState(createTextNode("word").setFormat("italic"), note, "w")),
        paragraph(setState(createHashtagNode("#tag"), note, "c1"), createTextNode(" more")))
    }
    try update(editor) {
      let last = try #require(
        (getRoot().getFirstChildOrThrow() as? ElementNode)?.getLastDescendant() as? TextNode)
      let first = try #require(
        (getRoot().getLastChildOrThrow() as? ElementNode)?.getFirstDescendant() as? TextNode)
      let s = createRangeSelection()
      try s.anchor.set(last.getKey(), last.getTextContentSize(), .text)
      try s.focus.set(first.getKey(), 0, .text)
      try setSelection(s)
      try s.removeText()
    }
    let rows = try read(editor)
    #expect(rows.first { $0[1] == "word" }?[2] == "w")
    #expect(rows.first { $0[1] == "#tag" }?[2] == "c1")
    try editor.getEditorState().read { () throws in
      #expect(try getRoot().getTextContent() == "one word#tag more")
      #expect(try getRoot().getAllTextNodes().allSatisfy { isTextNode($0) })
    }
  }
}

/// The fixtures of "a key the plain text type declares with another config
/// is parsed with that config": the entity and the text use different
/// StateConfigs for the key "tag".
private enum OtherConfigForKey {
  nonisolated(unsafe) static let entityTag = createState("tag", parse: { $0?.stringValue ?? "" })
  nonisolated(unsafe) static let textTag = createState(
    "tag",
    parse: { (v: JSONValue?) -> String in v?.stringValue.map { "text:\($0)" } ?? "" },
    unparse: { .string($0.hasPrefix("text:") ? String($0.dropFirst(5)) : $0) })

  final class TagEntityNode: TextNode {
    override class func config() -> NodeConfig? {
      NodeConfig(
        TagEntityNode.self, type: "test-tag-entity", extends: TextNode.self,
        stateConfigs: [RequiredNodeStateConfig(entityTag)])
    }
    override func isTextEntity() -> Bool { true }
  }

  final class TagTextNode: TextNode {
    override class func config() -> NodeConfig? {
      NodeConfig(
        TagTextNode.self, type: "test-tag-text", extends: TextNode.self,
        stateConfigs: [RequiredNodeStateConfig(textTag)])
    }
  }

  static func run() throws {
    let editor = try createMountedEditor([
      .klass(TagEntityNode.self), .klass(TagTextNode.self),
      .replacement(
        LexicalNodeReplacement(
          replace: TextNode.self, with: { try TagTextNode(($0 as! TextNode).__text) },
          withKlass: TagTextNode.self)),
    ])
    _ = try registerLexicalTextEntity(editor, getHashMatch, TagEntityNode.self) {
      try applyNodeReplacement(TagEntityNode($0.getTextContent()))
    }
    try update(editor) {
      try getRoot().append(
        paragraph(createTextNode("one "), setState(TagEntityNode("#tag"), entityTag, "v")))
    }
    try update(editor) {
      try getRoot().getAllTextNodes()[1].select(0, 1).removeText()
    }
    try editor.getEditorState().read { () throws in
      let node = try getRoot().getAllTextNodes()[1]
      #expect(node.getType() == "test-tag-text")
      #expect(try getState(node, textTag) == "text:v")
    }
  }
}
