import Lexical
import LexicalExtension
import LexicalHashtag
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of the "replaced and re-created text" describe of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.
//
// CONTROLLED_TEXT_INSERTION_COMMAND takes the text in Swift. The
// autocorrection that upstream dispatches as an `insertReplacementText`
// InputEvent payload is dispatched as that `beforeinput` event
// (BEFORE_INPUT_COMMAND), which the core turns into the command with the
// event's text, in the same update.

enum InsertionPayload: Sendable {
  case text(String)
  /// The text of an autocorrection, as a beforeinput event carries it.
  case replacement(String)
}

func dispatchInsertion(_ editor: Editor, _ payload: InsertionPayload) throws {
  switch payload {
  case .text(let text):
    _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, text)
  case .replacement(let text):
    let event = InputEvent(
      inputType: "insertReplacementText", dataTransfer: dataTransfer([("text/plain", text)]))
    _ = try editor.dispatchCommand(BEFORE_INPUT_COMMAND, event)
  }
}

func markSelectionWith(
  _ selection: RangeSelection, _ withMarkNode: Bool, _ id: String = "c1"
)
  throws
{
  if withMarkNode {
    try wrapSelectionInMarkNode(selection, false, id)
  } else {
    try addMarkID(id, selection)
  }
}

/// Mark `[markStart, markEnd)` of `text` with c1, as MarkState ids or as a
/// MarkNode, then replace `[start, end)` with `payload`. The selection is
/// committed first, as when the user selects the text, or set in the same
/// update, as a beforeinput event sets it from its target range.
private func replaceText(
  _ withMarkNode: Bool, _ text: String, _ marked: (Int, Int), _ range: (Int, Int),
  _ payload: InsertionPayload, _ sameUpdate: Bool
) throws -> String {
  let editor = try createMarkEditor()
  try update(editor) { try initDocument(text) }
  try update(editor) { try markSelectionWith(select(marked.0, marked.1), withMarkNode) }
  if !sameUpdate {
    try update(editor) { try select(range.0, range.1) }
  }
  try update(editor) {
    if sameUpdate {
      try select(range.0, range.1)
    }
    try dispatchInsertion(editor, payload)
  }
  return try tree(editor)
}

struct ReplaceCase: Sendable, CustomTestStringConvertible {
  let label: String
  let text: String
  let marked: (Int, Int)
  let range: (Int, Int)
  let payload: InsertionPayload
  let state: String
  let markNode: String
  var testDescription: String { label }

