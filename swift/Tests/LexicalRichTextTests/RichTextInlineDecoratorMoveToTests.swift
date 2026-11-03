import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Ports of RichTextInlineDecoratorMoveToEnd.test.ts and
// RichTextInlineDecoratorMoveToStart.test.ts from
// packages/lexical-rich-text/src/__tests__/unit.

private func buildMoveToEditor(_ initialState: (() throws -> Void)? = nil) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [RichTextExtension],
      nodes: { [.klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: initialState.map { fn in .update { _ in try fn() } })))
}

private func dispatchMoveToEnd(_ editor: Editor, _ shiftKey: Bool) throws {
  try editor.dispatchCommand(
    MOVE_TO_END, KeyboardEvent("keydown", key: "ArrowRight", ctrlKey: true, shiftKey: shiftKey))
}

private func dispatchMoveToStart(_ editor: Editor, _ shiftKey: Bool) throws {
  try editor.dispatchCommand(
    MOVE_TO_START, KeyboardEvent("keydown", key: "ArrowLeft", ctrlKey: true, shiftKey: shiftKey))
}

private struct PointSnapshot: Equatable {
  let type: PointType
  let key: NodeKey
  let offset: Int
}

private struct SelectionSnapshot: Equatable {
  let anchor: PointSnapshot
  let focus: PointSnapshot
}

