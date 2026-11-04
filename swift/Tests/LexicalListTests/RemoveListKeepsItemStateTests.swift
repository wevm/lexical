import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "remove-list-state-host", dependencies: [ListExtension]))
}

private struct ParagraphState: Equatable {
  var direction: ElementDirection?
  var format: ElementFormatType
  var indent: Int
  var style: String
  var text: String
}

private func paragraphState() throws -> [ParagraphState] {
  try getRoot().getChildren().compactMap { $0 as? ParagraphNode }.map {
    ParagraphState(
      direction: try $0.getDirection(), format: try $0.getFormatType(), indent: try $0.getIndent(),
      style: try $0.getStyle(), text: try $0.getTextContent())
  }
}

@Suite struct RemoveListKeepsItemStateTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts::$removeList keeps the list item state > the paragraphs keep the items format, indent, direction and style"
    ))
  func paragraphsKeepItemState() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet")
      let outer = try createListItemNode()
        .setFormat("center")
        .setDirection("rtl")
        .setStyle("color: red;")
      try outer.append(createTextNode("a"))
      try list.append(outer)
      // A nested item renders one level in; getIndent() derives that from the
      // nesting, so the paragraph has to carry it to look the same.
      let wrapper = try createListItemNode()
      let nested = try createListNode("bullet")
      try nested.append(createListItemNode().append(createTextNode("b")))
      try wrapper.append(nested)
      try list.append(wrapper)
      try getRoot().clear().append(list)
    }
    try editor.update(discrete: true) {
      _ = try selectAll()
      try removeList()
    }
    try editor.read { () throws -> Void in
      #expect(
        try paragraphState() == [
          ParagraphState(
            direction: "rtl", format: "center", indent: 0, style: "color: red;", text: "a"),
          ParagraphState(direction: nil, format: "", indent: 1, style: "", text: "b"),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsItemState.test.ts::$removeList keeps the list item state > a formatted paragraph round-trips through a list"
    ))
  func formattedParagraphRoundTrips() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // $createListOrMerge copies the block's format and indent onto the list
      // item it creates, so removing the list again has to give them back.
      let paragraph = try createParagraphNode().setFormat("right").setIndent(2)
      try paragraph.append(createTextNode("x"))
      try getRoot().clear().append(paragraph)
    }
    try editor.update(discrete: true) {
      _ = try selectAll()
      try insertList("bullet")
    }
    try editor.update(discrete: true) {
      _ = try selectAll()
      try removeList()
    }
    try editor.read { () throws -> Void in
      #expect(
        try paragraphState() == [
          ParagraphState(direction: nil, format: "right", indent: 2, style: "", text: "x")
        ])
    }
  }
}
