import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts.
//
// A format toggled with nothing selected is armed for the *next* insertion
// and is deliberately not backed by any node yet. Edits reposition the caret
// through ElementNode.select()/TextNode.select() as an implementation detail
// (Enter moves it into the block `insertNewAfter` just created, paste moves
// it to the end of the pasted content), so anything that re-derives the
// format from the node those land on silently cancels what the user armed.

private let ext = defineExtension(
  name: "@test/armed-format", dependencies: [RichTextExtension, ListExtension])

private func arm() throws -> RangeSelection {
  let selection = try #require(try getSelection() as? RangeSelection)
  try selection.formatText("bold")
  #expect(selection.format == IS_BOLD)
  return selection
}

private func armBoldThen(
  _ seed: @escaping () throws -> Void, _ op: @escaping (RangeSelection) throws -> Void
) throws -> Int {
  let editor = try buildEditorFromExtensions(ext)
  defer { editor.dispose() }
  try editor.update(discrete: true, seed)
  try editor.update(discrete: true) { try op(arm()) }
  return try editor.read { try #require(try getSelection() as? RangeSelection).format }
}

private func seedAtEndOf(_ make: @escaping () throws -> ElementNode) -> () throws -> Void {
  {
    let block = try make()
    try getRoot().clear().append(block)
    _ = try block.selectEnd()
  }
}

private func seedParagraph() throws {
  try seedAtEndOf { try createParagraphNode().append(createTextNode("abc")) }()
}

@Suite struct ArmedFormatSurvivesEditsTests {
  private func enterCases() -> [(String, () throws -> Void)] {
    [
      ("a heading", seedAtEndOf { try createHeadingNode("h1").append(createTextNode("abc")) }),
      ("a paragraph", seedParagraph),
      ("a quote", seedAtEndOf { try createQuoteNode().append(createTextNode("abc")) }),
      (
        "a list item",
        seedAtEndOf {
          try createListNode("bullet").append(createListItemNode().append(createTextNode("abc")))
        }
      ),
      (
        // Splitting mid-block moves the trailing TextNode into the new block,
        // so the caret lands on a node that carries its own (unbolded) format.
        "the middle of a paragraph",
        {
          let text = try createTextNode("abcdef")
          try getRoot().clear().append(createParagraphNode().append(text))
          _ = try text.select(3, 3)
        }
      ),
    ]
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a heading"
    ),
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a paragraph"
    ),
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a quote"
    ),
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in a list item"
    ),
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Enter in the middle of a paragraph"
    ))
  func enter() throws {
    for (kind, seed) in enterCases() {
      #expect(try armBoldThen(seed) { _ = try $0.insertParagraph() } == IS_BOLD, "Enter in \(kind)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > Shift+Enter"
    ))
  func shiftEnter() throws {
    #expect(try armBoldThen(seedParagraph) { try $0.insertLineBreak() } == IS_BOLD)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > pasting inline content"
    ))
  func pastingInlineContent() throws {
    #expect(
      try armBoldThen(seedParagraph) { try $0.insertNodes([try createTextNode("pasted")]) }
        == IS_BOLD)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > pasting a block"
    ))
  func pastingABlock() throws {
    #expect(
      try armBoldThen(seedParagraph) {
        try $0.insertNodes([try createParagraphNode().append(createTextNode("pasted"))])
      } == IS_BOLD)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/ArmedFormatSurvivesEdits.test.ts::a format armed on a collapsed caret survives an edit > the armed format reaches the text typed after Enter"
    ))
  func armedFormatReachesTypedText() throws {
    let editor = try buildEditorFromExtensions(ext)
    defer { editor.dispose() }
    try editor.update(discrete: true, seedParagraph)
    try editor.update(discrete: true) {
      let selection = try arm()
      _ = try selection.insertParagraph()
      try selection.insertText("typed")
    }
    try editor.read { () throws -> Void in
      let typed = try #require(try getRoot().getLastDescendant() as? TextNode)
      #expect(try typed.getTextContent() == "typed")
      #expect(try typed.getFormat() == IS_BOLD)
    }
  }
}
