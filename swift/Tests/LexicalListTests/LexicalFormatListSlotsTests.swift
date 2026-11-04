import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts.
// A named-slot value has no __parent (its up-link is __slotHost), so it is
// not eligible for generic list conversion: the slot assignment is managed by
// the node or extension that owns the slot.

private func runInEditor(_ fn: @escaping () throws -> Void) throws {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "@formatList-slots-test", nodes: { listTestNodes }))
  defer { editor.dispose() }
  try editor.update(discrete: true, fn)
}

private func createHostWithSlottedList() throws -> (
  host: ParagraphNode, list: ListNode, text: TextNode
) {
  let host = try createParagraphNode()
  try getRoot().append(host)
  let list = try createListNode("bullet")
  let text = try createTextNode("item")
  try list.append(createListItemNode().append(text))
  _ = try setSlot(host, "items", list)
  return (host, list, text)
}

@Suite struct LexicalFormatListSlotsTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > no-ops on a non-empty bare-block slot value"
    ))
  func insertListNonEmptySlotParagraph() throws {
    try runInEditor {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let text = try createTextNode("title text")
      let slotPara = try createParagraphNode().append(text)
      _ = try setSlot(host, "title", slotPara)
      _ = try text.select(0, 0)
      try insertList("bullet")
      let value = try #require(try getSlot(host, "title"))
      #expect(value.is(slotPara))
      #expect(isParagraphNode(value))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > no-ops on an empty bare-block slot value"
    ))
  func insertListEmptySlotParagraph() throws {
    try runInEditor {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let slotPara = try createParagraphNode()
      _ = try setSlot(host, "title", slotPara)
      _ = try slotPara.select()
      try insertList("bullet")
      let value = try #require(try getSlot(host, "title"))
      #expect(value.is(slotPara))
      #expect(isParagraphNode(value))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > does not change the type of a slotted list"
    ))
  func insertListSlottedList() throws {
    try runInEditor {
      let (host, list, text) = try createHostWithSlottedList()
      _ = try text.select(0, 0)
      try insertList("number")
      let value = try #require(try getSlot(host, "items"))
      #expect(value.is(list))
      #expect(isListNode(value))
      #expect(try list.getListType() == "bullet")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$insertList and named slots > does not change the type of a slotted list from an empty item"
    ))
  func insertListSlottedListEmptyItem() throws {
    try runInEditor {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let list = try createListNode("bullet")
      let item = try createListItemNode()
      try list.append(item)
      _ = try setSlot(host, "items", list)
      _ = try item.select()
      try insertList("number")
      let value = try #require(try getSlot(host, "items"))
      #expect(value.is(list))
      #expect(try list.getListType() == "bullet")
      #expect(try item.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/LexicalFormatListSlots.test.ts::$removeList and named slots > no-ops on a slotted list"
    ))
  func removeListSlottedList() throws {
    try runInEditor {
      let (host, list, text) = try createHostWithSlottedList()
      _ = try text.select(0, 0)
      try removeList()
      let value = try #require(try getSlot(host, "items"))
      #expect(value.is(list))
      #expect(isListNode(value))
      #expect(try list.getTextContent() == "item")
    }
  }
}
