import Lexical
import LexicalExtension
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts.
// A bare block used as a named-slot value has no parent (its up-link is the
// slot host), so generic block conversion leaves it alone.

private func runInEditor(_ fn: @escaping () throws -> Void) throws {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "@setBlocksType-slots-test",
      nodes: {
        [.klass(TestShadowRootNode.self), .klass(HeadingNode.self), .klass(QuoteNode.self)]
      }))
  defer { editor.dispose() }
  try editor.update(discrete: true, fn)
}

@Suite struct LexicalSetBlocksTypeSlotsTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > no-ops on a bare-block slot value"
    ))
  func setBlocksTypeBareSlot() throws {
    try runInEditor {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let text = try createTextNode("title text")
      let slotPara = try createParagraphNode().append(text)
      try setSlot(host, "title", slotPara)
      let selection = try text.select(0, 0)
      try setBlocksType(selection) { try createHeadingNode("h2") }
      let value = try #require(try getSlot(host, "title"))
      #expect(value.is(slotPara))
      #expect(value is ParagraphNode)
      #expect(try value.getTextContent() == "title text")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > no-ops on an empty bare-block slot value"
    ))
  func setBlocksTypeEmptySlot() throws {
    try runInEditor {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let slotPara = try createParagraphNode()
      try setSlot(host, "title", slotPara)
      let selection = try slotPara.select()
      try setBlocksType(selection) { try createHeadingNode("h2") }
      let value = try #require(try getSlot(host, "title"))
      #expect(value.is(slotPara))
      #expect(value is ParagraphNode)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > converting a slot host does not carry its slots onto the new block"
    ))
  func hostSlotsStay() throws {
    try runInEditor { () throws in
      let host = try createParagraphNode()
      let bodyText = try createTextNode("body")
      try host.append(bodyText)
      try getRoot().append(host)
      let slotPara = try createParagraphNode().append(try createTextNode("title"))
      try setSlot(host, "title", slotPara)
      let selection = try bodyText.select(0, 0)
      try setBlocksType(selection) { try createHeadingNode("h2") }
      let heading = try #require(try getRoot().getLastChild() as? HeadingNode)
      #expect(try heading.getTextContent() == "body")
      // The slots did not transfer; they remain on the detached host.
      #expect(try getSlotNames(heading) == [])
      #expect(try !host.isAttached())
      #expect(try getSlot(host, "title")?.is(slotPara) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$setBlocksType and named slots > converts blocks inside a shadow-root slot value normally"
    ))
  func shadowRootSlotConverts() throws {
    try runInEditor { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      let container = try createTestShadowRootNode()
      let text = try createTextNode("quoted")
      let innerPara = try createParagraphNode().append(text)
      try container.append(innerPara)
      try setSlot(host, "quote", container)
      let selection = try text.select(0, 0)
      try setBlocksType(selection) { try createHeadingNode("h2") }
      // The container keeps its slot; the block inside it converted.
      let value = try #require(try getSlot(host, "quote"))
      #expect(value.is(container))
      let child = try #require(try container.getFirstChild() as? HeadingNode)
      #expect(try child.getTextContent() == "quoted")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSetBlocksTypeSlots.test.ts::$wrapNodes and named slots > no-ops on a bare-block slot value"
    ))
  func wrapNodesBareSlot() throws {
    try runInEditor { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      let text = try createTextNode("title text")
      let slotPara = try createParagraphNode().append(text)
      try setSlot(host, "title", slotPara)
      let selection = try text.select(0, 0)
      try wrapNodes(selection) { try createQuoteNode() }
      let value = try #require(try getSlot(host, "title"))
      #expect(value.is(slotPara))
      #expect(value is ParagraphNode)
      #expect(try value.getTextContent() == "title text")
    }
  }
}
