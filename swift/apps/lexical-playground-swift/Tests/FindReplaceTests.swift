import Lexical
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import Testing

@testable import LexicalPlayground

// Port of packages/lexical-playground/__tests__/unit/FindReplace.test.ts:
// the search, offset map and replace functions of FindReplaceExtension,
// and its commands and signals.

/// `TestDecoratorNode`: a block decorator whose text is `URL`.
private final class TestDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestDecoratorNode.self, type: "test-decorator", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
  override func getTextContent() throws -> String { "URL" }
}

private func createTestDecoratorNode() throws -> TestDecoratorNode {
  try create(TestDecoratorNode.self)
}

/// `SlotDecoratorNode`: a block decorator with the default text, its
/// slots' text.
private final class SlotDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SlotDecoratorNode.self, type: "slot-decorator", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private func createSlotDecoratorNode() throws -> SlotDecoratorNode {
  try create(SlotDecoratorNode.self)
}

/// An editor with upstream's `TestExtension`: rich text, lists, links and
/// the two decorators.
@MainActor
private func makeTestEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[test-find-replace]",
      dependencies: [RichTextExtension, FindReplaceExtension, ListExtension, LinkExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(SlotDecoratorNode.self)] }))
}

/// An offset entry's `[globalStart, globalEnd]`, for `toMatchObject`.
private func span(_ entry: OffsetEntry) -> [Int] { [entry.globalStart, entry.globalEnd] }

/// A match as the cases write it.
struct ExpectedMatch: Sendable {
  var end: Int
  var matchText: String
  var start: Int

  var textMatch: TextMatch { TextMatch(end: end, matchText: matchText, start: start) }
}

struct FindMatchesCase: Sendable, CustomTestStringConvertible {
  var caseSensitive = false
  var expected: [ExpectedMatch]
  var isRegex = false
  var label: String
  var term: String
  var text: String

  var testDescription: String { label }
}

@MainActor
struct FindMatchesTests {
  /// `$label`
  @Test(
    arguments: [
      FindMatchesCase(expected: [], label: "empty search term", term: "", text: "hello world"),
      FindMatchesCase(expected: [], label: "empty text", term: "hello", text: ""),
      FindMatchesCase(
        expected: [ExpectedMatch(end: 11, matchText: "world", start: 6)],
        label: "single match with matchText", term: "world", text: "hello world"),
      FindMatchesCase(
        expected: [
          ExpectedMatch(end: 3, matchText: "foo", start: 0),
          ExpectedMatch(end: 11, matchText: "foo", start: 8),
          ExpectedMatch(end: 19, matchText: "foo", start: 16),
        ],
        label: "multiple non-overlapping matches", term: "foo", text: "foo bar foo baz foo"),
      FindMatchesCase(
        expected: [
          ExpectedMatch(end: 5, matchText: "Hello", start: 0),
          ExpectedMatch(end: 11, matchText: "HELLO", start: 6),
          ExpectedMatch(end: 17, matchText: "hello", start: 12),
        ],
        label: "case-insensitive preserves original case in matchText", term: "hello",
        text: "Hello HELLO hello"),
      FindMatchesCase(
        caseSensitive: true, expected: [ExpectedMatch(end: 17, matchText: "hello", start: 12)],
        label: "case-sensitive search", term: "hello", text: "Hello HELLO hello"),
      FindMatchesCase(
        expected: [
          ExpectedMatch(end: 6, matchText: "123", start: 3),
          ExpectedMatch(end: 12, matchText: "456", start: 9),
        ],
        isRegex: true, label: "regex: basic pattern", term: #"\d+"#, text: "foo123bar456"),
      FindMatchesCase(
        expected: [ExpectedMatch(end: 7, matchText: "2026-07", start: 0)], isRegex: true,
        label: "regex: with groups uses full match range", term: #"(\d{4})-(\d{2})"#,
        text: "2026-07-03"),
      FindMatchesCase(
        expected: [], isRegex: true, label: "regex: invalid regex returns empty",
        term: "[invalid", text: "hello"),
      FindMatchesCase(
        expected: [ExpectedMatch(end: 15, matchText: "$10.00", start: 9)],
        label: "non-regex escapes special characters", term: "$10.00",
        text: "price is $10.00"),
      FindMatchesCase(
        expected: [ExpectedMatch(end: 3, matchText: "abc", start: 0)],
        label: "matches at string boundaries", term: "abc", text: "abc"),
      FindMatchesCase(
        expected: [ExpectedMatch(end: 5, matchText: "hello", start: 0)],
        label: "matches at start of string", term: "hello", text: "hello world"),
      FindMatchesCase(
        expected: [], label: "no match returns empty", term: "xyz", text: "hello world"),
      FindMatchesCase(
        expected: [], isRegex: true, label: "skips zero-length regex matches", term: "(?=a)",
        text: "abc"),
    ])
  func findsMatches(_ testCase: FindMatchesCase) {
    #expect(
      findMatches(testCase.text, testCase.term, testCase.caseSensitive, testCase.isRegex)
        == testCase.expected.map(\.textMatch))
  }
}

