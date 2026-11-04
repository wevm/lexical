import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts.
// The LinkNode case waits for @lexical/link; this case does not use the
// LinkExtension upstream's host also lists.

@Suite struct ListSpliceKeepsFormattingTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts::ListNode.splice converting a block into a list item > keeps the text formats of the block it unwraps"
    ))
  func keepsTextFormats() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "list-splice-host", dependencies: [ListExtension]))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet").append(
        createListItemNode().append(createTextNode("A")),
        createListItemNode().append(createTextNode("C")))
      try getRoot().clear().append(list)
      let paragraph = try createParagraphNode()
      try paragraph.append(
        createTextNode("bold").toggleFormat("bold"),
        createTextNode("styled").setStyle("color: red;"))
      _ = try list.splice(1, 0, [paragraph])
    }
    try editor.read { () throws -> Void in
      let list = try #require(try getRoot().getFirstChild() as? ListNode)
      let item = try #require(try list.getChildAtIndex(1) as? ListItemNode)
      let children = try item.getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["bold", "styled"])
      let bold = try #require(children[0] as? TextNode)
      let styled = try #require(children[1] as? TextNode)
      #expect(try bold.hasFormat("bold"))
      #expect(try styled.getStyle() == "color: red;")
    }
  }
}
