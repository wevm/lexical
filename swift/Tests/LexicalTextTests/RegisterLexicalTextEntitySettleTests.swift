import Foundation
import Lexical
import LexicalMarkdown
import LexicalTesting
import LexicalText
import Testing

// Port of packages/lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts.
//
// registerLexicalTextEntity's transforms must settle wherever an edit leaves
// text between text nodes of other formats (#9637), and settle on the same
// entities as a fresh parse of the text they leave.

private final class KeywordNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(KeywordNode.self, type: "test-keyword", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
  override func canInsertTextBefore() throws -> Bool { false }
  override func canInsertTextAfter() throws -> Bool { false }
}

private final class TagNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TagNode.self, type: "test-tag", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
  override func canInsertTextBefore() throws -> Bool { false }
}

private typealias GetMatch = (String) -> EntityMatch?

/// A match of `regex` as an `EntityMatch`, shifted past group 1 when
/// `skipGroup1` (the JS `m.index + m[1].length`).
private func execMatch(
  _ regex: JSRegExp, _ text: String, skipGroup1: Bool = false
)
  -> EntityMatch?
{
  guard let m = regex.exec(text) else { return nil }
  if skipGroup1 {
    let start = m.index + (m[1]?.utf16.count ?? 0)
    return EntityMatch(start: start, end: start + (m[2]?.utf16.count ?? 0))
  }
  return EntityMatch(start: m.index, end: m.index + m.length)
}

private let keywordRegex = JSRegExp("(^|[^a-z#])(cat|dog)(?=[^a-z]|$)")
private let tagRegex = JSRegExp(#"(^|\s)(#[a-z]+)"#)
private let spacedRegex = JSRegExp("^#[a-z]+$|#[a-z]+(?= )")

/// The issue's matcher: `cat` or `dog` with no letter next to them.
private func keywordMatch(_ text: String) -> EntityMatch? {
  execMatch(keywordRegex, text, skipGroup1: true)
}

/// A hashtag-like matcher: `#` and letters, after the start or a space.
private func tagMatch(_ text: String) -> EntityMatch? {
  execMatch(tagRegex, text, skipGroup1: true)
}

/// A hashtag when a space follows it, or when it is all of the text.
private func spaced(_ text: String) -> EntityMatch? {
  execMatch(spacedRegex, text)
}

/// `text.indexOf(word, from)` in UTF-16 code units.
private func indexOf(_ text: [UInt16], _ word: [UInt16], _ from: Int) -> Int {
  var i = max(from, 0)
  while i + word.count <= text.count {
    if Array(text[i..<(i + word.count)]) == word {
      return i
    }
    i += 1
  }
  return -1
}

/// `dog` with no letter before or after it, where the letter before is looked
/// at but not taken into the match, like a regex lookbehind
/// (`/(?<![a-z])dog(?![a-z])/`).
private func lookbehindMatch(_ text: String, _ word: String = "dog") -> EntityMatch? {
  let units = Array(text.utf16)
  let wordUnits = Array(word.utf16)
  func isLetter(_ i: Int) -> Bool {
    guard i >= 0, i < units.count else { return false }
    let c = units[i]
    return (65...90).contains(c) || (97...122).contains(c)
  }
  var i = indexOf(units, wordUnits, 0)
  while i != -1 {
    if !isLetter(i - 1) && !isLetter(i + wordUnits.count) {
      return EntityMatch(start: i, end: i + wordUnits.count)
    }
    i = indexOf(units, wordUnits, i + 1)
  }
  return nil
}

private enum Matcher: String, CaseIterable, CustomStringConvertible {
  case keyword, tag, lookbehind

  var description: String { rawValue }

  var getMatch: GetMatch {
    switch self {
    case .keyword: keywordMatch
    case .lookbehind: { lookbehindMatch($0) }
    case .tag: tagMatch
    }
  }
}

private func makeEditor(
  _ matcher: Matcher = .keyword, _ getMatch: GetMatch? = nil
) throws
  -> Editor
{
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(KeywordNode.self), .klass(TagNode.self)], onError: { throw $0 }))
  if matcher == .tag {
    _ = try registerLexicalTextEntity(editor, getMatch ?? matcher.getMatch, TagNode.self) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
  } else {
    _ = try registerLexicalTextEntity(editor, matcher.getMatch, KeywordNode.self) {
      try create(KeywordNode.self).setTextContent($0.getTextContent())
    }
  }
  return editor
}

/// An editor with only `KeywordNode` or `TagNode`, and one registration.
private func makeEditor<T: TextNode>(
  _ klass: T.Type, _ getMatch: @escaping GetMatch,
  _ createNode: @escaping (TextNode) throws -> T
) throws -> Editor {
  let editor = try createEditor(CreateEditorArgs(nodes: [.klass(klass)], onError: { throw $0 }))
  _ = try registerLexicalTextEntity(editor, getMatch, klass, createNode)
  return editor
}

/// A run of text: `*` bold, `_` italic, `` ` `` code, then the text.
private typealias Run = String

private func parseRun(_ run: Run) -> (formats: [TextFormatType], text: String) {
  var formats: [TextFormatType] = []
  var rest = Substring(run)
  loop: while let c = rest.first {
    switch c {
    case "*": formats.append("bold")
    case "_": formats.append("italic")
    case "`": formats.append("code")
    default: break loop
    }
    rest = rest.dropFirst()
  }
  return (formats, String(rest))
}

