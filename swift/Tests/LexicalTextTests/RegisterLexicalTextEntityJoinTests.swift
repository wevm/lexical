import Lexical
import LexicalHashtag
import LexicalList
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts.
//
// #9704: joining a block onto text that ends in another format (or a tab)
// turns a hashtag at the start of the joined block into text (`word#tag`
// holds no hashtag), and its characters keep their NodeState. The tests that
// use `@lexical/mark` (`MarkStateExtension`, `$addMarkID`) are not ported.

// A NodeState on the hashtag, as a comment mark stored as NodeState is.
nonisolated(unsafe) private let note = createState("note", parse: { $0?.stringValue ?? "" })

private func createHashtagEditor() throws -> Editor {
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(HashtagNode.self), .klass(ListNode.self), .klass(ListItemNode.self)],
      onError: { throw $0 }))
  try editor.setRootElement(TestRootElement())
  _ = try registerLexicalTextEntity(
    editor, HashtagConfig().getHashtagMatch, HashtagNode.self
  ) { try createHashtagNode($0.getTextContent()) }
  return editor
}

private struct Row: Equatable, CustomStringConvertible {
  var type: String
  var text: String
  var format: Int
  var note: String

  init(_ type: String, _ text: String, _ format: Int, _ note: String) {
    self.type = type
    self.text = text
    self.format = format
    self.note = note
  }

  var description: String { "[\(type), \(text), \(format), \(note)]" }
}

// [type, text, format, state] for every text node.
private func read(_ editor: Editor) throws -> [Row] {
  try editor.read {
    try getRoot().getAllTextNodes().map {
      Row($0.getType(), try $0.getTextContent(), try $0.getFormat(), try getState($0, note))
    }
  }
}

// Backspace at the start of the last block: select from the end of the first
// block to the start of the last one and remove.
private func joinLastBlock() throws {
  let last = try (getRoot().getFirstChildOrThrow() as! ElementNode).getLastDescendant() as! TextNode
  let first =
    try (getRoot().getLastChildOrThrow() as! ElementNode).getFirstDescendant() as! TextNode
  let s = createRangeSelection()
  try s.anchor.set(last.getKey(), last.getTextContentSize(), .text)
  try s.focus.set(first.getKey(), 0, .text)
  try setSelection(s)
  try s.removeText()
  try setSelection(nil)
}

private func list(_ children: LexicalNode...) throws -> ListNode {
  try createListNode("bullet").append(createListItemNode().append(contentsOf: children))
}

