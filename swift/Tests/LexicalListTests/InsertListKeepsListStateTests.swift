import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "insert-list-state-host", dependencies: [ListExtension]))
}

private func rootList() throws -> ListNode {
  try #require(try getRoot().getFirstChild() as? ListNode, "expected a ListNode at the root")
}

@Suite struct InsertListKeepsListStateTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts::$insertList keeps the replaced list state > converting a populated list keeps its direction, format and start"
    ))
  func populatedList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("number", start: 4)
        .setDirection("rtl")
        .setFormat("center")
        .setStyle("color: red;")
      try list.append(
        createListItemNode().append(createTextNode("a")),
        createListItemNode().append(createTextNode("b")))
      try getRoot().clear().append(list)
      _ = try list.getFirstChild()!.selectEnd()
    }
    try editor.update(discrete: true) { try insertList("bullet") }
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "bullet")
      #expect(try list.getDirection() == "rtl")
      #expect(try list.getFormatType() == "center")
      #expect(try list.getStyle() == "color: red;")
      #expect(try list.getStart() == 4)
      #expect(try list.getChildren().map { try $0.getTextContent() } == ["a", "b"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/InsertListKeepsListState.test.ts::$insertList keeps the replaced list state > converting from an empty list item keeps the list state"
    ))
  func emptyListItem() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet").setDirection("rtl").setFormat("right")
      try list.append(createListItemNode())
      try getRoot().clear().append(list)
      _ = try list.getFirstChild()!.selectEnd()
    }
    try editor.update(discrete: true) { try insertList("number") }
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "number")
      #expect(try list.getDirection() == "rtl")
      #expect(try list.getFormatType() == "right")
    }
  }
}