private func snapshotSelection(_ editor: Editor) throws -> SelectionSnapshot? {
  try editor.read {
    guard let s = try getSelection() as? RangeSelection else { return nil }
    return SelectionSnapshot(
      anchor: PointSnapshot(type: s.anchor.type, key: s.anchor.key, offset: s.anchor.offset),
      focus: PointSnapshot(type: s.focus.type, key: s.focus.key, offset: s.focus.offset))
  }
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

private func firstParagraph() throws -> LexicalNode {
  try getRoot().getFirstChildOrThrow()
}

/// `[inline decorator][text]` in a paragraph, returning the text node.
@discardableResult
private func setUpDecoratorText(
  _ content: String = "hello", inline: Bool = true
) throws
  -> (ParagraphNode, TextNode)
{
  let decorator = try createTestDecoratorNode().setIsInline(inline)
  let text = try createTextNode(content)
  let paragraph = try createParagraphNode().append(decorator, text)
  try getRoot().clear().append(paragraph)
  return (paragraph, text)
}

/// `[inline decorator][text][inline decorator]` in a paragraph.
private func setUpSandwich() throws -> (ParagraphNode, TextNode) {
  let leading = try createTestDecoratorNode().setIsInline(true)
  let text = try createTextNode("hello")
  let trailing = try createTestDecoratorNode().setIsInline(true)
  let paragraph = try createParagraphNode().append(leading, text, trailing)
  try getRoot().clear().append(paragraph)
  return (paragraph, text)
}

private func setUpDecoratorOnly() throws -> ParagraphNode {
  let decorator = try createTestDecoratorNode().setIsInline(true)
  let paragraph = try createParagraphNode().append(decorator)
  try getRoot().clear().append(paragraph)
  return paragraph
}

/// Upstream's `test.for` no-op cases, in order: text first child, block
/// decorator first child, and the caret case.
private func noOpSetup(_ label: String, moveToEnd: Bool) throws {
  switch label {
  case "text":
    let text = try createTextNode("plain")
    let paragraph = try createParagraphNode().append(text)
    try getRoot().clear().append(paragraph)
    if moveToEnd { try paragraph.select(0, 0) } else { try text.select(3, 3) }
  case "block":
    let (paragraph, text) = try setUpDecoratorText("after", inline: false)
    if moveToEnd { try paragraph.select(0, 0) } else { try text.select(2, 2) }
  default:
    let (paragraph, _) = try setUpDecoratorText()
    if moveToEnd { try paragraph.select(1, 1) } else { try paragraph.select(0, 0) }
  }
}

/// Upstream's NodeSelection `test.for` cases. Returns nothing; the expected
/// key/offset is computed after the command by `nodeSelectionExpected`.
private func nodeSelectionSetup(_ label: String, moveToEnd: Bool) throws {
  switch label {
  case "single":
    let decorator = try createTestDecoratorNode().setIsInline(true)
    let text = try createTextNode("hello")
    let paragraph =
      moveToEnd
      ? try createParagraphNode().append(decorator, text)
      : try createParagraphNode().append(text, decorator)
    try getRoot().clear().append(paragraph)
    let ns = createNodeSelection()
    ns.add(decorator.getKey())
    try setSelection(ns)
  case "multi":
    let firstDecorator = try createTestDecoratorNode().setIsInline(true)
    let firstText = try createTextNode("A")
    let firstParagraph =
      moveToEnd
      ? try createParagraphNode().append(firstDecorator, firstText)
      : try createParagraphNode().append(firstText, firstDecorator)
    let secondDecorator = try createTestDecoratorNode().setIsInline(true)
    let secondText = try createTextNode("B")
    let secondParagraph =
      moveToEnd
      ? try createParagraphNode().append(secondDecorator, secondText)
      : try createParagraphNode().append(secondText, secondDecorator)
    try getRoot().clear().append(firstParagraph, secondParagraph)
    let ns = createNodeSelection()
    ns.add(firstDecorator.getKey())
    ns.add(secondDecorator.getKey())
    try setSelection(ns)
  default:
    let text = try createTextNode("hello")
    let paragraph = try createParagraphNode().append(text)
    try getRoot().clear().append(paragraph)
    let ns = createNodeSelection()
    ns.add(paragraph.getKey())
    try setSelection(ns)
  }
}

private func nodeSelectionExpected(_ label: String, moveToEnd: Bool) throws -> (NodeKey, Int) {
  switch label {
  case "single":
    return (try firstParagraph().getKey(), moveToEnd ? 2 : 0)
  case "multi":
    if moveToEnd {
      let secondParagraph = try #require(try getRoot().getChildAtIndex(1))
      return (secondParagraph.getKey(), 2)
    }
    return (try firstParagraph().getKey(), 0)
  default:
    return ("root", moveToEnd ? try getRoot().getChildrenSize() : 0)
  }
}

@Suite struct RichTextInlineDecoratorMoveToEndTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Cmd+ArrowRight at offset 0 moves caret past the inline decorator"
    ))
  func moveToEndPastLeadingDecorator() throws {
    let editor = try buildMoveToEditor {
      let (paragraph, _) = try setUpDecoratorText()
      try paragraph.select(0, 0)
    }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(selection.isCollapsed())
      #expect(selection.focus.type == .text)
      #expect(selection.focus.offset == "hello".count)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Shift+Cmd+ArrowRight at offset 0 selects to end of element"
    ))
  func shiftMoveToEndSelectsToEnd() throws {
    let editor = try buildMoveToEditor {
      let (paragraph, _) = try setUpDecoratorText()
      try paragraph.select(0, 0)
    }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, true)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(!selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.offset == 0)
      #expect(selection.focus.offset == "hello".count)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a leading inline DecoratorNode (Issue #8555) > Same fix applies inside HeadingNode"
    ))
  func moveToEndInHeading() throws {
    let editor = try buildMoveToEditor {
      let decorator = try createTestDecoratorNode().setIsInline(true)
      let text = try createTextNode("world")
      let heading = try createHeadingNode("h1").append(decorator, text)
      try getRoot().clear().append(heading)
      try heading.select(0, 0)
    }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(selection.isCollapsed())
      #expect(selection.focus.offset == "world".count)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: first child is text, not a decorator"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: first child is a block (non-inline) dec…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END no-op cases (Issue #8555) > no-op: caret is past offset 0"
    ),
    arguments: ["text", "block", "caret"])
  func moveToEndNoOp(label: String) throws {
    let editor = try buildMoveToEditor { try noOpSetup(label, moveToEnd: true) }
    defer { editor.dispose() }
    let before = try snapshotSelection(editor)
    try dispatchMoveToEnd(editor, false)
    #expect(try snapshotSelection(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Cmd+ArrowRight on decorator-only element moves caret past the last child"
    ))
  func moveToEndDecoratorOnly() throws {
    let editor = try buildMoveToEditor { try setUpDecoratorOnly().select(0, 0) }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(selection.isCollapsed())
      #expect(selection.focus.type == .element)
      #expect(selection.focus.key == paragraph.getKey())
      #expect(selection.focus.offset == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Shift+Cmd+ArrowRight on decorator-only element selects past the last child"
    ))
  func shiftMoveToEndDecoratorOnly() throws {
    let editor = try buildMoveToEditor { try setUpDecoratorOnly().select(0, 0) }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, true)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(!selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.key == paragraph.getKey())
      #expect(selection.anchor.offset == 0)
      #expect(selection.focus.type == .element)
      #expect(selection.focus.key == paragraph.getKey())
      #expect(selection.focus.offset == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END with no trailing text (Issue #8601) > Cmd+ArrowRight on [decorator][text][decorator] sandwich moves caret past the trailing decorator"
    ))
  func moveToEndSandwich() throws {
    let editor = try buildMoveToEditor { try setUpSandwich().0.select(0, 0) }
    defer { editor.dispose() }
    try dispatchMoveToEnd(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(selection.isCollapsed())
      #expect(selection.focus.type == .element)
      #expect(selection.focus.key == paragraph.getKey())
      #expect(selection.focus.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > single inline decorator collapses to th…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > multi-node selection picks the iteratio…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToEnd.test.ts::MOVE_TO_END on a NodeSelection (Issue #8604) > whole-element non-inline ElementNode re…"
    ),
    arguments: ["single", "multi", "whole"])
  func moveToEndNodeSelection(label: String) throws {
    let editor = try buildMoveToEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try nodeSelectionSetup(label, moveToEnd: true) }
    try dispatchMoveToEnd(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let (key, offset) = try nodeSelectionExpected(label, moveToEnd: true)
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.key == key)
      #expect(selection.anchor.offset == offset)
    }
  }
}