private final class MentionNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(MentionNode.self, type: "join-mention", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private final class SegmentedNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SegmentedNode.self, type: "join-segmented", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private final class Counter {
  var calls = 0
}

@Suite struct RegisterLexicalTextEntityJoinTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > #9704: a list item that starts with a hashtag, joined onto italic text"
    ))
  func listItemJoinedOntoItalic() throws {
    let editor = try createHashtagEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("one "), createTextNode("word").setFormat("italic")),
        list(setState(createHashtagNode("#tag"), note, "c1"), createTextNode(" more")))
    }
    try editor.update(discrete: true) { try joinLastBlock() }
    // "word#tag" is no hashtag (a fresh parse finds none), so the hashtag
    // becomes text; its characters keep their NodeState.
    #expect(
      try read(editor) == [
        Row("text", "one ", 0, ""),
        Row("text", "word", 2, ""),
        Row("text", "#tag", 0, "c1"),
        Row("text", " more", 0, ""),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > the same with backspace from the start of the item (deleteCharacter)"
    ))
  func backspaceFromStartOfItem() throws {
    let editor = try createHashtagEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("one "), createTextNode("word").setFormat("italic")),
        createParagraphNode().append(
          setState(createHashtagNode("#tag"), note, "c1"), createTextNode(" more")))
    }
    try editor.update(discrete: true) {
      let hashtag =
        try (getRoot().getLastChildOrThrow() as! ElementNode).getFirstDescendant() as! TextNode
      try hashtag.select(0, 0).deleteCharacter(true)
      try setSelection(nil)
    }
    #expect(
      try read(editor) == [
        Row("text", "one ", 0, ""),
        Row("text", "word", 2, ""),
        Row("text", "#tag", 0, "c1"),
        Row("text", " more", 0, ""),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > forward delete at the end of the text before it"
    ))
  func forwardDeleteAtEndOfTextBefore() throws {
    let editor = try createHashtagEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("one "), createTextNode("word").setFormat("bold")),
        createParagraphNode().append(setState(createHashtagNode("#tag"), note, "c1")))
    }
    try editor.update(discrete: true) {
      let word =
        try (getRoot().getFirstChildOrThrow() as! ElementNode).getLastDescendant() as! TextNode
      try word.select(4, 4).deleteCharacter(false)
      try setSelection(nil)
    }
    #expect(
      try read(editor) == [
        Row("text", "one ", 0, ""),
        Row("text", "word", 1, ""),
        Row("text", "#tag", 0, "c1"),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > the block before ends with a tab, the text and tab carry NodeState"
    ))
  func blockBeforeEndsWithTab() throws {
    // From a long-session test: a block ending with a tab (both with comment
    // marks) joined with a list item starting with a commented hashtag.
    let editor = try createHashtagEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          setState(createTextNode("t5015z t5016z"), note, "c1,c4"),
          setState(createTabNode(), note, "c1,c4")),
        list(
          setState(createHashtagNode("#t5017z"), note, "c1,c4"),
          setState(createTextNode(" t5018z"), note, "c4")))
    }
    try editor.update(discrete: true) { try joinLastBlock() }
    let rows = try read(editor)
    #expect(try editor.read { try getRoot().getTextContent() } == "t5015z t5016z\t#t5017z t5018z")
    // Whatever the hashtag becomes, its characters keep their NodeState
    #expect(rows.first { $0.text.contains("#t5017z") }?.note == "c1,c4")
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity turned back into text does not take the text after it first > two hashtags turned into text: the text after them keeps its format, NodeState and the selection"
    ))
  func twoHashtagsTurnedIntoText() throws {
    let editor = try createHashtagEditor()
    try editor.update(discrete: true) {
      let suffix = try setState(createTextNode("z!").setFormat("italic"), note, "suffix")
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("x"),
          createTextNode("a").setFormat("bold"),
          createHashtagNode("#b").setFormat("bold"),
          setState(createHashtagNode("#c"), note, "entity"),
          suffix))
      _ = try suffix.select(2, 2)
    }
    #expect(
      try read(editor) == [
        Row("text", "x", 0, ""),
        Row("text", "a#b", 1, ""),
        Row("text", "#c", 0, "entity"),
        Row("text", "z!", 2, "suffix"),
      ])
    try editor.read {
      let selection = try getSelection()
      #expect(isRangeSelection(selection))
      let anchor = try #require(selection as? RangeSelection).anchor
      #expect(try anchor.getNode().getTextContent() == "z!")
      #expect(anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::entities of two registrations that touch > typing before a hashtag that a token and a segmented entity follow settles"
    ))
  func twoRegistrationsThatTouchSettle() throws {
    let counter = Counter()
    func counted(_ getMatch: @escaping (String) -> EntityMatch?) -> (String) -> EntityMatch? {
      { text in
        counter.calls += 1
        return getMatch(text)
      }
    }
    let editor = try createEditor(
      CreateEditorArgs(
        nodes: [.klass(HashtagNode.self), .klass(MentionNode.self), .klass(SegmentedNode.self)],
        onError: { throw $0 }))
    _ = try registerLexicalTextEntity(
      editor, counted(HashtagConfig().getHashtagMatch), HashtagNode.self
    ) { try createHashtagNode($0.getTextContent()) }
    _ = try registerLexicalTextEntity(editor, counted(regexMatch("@[a-z]+")), MentionNode.self) {
      try create(MentionNode.self).setTextContent($0.getTextContent()).setMode("token")
    }
    _ = try registerLexicalTextEntity(
      editor, counted(regexMatch(#"\$[A-Z]+"#)), SegmentedNode.self
    ) {
      try create(SegmentedNode.self).setTextContent($0.getTextContent()).setMode("segmented")
    }
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("x"),
          createTextNode(" #b").setFormat("bold"),
          createTextNode("$B@x").setFormat("code")))
    }
    counter.calls = 0
    try editor.update(discrete: true) {
      let space = try getRoot().getAllTextNodes()[1]
      let selection = try space.select(1, 1)
      selection.format = try space.getFormat()
      try selection.insertText("a")
    }
    #expect(try editor.read { try getRoot().getTextContent() } == "x a#b$B@x")
    // The transforms of the two that touch run a bounded number of times
    #expect(counter.calls < 200)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 7 earlier matches, a later match in the text after it is still made"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 8 earlier matches, a later match in the text after it is still made"
    ),
    arguments: [7, 8])
  func laterMatchIsStillMade(_ repeatCount: Int) throws {
    let editor = try createEditor(
      CreateEditorArgs(nodes: [.klass(HashtagNode.self)], onError: { throw $0 }))
    _ = try registerLexicalTextEntity(
      editor, HashtagConfig().getHashtagMatch, HashtagNode.self
    ) { try createHashtagNode($0.getTextContent()) }
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("#").setFormat("bold"),
          createTextNode("a "),
          createTextNode("#b").setFormat("italic")))
    }
    let repeated = String(repeating: "#x ", count: repeatCount)
    try editor.update(discrete: true) {
      _ = try getRoot().getAllTextNodes().first { try $0.getTextContent() == "a " }!
        .spliceText(2, 0, repeated)
    }
    try editor.update(discrete: true) {
      try getRoot().getAllTextNodes().first { try $0.getTextContent() == "#b" }!
        .selectEnd()
        .insertNodes([createTextNode("c rest #d").setFormat("bold")])
    }
    try editor.read {
      #expect(try getRoot().getTextContent() == "#a " + repeated + "#bc rest #d")
      let hashtags = try getRoot().getAllTextNodes().filter { $0.getType() == "hashtag" }.map {
        try $0.getTextContent()
      }
      #expect(hashtags.contains("#d"))
    }
  }
}