@MainActor
struct FindMatchesZeroLengthRegexEdgeCasesTests {
  /// `$label`
  @Test(
    arguments: [
      FindMatchesCase(
        expected: [ExpectedMatch(end: 3, matchText: "abc", start: 0)],
        label: ".* matches entire text as single match", term: ".*", text: "abc"),
      FindMatchesCase(
        expected: [
          ExpectedMatch(end: 2, matchText: "aa", start: 0),
          ExpectedMatch(end: 5, matchText: "aa", start: 3),
        ],
        label: "a* matches non-empty runs only", term: "a*", text: "aabaa"),
      FindMatchesCase(
        expected: [], label: "x? on text without x returns no matches", term: "x?", text: "abc"),
    ])
  func findsMatchesWithZeroLengthRegexes(_ testCase: FindMatchesCase) {
    #expect(
      findMatches(testCase.text, testCase.term, false, true)
        == testCase.expected.map(\.textMatch))
  }
}

struct ExpandReplacementCase: Sendable, CustomTestStringConvertible {
  var expected: String
  var label: String
  var matchText: String
  /// The source of the case's regex, without one when `nil`.
  var regex: String?
  var template: String

  var testDescription: String { label }
}

@MainActor
struct ExpandReplacementTests {
  /// `$label`
  @Test(
    arguments: [
      ExpandReplacementCase(
        expected: "$1", label: "returns template as-is when no regex", matchText: "foo",
        regex: nil, template: "$1"),
      ExpandReplacementCase(
        expected: "2026-07", label: "expands $1/$2 capture groups", matchText: "2026-07",
        regex: #"(\d{4})-(\d{2})"#, template: "$1-$2"),
      ExpandReplacementCase(
        expected: "year=2026", label: "expands named capture groups with different template",
        matchText: "2026-07", regex: #"(\d{4})-(\d{2})"#, template: "year=$1"),
      ExpandReplacementCase(
        expected: "[foo]", label: "expands $& for full match", matchText: "foo", regex: "foo",
        template: "[$&]"),
      ExpandReplacementCase(
        expected: "host/user", label: "swaps groups", matchText: "user@host",
        regex: #"(\w+)@(\w+)"#, template: "$2/$1"),
      ExpandReplacementCase(
        expected: "$1", label: "literal $1 with no groups in regex", matchText: "hello",
        regex: "hello", template: "$1"),
    ])
  func expandsReplacements(_ testCase: ExpandReplacementCase) {
    let regex = testCase.regex.map { JSRegExp($0) }
    #expect(
      expandReplacement(testCase.template, testCase.matchText, regex) == testCase.expected)
  }
}

@MainActor
struct BuildOffsetMapTests {
  /// maps single paragraph with one TextNode
  @Test func mapsSingleParagraphWithOneTextNode() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("hello"))
      try getRoot().clear().append(p)
    }
    let map = try editor.read { try buildOffsetMap() }
    try #require(map.count == 1)
    #expect(map[0].globalStart == 0)
    #expect(map[0].globalEnd == 5)
  }

  /// maps paragraph with multiple TextNodes (bold boundary)
  @Test func mapsParagraphWithMultipleTextNodesBoldBoundary() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let plain = try createTextNode("hel")
      let bold = try createTextNode("lo")
      try bold.setFormat(IS_BOLD)
      try p.append(plain, bold)
      try getRoot().clear().append(p)
    }
    let map = try editor.read { try buildOffsetMap() }
    try #require(map.count == 2)
    #expect(map[0].globalStart == 0)
    #expect(map[0].globalEnd == 3)
    #expect(map[1].globalStart == 3)
    #expect(map[1].globalEnd == 5)
  }

  /// maps across multiple paragraphs with double line break offset
  @Test func mapsAcrossMultipleParagraphsWithDoubleLineBreakOffset() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p1 = try createParagraphNode()
      try p1.append(createTextNode("abc"))
      let p2 = try createParagraphNode()
      try p2.append(createTextNode("def"))
      try getRoot().clear().append(p1, p2)
    }
    let map = try editor.read { try buildOffsetMap() }
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 3])
    #expect(span(map[1]) == [5, 8])
  }

  /// maps one block node inside a slot without extra offset
  @Test func mapsOneBlockNodeInsideASlotWithoutExtraOffset() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slotP = try createParagraphNode()
      try slotP.append(createTextNode("hello"))
      try setSlot(host, "mySlot", slotP)
      try getRoot().clear().append(host)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    try #require(map.count == 1)
    #expect(span(map[0]) == [0, 5])
    #expect(text == "hello")
  }

  /// maps multiple block slots without extra offset between them
  @Test func mapsMultipleBlockSlotsWithoutExtraOffsetBetweenThem() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot1 = try createParagraphNode()
      try slot1.append(createTextNode("abc"))
      let slot2 = try createParagraphNode()
      try slot2.append(createTextNode("def"))
      try setSlot(host, "slot1", slot1)
      try setSlot(host, "slot2", slot2)
      try getRoot().clear().append(host)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 3])
    #expect(span(map[1]) == [3, 6])
    #expect(text == "abcdef")
  }

  /// maps slot content followed by normal children without extra offset
  @Test func mapsSlotContentFollowedByNormalChildrenWithoutExtraOffset() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createParagraphNode()
      try slot.append(createTextNode("slot"))
      try setSlot(host, "mySlot", slot)
      try host.append(createTextNode("child"))
      try getRoot().clear().append(host)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 4])
    #expect(span(map[1]) == [4, 9])
    #expect(text == "slotchild")
  }

  /// maps across DecoratorNode without extra offset loss
  @Test func mapsAcrossDecoratorNodeWithoutExtraOffsetLoss() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p1 = try createParagraphNode()
      try p1.append(createTextNode("A"))
      let dec = try createTestDecoratorNode()
      let p2 = try createParagraphNode()
      try p2.append(createTextNode("B"))
      try getRoot().clear().append(p1, dec, p2)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 1])
    #expect(span(map[1]) == [6, 7])
    #expect(text == "A\n\nURLB")
  }

  /// maps across DecoratorNode with block slot content without extra offset loss
  @Test func mapsAcrossDecoratorNodeWithBlockSlotContentWithoutExtraOffsetLoss() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p1 = try createParagraphNode()
      try p1.append(createTextNode("A"))
      let dec = try createSlotDecoratorNode()
      let slotP = try createParagraphNode()
      try slotP.append(createTextNode("S"))
      try setSlot(dec, "mySlot", slotP)
      let p2 = try createParagraphNode()
      try p2.append(createTextNode("B"))
      try getRoot().clear().append(p1, dec, p2)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    try #require(map.count == 3)
    #expect(span(map[0]) == [0, 1])
    #expect(span(map[1]) == [3, 4])  // S
    #expect(span(map[2]) == [4, 5])  // B
    #expect(text == "A\n\nSB")
  }

  /// handles empty editor
  @Test func handlesEmptyEditor() throws {
    let editor = try makeTestEditor()
    let map = try editor.read { try buildOffsetMap() }
    #expect(map == [])
  }

  /// accounts for LineBreakNode in offset calculation
  @Test func accountsForLineBreakNodeInOffsetCalculation() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("hello"), createLineBreakNode(), createTextNode("world"))
      try getRoot().clear().append(p)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    #expect(text == "hello\nworld")
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 5])
    #expect(span(map[1]) == [6, 11])
  }

  /// handles empty paragraph between text paragraphs
  @Test func handlesEmptyParagraphBetweenTextParagraphs() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p1 = try createParagraphNode()
      try p1.append(createTextNode("abc"))
      let p2 = try createParagraphNode()
      let p3 = try createParagraphNode()
      try p3.append(createTextNode("def"))
      try getRoot().clear().append(p1, p2, p3)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    #expect(text == "abc\n\n\n\ndef")
    try #require(map.count == 2)
    #expect(span(map[0]) == [0, 3])
    #expect(span(map[1]) == [7, 10])
  }

  /// maps list items without extra offset for parent→child
  @Test func mapsListItemsWithoutExtraOffsetForParentToChild() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("before"))
      let list = try createListNode("bullet")
      try list.append(
        createListItemNode().append(createTextNode("aaa")),
        createListItemNode().append(createTextNode("bbb")))
      try getRoot().clear().append(p, list)
    }
    let (map, text) = try editor.read { (try buildOffsetMap(), try getRoot().getTextContent()) }
    #expect(text == "before\n\naaa\n\nbbb")
    try #require(map.count == 3)
    #expect(span(map[0]) == [0, 6])
    #expect(span(map[1]) == [8, 11])
    #expect(span(map[2]) == [13, 16])
  }
}

