import Lexical
import LexicalExtension
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "remove-list-selection-host", dependencies: [ListExtension],
      nodes: { [.klass(TestDecoratorNode.self)] }))
}

private typealias NodeOffset = (node: LexicalNode, offset: Int)

private func pointType(_ node: LexicalNode) -> PointType { node is TextNode ? .text : .element }

private func select(_ anchor: NodeOffset, _ focus: NodeOffset? = nil) throws {
  let focus = focus ?? anchor
  let selection = createRangeSelection()
  try selection.anchor.set(anchor.node.getKey(), anchor.offset, pointType(anchor.node))
  try selection.focus.set(focus.node.getKey(), focus.offset, pointType(focus.node))
  try setSelection(selection)
}

/// A point as `key node offset type`, where `node` names the node by its
/// text or its type, so that a failure shows where a point went.
private func describePoint(_ point: NodeOffset, _ type: PointType? = nil) throws -> String {
  let node = point.node
  let name = node is TextNode ? try node.getTextContent() : node.getType()
  return "\(node.getKey()) \(name) \(point.offset) \(type ?? pointType(node))"
}

private func expectSelection(_ anchor: NodeOffset, _ focus: NodeOffset? = nil) throws {
  let selection = try #require(try getSelection() as? RangeSelection, "Expected a RangeSelection")
  let points = try [selection.anchor, selection.focus].map {
    try describePoint(($0.getNode(), $0.offset), $0.type)
  }
  #expect(try points == [describePoint(anchor), describePoint(focus ?? anchor)])
}

private func paragraphs() throws -> [ParagraphNode] {
  try getRoot().getChildren().compactMap { $0 as? ParagraphNode }
}

/// A list item holding `text` and then an inline decorator.
private func createItemEndingInDecorator(_ text: String) throws -> ListItemNode {
  try createListItemNode().append(createTextNode(text), createTestDecoratorNode())
}

private func removeListAfter(_ editor: Editor, _ setUp: @escaping () throws -> Void) throws {
  try editor.update(discrete: true) {
    try getRoot().clear()
    try setUp()
  }
  try editor.update(discrete: true) { try removeList() }
}

@Suite struct RemoveListKeepsSelectionTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret after an inline decorator at the end of the item"
    ))
  func caretAfterDecorator() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let item = try createItemEndingInDecorator("ab ")
      try getRoot().append(createListNode("bullet").append(item))
      try select((item, 2))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[0]
      try expectSelection((paragraph, 2))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret between the text and an inline decorator"
    ))
  func caretBetweenTextAndDecorator() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let item = try createItemEndingInDecorator("ab ")
      try getRoot().append(createListNode("bullet").append(item))
      try select((item, 1))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[0]
      try expectSelection((paragraph.getFirstChildOrThrow(), 3))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a forward selection with its focus after an inline decorator"
    ))
  func forwardSelection() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let item = try createItemEndingInDecorator("ab ")
      try getRoot().append(createListNode("bullet").append(item))
      try select((item.getFirstChildOrThrow(), 1), (item, 2))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[0]
      try expectSelection((paragraph.getFirstChildOrThrow(), 1), (paragraph, 2))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a backward selection with its anchor after an inline decorator"
    ))
  func backwardSelection() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let item = try createItemEndingInDecorator("ab ")
      try getRoot().append(createListNode("bullet").append(item))
      try select((item, 2), (item.getFirstChildOrThrow(), 1))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[0]
      try expectSelection((paragraph, 2), (paragraph.getFirstChildOrThrow(), 1))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a selection from one item to the next"
    ))
  func selectionAcrossItems() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let first = try createItemEndingInDecorator("ab ")
      let second = try createItemEndingInDecorator("xy ")
      try getRoot().append(createListNode("bullet").append(first, second))
      try select((first, 2), (second, 2))
    }
    try editor.read { () throws -> Void in
      let paragraphs = try paragraphs()
      try expectSelection((paragraphs[0], 2), (paragraphs[1], 2))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret in the text"
    ))
  func caretInText() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let item = try createItemEndingInDecorator("ab ")
      try getRoot().append(createListNode("bullet").append(item))
      try select((item.getFirstChildOrThrow(), 2))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[0]
      try expectSelection((paragraph.getFirstChildOrThrow(), 2))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/RemoveListKeepsSelection.test.ts::$removeList keeps the selection > a caret in an empty nested item"
    ))
  func caretInEmptyNestedItem() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try removeListAfter(editor) {
      let nested = try createListItemNode()
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("ab ")),
          createListItemNode().append(createListNode("bullet").append(nested))))
      try select((nested, 0))
    }
    try editor.read { () throws -> Void in
      let paragraph = try paragraphs()[1]
      #expect(try paragraph.getChildrenSize() == 0)
      try expectSelection((paragraph, 0))
    }
  }
}
