import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Ports of RichTextNodeSelectionArrow.test.ts, RichTextNodeSelectionClick.test.ts,
// RichTextNodeSelectionBackspace.test.ts and RichTextNodeSelectionEnter.test.ts
// from packages/lexical-rich-text/src/__tests__/unit.

private func buildNodeSelectionEditor(
  _ initialState: @escaping () throws -> Void
) throws
  -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [RichTextExtension], nodes: { [.klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() })))
}

// MARK: - Arrow

private func createArrowEditor() throws -> Editor {
  try buildNodeSelectionEditor {
    try getRoot()
      .clear()
      .append(
        createTestDecoratorNode().setIsInline(false),
        createTestDecoratorNode().setIsInline(false),
        createTestDecoratorNode().setIsInline(false),
        createParagraphNode().append(createTextNode("text")))
  }
}

private func selectNodes(_ editor: Editor, _ indexes: [Int]) throws {
  try editor.update(discrete: true) {
    let selection = createNodeSelection()
    for index in indexes {
      selection.add(try #require(try getRoot().getChildAtIndex(index)).getKey())
    }
    try setSelection(selection)
  }
}

private func selectRoot(_ editor: Editor, _ offsets: (Int, Int)) throws {
  try editor.update(discrete: true) {
    _ = try getRoot().select(offsets.0, offsets.1)
  }
}

private struct ArrowResult: Equatable {
  var defaultPrevented: Bool
  var handled: Bool
}

private func dispatchShiftArrow(
  _ editor: Editor, _ command: LexicalCommand<KeyboardEvent>, _ key: String
) throws -> ArrowResult {
  let event = KeyboardEvent("keydown", key: key, shiftKey: true)
  let handled = try editor.dispatchCommand(command, event)
  return ArrowResult(defaultPrevented: event.defaultPrevented, handled: handled)
}

private func expectRootRange(_ editor: Editor, _ offsets: (Int, Int)) throws {
  try editor.read { () throws in
    let selection = try #require(try getSelection() as? RangeSelection)
    let rootKey = try getRoot().getKey()
    #expect(selection.anchor.key == rootKey)
    #expect(selection.anchor.offset == offsets.0)
    #expect(selection.anchor.type == .element)
    #expect(selection.focus.key == rootKey)
    #expect(selection.focus.offset == offsets.1)
    #expect(selection.focus.type == .element)
  }
}

struct ShiftArrowCase: Sendable, CustomTestStringConvertible {
  var converted: (Int, Int)
  var expected: (Int, Int)
  var handled: Bool
  var key: String
  var selectedIndexes: [Int]

  var command: LexicalCommand<KeyboardEvent> {
    switch key {
    case "ArrowRight": KEY_ARROW_RIGHT_COMMAND
    case "ArrowLeft": KEY_ARROW_LEFT_COMMAND
    case "ArrowUp": KEY_ARROW_UP_COMMAND
    default: KEY_ARROW_DOWN_COMMAND
    }
  }

  var testDescription: String { "\(key) with \(selectedIndexes) selected" }
}

private let shiftArrowCases: [ShiftArrowCase] = [
  ShiftArrowCase(
    converted: (0, 1), expected: (0, 2), handled: true, key: "ArrowRight", selectedIndexes: [0]),
  ShiftArrowCase(
    converted: (0, 1), expected: (0, 1), handled: false, key: "ArrowDown", selectedIndexes: [0]),
  ShiftArrowCase(
    converted: (2, 1), expected: (2, 0), handled: true, key: "ArrowLeft", selectedIndexes: [1]),
  ShiftArrowCase(
    converted: (2, 1), expected: (2, 1), handled: false, key: "ArrowUp", selectedIndexes: [1]),
  // The converted focus is already at the start of the root, so rich text
  // consumes the event instead of letting the browser move past it.
  ShiftArrowCase(
    converted: (1, 0), expected: (1, 0), handled: true, key: "ArrowUp", selectedIndexes: [0]),
  // Out of document order: the NodeSelection is sorted before conversion.
  ShiftArrowCase(
    converted: (0, 2), expected: (0, 3), handled: true, key: "ArrowRight",
    selectedIndexes: [1, 0]),
]

@Suite struct RichTextNodeSelectionArrowTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowRight with [ 0 ] selected converts to [ 0, 1 ] and matches the equivalent RangeSelection"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowDown with [ 0 ] selected converts to [ 0, 1 ] and matches the equivalent RangeSelection"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowLeft with [ 1 ] selected converts to [ 2, 1 ] and matches the equivalent RangeSelection"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowUp with [ 1 ] selected converts to [ 2, 1 ] and matches the equivalent RangeSelection"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowUp with [ 0 ] selected converts to [ 1, 0 ] and matches the equivalent RangeSelection"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > ArrowRight with [ 1, 0 ] selected converts to [ 0, 2 ] and matches the equivalent RangeSelection"
    ),
    arguments: shiftArrowCases)
  func shiftArrowConvertsNodeSelection(_ testCase: ShiftArrowCase) throws {
    let nodeSelectionEditor = try createArrowEditor()
    defer { nodeSelectionEditor.dispose() }
    try selectNodes(nodeSelectionEditor, testCase.selectedIndexes)
    let nodeSelectionResult = try dispatchShiftArrow(
      nodeSelectionEditor, testCase.command, testCase.key)

    #expect(
      nodeSelectionResult
        == ArrowResult(defaultPrevented: testCase.handled, handled: testCase.handled))
    try expectRootRange(nodeSelectionEditor, testCase.expected)

    // Starting from the converted RangeSelection produces the same outcome.
    let rangeSelectionEditor = try createArrowEditor()
    defer { rangeSelectionEditor.dispose() }
    try selectRoot(rangeSelectionEditor, testCase.converted)
    let rangeSelectionResult = try dispatchShiftArrow(
      rangeSelectionEditor, testCase.command, testCase.key)

    #expect(rangeSelectionResult == nodeSelectionResult)
    try expectRootRange(rangeSelectionEditor, testCase.expected)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionArrow.test.ts::Shift+Arrow on a NodeSelection (#9062) > does not convert a discontiguous NodeSelection to a range"
    ))
  func discontiguousNodeSelection() throws {
    let editor = try createArrowEditor()
    defer { editor.dispose() }
    try selectNodes(editor, [0, 2])

    let result = try dispatchShiftArrow(editor, KEY_ARROW_RIGHT_COMMAND, "ArrowRight")

    #expect(result.handled == true)
    #expect(result.defaultPrevented == true)
    try expectRootRange(editor, (1, 1))
  }
}

