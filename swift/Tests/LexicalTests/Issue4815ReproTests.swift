import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/Issue4815Repro.test.ts: pasting blocks after
// consecutive line breaks keeps the empty line and the pasted block.

private final class TestHeadingNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestHeadingNode.self, type: "test-heading", extends: ElementNode.self)
  }
}

private func editor() throws -> Editor {
  try createEditor(
    CreateEditorArgs(
      namespace: "test",
      nodes: [.klass(TestHeadingNode.self), .klass(TestInlineElementNode.self)],
      onError: { throw $0 }))
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

/// Builds the document, places the selection, and pastes `nodes` (by default
/// a heading followed by a paragraph) at it.
private func paste(
  _ editor: Editor, _ buildDocument: @escaping () throws -> [LexicalNode],
  _ placeSelection: @escaping () throws -> Void,
  nodes: @escaping () throws -> [LexicalNode] = {
    [
      try TestHeadingNode().append(try createTextNode("Heading 3")),
      try createParagraphNode().append(try createTextNode("Some paragraph")),
    ]
  }
) throws {
  try editor.update(discrete: true) {
    try getRoot().append(contentsOf: try buildDocument())
    try placeSelection()
    try rangeSelection().insertNodes(try nodes())
  }
}

/// The root children as `type("textContent")`.
private func describeRoot(_ editor: Editor) throws -> [String] {
  try editor.read {
    try getRoot().getChildren().map {
      "\($0.getType())(\(JSONValue.string(try $0.getTextContent()).stringify()))"
    }
  }
}

/// The collapsed selection as `"textContent"@offset`.
private func describeSelection(_ editor: Editor) throws -> String {
  try editor.read {
    let selection = try rangeSelection()
    #expect(selection.isCollapsed())
    return
      "\(JSONValue.string(try selection.anchor.getNode().getTextContent()).stringify())@\(selection.anchor.offset)"
  }
}

private func lines(_ tail: LexicalNode...) throws -> ParagraphNode {
  try createParagraphNode().append(
    contentsOf: [try createTextNode("Line of text"), try createLineBreakNode()] + tail)
}

@Suite struct Issue4815ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block at the end of a paragraph after consecutive line breaks"
    ))
  func pastedBlockAtEnd() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try lines(try createLineBreakNode())
        return [paragraph]
      }, { _ = try paragraph.select(3, 3) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraph")"#,
      ])
    #expect(try describeSelection(editor) == #""Some paragraph"@14"#)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block before text that follows consecutive line breaks"
    ))
  func pastedBlockBeforeText() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try lines(try createLineBreakNode(), try createTextNode("Paragraph 1"))
        return [paragraph]
      }, { _ = try paragraph.select(3, 3) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraphParagraph 1")"#,
      ])
    #expect(try describeSelection(editor) == #""Some paragraphParagraph 1"@14"#)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block for a text point at the start of the line"
    ))
  func textPointAtLineStart() throws {
    let editor = try editor()
    var text: TextNode!
    try paste(
      editor,
      {
        text = try createTextNode("Paragraph 1")
        return [try lines(try createLineBreakNode(), text)]
      }, { _ = try text.select(0, 0) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block inside an inline element at the start of the line"
    ))
  func insideInlineAtLineStart() throws {
    let editor = try editor()
    var text: TextNode!
    try paste(
      editor,
      {
        text = try createTextNode("Paragraph 1")
        return [
          try lines(try createLineBreakNode(), try createTestInlineElementNode().append(text))
        ]
      }, { _ = try text.select(0, 0) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves a pasted block when replacing a selection that starts after the empty line"
    ))
  func replacingSelectionAfterEmptyLine() throws {
    let editor = try editor()
    var text: TextNode!
    try paste(
      editor,
      {
        text = try createTextNode("Paragraph 1")
        return [try lines(try createLineBreakNode(), text)]
      }, { _ = try text.select(0, 9) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > preserves every line break of a longer run"
    ))
  func longerRun() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try lines(
          try createLineBreakNode(), try createLineBreakNode(), try createTextNode("Paragraph 1"))
        return [paragraph]
      }, { _ = try paragraph.select(4, 4) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n\n")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior after a single line break"
    ))
  func singleLineBreakMerges() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try lines(try createTextNode("Paragraph 1"))
        return [paragraph]
      }, { _ = try paragraph.select(2, 2) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\nHeading 3")"#, #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > gives a lone pasted paragraph its own block after an empty line"
    ))
  func lonePastedParagraph() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try lines(try createLineBreakNode(), try createTextNode("Paragraph 1"))
        return [paragraph]
      }, { _ = try paragraph.select(3, 3) },
      nodes: { [try createParagraphNode().append(try createTextNode("Some paragraph"))] })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n")"#, #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior at the start of a block"
    ))
  func startOfBlockMerges() throws {
    let editor = try editor()
    var paragraph: ParagraphNode!
    try paste(
      editor,
      {
        paragraph = try createParagraphNode().append(try createTextNode("Paragraph 1"))
        return [try createParagraphNode().append(try createTextNode("Above")), paragraph]
      }, { _ = try paragraph.select(0, 0) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Above")"#, #"test-heading("Heading 3")"#,
        #"paragraph("Some paragraphParagraph 1")"#,
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue4815Repro.test.ts::Regression #4815 > keeps the existing merge behavior in the middle of a line"
    ))
  func middleOfLineMerges() throws {
    let editor = try editor()
    var text: TextNode!
    try paste(
      editor,
      {
        text = try createTextNode("Paragraph 1")
        return [try lines(try createLineBreakNode(), text)]
      }, { _ = try text.select(2, 2) })
    #expect(
      try describeRoot(editor) == [
        #"paragraph("Line of text\n\nPaHeading 3")"#, #"paragraph("Some paragraphragraph 1")"#,
      ])
  }
}