@MainActor
struct ResolveMatchToPointsTests {
  /// single-node match returns correct points with format
  @Test func singleNodeMatchReturnsCorrectPointsWithFormat() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let text = try createTextNode("hello world")
      try text.setFormat(IS_BOLD)
      try p.append(text)
      try getRoot().clear().append(p)
    }
    let result = try editor.read {
      let map = try buildOffsetMap()
      return try resolveMatchToPoints(TextMatch(end: 5, matchText: "hello", start: 0), map)
    }
    let points = try #require(result, "expected non-null match points")
    #expect(points.anchorKey == points.focusKey)
    #expect(points.anchorOffset == 0)
    #expect(points.focusOffset == 5)
    #expect(points.format == IS_BOLD)
  }

  /// cross-node match returns different keys with format 0
  @Test func crossNodeMatchReturnsDifferentKeysWithFormat0() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let plain = try createTextNode("hel")
      let bold = try createTextNode("lo")
      try bold.setFormat(IS_BOLD)
      try p.append(plain, bold)
      try getRoot().clear().append(p)
    }
    let result = try editor.read {
      let map = try buildOffsetMap()
      return try resolveMatchToPoints(TextMatch(end: 5, matchText: "hello", start: 0), map)
    }
    let points = try #require(result, "expected non-null match points")
    #expect(points.anchorKey != points.focusKey)
    #expect(points.anchorOffset == 0)
    #expect(points.focusOffset == 2)
    #expect(points.format == 0)
  }

  /// match falling in paragraph boundary gap returns null
  @Test func matchFallingInParagraphBoundaryGapReturnsNull() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p1 = try createParagraphNode()
      try p1.append(createTextNode("abc"))
      let p2 = try createParagraphNode()
      try p2.append(createTextNode("def"))
      try getRoot().clear().append(p1, p2)
    }
    let result = try editor.read {
      let map = try buildOffsetMap()
      return try resolveMatchToPoints(TextMatch(end: 5, matchText: "c\n\nd", start: 3), map)
    }
    #expect(result == nil)
  }
}

