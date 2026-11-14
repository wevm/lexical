import Lexical
import LexicalExtension
import LexicalHashtag
import LexicalLink
import LexicalText
import Testing

/// Replays `vectors/entities/*.json`: `@lexical/hashtag`'s default matcher,
/// the documents its text entity transforms make, and `@lexical/link`'s
/// autolink matchers.
@Suite(.enabled(if: Vectors.available))
struct EntitiesTests {
  struct MatchCase: Decodable, CustomTestStringConvertible, Sendable {
    struct Match: Decodable, Sendable {
      let start: Int
      let end: Int
    }
    let text: String
    let match: Match?
    var testDescription: String { text }
  }

  struct Piece: Decodable, Sendable {
    let text: String
    let format: Int?
  }

  struct DocumentCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let pieces: [Piece]
    let json: String
    var testDescription: String { name }
  }

  struct TypingCase: Decodable, CustomTestStringConvertible, Sendable {
    let text: String
    let json: String
    let selection: [Int]
    var testDescription: String { text }
  }

  struct AutoLinkMatchCase: Decodable, CustomTestStringConvertible, Sendable {
    struct Match: Decodable, Equatable, Sendable {
      let index: Int
      let length: Int
      let text: String
      let url: String
    }
    let text: String
    let url: Match?
    let email: Match?
    var testDescription: String { text }
  }

  static let matchCases: [MatchCase] = (try? Vectors.load("entities/hashtag-match")) ?? []
  static let documentCases: [DocumentCase] =
    (try? Vectors.load("entities/hashtag-documents")) ?? []
  static let typingCases: [TypingCase] = (try? Vectors.load("entities/hashtag-typing")) ?? []
  static let autoLinkMatchCases: [AutoLinkMatchCase] =
    (try? Vectors.load("entities/autolink-match")) ?? []

  @Test(arguments: autoLinkMatchCases)
  func autoLinkMatch(_ vector: AutoLinkMatchCase) {
    func match(_ result: LinkMatcherResult?) -> AutoLinkMatchCase.Match? {
      result.map { .init(index: $0.index, length: $0.length, text: $0.text, url: $0.url) }
    }
    #expect(match(autoLinkUrlMatcher(vector.text)) == vector.url)
    #expect(match(autoLinkEmailMatcher(vector.text)) == vector.email)
  }

  @Test(arguments: matchCases)
  func hashtagMatch(_ vector: MatchCase) {
    let match = HashtagConfig().getHashtagMatch(vector.text)
    #expect(match?.start == vector.match?.start)
    #expect(match?.end == vector.match?.end)
  }

  private func build() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "conformance-hashtag", dependencies: [HashtagExtension],
        editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
  }

  @Test(arguments: documentCases)
  func hashtagDocument(_ vector: DocumentCase) throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          contentsOf: vector.pieces.map { try createTextNode($0.text).setFormat($0.format ?? 0) }
        ))
    }
    #expect(try editor.getEditorState().toJSON().stringify() == vector.json)
  }

  @Test(arguments: typingCases)
  func hashtagTyping(_ vector: TypingCase) throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(createParagraphNode()).selectEnd()
    }
    for character in vector.text {
      try editor.update(discrete: true) {
        if let selection = try getSelection() as? RangeSelection {
          try selection.insertText(String(character))
        }
      }
    }
    let selection = try editor.read { () -> [Int] in
      guard let s = try getSelection() as? RangeSelection else { return [] }
      let texts = try getRoot().getAllTextNodes()
      func index(_ key: NodeKey) -> Int { texts.firstIndex { $0.getKey() == key } ?? -1 }
      return [index(s.anchor.key), s.anchor.offset, index(s.focus.key), s.focus.offset]
    }
    #expect(try editor.getEditorState().toJSON().stringify() == vector.json)
    #expect(selection == vector.selection)
  }
}
