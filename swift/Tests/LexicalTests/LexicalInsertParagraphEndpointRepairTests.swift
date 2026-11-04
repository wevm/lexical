import Lexical
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts.

private let testExtension = defineExtension(
  name: "insert-paragraph-endpoint-repair", dependencies: [RichTextExtension],
  nodes: { [.klass(LinkNode.self)] })

@Suite struct LexicalInsertParagraphEndpointRepairTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts::insertParagraph over a range from an empty paragraph into styled text (backward=false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalInsertParagraphEndpointRepair.test.ts::insertParagraph over a range from an empty paragraph into styled text (backward=true)"
    ),
    arguments: [false, true])
  func fromEmptyParagraphIntoStyledText(backward: Bool) throws {
    let editor = try buildEditorFromExtensions(testExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let empty = try createParagraphNode()
      let text = try createTextNode("ead").setStyle("color: red")
      try getRoot().clear().append(
        empty,
        try createParagraphNode().append(
          text, try createLinkNode("https://example.com").append(try createTextNode("caa"))))
      let selection = createRangeSelection()
      let (start, end) =
        backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try start.set(empty.getKey(), 0, .element)
      try end.set(text.getKey(), 2, .text)
      try setSelection(selection)
      try selection.insertParagraph()
    }
    try editor.read(.latest) { () throws -> Void in
      let root = try getRoot()
      #expect(try root.getTextContent() == "\n\ndcaa")
      #expect(try root.getChildrenSize() == 2)
      #expect(try root.getFirstChildOrThrow().getTextContent() == "")
      let paragraph = try #require(try root.getLastChildOrThrow() as? ParagraphNode)
      #expect(try paragraph.getChildrenSize() == 2)
      let remainingText = try #require(try paragraph.getFirstChildOrThrow() as? TextNode)
      #expect(try remainingText.getTextContent() == "d")
      #expect(try remainingText.getStyle() == "color: red")
      let link = try #require(try paragraph.getLastChildOrThrow() as? LinkNode)
      #expect(try link.getURL() == "https://example.com")
      #expect(try link.getTextContent() == "caa")
    }
  }
}