@MainActor
struct ReplaceMatchTests {
  /// Replaces `match` in the document with `replacement`.
  private func replace(
    _ editor: Editor, _ match: TextMatch, _ replacement: String, _ regex: JSRegExp?
  ) throws {
    try editor.update(discrete: true) {
      let map = try buildOffsetMap()
      let points = try #require(
        try resolveMatchToPoints(match, map), "expected non-null match points")
      try replaceMatch(points, replacement, match, regex)
    }
  }

  /// single-node replace preserves text format
  @Test func singleNodeReplacePreservesTextFormat() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let text = try createTextNode("hello world")
      try text.setFormat(IS_BOLD)
      try p.append(text)
      try getRoot().clear().append(p)
    }
    try replace(editor, TextMatch(end: 5, matchText: "hello", start: 0), "goodbye", nil)
    try editor.read {
      #expect(try getRoot().getTextContent() == "goodbye world")
      let p = try #require(try getRoot().getFirstChild() as? ElementNode, "expected ElementNode")
      let firstChild = try #require(try p.getFirstChild() as? TextNode, "expected TextNode")
      #expect(try firstChild.getFormat() == IS_BOLD)
    }
  }

  /// multi-node replace produces plain text
  @Test func multiNodeReplaceProducesPlainText() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let plain = try createTextNode("hel")
      let bold = try createTextNode("lo")
      try bold.setFormat(IS_BOLD)
      try p.append(plain, bold)
      try getRoot().clear().append(p)
    }
    try replace(editor, TextMatch(end: 5, matchText: "hello", start: 0), "hi", nil)
    #expect(try editor.read { try getRoot().getTextContent() } == "hi")
  }

  /// replace with empty string deletes match
  @Test func replaceWithEmptyStringDeletesMatch() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("hello world"))
      try getRoot().clear().append(p)
    }
    try replace(editor, TextMatch(end: 6, matchText: "hello ", start: 0), "", nil)
    #expect(try editor.read { try getRoot().getTextContent() } == "world")
  }

  /// regex capture group replacement with $1/$2
  @Test func regexCaptureGroupReplacementWithDollar1AndDollar2() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("user@host"))
      try getRoot().clear().append(p)
    }
    try replace(
      editor, TextMatch(end: 9, matchText: "user@host", start: 0), "$2/$1",
      JSRegExp(#"(\w+)@(\w+)"#))
    #expect(try editor.read { try getRoot().getTextContent() } == "host/user")
  }

  /// non-regex mode ignores $1 in replacement
  @Test func nonRegexModeIgnoresDollar1InReplacement() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode("hello world"))
      try getRoot().clear().append(p)
    }
    try replace(editor, TextMatch(end: 5, matchText: "hello", start: 0), "$1", nil)
    #expect(try editor.read { try getRoot().getTextContent() } == "$1 world")
  }

  /// single-node replace inside LinkNode preserves link
  @Test func singleNodeReplaceInsideLinkNodePreservesLink() throws {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      let link = try createLinkNode("https://example.com")
      try link.append(createTextNode("repository"))
      try p.append(createTextNode("visit "), link, createTextNode(" now"))
      try getRoot().clear().append(p)
    }
    try replace(editor, TextMatch(end: 16, matchText: "repository", start: 6), "repo", nil)
    try editor.read {
      #expect(try getRoot().getTextContent() == "visit repo now")
      let p = try #require(try getRoot().getFirstChild() as? ElementNode, "expected ElementNode")
      let children = try p.getChildren()
      try #require(children.count == 3)
      #expect(isLinkNode(children[1]), "expected LinkNode")
      #expect(try children[1].getTextContent() == "repo")
    }
  }
}