// MARK: - Click

private func createClickEditor(inline: Bool) throws -> Editor {
  try buildNodeSelectionEditor {
    let decorator = try createTestDecoratorNode().setIsInline(inline)
    if inline {
      // Inline decorator flows inside the paragraph, as in #8907.
      try getRoot().append(
        createParagraphNode().append(createTextNode("before"), decorator))
    } else {
      // Block decorator sits at root level, next to the paragraph.
      try getRoot().append(
        createParagraphNode().append(createTextNode("before")), decorator)
    }
  }
}

/// The decorator is the last root child (block) or its last child (inline).
private func decorator() throws -> TestDecoratorNode {
  let last = try #require(try getRoot().getLastChild())
  if let last = last as? ElementNode {
    return try #require(try last.getLastChild() as? TestDecoratorNode)
  }
  return try #require(last as? TestDecoratorNode)
}

private func selectDecorator(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let selection = createNodeSelection()
    selection.add(try decorator().getKey())
    try setSelection(selection)
  }
}

private func decoratorKey(_ editor: Editor) throws -> NodeKey {
  try editor.read { try decorator().getKey() }
}

private func expectKeepsSelection(inline: Bool) throws {
  let editor = try createClickEditor(inline: inline)
  defer { editor.dispose() }
  try selectDecorator(editor)

  // JS appends a span inside the decorator's DOM and clicks it; in Swift the
  // view reports the hit node's key (DESIGN.md "Rich text").
  let key = try decoratorKey(editor)
  let handled = try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click", targetKey: key))
  #expect(handled == false)

  try editor.read { () throws in
    let selection = try #require(
      try getSelection() as? NodeSelection, "expected a NodeSelection")
    #expect(selection.has(key))
  }
}