private func createRun(_ run: Run) throws -> TextNode {
  let (formats, text) = parseRun(run)
  let node = try createTextNode(text)
  for format in formats {
    _ = try node.toggleFormat(format)
  }
  return node
}

private func paragraph() throws -> ElementNode {
  try getRoot().getFirstChildOrThrow() as! ElementNode
}

private struct Entry: Equatable, CustomStringConvertible {
  var type: String
  var text: String
  var format: Int

  init(_ type: String, _ text: String, _ format: Int) {
    self.type = type
    self.text = text
    self.format = format
  }

  var description: String { "[\(type), \(text.debugDescription), \(format)]" }
}

private typealias Out = [Entry]

private func readOut() throws -> Out {
  try paragraph().getChildren().compactMap { $0 as? TextNode }.map {
    Entry($0.getType(), try $0.getTextContent(), try $0.getFormat())
  }
}

private func read(_ editor: Editor) throws -> Out {
  try editor.getEditorState().read { try readOut() }
}

private func setUp(_ editor: Editor, _ runs: [Run]) throws {
  try editor.update(discrete: true) {
    _ = try getRoot().clear().append(
      createParagraphNode().append(contentsOf: runs.map(createRun)))
  }
}

@discardableResult
private func select(_ node: TextNode, _ offset: Int) throws -> RangeSelection {
  let sel = createRangeSelection()
  try sel.anchor.set(node.getKey(), offset, .text)
  try sel.focus.set(node.getKey(), offset, .text)
  try setSelection(sel)
  return sel
}

private func textAt(_ index: Int) throws -> TextNode {
  try paragraph().getChildAtIndex(index) as! TextNode
}

/// Type into run `index` at `offset` of a paragraph made of `runs`.
private func typeInto(
  _ runs: [Run], _ index: Int, _ offset: Int, _ typed: String, _ matcher: Matcher = .keyword
) throws -> Editor {
  let editor = try makeEditor(matcher)
  try setUp(editor, runs)
  try editor.update(discrete: true) {
    try select(try textAt(index), offset).insertText(typed)
  }
  return editor
}

/// The text of a paragraph as runs of one format each, adjacent runs of the
/// same format joined: what a fresh parse starts from.
private func toRuns(_ out: Out) -> [(text: String, format: Int)] {
  var runs: [(text: String, format: Int)] = []
  for entry in out {
    if let last = runs.last, last.format == entry.format {
      runs[runs.count - 1].text += entry.text
    } else {
      runs.append((entry.text, entry.format))
    }
  }
  return runs
}

/// The nodes the transforms settle on when they parse `runs` from scratch.
private func freshParse(
  _ runs: [(text: String, format: Int)], _ matcher: Matcher, _ getMatch: GetMatch? = nil
) throws -> Out {
  let editor = try makeEditor(matcher, getMatch)
  try editor.update(discrete: true) {
    _ = try getRoot().clear().append(
      createParagraphNode().append(
        contentsOf: runs.map { try createTextNode($0.text).setFormat($0.format) }))
  }
  return try read(editor)
}

private func expectFreshParse(
  _ editor: Editor, _ matcher: Matcher, sourceLocation: SourceLocation = #_sourceLocation
) throws {
  let out = try read(editor)
  #expect(out == (try freshParse(toRuns(out), matcher)), sourceLocation: sourceLocation)
}

private let B = 1  // IS_BOLD
private let I = 2  // IS_ITALIC

private struct Failure: Error {}