@MainActor
struct ReplaceAllMatchesTests {
  /// An editor whose document is a paragraph of `text`.
  private func makeEditor(_ text: String) throws -> Editor {
    let editor = try makeTestEditor()
    try editor.update(discrete: true) {
      let p = try createParagraphNode()
      try p.append(createTextNode(text))
      try getRoot().clear().append(p)
    }
    return editor
  }

  /// replaces all matches in reverse order
  @Test func replacesAllMatchesInReverseOrder() throws {
    let editor = try makeEditor("foo bar foo")
    var count = 0
    try editor.update(discrete: true) {
      let map = try buildOffsetMap()
      let matches = findMatches("foo bar foo", "foo", false, false)
      count = try replaceAllMatches(matches, map, "baz", nil)
    }
    #expect(count == 2)
    #expect(try editor.read { try getRoot().getTextContent() } == "baz bar baz")
  }

  /// replaces all with shorter replacement
  @Test func replacesAllWithShorterReplacement() throws {
    let editor = try makeEditor("foo bar foo")
    var count = 0
    try editor.update(discrete: true) {
      let text = try getRoot().getTextContent()
      let map = try buildOffsetMap()
      let matches = findMatches(text, "foo", false, false)
      count = try replaceAllMatches(matches, map, "x", nil)
    }
    #expect(count == 2)
    #expect(try editor.read { try getRoot().getTextContent() } == "x bar x")
  }

