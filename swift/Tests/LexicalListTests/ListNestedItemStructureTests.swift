import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts.
// Upstream's `$shape` objects are written as text: a list `bullet(…)`, an
// item `[…]`, another element `paragraph(…)`, a line break `⏎`, and a text
// node its text, with children separated by ", ".

private func buildEditor(_ structure: Bool) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "nested-list-item-structure",
      dependencies: [RichTextExtension, ListExtension]
        + (structure ? [NodeStructureExtension] : []),
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
}

private func shape(_ node: LexicalNode) throws -> String {
  if let list = node as? ListNode {
    return "\(try list.getListType())(\(try list.getChildren().map(shape).joined(separator: ", ")))"
  }
  if let item = node as? ListItemNode {
    return "[\(try item.getChildren().map(shape).joined(separator: ", "))]"
  }
  if node is LineBreakNode {
    return "⏎"
  }
  if let element = node as? ElementNode {
    return "\(element.getType())(\(try element.getChildren().map(shape).joined(separator: ", ")))"
  }
  return try node.getTextContent()
}

private func shape(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getChildren().map(shape).joined(separator: ", ") }
}

private func text(_ text: String) throws -> TextNode {
  try #require(
    try getRoot().getAllTextNodes().first { try $0.getTextContent() == text },
    "no text node \(text)")
}

private func item(_ children: LexicalNode...) throws -> ListItemNode {
  try createListItemNode().append(contentsOf: children)
}

private func list(_ listType: ListType, _ items: ListItemNode...) throws -> ListNode {
  try createListNode(listType).append(contentsOf: items)
}

/// bullet [ "top", [ number ["one", "two", "three"], "stray" ] ]
private func strayAfterNestedList() throws {
  try getRoot().clear().append(
    list(
      "bullet",
      item(createTextNode("top")),
      item(
        list(
          "number",
          item(createTextNode("one")),
          item(createTextNode("two")),
          item(createTextNode("three"))),
        createTextNode("stray"))))
}

private func outdent(_ editor: Editor, _ textContent: String) throws {
  try editor.update(discrete: true) {
    _ = try text(textContent).select(1, 1)
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())
  }
}

private func itemValues(_ editor: Editor) throws -> [Int] {
  try editor.read {
    try (getRoot().getFirstChildOrThrow() as! ListNode).getChildren().map {
      try ($0 as! ListItemNode).getValue()
    }
  }
}

/// The paste-then-outdent case, with or without NodeStructureExtension.
private func pasteThenOutdent(structure: Bool) throws {
  let editor = try buildEditor(structure)
  defer { editor.dispose() }
  try editor.update(discrete: true) {
    try getRoot().clear().append(
      list(
        "bullet",
        item(createTextNode("one")),
        item(list("bullet", item(createTextNode("two")), item(createTextNode("three"))))))
  }
  try editor.update(discrete: true) {
    _ = try text("three").select(5, 5)
    try insertGeneratedNodes(
      editor,
      [
        createParagraphNode().append(createTextNode("pasted one")),
        createParagraphNode().append(createTextNode("pasted two")),
      ],
      #require(try getSelection()))
  }
  // Lexical's paste puts the last paragraph into the wrapper item, after
  // its nested list (NodeStructureExtension unwraps it to its text); the
  // rule moves that text into a following item.
  #expect(
    try shape(editor)
      == (structure
        ? "bullet([one], [bullet([two], [threepasted one])], [pasted two])"
        : "bullet([one], [bullet([two], [threepasted one]), paragraph(pasted two)])"))
  // The caret stays at the end of the pasted text.
  try editor.read { () throws -> Void in
    let selection = try #require(try getSelection() as? RangeSelection)
    #expect(selection.isCollapsed())
    #expect(try selection.anchor.getNode().getTextContent() == "pasted two")
    #expect(selection.anchor.offset == "pasted two".count)
  }
  try outdent(editor, "two")
  #expect(
    try editor.read { try getRoot().getTextContent() }
      == "one\n\ntwo\n\nthreepasted one\n\npasted two")
}