@Suite struct RegisterLexicalTextEntitySettleTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bold \"x\" | \"dogcat\" | bold \"y\", a space after \"dog\": settles, no entity"
    ))
  func spaceAfterDog() throws {
    let editor = try typeInto(["*x", "dogcat", "*y"], 1, 3, " ")
    #expect(
      try read(editor) == [
        Entry("text", "x", B),
        Entry("text", "dog cat", 0),
        Entry("text", "y", B),
      ])
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bold \"x\" | \"dogcat\" | bold \"dog\", an emoji after \"dog\": settles, no entity"
    ))
  func emojiAfterDog() throws {
    let editor = try typeInto(["*x", "dogcat", "*dog"], 1, 3, "😀")
    #expect(
      try read(editor) == [
        Entry("text", "x", B),
        Entry("text", "dog😀cat", 0),
        Entry("text", "dog", B),
      ])
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a later match in the same node is still an entity, and one touching the next node is not"
    ))
  func laterMatchInSameNode() throws {
    let editor = try typeInto(["*x", "dogcat dog", "*y"], 1, 3, " ")
    #expect(
      try read(editor) == [
        Entry("text", "x", B),
        Entry("text", "dog ", 0),
        Entry("test-keyword", "cat", 0),
        Entry("text", " dog", 0),
        Entry("text", "y", B),
      ])
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > control: \"dogcat\" | bold \"y\", nothing before"
    ))
  func controlNothingBefore() throws {
    let editor = try typeInto(["dogcat", "*y"], 0, 3, " ")
    #expect(
      try read(editor) == [
        Entry("test-keyword", "dog", 0),
        Entry("text", " cat", 0),
        Entry("text", "y", B),
      ])
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > control: bold \" \" | \"dogcat\" | bold \"y\", a space before"
    ))
  func controlSpaceBefore() throws {
    let editor = try typeInto(["* ", "dogcat", "*y"], 1, 3, " ")
    #expect(
      try read(editor) == [
        Entry("text", " ", B),
        Entry("test-keyword", "dog", 0),
        Entry("text", " cat", 0),
        Entry("text", "y", B),
      ])
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a match that needs a lookbehind is checked against the previous node"
    ))
  func lookbehindAgainstPreviousNode() throws {
    let touching = try typeInto(["*x", "dogcat", "*y"], 1, 3, " ", .lookbehind)
    #expect(
      try read(touching) == [
        Entry("text", "x", B),
        Entry("text", "dog cat", 0),
        Entry("text", "y", B),
      ])
    let spacedEditor = try typeInto(["*x ", "dogcat", "*y"], 1, 3, " ", .lookbehind)
    #expect(
      try read(spacedEditor) == [
        Entry("text", "x ", B),
        Entry("test-keyword", "dog", 0),
        Entry("text", " cat", 0),
        Entry("text", "y", B),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a hashtag after a formatted letter is not an entity, after a formatted space it is"
    ))
  func hashtagAfterFormattedLetterOrSpace() throws {
    let letter = try typeInto(["*a", "#tagx", "*."], 1, 4, " ", .tag)
    #expect(
      try read(letter) == [
        Entry("text", "a", B),
        Entry("text", "#tag x", 0),
        Entry("text", ".", B),
      ])
    try expectFreshParse(letter, .tag)
    let space = try typeInto(["*a ", "#tagx", "*."], 1, 4, " ", .tag)
    #expect(
      try read(space) == [
        Entry("text", "a ", B),
        Entry("test-tag", "#tag", 0),
        Entry("text", " x", 0),
        Entry("text", ".", B),
      ])
    try expectFreshParse(space, .tag)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > of two matches next to each other, the second stays text, as before"
    ))
  func twoAdjacentMatches() throws {
    let editor = try typeInto(["*x ", "a#b", "*."], 1, 0, "#", .tag)
    #expect(
      try read(editor) == [
        Entry("text", "x ", B),
        Entry("test-tag", "#a", 0),
        Entry("text", "#b", 0),
        Entry("text", ".", B),
      ])
    try expectFreshParse(editor, .tag)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > entity at the edges of a node, every format combination around it"
    ))
  func everyFormatCombination() throws {
    let formats = ["*", "_", "`", "*_"]
    for before in formats {
      for after in formats {
        for (left, right) in [("x", "y"), (" ", " "), ("x", " "), (" ", "y")] {
          for typed in [" ", "😀", "!", "z"] {
            let runs = [before + left, "dogcat", after + right]
            let editor: Editor
            do {
              editor = try typeInto(runs, 1, 3, typed)
            } catch {
              Issue.record("\(runs) \(typed.debugDescription): \(error)")
              continue
            }
            let out = try read(editor)
            #expect(out.map(\.text).joined() == left + "dog" + typed + "cat" + right)
            try expectFreshParse(editor, .keyword)
          }
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > IME composition between two formatted nodes settles, and so does its end"
    ))
  func imeComposition() throws {
    let editor = try makeEditor()
    try setUp(editor, ["*x", "dogcat", "*y"])
    try editor.update(discrete: true) {
      let node = try textAt(1)
      try select(node, 3)
      try setCompositionKey(node.getKey())
    }
    for typed in ["、", " "] {
      #expect(throws: Never.self) {
        try editor.update(discrete: true) {
          try select(try textAt(1), 3).insertText(typed)
        }
      }
    }
    #expect(throws: Never.self) {
      try editor.update(discrete: true) {
        try setCompositionKey(nil)
        for node in try paragraph().getChildren() {
          try node.markDirty()
        }
      }
    }
    #expect(try read(editor).map(\.text).joined() == "xdog 、caty")
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > pasting formatted runs around a keyword settles"
    ))
  func pastingFormattedRuns() throws {
    let editor = try makeEditor()
    try setUp(editor, ["dogcat"])
    #expect(throws: Never.self) {
      try editor.update(discrete: true) {
        let node = try paragraph().getFirstChildOrThrow() as! TextNode
        try select(node, 3).insertNodes([
          createRun("*x"), createRun(" dog "), createRun("*y"),
        ])
      }
    }
    #expect(try read(editor).map(\.text).joined() == "dogx dog ycat")
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > bolding the text before an entity turns it back into text"
    ))
  func boldingTextBeforeEntity() throws {
    let editor = try makeEditor()
    try setUp(editor, ["xdog cat"])
    try editor.update(discrete: true) {
      let node = try paragraph().getFirstChildOrThrow() as! TextNode
      let rest = try node.splitText(1)[1]
      _ = try (try paragraph().getFirstChildOrThrow() as! TextNode).toggleFormat("bold")
      try rest.markDirty()
    }
    try expectFreshParse(editor, .keyword)
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing a letter before an entity after text in another format turns it back into text"
    ))
  func typingLetterBeforeEntity() throws {
    let editor = try makeEditor()
    try setUp(editor, ["_😀", "dog", "*!"])
    #expect(
      try read(editor) == [
        Entry("text", "😀", 2),
        Entry("test-keyword", "dog", 0),
        Entry("text", "!", B),
      ])
    try editor.update(discrete: true) {
      try select(try textAt(1), 0).insertText("a")
    }
    #expect(
      try read(editor) == [
        Entry("text", "😀", 2),
        Entry("text", "adog", 0),
        Entry("text", "!", B),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an earlier match that ends where the text ends is handled as before"
    ))
  func earlierMatchEndingWithText() throws {
    // `<word> dog` at the end of the text, or else `dog` anywhere.
    let wholeRegex = JSRegExp("[a-z]+ dog$")
    let phrase: GetMatch = { text in
      let whole = wholeRegex.exec(text)
      let word = keywordMatch(text)
      if let whole, word == nil || whole.index <= word!.start {
        return EntityMatch(start: whole.index, end: whole.index + whole.length)
      }
      return word
    }
    let editor = try makeEditor(KeywordNode.self, phrase) {
      try create(KeywordNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["*z", "ab dog!"])
    #expect(
      try read(editor) == [
        Entry("text", "z", B),
        Entry("text", "ab ", 0),
        Entry("test-keyword", "dog", 0),
        Entry("text", "!", 0),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a matcher that needs the text after a match settles as before"
    ))
  func matcherNeedsTextAfter() throws {
    // `dog` when " cat" follows it, or when it is all of the text.
    let followedRegex = JSRegExp("^dog$|dog(?= cat)")
    let editor = try makeEditor(KeywordNode.self, { execMatch(followedRegex, $0) }) {
      try create(KeywordNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["*x", "_a dog cat"])
    #expect(
      try read(editor) == [
        Entry("text", "x", B),
        Entry("text", "a ", I),
        Entry("test-keyword", "dog", I),
        Entry("text", " cat", I),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a match at the start of a node that needs the text after it settles after a formatted node"
    ))
  func matchAtStartNeedsTextAfter() throws {
    func make() throws -> Editor {
      try makeEditor(TagNode.self, spaced) {
        try create(TagNode.self).setTextContent($0.getTextContent())
      }
    }
    let typed = try make()
    try setUp(typed, ["*x", "#tagx"])
    try typed.update(discrete: true) {
      try select(try textAt(1), 4).insertText(" ")
    }
    #expect(
      try read(typed) == [
        Entry("text", "x", B),
        Entry("test-tag", "#tag", 0),
        Entry("text", " x", 0),
      ])

    // Removing the space after the entity turns it back into text at once,
    // as a fresh parse of the text does.
    let removed = try make()
    try setUp(removed, ["*g", "g#x "])
    #expect(
      try read(removed) == [
        Entry("text", "g", B),
        Entry("text", "g", 0),
        Entry("test-tag", "#x", 0),
        Entry("text", " ", 0),
      ])
    try removed.update(discrete: true) {
      try paragraph().getLastChildOrThrow().remove()
    }
    #expect(
      try read(removed) == [
        Entry("text", "g", B),
        Entry("text", "g#x", 0),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing before a formatted node with a token or segmented match leaves it as it was"
    ))
  func typingBeforeTokenOrSegmentedMatch() throws {
    for mode in ["token", "segmented"] {
      let editor = try makeEditor(TagNode.self, tagMatch) {
        try create(TagNode.self).setTextContent($0.getTextContent()).setMode(mode)
      }
      try setUp(editor, ["#", "*#b "])
      #expect(throws: Never.self) {
        try editor.update(discrete: true) {
          try select(try textAt(0), 1).insertText(" ")
        }
      }
      #expect(
        try read(editor) == [
          Entry("text", "# ", 0),
          Entry("text", "#b ", B),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a node after an entity in the same format and style but another detail is not its source"
    ))
  func nodeWithAnotherDetailIsNotSource() throws {
    let editor = try makeEditor(TagNode.self, spaced) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
    try editor.update(discrete: true) {
      _ = try getRoot().append(
        createParagraphNode().append(
          createTextNode("#x"),
          createTextNode(" ").setDetail(2)  // IS_UNMERGEABLE
        ))
    }
    #expect(try read(editor).first == Entry("test-tag", "#x", 0))
    try editor.update(discrete: true) {
      try select(try textAt(0), 0).insertText("g")
    }
    #expect(
      try read(editor) == [
        Entry("text", "g#x", 0),
        Entry("text", " ", 0),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity after a node with an earlier match is checked too"
    ))
  func entityAfterNodeWithEarlierMatch() throws {
    let editor = try makeEditor(TagNode.self, spaced) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["*x", "_z ", "#b "])
    #expect(try read(editor)[2] == Entry("test-tag", "#b", 0))
    try editor.update(discrete: true) {
      _ = try textAt(1).setTextContent("#a ")
    }
    try editor.update(discrete: true) {
      try paragraph().getLastChildOrThrow().remove()
    }
    let out = try read(editor)
    #expect(out == (try freshParse(toRuns(out), .tag, spaced)))
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > removing the text between an entity and a token after it turns the entity back into text"
    ))
  func removingTextBeforeToken() throws {
    let congratsRegex = JSRegExp("(^|[^a-z])(congrats)(?=[^a-z]|$)", "i")
    let congrats: GetMatch = { text in
      guard let m = congratsRegex.exec(text) else { return nil }
      let start = m.index + (m[1]?.utf16.count ?? 0)
      return EntityMatch(start: start, end: start + 8)
    }
    let editor = try makeEditor(KeywordNode.self, congrats) {
      try create(KeywordNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["_!", "*!congrats "])
    try editor.update(discrete: true) {
      _ = try paragraph().append(createRun("*Alice").setMode("token"))
    }
    #expect(
      try read(editor) == [
        Entry("text", "!", 2),
        Entry("text", "!", B),
        Entry("test-keyword", "congrats", B),
        Entry("text", " ", B),
        Entry("text", "Alice", B),
      ])
    try editor.update(discrete: true) {
      try #require(try paragraph().getChildAtIndex(3)).remove()
    }
    #expect(
      try read(editor) == [
        Entry("text", "!", 2),
        Entry("text", "!congrats", B),
        Entry("text", "Alice", B),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity is not kept by text after it in another format"
    ))
  func entityNotKeptByTextInAnotherFormat() throws {
    let editor = try makeEditor(TagNode.self, spaced) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["#x", "* "])
    #expect(
      try read(editor) == [
        Entry("test-tag", "#x", 0),
        Entry("text", " ", B),
      ])
    try editor.update(discrete: true) {
      try select(try textAt(0), 0).insertText("g")
    }
    // As a fresh parse of "g#x" and a bold " " makes it.
    #expect(
      try read(editor) == [
        Entry("text", "g#x", 0),
        Entry("text", " ", B),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an entity is checked again in each update"
    ))
  func entityCheckedAgainEachUpdate() throws {
    let editor = try makeEditor(TagNode.self, spaced) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
    try setUp(editor, ["*#ab#ab ", "_a"])
    #expect(
      try read(editor) == [
        Entry("text", "#ab", B),
        Entry("test-tag", "#ab", B),
        Entry("text", " ", B),
        Entry("text", "a", 2),
      ])
    try editor.update(discrete: true) {
      try select(try textAt(1), 2).insertText("x")
    }
    try editor.update(discrete: true) {
      _ = try textAt(1).toggleFormat("bold")
    }
    let out = try read(editor)
    #expect(out == (try freshParse(toRuns(out), .tag, spaced)))
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in text before a formatted hashtag marks only that text dirty, so history merges the keystrokes"
    ))
  func typingBeforeFormattedHashtagMarksOnlyThatText() throws {
    let editor = try makeEditor(.tag)
    try setUp(editor, ["*a", "#foo ", "_#bar"])
    var dirty: [Int] = []
    editor.registerUpdateListener { payload in
      dirty.append(payload.dirtyLeaves.size)
    }
    for c in ["x", "y", "z"] {
      try editor.update(discrete: true) {
        let node = try textAt(1)
        try select(node, try node.getTextContentSize() - 2).insertText(c)
      }
    }
    #expect(dirty == [1, 1, 1])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing that makes a match at the start of the next node one leaves it text until that node changes"
    ))
  func matchAtStartOfNextNodeWaits() throws {
    let editor = try makeEditor(.tag)
    try setUp(editor, ["a", "*b", "#foo #bar"])
    #expect(
      try read(editor) == [
        Entry("text", "a", 0),
        Entry("text", "b", B),
        Entry("text", "#foo ", 0),
        Entry("test-tag", "#bar", 0),
      ])
    try editor.update(discrete: true) {
      _ = try textAt(1).spliceText(1, 0, " ", moveSelection: false)
    }
    #expect(
      try read(editor) == [
        Entry("text", "a", 0),
        Entry("text", "b ", B),
        Entry("text", "#foo ", 0),
        Entry("test-tag", "#bar", 0),
      ])
    try editor.update(discrete: true) {
      try #require(try paragraph().getChildAtIndex(2)).markDirty()
    }
    #expect(
      try read(editor) == [
        Entry("text", "a", 0),
        Entry("text", "b ", B),
        Entry("test-tag", "#foo", 0),
        Entry("text", " ", 0),
        Entry("test-tag", "#bar", 0),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a token or segmented entity with text after it in its node settles"
    ))
  func tokenOrSegmentedEntityWithTextAfter() throws {
    for mode in ["token", "segmented"] {
      let editor = try makeEditor(TagNode.self, tagMatch) {
        try create(TagNode.self).setTextContent($0.getTextContent()).setMode(mode)
      }
      try setUp(editor, ["#a b"])
      #expect(
        try read(editor) == [
          Entry("test-tag", "#a", 0),
          Entry("text", " b", 0),
        ])
      try editor.update(discrete: true) {
        try select(try paragraph().getLastChild() as! TextNode, 2).insertText("c")
      }
      #expect(
        try read(editor) == [
          Entry("test-tag", "#a", 0),
          Entry("text", " bc", 0),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a token or segmented entity after a match left as text settles"
    ))
  func tokenOrSegmentedEntityAfterMatchLeftAsText() throws {
    for mode in ["token", "segmented"] {
      let editor = try makeEditor(TagNode.self, tagMatch) {
        try create(TagNode.self).setTextContent($0.getTextContent()).setMode(mode)
      }
      try setUp(editor, [" DOG x DOG", "*_#ab ##ab"])
      #expect(throws: Never.self) {
        try editor.update(discrete: true) {
          let node = try textAt(1)
          let selection = createRangeSelection()
          try selection.anchor.set(node.getKey(), 5, .text)
          try selection.focus.set(node.getKey(), 7, .text)
          try setSelection(selection)
          try selection.insertText("x")
        }
      }
      // `#x` is turned back into text twice, then kept, and takes the `b` its
      // match extends into (3: bold and italic).
      #expect(
        try read(editor) == [
          Entry("text", " DOG x DOG", 0),
          Entry("text", "#ab ", 3),
          Entry("test-tag", "#xb", 3),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > an update that fails does not count toward the next one"
    ))
  func failedUpdateDoesNotCount() throws {
    let editor = try makeEditor(TagNode.self, tagMatch) {
      try create(TagNode.self).setTextContent($0.getTextContent()).setMode("token")
    }
    var failures = 2
    try editor.registerNodeTransform(TextNode.self) { node in
      if try failures > 0 && node.getTextContent() == "#foo x" {
        failures -= 1
        throw Failure()
      }
    }
    try editor.update(discrete: true, skipTransforms: true) {
      _ = try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("a").setFormat("bold"),
          create(TagNode.self).setTextContent("#foo").setMode("token"),
          createTextNode(" x")))
    }
    for _ in 0..<3 {
      // The first two updates fail and are discarded.
      try? editor.update(discrete: true) {
        try paragraph().getLastChildOrThrow().markDirty()
      }
    }
    #expect(failures == 0)
    #expect(
      try read(editor) == [
        Entry("text", "a", B),
        Entry("text", "#foo x", 0),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > two registrations, one making token or segmented entities, settle"
    ))
  func twoRegistrations() throws {
    let upperRegex = JSRegExp("[A-Z]+")
    let upperMatch: GetMatch = { execMatch(upperRegex, $0) }
    let caseKeywordMatch: GetMatch = { keywordMatch($0.lowercased()) }
    for mode in ["token", "segmented"] {
      for tokenFirst in [true, false] {
        let editor = try createEditor(
          CreateEditorArgs(
            nodes: [.klass(KeywordNode.self), .klass(TagNode.self)], onError: { throw $0 }))
        func registerToken() throws {
          _ = try registerLexicalTextEntity(editor, upperMatch, TagNode.self) {
            try create(TagNode.self).setTextContent($0.getTextContent()).setMode(mode)
          }
        }
        if tokenFirst {
          try registerToken()
        }
        _ = try registerLexicalTextEntity(editor, caseKeywordMatch, KeywordNode.self) {
          try create(KeywordNode.self).setTextContent($0.getTextContent())
        }
        if !tokenFirst {
          try registerToken()
        }
        try setUp(editor, ["catDOGg"])
        #expect(
          try read(editor) == [
            Entry("text", "cat", 0),
            Entry("test-tag", "DOG", 0),
            Entry("text", "g", 0),
          ])
        try setUp(editor, ["xa", "*d", "DOG!dog"])
        let settled: Out =
          [
            Entry("text", "d", B),
            Entry("test-tag", "DOG", 0),
          ]
          + (tokenFirst
            ? [Entry("text", "!dog", 0)]
            : [
              Entry("text", "!", 0),
              Entry("test-keyword", "dog", 0),
            ])
        #expect(try read(editor) == [Entry("text", "xa", 0)] + settled)
        try editor.update(discrete: true) {
          try select(try textAt(0), 2).insertText("x")
        }
        #expect(try read(editor) == [Entry("text", "xax", 0)] + settled)
      }
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a createNode that adds text or changes its case settles"
    ))
  func createNodeThatChangesText() throws {
    let anywhereRegex = JSRegExp("dog|cat")
    let anywhere: GetMatch = { execMatch(anywhereRegex, $0) }
    func settle(_ runs: [Run], _ createText: @escaping (String) -> String) throws -> Out {
      let editor = try makeEditor(KeywordNode.self, anywhere) {
        try create(KeywordNode.self).setTextContent(createText(try $0.getTextContent()))
      }
      try setUp(editor, runs)
      return try read(editor)
    }
    #expect(
      try settle(["*z", "x", "_dog", "*!"]) { $0 + " " } == [
        Entry("text", "z", B),
        Entry("text", "x", 0),
        Entry("test-keyword", "dog", I),
        Entry("text", " ", I),
        Entry("text", "!", B),
      ])
    // `dog` before "cat", or `DOG`: the entity `createNode` makes is a
    // match after the bold "x" even though the text it replaces is one only
    // with the "cat" after it.
    let beforeRegex = JSRegExp("dog(?=cat)|DOG")
    let upper = try makeEditor(KeywordNode.self, { execMatch(beforeRegex, $0) }) {
      try create(KeywordNode.self).setTextContent(try $0.getTextContent().uppercased())
    }
    try setUp(upper, ["*x", "dogcat", "*y"])
    #expect(
      try read(upper) == [
        Entry("text", "x", B),
        Entry("test-keyword", "DOG", 0),
        Entry("text", "cat", 0),
        Entry("text", "y", B),
      ])
    // The `DOG` made from "xdog" is no entity after the bold "z" either, so
    // it is turned back into text before it grows over the italic `DOG`
    // (before, it grew over it first, and that text lost its italic).
    #expect(
      try settle(["*z", "xdog", "_dog cat", "*!"]) { $0.uppercased() } == [
        Entry("text", "z", B),
        Entry("text", "xDOG", 0),
        Entry("text", "DOG CAT", I),
        Entry("text", "!", B),
      ])
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in a long text node before an entity calls getMatch a few times, however long it is"
    ))
  func longTextNodeCallsGetMatchFewTimes() throws {
    for repeatCount in [2000, 8000] {
      var calls = 0
      let editor = try makeEditor(
        KeywordNode.self,
        { text in
          calls += 1
          return keywordMatch(text.lowercased())
        }
      ) {
        try create(KeywordNode.self).setTextContent($0.getTextContent())
      }
      // "do" + "g" is a match that starts in the bold node, so the long node
      // is left as text, and the entity after it stays one.
      try editor.update(discrete: true) {
        _ = try getRoot().append(
          createParagraphNode().append(
            createRun("*do"),
            createTextNode("g" + String(repeating: " cat", count: repeatCount) + " "),
            create(KeywordNode.self).setTextContent("dog")))
      }
      calls = 0
      try editor.update(discrete: true) {
        _ = try textAt(1).spliceText(3, 0, "x", moveSelection: false)
      }
      #expect(calls <= 10)
      #expect(try read(editor)[2] == Entry("test-keyword", "dog", 0))
    }
  }

  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > a createNode that changes the text still makes an entity, as before"
    ))
  func createNodeChangingTextStillMakesEntity() throws {
    let editor = try makeEditor(
      KeywordNode.self, { lookbehindMatch($0, "DOG") ?? lookbehindMatch($0, "dog") }
    ) {
      try create(KeywordNode.self).setTextContent(try $0.getTextContent().lowercased())
    }
    try setUp(editor, ["* ", "DOGcat", "*y"])
    try editor.update(discrete: true) {
      try select(try textAt(1), 3).insertText(" ")
    }
    #expect(
      try read(editor) == [
        Entry("text", " ", B),
        Entry("test-keyword", "dog", 0),
        Entry("text", " cat", 0),
        Entry("text", "y", B),
      ])
  }
}