  /// replaces all with longer replacement
  @Test func replacesAllWithLongerReplacement() throws {
    let editor = try makeEditor("foo bar foo")
    var count = 0
    try editor.update(discrete: true) {
      let text = try getRoot().getTextContent()
      let map = try buildOffsetMap()
      let matches = findMatches(text, "foo", false, false)
      count = try replaceAllMatches(matches, map, "quxxx", nil)
    }
    #expect(count == 2)
    #expect(try editor.read { try getRoot().getTextContent() } == "quxxx bar quxxx")
  }

  /// replaces all with regex capture groups
  @Test func replacesAllWithRegexCaptureGroups() throws {
    let editor = try makeEditor("a@b and c@d")
    var count = 0
    try editor.update(discrete: true) {
      let text = try getRoot().getTextContent()
      let map = try buildOffsetMap()
      let matches = findMatches(text, #"(\w+)@(\w+)"#, false, true)
      let regex = JSRegExp(#"(\w+)@(\w+)"#)
      count = try replaceAllMatches(matches, map, "$2/$1", regex)
    }
    #expect(count == 2)
    #expect(try editor.read { try getRoot().getTextContent() } == "b/a and d/c")
  }
}

@MainActor
struct FindReplaceExtensionCommandDispatchIntegrationTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/FindReplaceTests",
        dependencies: [RichTextExtension, FindReplaceExtension]))
  }

  private func findReplaceOutput(_ editor: Editor) throws -> FindReplaceOutput {
    try getExtensionDependencyFromEditor(editor, FindReplaceExtension).output
  }

  /// Makes the document a paragraph of `text`.
  private func setText(_ editor: Editor, _ text: String) throws {
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
    }
  }

  /// TOGGLE_FIND_REPLACE_COMMAND toggles isOpen signal
  @Test func toggleFindReplaceCommandTogglesIsOpenSignal() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    #expect(output.isOpen.peek() == false)
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    #expect(output.isOpen.peek() == true)
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    #expect(output.isOpen.peek() == false)
  }

  /// CLOSE_FIND_REPLACE_COMMAND sets isOpen to false
  @Test func closeFindReplaceCommandSetsIsOpenToFalse() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    #expect(output.isOpen.peek() == true)
    try editor.dispatchCommand(CLOSE_FIND_REPLACE_COMMAND)
    #expect(output.isOpen.peek() == false)
  }

  /// matches computed signal updates when searchTerm and text change
  @Test func matchesComputedSignalUpdatesWhenSearchTermAndTextChange() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "hello world hello")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "hello"
    let matches = output.matches.peek()
    try #require(matches.count == 2)
    #expect(matches[0].matchText == "hello")
    #expect(matches[0].start == 0)
  }

  /// FIND_NEXT_COMMAND / FIND_PREV_COMMAND cycle currentIndex
  @Test func findNextAndFindPrevCommandsCycleCurrentIndex() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "a b a b a")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "a"
    #expect(output.matches.peek().count == 3)
    #expect(output.currentIndex.peek() == 0)

    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 1)

    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 2)

    // wrap around
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 0)

    // backward wrap
    try editor.dispatchCommand(FIND_PREV_COMMAND)
    #expect(output.currentIndex.peek() == 2)
  }

  /// FIND_NEXT_COMMAND with zero matches is a no-op
  @Test func findNextCommandWithZeroMatchesIsANoOp() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "nonexistent"
    #expect(output.matches.peek().count == 0)
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 0)
    try editor.dispatchCommand(FIND_PREV_COMMAND)
    #expect(output.currentIndex.peek() == 0)
  }

  /// matches returns empty when isOpen is false
  @Test func matchesReturnsEmptyWhenIsOpenIsFalse() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "hello")
    output.searchTerm.value = "hello"
    #expect(output.isOpen.peek() == false)
    #expect(output.matches.peek() == [])
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    #expect(output.matches.peek().count == 1)
  }

  /// effectiveIndex clamps when match count decreases
  @Test func effectiveIndexClampsWhenMatchCountDecreases() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "a b a b a")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "a"
    #expect(output.matches.peek().count == 3)
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 2)
    output.searchTerm.value = "a b"
    #expect(output.matches.peek().count == 2)
    #expect(output.effectiveIndex.peek() == 1)
  }

  /// regexError computed returns true for invalid regex
  @Test func regexErrorComputedReturnsTrueForInvalidRegex() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.isRegex.value = true
    output.searchTerm.value = "[invalid"
    #expect(output.regexError.peek() == true)
    output.searchTerm.value = #"\d+"#
    #expect(output.regexError.peek() == false)
  }

  /// SET_SEARCH_TERM_COMMAND resets currentIndex to 0
  @Test func setSearchTermCommandResetsCurrentIndexTo0() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "a b a b a")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "a"
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 2)
    try editor.dispatchCommand(SET_SEARCH_TERM_COMMAND, "b")
    #expect(output.currentIndex.peek() == 0)
    #expect(output.searchTerm.peek() == "b")
  }

  /// TOGGLE_CASE_SENSITIVE / TOGGLE_REGEX reset currentIndex to 0
  @Test func toggleCaseSensitiveAndToggleRegexResetCurrentIndexTo0() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "a b a b a")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "a"
    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 1)
    try editor.dispatchCommand(TOGGLE_CASE_SENSITIVE_COMMAND)
    #expect(output.currentIndex.peek() == 0)
    #expect(output.caseSensitive.peek() == true)

    try editor.dispatchCommand(FIND_NEXT_COMMAND)
    #expect(output.currentIndex.peek() == 1)
    try editor.dispatchCommand(TOGGLE_REGEX_COMMAND)
    #expect(output.currentIndex.peek() == 0)
    #expect(output.isRegex.peek() == true)
  }

  /// REPLACE_CURRENT_COMMAND replaces the current match
  @Test func replaceCurrentCommandReplacesTheCurrentMatch() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "foo bar foo")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "foo"
    output.replaceTerm.value = "baz"
    #expect(output.matches.peek().count == 2)
    try editor.dispatchCommand(REPLACE_CURRENT_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "baz bar foo")
  }

  /// REPLACE_ALL_COMMAND replaces all matches
  @Test func replaceAllCommandReplacesAllMatches() throws {
    let editor = try makeEditor()
    let output = try findReplaceOutput(editor)
    try setText(editor, "foo bar foo")
    try editor.dispatchCommand(TOGGLE_FIND_REPLACE_COMMAND)
    output.searchTerm.value = "foo"
    output.replaceTerm.value = "qux"
    #expect(output.matches.peek().count == 2)
    try editor.dispatchCommand(REPLACE_ALL_COMMAND)
    #expect(try editor.read { try getRoot().getTextContent() } == "qux bar qux")
  }
}