@Suite struct ListNestedItemStructureTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, content after the nested list moves into a following item"
    ))
  func contentAfterNestedListMoves() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true, strayAfterNestedList)
    #expect(try shape(editor) == "bullet([top], [number([one], [two], [three])], [stray])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, content before the nested list stays in the item and the list moves into a following one"
    ))
  func contentBeforeNestedListStays() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        list(
          "number",
          item(createTextNode("before"), list("bullet", item(createTextNode("inner")))),
          item(createTextNode("next"))))
    }
    #expect(try shape(editor) == "number([before], [bullet([inner])], [next])")
    // Numbered as indent builds it: the item that only wraps the list has no
    // number of its own.
    #expect(try itemValues(editor) == [1, 2, 2])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, two nested lists and the text between them each get their own item, in order"
    ))
  func twoNestedListsAndText() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        list(
          "bullet",
          item(
            createTextNode("a"),
            list("number", item(createTextNode("one"))),
            createTextNode("b"),
            createLineBreakNode(),
            createTextNode("c"),
            list("bullet", item(createTextNode("two"))))))
    }
    #expect(
      try shape(editor) == "bullet([a], [number([one])], [b, ⏎, c], [bullet([two])])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a later edit that adds content next to a nested list (or a list next to content) is repaired"
    ))
  func laterEditRepaired() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        list(
          "bullet",
          item(createTextNode("text")),
          item(list("bullet", item(createTextNode("inner"))))))
    }
    try editor.update(discrete: true) {
      let list0 = try #require(try getRoot().getFirstChildOrThrow() as? ListNode)
      try #require(try list0.getLastChildOrThrow() as? ListItemNode).append(createTextNode("x"))
      try #require(try list0.getFirstChildOrThrow() as? ListItemNode).append(
        list("number", item(createTextNode("n"))))
    }
    #expect(
      try shape(editor) == "bullet([text], [number([n])], [bullet([inner])], [x])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a check state stays on the item that keeps it, and moved content starts unchecked"
    ))
  func checkStateStays() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createListNode("check").append(
          createListItemNode(true).append(
            createTextNode("done"),
            list("bullet", item(createTextNode("inner"))),
            createTextNode("after"))))
    }
    let items = try editor.read {
      try (getRoot().getFirstChildOrThrow() as! ListNode).getChildren().map {
        "\(try $0.getTextContent()) \(String(describing: try ($0 as! ListItemNode).getChecked()))"
      }
    }
    #expect(items == ["done Optional(true)", "inner Optional(false)", "after Optional(false)"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, an element point in the item follows the content it was next to"
    ))
  func elementPointFollowsContent() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let wrapper = try item(
        list("number", item(createTextNode("one"))), createTextNode("stray"))
      try getRoot().clear().append(list("bullet", wrapper))
      let selection = createRangeSelection()
      // Before "stray" to the end of the item.
      try selection.anchor.set(wrapper.getKey(), 1, .element)
      try selection.focus.set(wrapper.getKey(), 2, .element)
      try setSelection(selection)
    }
    try editor.read { () throws -> Void in
      let selection = try #require(try getSelection() as? RangeSelection)
      // The normalization puts the points in the text they were next to.
      #expect(try selection.getTextContent() == "stray")
      let stray = try text("stray")
      #expect(try selection.anchor.getNode().is(stray))
      #expect(selection.anchor.offset == 0)
      #expect(try selection.focus.getNode().is(stray))
      #expect(selection.focus.offset == "stray".count)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > with NodeStructureExtension, a shape the document already holds is kept by an unrelated edit"
    ))
  func existingShapeKept() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    // A document stored that way (written without the normalization).
    try editor.update(discrete: true, skipTransforms: true, strayAfterNestedList)
    // Typing elsewhere in the list, and in the item that holds the shape.
    for (textContent, offset) in [("top", 3), ("stray", 5), ("two", 3)] {
      try editor.update(discrete: true) {
        _ = try text(textContent).select(offset, offset)
        try #require(try getSelection()).insertText("!")
      }
    }
    #expect(try shape(editor) == "bullet([top!], [number([one], [two!], [three]), stray!])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > without NodeStructureExtension, the shape is kept as it is"
    ))
  func shapeKeptWithoutExtension() throws {
    let editor = try buildEditor(false)
    defer { editor.dispose() }
    try editor.update(discrete: true, strayAfterNestedList)
    #expect(try shape(editor) == "bullet([top], [number([one], [two], [three]), stray])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > a paste of paragraphs at the end of a nested list item, then an outdent, keeps every word (NodeStructureExtension: true)"
    ))
  func pasteThenOutdentWithExtension() throws {
    try pasteThenOutdent(structure: true)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::a list item holding a nested list holds nothing else > a paste of paragraphs at the end of a nested list item, then an outdent, keeps every word (NodeStructureExtension: false)"
    ))
  func pasteThenOutdentWithoutExtension() throws {
    try pasteThenOutdent(structure: false)
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > outdenting a middle item keeps the text after the nested list, in reading order"
    ))
  func outdentMiddleItem() throws {
    let editor = try buildEditor(false)
    defer { editor.dispose() }
    try editor.update(discrete: true, strayAfterNestedList)
    try outdent(editor, "two")
    #expect(
      try shape(editor) == "bullet([top], [number([one])], [two], [number([three])], [stray])")
    // The caret stays in the outdented item.
    try editor.read { () throws -> Void in
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try selection.anchor.getNode().getTextContent() == "two")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > outdenting the first and the last item keeps the text after the nested list"
    ))
  func outdentFirstAndLast() throws {
    let editor = try buildEditor(false)
    defer { editor.dispose() }
    try editor.update(discrete: true, strayAfterNestedList)
    // The first and the last of several items move as they always did
    // (the wrapper keeps the nested list and the text); the only item left
    // takes the outdent that keeps the wrapper's text.
    try outdent(editor, "one")
    #expect(try shape(editor) == "bullet([top], [one], [number([two], [three]), stray])")
    try outdent(editor, "three")
    #expect(try shape(editor) == "bullet([top], [one], [number([two]), stray], [three])")
    try outdent(editor, "two")
    #expect(try shape(editor) == "bullet([top], [one], [two], [stray], [three])")
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNestedItemStructure.test.ts::outdent next to other content in the wrapper item (without NodeStructureExtension) > text before the nested list stays before the items it preceded"
    ))
  func textBeforeNestedListStays() throws {
    let editor = try buildEditor(false)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        list(
          "bullet",
          item(
            createTextNode("before"),
            list("bullet", item(createTextNode("one")), item(createTextNode("two"))))))
    }
    try outdent(editor, "two")
    #expect(try shape(editor) == "bullet([before, bullet([one])], [two])")
    try outdent(editor, "one")
    #expect(try shape(editor) == "bullet([before], [one], [two])")
  }
}