/// Mulberry32, so a failure names a seed that reproduces it.
private func rng(_ seed: Int) -> () -> Double {
  var a = UInt32(truncatingIfNeeded: seed)
  return {
    a = a &+ 0x6d2b_79f5
    var t = a
    t = (t ^ (t >> 15)) &* (t | 1)
    t ^= t &+ ((t ^ (t >> 7)) &* (t | 61))
    return Double(t ^ (t >> 14)) / 4_294_967_296
  }
}

private func markAll() throws {
  for node in try paragraph().getChildren() {
    try node.markDirty()
  }
}

/// Runs `fn`, and returns the error it throws as a message, or nil.
private func thrown(_ fn: () throws -> Void) -> String? {
  do {
    try fn()
    return nil
  } catch {
    return String(String(describing: error).prefix(80))
  }
}

private let SEEDS =
  Int(ProcessInfo.processInfo.environment["TEXT_ENTITY_SETTLE_SEEDS"] ?? "")
  ?? 2000
private let MATCHER_NAMES: [Matcher] = [.keyword, .tag, .lookbehind]

private let PIECES: [Matcher: [String]] = [
  .keyword: ["dog", "cat", " ", "x", "a", "#", "😀", "!", "do", "g"],
  .lookbehind: ["dog", " ", "x", "d", "og", "😀", "!"],
  .tag: ["#", "#a", "#ab", "a", "b", " ", "😀", "!", "x"],
]
private let TYPED = [" ", "😀", "a", "x", "#", "g", "d", "!", "\u{00a0}"]
private let FORMATS = ["", "*", "_", "*_", "`"]

