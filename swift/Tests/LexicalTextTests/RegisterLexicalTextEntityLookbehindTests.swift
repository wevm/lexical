import Lexical
import LexicalHashtag
import LexicalText
import Testing

// Port of
// packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts.

private final class Letter: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(Letter.self, type: "letter", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

nonisolated(unsafe) private let note = createState("note", parse: { $0?.stringValue ?? "" })

// #9725 (known limit, see transforms.md): typing into a run of repeated
// characters, read as text inserted at the end of the run, drops the NodeState
// of the last existing character when a matcher that looks back into the run
// makes it an entity. `test.fails` upstream until the edit's reading is fixed.

// Matches an "A" only when "$AA" comes right before it.
private let getMatch = regexMatch(#"^x$|^A$|(?<=^#)x|(?<=\$AA)A"#)

private struct Char: Equatable {
  var note: String
  var text: String
}

// Types an "A" into "$AA" right after "$" and returns the characters with
// their NodeState.
private func typeIntoRun() throws -> [Char] {
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(HashtagNode.self), .klass(Letter.self)], onError: { throw $0 }))
  _ = try registerLexicalTextEntity(
    editor, HashtagConfig().getHashtagMatch, HashtagNode.self
  ) { try createHashtagNode($0.getTextContent()) }
  _ = try registerLexicalTextEntity(editor, getMatch, Letter.self) {
    try create(Letter.self).setTextContent($0.getTextContent())
  }
  try editor.update(discrete: true) {
    try getRoot().append(
      createParagraphNode().append(
        createTextNode("y").setFormat("bold"),
        createTextNode("a"),
        createHashtagNode("#x"),
        setState(createTextNode("$AA"), note, "c1")))
  }
  try editor.update(discrete: true) {
    try getRoot().getAllTextNodes().first { try $0.getTextContent() == "$AA" }!
      .select(1, 1)
      .insertText("A")
  }
  let nodes = try editor.read {
    try getRoot().getAllTextNodes().map {
      (note: try getState($0, note), text: try $0.getTextContent())
    }
  }
  // `text.split('')`: one entry per UTF-16 code unit (all ASCII here)
  return nodes.flatMap { node in node.text.map { Char(note: node.note, text: String($0)) } }
}

@Suite struct RegisterLexicalTextEntityLookbehindTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts::typing into a run of repeated characters keeps the text"
    ))
  func keepsTheText() throws {
    // the typed A is at index 5
    #expect(try typeIntoRun().map(\.text).joined() == "ya#x$AAA")
  }

  // `test.fails` upstream: the assertion is expected to fail (#9725), so it
  // runs inside `withKnownIssue`, and this test passes only while it does.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts::typing into a run of repeated characters keeps NodeState on an existing character"
    ))
  func keepsNodeStateOnAnExistingCharacter() {
    withKnownIssue("#9725") {
      // the A that was last before typing is at index 7
      let chars = try typeIntoRun()
      #expect(chars.count > 7 && chars[7] == Char(note: "c1", text: "A"))
    }
  }
}