@Suite struct RichTextNodeSelectionClickTests {
  // Regression for meta/lexical#8907: a click on an already-selected
  // decorator used to clear the NodeSelection. A click inside the selected
  // node is an interaction with that node, not a deselect gesture.
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > inline decorator: click inside the node keeps the selection"
    ))
  func inlineClickInsideKeepsSelection() throws {
    try expectKeepsSelection(inline: true)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > block decorator: click inside the node keeps the selection"
    ))
  func blockClickInsideKeepsSelection() throws {
    try expectKeepsSelection(inline: false)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > click outside the selected node still deselects"
    ))
  func clickOutsideDeselects() throws {
    let editor = try createClickEditor(inline: true)
    defer { editor.dispose() }
    try selectDecorator(editor)

    // A hit on a node outside the selected decorator: the "before" text.
    let outside = try editor.read { try #require(try getRoot().getFirstDescendant()).getKey() }

    let handled = try editor.dispatchCommand(
      CLICK_COMMAND, MouseEvent("click", targetKey: outside))
    #expect(handled == true)

    try editor.read { () throws in
      #expect(try getSelection() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionClick.test.ts::CLICK_COMMAND on a NodeSelection > click with a non-element target still deselects"
    ))
  func clickWithoutTargetDeselects() throws {
    let editor = try createClickEditor(inline: true)
    defer { editor.dispose() }
    try selectDecorator(editor)

    let handled = try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click", targetKey: nil))
    #expect(handled == true)

    try editor.read { () throws in
      #expect(try getSelection() == nil)
    }
  }
}

// MARK: - Backspace / Delete

private func createDeleteEditor(inline: Bool) throws -> Editor {
  try buildNodeSelectionEditor {
    try getRoot().append(
      createParagraphNode().append(createTextNode("before")),
      createTestDecoratorNode().setIsInline(inline))
  }
}

private func selectLastChild(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let decorator = try #require(try getRoot().getLastChild())
    let selection = createNodeSelection()
    selection.add(decorator.getKey())
    try setSelection(selection)
  }
}

@Suite struct RichTextNodeSelectionBackspaceTests {
  // Regression for meta/lexical#8712. JS gives the event a target inside the
  // decorator's DOM to exercise the pass-through guard; Swift key events have
  // no target (DESIGN.md "Rich text"), so only the deletion is checked.
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionBackspace.test.ts::KEY_BACKSPACE_COMMAND on a single-decorator NodeSelection > block decorator: deletes the node"
    ))
  func backspaceDeletesBlockDecorator() throws {
    let editor = try createDeleteEditor(inline: false)
    defer { editor.dispose() }
    try selectLastChild(editor)

    let handled = try editor.dispatchCommand(
      KEY_BACKSPACE_COMMAND, KeyboardEvent("keydown", key: "Backspace"))
    #expect(handled == true)

    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(!(try root.getChildren()[0] is TestDecoratorNode))
      // Selection collapses to a RangeSelection at the sibling.
      #expect(try getSelection() is RangeSelection)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionBackspace.test.ts::KEY_DELETE_COMMAND on a single-decorator NodeSelection > block decorator: deletes the node"
    ))
  func deleteDeletesBlockDecorator() throws {
    let editor = try createDeleteEditor(inline: false)
    defer { editor.dispose() }
    try selectLastChild(editor)

    let handled = try editor.dispatchCommand(
      KEY_DELETE_COMMAND, KeyboardEvent("keydown", key: "Delete"))
    #expect(handled == true)

    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(!(try root.getChildren()[0] is TestDecoratorNode))
    }
  }
}

// MARK: - Enter

private func createEnterEditor(inline: Bool) throws -> Editor {
  try buildNodeSelectionEditor {
    let decorator = try createTestDecoratorNode().setIsInline(inline)
    try getRoot().append(decorator)
    let selection = createNodeSelection()
    selection.add(decorator.getKey())
    try setSelection(selection)
  }
}

@Suite struct RichTextNodeSelectionEnterTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionEnter.test.ts::KEY_ENTER_COMMAND on a single-decorator NodeSelection > block decorator: inserts a paragraph after it and moves the caret"
    ))
  func enterAfterBlockDecorator() throws {
    let editor = try createEnterEditor(inline: false)
    defer { editor.dispose() }

    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)

    try editor.read { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let children = try root.getChildren()
      #expect(children[0] is TestDecoratorNode)
      #expect(children[1] is ParagraphNode)
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == children[1].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextNodeSelectionEnter.test.ts::KEY_ENTER_COMMAND on a single-decorator NodeSelection > inline decorator: no-op"
    ))
  func enterOnInlineDecoratorIsNoOp() throws {
    let editor = try createEnterEditor(inline: true)
    defer { editor.dispose() }

    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)

    try editor.read { () throws in
      let root = try getRoot()
      // inline nodes are wrapped
      let children = try root.getChildren()
      #expect(children.count == 1)
      let paragraph = try #require(children.first as? ParagraphNode)
      let paragraphChildren = try paragraph.getChildren()
      #expect(paragraphChildren.count == 1)
      #expect(paragraphChildren.first is TestDecoratorNode)
      #expect(try getSelection() is NodeSelection)
    }
  }
}