  // label, text, marked, replaced, inserted, MarkState, MarkNode
  static let all: [ReplaceCase] = [
    ReplaceCase(
      label: "a word inside a run, by its first letter", text: "x teh y", marked: (0, 7),
      range: (2, 5), payload: .text("t"), state: ##"paragraph("x t y"[c1])"##,
      markNode: ##"paragraph(mark[c1]("x t y"))"##),
    ReplaceCase(
      label: "a word inside a run, by its last letter", text: "x teh y", marked: (0, 7),
      range: (2, 5), payload: .text("h"), state: ##"paragraph("x h y"[c1])"##,
      markNode: ##"paragraph(mark[c1]("x h y"))"##),
    ReplaceCase(
      label: "a word inside a run, by the corrected word", text: "x teh y", marked: (0, 7),
      range: (2, 5), payload: .text("the"), state: ##"paragraph("x the y"[c1])"##,
      markNode: ##"paragraph(mark[c1]("x the y"))"##),
    ReplaceCase(
      label: "a word inside a run, by an autocorrection", text: "x teh y", marked: (0, 7),
      range: (2, 5), payload: .replacement("the"), state: ##"paragraph("x the y"[c1])"##,
      markNode: ##"paragraph(mark[c1]("x the y"))"##),
    // MarkNode loses the mark when all of its text is replaced (the empty
    // MarkNode is removed); the ids are kept, so retyping a commented word
    // keeps the comment
    ReplaceCase(
      label: "a whole run, by its first letter", text: "a grew b", marked: (2, 6),
      range: (2, 6), payload: .text("g"), state: ##"paragraph("a " "g"[c1] " b")"##,
      markNode: ##"paragraph("a g b")"##),
    ReplaceCase(
      label: "a whole run, by an autocorrection", text: "a recieve b", marked: (2, 9),
      range: (2, 9), payload: .replacement("receive"),
      state: ##"paragraph("a " "receive"[c1] " b")"##, markNode: ##"paragraph("a receive b")"##
    ),
    ReplaceCase(
      label: "across the start of a run, by text that starts with the same letter",
      text: "abcde", marked: (2, 5), range: (1, 3), payload: .text("bX"),
      state: ##"paragraph("abX" "de"[c1])"##, markNode: ##"paragraph("abX" mark[c1]("de"))"##),
  ]
}

/// `createEntityEditor(withMarkNode)`: hashtags, and a mentions transform
/// that replaces "@name" by a new token node. With a root element, as
/// deleting a character can read the DOM.
func createEntityEditor(_ withMarkNode: Bool) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[root]",
      dependencies: [RichTextExtension, HashtagExtension, MarkExtension]
        + (withMarkNode ? [] : [MarkStateExtension]),
      nodes: { [.klass(MentionNode.self)] },
      register: { entityEditor, _, _ in
        try entityEditor.registerNodeTransform(TextNode.self) { node in
          guard try node.isSimpleText(), let match = mentionMatch(try node.getTextContent())
          else { return }
          let parts = try node.splitText(match.0, match.0 + match.1)
          let target = parts[match.0 == 0 ? 0 : 1]
          _ = try target.replace(createMentionNode(target.getTextContent()))
        }
      }))
  try editor.setRootElement(TestRootElement())
  return editor
}

/// `/@[a-z]+/.exec(text)`: the UTF-16 index and length of the match.
private func mentionMatch(_ text: String) -> (Int, Int)? {
  let units = Array(text.utf16)
  var i = 0
  while i < units.count {
    if units[i] == 0x40 {
      var j = i + 1
      while j < units.count, (0x61...0x7A).contains(units[j]) {
        j += 1
      }
      if j > i + 1 {
        return (i, j - i)
      }
    }
    i += 1
  }
  return nil
}

func markAll(_ editor: Editor, _ withMarkNode: Bool) throws {
  try update(editor) {
    try markSelectionWith(select(0, paragraph().getTextContentSize()), withMarkNode)
  }
}

@Suite struct MarkStateReplacedTextTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by its first letter"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by its last letter"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by the corrected word"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a word inside a run, by an autocorrection"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a whole run, by its first letter"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing a whole run, by an autocorrection"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > replacing across the start of a run, by text that starts with the same letter"
    ),
    arguments: ReplaceCase.all)
  func replacing(_ c: ReplaceCase) throws {
    for sameUpdate in [false, true] {
      #expect(try replaceText(false, c.text, c.marked, c.range, c.payload, sameUpdate) == c.state)
      #expect(
        try replaceText(true, c.text, c.marked, c.range, c.payload, sameUpdate) == c.markNode)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > text replaced by code that dispatches no command keeps the letters it kept"
    ))
  func replacedByCode() throws {
    // Without the selection the insertion command replaced, the smallest
    // edit tells the kept letters from the inserted ones: "t" and "e" are
    // kept with their ids, "h" is inserted between them
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("x teh y") }
    try mark(editor, 0, 7, "c1")
    try mark(editor, 2, 3, "c2")
    #expect(try tree(editor) == ##"paragraph("x "[c1] "t"[c1,c2] "eh y"[c1])"##)
    try update(editor) { try select(2, 5).insertText("the") }
    #expect(try tree(editor) == ##"paragraph("x "[c1] "t"[c1,c2] "he y"[c1])"##)
    // The same edit through the command replaces the selection: the new
    // text gets the ids all of the replaced text had
    let typed = try createMarkEditor()
    try update(typed) { try initDocument("x teh y") }
    try mark(typed, 0, 7, "c1")
    try mark(typed, 2, 3, "c2")
    try update(typed) { try select(2, 5) }
    try typeText(typed, "the")
    #expect(try tree(typed) == ##"paragraph("x the y"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > the replaced selection is known when the command is dispatched outside an update"
    ))
  func dispatchedOutsideUpdate() throws {
    // Each priority level of a command dispatched outside an update runs in
    // an update of its own, and the DOM paste event dispatches PASTE_COMMAND
    // that way (the Swift root has no DOM events, so "paste event"
    // dispatches it the same way, on an editor with a root)
    let outsideCases: [(String, (Int, Int), (Int, Int), String, String)] = [
      ("abcde", (2, 5), (1, 3), "bX", ##"paragraph("abX" "de"[c1])"##),
      ("abcde", (0, 3), (2, 4), "cX", ##"paragraph("ab"[c1] "cXe")"##),
      ("x teh y", (0, 7), (2, 5), "t", ##"paragraph("x t y"[c1])"##),
      ("a grew b", (2, 6), (2, 6), "g", ##"paragraph("a " "g"[c1] " b")"##),
    ]
    for (text, marked, range, payload, expected) in outsideCases {
      for how in ["command", "paste", "paste event"] {
        let editor = try createMarkEditor(MarkEditorOptions(withRoot: how == "paste event"))
        try update(editor) { try initDocument(text) }
        try mark(editor, marked.0, marked.1, "c1")
        try update(editor) { try select(range.0, range.1) }
        if how == "command" {
          _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, payload)
        } else {
          _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(payload))
        }
        try drainMicrotasks()
        #expect([how, try tree(editor)] == [how, expected])
      }
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > the selection of a command that changed nothing is not used by a later update"
    ))
  func commandThatChangedNothing() throws {
    let editor = try createMarkEditor()
    _ = editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND, { _, _ in true }, COMMAND_PRIORITY_HIGH)
    try update(editor) { try initDocument("abcde") }
    try mark(editor, 2, 5, "c1")
    try update(editor) { try select(1, 3) }
    _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "bX")
    #expect(try tree(editor) == ##"paragraph("ab" "cde"[c1])"##)
    try drainMicrotasks()
    // Code that dispatches no command: the smallest edit
    try update(editor) { try (getSelection() as? RangeSelection)?.insertText("bX") }
    #expect(try tree(editor) == ##"paragraph("ab" "Xde"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an id removed in the same update is not given back"
    ))
  func idRemovedNotGivenBack() throws {
    for range in [false, true] {
      let editor = try createEntityEditor(false)
      try update(editor) { try initDocument("see foo bar") }
      try markAll(editor, false)
      try update(editor) { try select(4) }
      try update(editor) {
        _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "#")
        try removeMarkID("c1", range ? select(4, 8) : nil)
      }
      #expect(
        try tree(editor)
          == (range
            ? ##"paragraph("see "[c1] "#foo" " bar"[c1])"## : ##"paragraph("see " "#foo" " bar")"##)
      )
      #expect(try registryText(editor) == (range ? ["c1": [" bar", "see "]] : [:]))
    }
    // Nor to text that replaces a selection, before or after the removal
    for removeFirst in [false, true] {
      let editor = try createMarkEditor()
      try update(editor) { try initDocument("x teh y") }
      try mark(editor, 0, 7, "c1")
      try update(editor) {
        if removeFirst {
          try removeMarkID("c1")
        }
        try select(2, 5)
        _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "the")
        if !removeFirst {
          try removeMarkID("c1")
        }
      }
      #expect(try tree(editor) == ##"paragraph("x the y")"##)
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a long TextNode is compared around the change"
    ))
  func longTextNode() throws {
    let long = String(repeating: "abcdefghij", count: 2000)
    let editor = try createMarkEditor()
    try update(editor) { try initDocument(long) }
    try mark(editor, 10000, 10010, "c1")
    // At either edge, inside, and a letter equal to the one after the edge
    let steps: [(Int, String, String)] = [
      (10010, "X", ##""abcdefghij"[c1] "Xabc"##),
      (10000, "Y", ##"Y" "abcdefghij"[c1] "Xabc"##),
      (10007, "Z", ##"Y" "abcdefZghij"[c1] "Xabc"##),
      (10012, "X", ##"Y" "abcdefZghij"[c1] "XXabc"##),
    ]
    for (at, text, run) in steps {
      try update(editor) { try select(at) }
      try typeText(editor, text)
      #expect(try tree(editor).contains(run))
    }
    // A selection replaced across the end of the mark
    try update(editor) { try select(10011, 10013) }
    try typeText(editor, "jQ")
    #expect(try tree(editor).contains(##"Y" "abcdefZghi"[c1] "jQXabc"##))
    #expect(try editor.read { try getRoot().getTextContentSize() } == 20004)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > no edit splits a surrogate pair"
    ))
  func noEditSplitsSurrogatePair() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("x\u{1F600}\u{1F642}y") }
    try mark(editor, 1, 5, "c1")
    // An emoji typed between the marked emoji joins the run
    try update(editor) { try select(3) }
    try typeText(editor, "\u{1F603}")
    #expect(try tree(editor) == "paragraph(\"x\" \"\u{1F600}\u{1F603}\u{1F642}\"[c1] \"y\")")
    // Emoji replaced without a command by one that shares their first code
    // unit: the kept text is found by code point
    let other = try createMarkEditor()
    try update(other) { try initDocument("x\u{1F600}\u{1F642}y") }
    try mark(other, 1, 3, "c1")
    try update(other) { try select(1, 5).insertText("\u{1F603}") }
    #expect(try tree(other) == "paragraph(\"x\u{1F603}y\")")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a handler that wraps the selection instead of replacing it keeps its ids"
    ))
  func handlerWrapsSelection() throws {
    let editor = try createMarkEditor()
    try update(editor) { try initDocument("x teh y") }
    try mark(editor, 0, 7, "c1")
    try mark(editor, 2, 4, "c2")
    #expect(try tree(editor) == ##"paragraph("x "[c1] "te"[c1,c2] "h y"[c1])"##)
    _ = editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { payload, _ in
        guard payload == "\"", let selection = try getSelection() as? RangeSelection,
          !selection.isCollapsed()
        else { return false }
        let (start, end) =
          try selection.isBackward()
          ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
        let startNode = try start.getNode() as! TextNode
        let endNode = try end.getNode() as! TextNode
        let (startOffset, endOffset) = (start.offset, end.offset)
        _ = try endNode.spliceText(endOffset, 0, "\"")
        _ = try startNode.spliceText(startOffset, 0, "\"")
        let after = endOffset + (startNode.is(endNode) ? 2 : 1)
        _ = try endNode.select(after, after)
        return true
      }, COMMAND_PRIORITY_HIGH)
    try update(editor) { try select(2, 5) }
    try typeText(editor, "\"")
    #expect(
      try tree(editor)
        == "paragraph(\(quoted("x \""))[c1] \"te\"[c1,c2] \(quoted("h\" y"))[c1])")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a hashtag created from marked text keeps its ids, like MarkNode"
    ))
  func hashtagFromMarkedText() throws {
    for withMarkNode in [false, true] {
      let editor = try createEntityEditor(withMarkNode)
      try update(editor) { try initDocument("see foo bar") }
      try markAll(editor, withMarkNode)
      try update(editor) { try select(4) }
      try typeText(editor, "#")
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph(mark[c1]("see " "#foo" " bar"))"##
            : ##"paragraph("see "[c1] "#foo"[c1] " bar"[c1])"##))
      #expect(
        try editor.read { try paragraph().getAllTextNodes().map { $0.getType() } }
          == ["text", "hashtag", "text"])
      // Deleting the "#" turns it back into text, which keeps them too
      try update(editor) { try select(4, 5) }
      try update(editor) { _ = try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true) }
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph(mark[c1]("see foo bar"))"## : ##"paragraph("see foo bar"[c1])"##))
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > a token node created from marked text keeps its ids, like MarkNode"
    ))
  func tokenFromMarkedText() throws {
    for withMarkNode in [false, true] {
      let editor = try createEntityEditor(withMarkNode)
      try update(editor) { try initDocument("hi bob x") }
      try markAll(editor, withMarkNode)
      try update(editor) { try select(3) }
      try typeText(editor, "@")
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph(mark[c1]("hi " "@bob" " x"))"##
            : ##"paragraph("hi "[c1] "@bob"[c1] " x"[c1])"##))
      #expect(
        try editor.read {
          try paragraph().getAllTextNodes().map { "\($0.getType()):\(try $0.getMode())" }
        } == ["text:normal", "test-mention:token", "text:normal"])
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an atomic node created from text with different ids gets all of them"
    ))
  func atomicNodeGetsAllIDs() throws {
    // "#" typed at the start of a marked paragraph is at the edge of the
    // mark, but the hashtag it makes is one node: it keeps the ids of "foo"
    let editor = try createEntityEditor(false)
    try update(editor) { try initDocument("foo") }
    try markAll(editor, false)
    try update(editor) { try select(0) }
    try typeText(editor, "#")
    #expect(try tree(editor) == ##"paragraph("#foo"[c1])"##)
    #expect(try editor.read { try paragraph().getFirstChildOrThrow().getType() } == "hashtag")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > typing inside a segmented node in a marked run keeps its ids, like MarkNode"
    ))
  func typingInsideSegmented() throws {
    for withMarkNode in [false, true] {
      let editor = try createMarkEditor()
      try update(editor) {
        try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode("hi "), createTextNode("Alice Smith").setMode("segmented"),
            createTextNode(" ok")))
      }
      try markAll(editor, withMarkNode)
      try update(editor) { try select(8) }
      try typeText(editor, "X")
      #expect(
        try tree(editor)
          == (withMarkNode
            ? ##"paragraph(mark[c1]("hi AliceX Smith ok"))"##
            : ##"paragraph("hi AliceX Smith ok"[c1])"##))
    }
  }

  private func compose(_ editor: Editor, _ taggedEdit: @escaping () throws -> Void) throws {
    try update(editor) { try select(15) }
    try update(editor) { try dispatchCompositionStart(editor) }
    try update(editor) {
      let marked = try paragraph().getChildAtIndex(1) as! TextNode
      _ = try marked.setTextContent("grew 4%に")
      _ = try marked.select(8, 8)
    }
    try update(editor) {
      try addUpdateTag(COLLABORATION_TAG)
      try taggedEdit()
    }
    try update(editor) {
      let marked = try paragraph().getChildAtIndex(1) as! TextNode
      let text = try marked.getTextContent().replacingFirst("に", "日本")
      _ = try marked.setTextContent(text)
      _ = try marked.select(text.utf16.count, text.utf16.count)
    }
    try update(editor) { try setCompositionKey(nil) }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > replaced and re-created text > an update tagged COLLABORATION_TAG editing the paragraph of a composition"
    ))
  func collaborationTaggedDuringComposition() throws {
    // Comparing with the paragraph from before the composition would take
    // the "P" of the tagged update for composed text and change its ids, so
    // the composed text is left in the node it was composed in
    let editor = try createMarkEditor()
    try update(editor) { try initDocument() }
    try mark(editor, 8, 15, "c1")
    try compose(editor) {
      let marked = try paragraph().getChildAtIndex(1) as! TextNode
      _ = try marked.setTextContent("grew 4%Pに")
      _ = try marked.select(9, 9)
    }
    #expect(try tree(editor) == ##"paragraph("Revenue " "grew 4%P日本"[c1] " in Q3.")"##)
    // An edit in another paragraph does not change how it is compared
    let other = try createMarkEditor()
    try update(other) {
      try initDocument()
      _ = try getRoot().append(createParagraphNode().append(createTextNode("z")))
    }
    try mark(other, 8, 15, "c1")
    try compose(other) { _ = try firstText(1).setTextContent("zz") }
    #expect(
      try tree(other)
        == ##"paragraph("Revenue " "grew 4%"[c1] "日本 in Q3.") / paragraph("zz")"##)
  }
}

extension String {
  /// `String.prototype.replace(search, replacement)` with a string search.
  func replacingFirst(_ search: String, _ replacement: String) -> String {
    guard let range = range(of: search) else { return self }
    return replacingCharacters(in: range, with: replacement)
  }
}
