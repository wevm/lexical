import Lexical
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "list-splice-host", dependencies: [ListExtension, LinkExtension]))
}

private func splicedItemChildren() throws -> [LexicalNode] {
  let list = try #require(
    try getRoot().getFirstChild() as? ListNode, "expected a ListNode at the root")
  let item = try #require(try list.getChildAtIndex(1) as? ListItemNode, "expected a ListItemNode")
  return try item.getChildren()
}

@Suite struct ListSpliceKeepsFormattingTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts::ListNode.splice converting a block into a list item > keeps an inline node of the block it unwraps"
    ))
  func keepsInlineNode() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet").append(
        createListItemNode().append(createTextNode("A")),
        createListItemNode().append(createTextNode("C")))
      try getRoot().clear().append(list)
      let paragraph = try createParagraphNode()
      try paragraph.append(createLinkNode("https://lexical.dev/").append(createTextNode("link")))
      _ = try list.splice(1, 0, [paragraph])
    }
    try editor.read { () throws -> Void in
      let children = try splicedItemChildren()
      #expect(children.count == 1)
      let link = try #require(children[0] as? LinkNode, "expected the LinkNode to survive")
      #expect(try link.getURL() == "https://lexical.dev/")
      #expect(try link.getTextContent() == "link")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListSpliceKeepsFormatting.test.ts::ListNode.splice converting a block into a list item > keeps the text formats of the block it unwraps"
    ))
  func keepsTextFormats() throws {
    let editor = try buildEditor()
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
      let children = try splicedItemChildren()
      #expect(try children.map { try $0.getTextContent() } == ["bold", "styled"])
      let bold = try #require(children[0] as? TextNode)
      let styled = try #require(children[1] as? TextNode)
      #expect(try bold.hasFormat("bold"))
      #expect(try styled.getStyle() == "color: red;")
    }
  }
}