@Suite struct RichTextInlineDecoratorMoveToStartTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a leading inline DecoratorNode (Issue #8555) > Cmd+ArrowLeft from text caret moves caret before the inline decorator"
    ))
  func moveToStartBeforeLeadingDecorator() throws {
    let editor = try buildMoveToEditor { try setUpDecoratorText().1.select(3, 3) }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a leading inline DecoratorNode (Issue #8555) > Shift+Cmd+ArrowLeft selects from text caret back to element start"
    ))
  func shiftMoveToStartSelectsToStart() throws {
    let editor = try buildMoveToEditor { try setUpDecoratorText().1.select(5, 5) }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, true)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(!selection.isCollapsed())
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.offset == 5)
      #expect(selection.focus.type == .element)
      #expect(selection.focus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a leading inline DecoratorNode (Issue #8555) > Same fix applies inside HeadingNode"
    ))
  func moveToStartInHeading() throws {
    let editor = try buildMoveToEditor {
      let decorator = try createTestDecoratorNode().setIsInline(true)
      let text = try createTextNode("world")
      let heading = try createHeadingNode("h1").append(decorator, text)
      try getRoot().clear().append(heading)
      try text.select(2, 2)
    }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START no-op cases (Issue #8555) > no-op: first child is text, not a decorator"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START no-op cases (Issue #8555) > no-op: first child is a block (non-inline) dec…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START no-op cases (Issue #8555) > no-op: caret already at element offset 0"
    ),
    arguments: ["text", "block", "caret"])
  func moveToStartNoOp(label: String) throws {
    let editor = try buildMoveToEditor { try noOpSetup(label, moveToEnd: false) }
    defer { editor.dispose() }
    let before = try snapshotSelection(editor)
    try dispatchMoveToStart(editor, false)
    #expect(try snapshotSelection(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START with no leading text (Issue #8601) > Cmd+ArrowLeft on decorator-only element moves caret to element offset 0"
    ))
  func moveToStartDecoratorOnly() throws {
    let editor = try buildMoveToEditor { try setUpDecoratorOnly().select(1, 1) }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.key == paragraph.getKey())
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START with no leading text (Issue #8601) > Cmd+ArrowLeft from text caret inside [decorator][text][decorator] moves to element offset 0"
    ))
  func moveToStartSandwich() throws {
    let editor = try buildMoveToEditor { try setUpSandwich().1.select(2, 2) }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.key == paragraph.getKey())
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START with no leading text (Issue #8601) > Shift+Cmd+ArrowLeft from text caret inside [decorator][text][decorator] selects back to element offset 0"
    ))
  func shiftMoveToStartSandwich() throws {
    let editor = try buildMoveToEditor { try setUpSandwich().1.select(3, 3) }
    defer { editor.dispose() }
    try dispatchMoveToStart(editor, true)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let paragraph = try firstParagraph()
      #expect(!selection.isCollapsed())
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.offset == 3)
      #expect(selection.focus.type == .element)
      #expect(selection.focus.key == paragraph.getKey())
      #expect(selection.focus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a NodeSelection (Issue #8604) > single inline decorator collapses to th…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a NodeSelection (Issue #8604) > multi-node selection picks the iteratio…"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextInlineDecoratorMoveToStart.test.ts::MOVE_TO_START on a NodeSelection (Issue #8604) > whole-element non-inline ElementNode re…"
    ),
    arguments: ["single", "multi", "whole"])
  func moveToStartNodeSelection(label: String) throws {
    let editor = try buildMoveToEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try nodeSelectionSetup(label, moveToEnd: false) }
    try dispatchMoveToStart(editor, false)
    try editor.read { () throws in
      let selection = try rangeSelection()
      let (key, offset) = try nodeSelectionExpected(label, moveToEnd: false)
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .element)
      #expect(selection.anchor.key == key)
      #expect(selection.anchor.offset == offset)
    }
  }
}