private func randomRuns(_ random: () -> Double, _ matcher: Matcher) -> [Run] {
  func pick<T>(_ xs: [T]) -> T { xs[Int(random() * Double(xs.count))] }
  var runs: [Run] = []
  var lastFormat: String?
  var i = 2 + Int(random() * 3)
  while i > 0 {
    var format = pick(FORMATS)
    if format == lastFormat {
      format = format == "" ? "*" : ""
    }
    lastFormat = format
    var text = ""
    var j = 1 + Int(random() * 4)
    while j > 0 {
      text += pick(PIECES[matcher]!)
      j -= 1
    }
    runs.append(format + text)
    i -= 1
  }
  return runs
}

extension RegisterLexicalTextEntitySettleTests {
  // A fresh parse settles, and transforming every node again leaves it as
  // it is.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random layouts parse, and parse again the same"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random layouts parse, and parse again the same"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random layouts parse, and parse again the same"
    ),
    arguments: MATCHER_NAMES)
  fileprivate func randomLayoutsParseAgainTheSame(_ matcher: Matcher) throws {
    var failures: [String] = []
    var seed = 0
    while seed < SEEDS && failures.count < 5 {
      defer { seed += 1 }
      let random = rng(seed * 7 + MATCHER_NAMES.firstIndex(of: matcher)!)
      let runs = randomRuns(random, matcher)
      let editor = try makeEditor(matcher)
      let label = "seed \(seed) \(runs)"
      if let error = thrown({ try setUp(editor, runs) }) {
        failures.append("\(label): \(error)")
        continue
      }
      let parsed = try read(editor)
      if let again = thrown({ try editor.update(discrete: true) { try markAll() } }) {
        failures.append("\(label) again: \(again)")
      } else if try read(editor) != parsed {
        failures.append("\(label) again: \(try read(editor))")
      }
    }
    #expect(failures == [])
  }

  // #9637's shape: a node with random text between two nodes of other
  // formats whose text has no match of its own, and starts none with it
  // (a hashtag's `#` would). Typing or deleting anywhere in it settles on
  // the entities a fresh parse of the result finds.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random edits between two formatted nodes settle on a fresh parse"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random edits between two formatted nodes settle on a fresh parse"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random edits between two formatted nodes settle on a fresh parse"
    ),
    arguments: MATCHER_NAMES)
  fileprivate func randomEditsBetweenFormattedNodes(_ matcher: Matcher) throws {
    let neighbours = ["x", "y", " ", "!", "😀", "a", "xa", " !"] + (matcher == .tag ? [] : ["#"])
    var failures: [String] = []
    var seed = 0
    while seed < SEEDS && failures.count < 5 {
      defer { seed += 1 }
      let random = rng(seed * 7 + 3 + MATCHER_NAMES.firstIndex(of: matcher)!)
      func pick<T>(_ xs: [T]) -> T { xs[Int(random() * Double(xs.count))] }
      let middle = randomRuns(random, matcher)[0]
      let runs = [
        pick(Array(FORMATS.dropFirst())) + pick(neighbours),
        parseRun(middle).text,
        pick(Array(FORMATS.dropFirst())) + pick(neighbours),
      ]
      let editor = try makeEditor(matcher)
      let label0 = "seed \(seed) \(runs)"
      if let setUpError = thrown({ try setUp(editor, runs) }) {
        failures.append("\(label0): \(setUpError)")
        continue
      }
      let nodes = try read(editor)
      // An edit in one of the nodes the middle text became.
      let index = 1 + Int(random() * Double(nodes.count - 2))
      let size = nodes[index].text.utf16.count
      let typing = size == 0 || random() < 0.7
      let offset = Int(random() * Double(typing ? size + 1 : size))
      let typed = pick(TYPED)
      let label =
        "\(label0) \(nodes) \(typing ? "type \(typed.debugDescription)" : "delete") at \(index):\(offset)"
      let error = thrown {
        try editor.update(discrete: true) {
          let node = try textAt(index)
          if typing {
            try select(node, offset).insertText(typed)
          } else {
            _ = try node.spliceText(offset, 1, "", moveSelection: false)
          }
        }
      }
      if let error {
        failures.append("\(label): \(error)")
        continue
      }
      let out = try read(editor)
      let fresh = try freshParse(toRuns(out), matcher)
      if out != fresh {
        failures.append("\(label): \(out) != \(fresh)")
      }
    }
    #expect(failures == [])
  }

  // Any edit (typing or deleting anywhere, or bolding a node) settles, and
  // so does transforming every node again.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > keyword: 2000 random sessions of 4 edits anywhere settle"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > tag: 2000 random sessions of 4 edits anywhere settle"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity on random layouts > lookbehind: 2000 random sessions of 4 edits anywhere settle"
    ),
    arguments: MATCHER_NAMES)
  fileprivate func randomSessionsSettle(_ matcher: Matcher) throws {
    var failures: [String] = []
    var seed = 0
    while seed < SEEDS && failures.count < 5 {
      defer { seed += 1 }
      let random = rng(seed * 7 + 5 + MATCHER_NAMES.firstIndex(of: matcher)!)
      func pick<T>(_ xs: [T]) -> T { xs[Int(random() * Double(xs.count))] }
      let runs = randomRuns(random, matcher)
      let editor = try makeEditor(matcher)
      if thrown({ try setUp(editor, runs) }) != nil {
        continue
      }
      for edit in 0..<4 {
        let nodes = try read(editor)
        if nodes.isEmpty {
          break
        }
        let index = Int(random() * Double(nodes.count))
        let size = nodes[index].text.utf16.count
        let op = random()
        let offset = Int(random() * Double(size + 1))
        let typed = pick(TYPED)
        let label =
          "seed \(seed) edit \(edit) \(nodes) op \(String(format: "%.2f", op)) at \(index):\(offset) \(typed.debugDescription)"
        let error = thrown {
          try editor.update(discrete: true) {
            let node = try textAt(index)
            if op < 0.6 {
              try select(node, offset).insertText(typed)
            } else if op < 0.85 {
              if size > 0 {
                _ = try node.spliceText(min(offset, size - 1), 1, "", moveSelection: false)
              }
            } else {
              _ = try node.toggleFormat("bold")
            }
          }
        }
        let again = error ?? thrown({ try editor.update(discrete: true) { try markAll() } })
        if let again {
          failures.append("\(label): \(again)")
          break
        }
      }
    }
    #expect(failures == [])
  }
}
