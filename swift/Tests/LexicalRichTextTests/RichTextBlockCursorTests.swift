import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Ports of RichTextBlockCursorShadowRoot.test.ts and
// RichTextBlockCursorRootBoundary.test.ts from
// packages/lexical-rich-text/src/__tests__/unit.

private func makeArrowEvent(_ key: String, shiftKey: Bool = false) -> KeyboardEvent {
  KeyboardEvent("keydown", key: key, shiftKey: shiftKey)
}

private func arrowCommand(_ key: String) -> LexicalCommand<KeyboardEvent> {
  switch key {
  case "ArrowDown": KEY_ARROW_DOWN_COMMAND
  case "ArrowRight": KEY_ARROW_RIGHT_COMMAND
  case "ArrowUp": KEY_ARROW_UP_COMMAND
  default: KEY_ARROW_LEFT_COMMAND
  }
}

private func buildEditor(_ initialState: @escaping () throws -> Void) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [RichTextExtension],
      nodes: { [.klass(TestDecoratorNode.self), .klass(TestShadowRootNode.self)] },
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() })))
}

private func elementChild(_ parent: ElementNode, _ index: Int) throws -> ElementNode {
  try #require(try parent.getChildAtIndex(index) as? ElementNode)
}

private func firstElement(_ parent: ElementNode) throws -> ElementNode {
  try #require(try parent.getFirstChild() as? ElementNode)
}

private func firstText(_ parent: ElementNode) throws -> TextNode {
  try #require(try parent.getFirstChild() as? TextNode)
}

private func selectNode(_ editor: Editor, at index: Int) throws {
  try editor.update(discrete: true) {
    let decorator = try #require(try getRoot().getChildAtIndex(index))
    let ns = createNodeSelection()
    ns.add(decorator.getKey())
    try setSelection(ns)
  }
}

private func selectRoot(_ editor: Editor, _ anchor: Int, _ focus: Int) throws {
  try editor.update(discrete: true) { try getRoot().select(anchor, focus) }
}

/// Expects a collapsed block cursor at `root:offset`.
private func expectBlockCursorAt(_ editor: Editor, _ offset: Int) throws {
  try editor.read { () throws in
    let s = try #require(try getSelection() as? RangeSelection)
    #expect(s.isCollapsed())
    #expect(s.anchor.type == .element)
    #expect(try s.anchor.key == getRoot().getKey())
    #expect(s.anchor.offset == offset)
  }
}

// Structure used across the shadow root tests:
//   root
//     decorator (block)       — index 0
//     shadowRoot              — index 1
//       paragraph > text
//     decorator (block)       — index 2
private func createDecoratorShadowRootEditor() throws -> Editor {
  try buildEditor {
    let decorator1 = try createTestDecoratorNode().setIsInline(false)
    let shadow = try createTestShadowRootNode()
    let paragraph = try createParagraphNode().append(createTextNode("inside"))
    try shadow.append(paragraph)
    let decorator2 = try createTestDecoratorNode().setIsInline(false)
    try getRoot().clear().append(decorator1, shadow, decorator2)
  }
}

/// The text node inside the shadow root at root index 1.
private func shadowText() throws -> TextNode {
  try firstText(firstElement(elementChild(getRoot(), 1)))
}

/// Expects a collapsed selection on the shadow root's text at `offset`.
private func expectInShadowText(_ editor: Editor, _ offset: Int) throws {
  try editor.read { () throws in
    let s = try #require(try getSelection() as? RangeSelection)
    #expect(s.isCollapsed())
    #expect(try s.anchor.key == shadowText().getKey())
    #expect(s.anchor.offset == offset)
  }
}

// Structure:
//   root
//     decorator (block)
//     outerShadowRoot
//       innerShadowRoot
//         paragraph > text
//     decorator (block)
private func createNestedShadowRootEditor() throws -> Editor {
  try buildEditor {
    let decorator1 = try createTestDecoratorNode().setIsInline(false)
    let outer = try createTestShadowRootNode()
    let inner = try createTestShadowRootNode()
    let paragraph = try createParagraphNode().append(createTextNode("nested"))
    try inner.append(paragraph)
    try outer.append(inner)
    let decorator2 = try createTestDecoratorNode().setIsInline(false)
    try getRoot().clear().append(decorator1, outer, decorator2)
  }
}

// Structure used across the root boundary tests:
//   root
//     decorator (block)   — index 0
//     paragraph > text    — index 1
//     decorator (block)   — index 2
private func createBoundaryEditor() throws -> Editor {
  try buildEditor {
    let decorator1 = try createTestDecoratorNode().setIsInline(false)
    let paragraph = try createParagraphNode().append(createTextNode("text"))
    let decorator2 = try createTestDecoratorNode().setIsInline(false)
    try getRoot().clear().append(decorator1, paragraph, decorator2)
  }
}

@Suite struct RichTextBlockCursorShadowRootTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowDown from decorator[0] lands at block cursor (offset 1)"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowRight from decorator[0] lands at block cursor (offset 1)"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowUp from decorator[2] lands at block cursor (offset 2)"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$exitNodeSelectionToward — decorator → block cursor beside shadow root (#8736) > ArrowLeft from decorator[2] lands at block cursor (offset 2)"
    ),
    arguments: [
      ("ArrowDown", 0, 1), ("ArrowRight", 0, 1), ("ArrowUp", 2, 2), ("ArrowLeft", 2, 2),
    ])
  func exitNodeSelectionToward(key: String, index: Int, offset: Int) throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try selectNode(editor, at: index)
    try editor.dispatchCommand(arrowCommand(key), makeArrowEvent(key))
    try expectBlockCursorAt(editor, offset)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowRight from block cursor before shadow root enters at selectStart"
    ))
  func arrowRightEntersAtSelectStart() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    // Place selection at root offset 1 (block cursor between decorator[0] and shadow root)
    try selectRoot(editor, 1, 1)
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    // selectStart() resolves to the leaf text node
    try expectInShadowText(editor, 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowDown from block cursor before shadow root enters at selectStart"
    ))
  func arrowDownEntersAtSelectStart() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 1, 1)
    try editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, makeArrowEvent("ArrowDown"))
    try expectInShadowText(editor, 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowUp from block cursor after shadow root enters at selectEnd"
    ))
  func arrowUpEntersAtSelectEnd() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 2, 2)
    try editor.dispatchCommand(KEY_ARROW_UP_COMMAND, makeArrowEvent("ArrowUp"))
    try expectInShadowText(editor, 6)  // "inside".length
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryEnterFromBlockCursor — block cursor → enter shadow root (#8736) > ArrowLeft from block cursor after shadow root enters at selectEnd"
    ))
  func arrowLeftEntersAtSelectEnd() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    // Place selection at root offset 2 (block cursor between shadow root and decorator[2])
    try selectRoot(editor, 2, 2)
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, makeArrowEvent("ArrowLeft"))
    // selectEnd() resolves to the leaf text node at end offset
    try expectInShadowText(editor, 6)  // "inside".length
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowLeft at start of shadow root exits to block cursor before it"
    ))
  func arrowLeftExitsBefore() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try firstElement(elementChild(getRoot(), 1)).select(0, 0)
    }
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, makeArrowEvent("ArrowLeft"))
    try expectBlockCursorAt(editor, 1)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowUp at start of shadow root exits to block cursor before it"
    ))
  func arrowUpExitsBefore() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try firstElement(elementChild(getRoot(), 1)).select(0, 0)
    }
    try editor.dispatchCommand(KEY_ARROW_UP_COMMAND, makeArrowEvent("ArrowUp"))
    try expectBlockCursorAt(editor, 1)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowDown at end of shadow root exits to block cursor after it"
    ))
  func arrowDownExitsAfter() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try shadowText().select(6, 6) }
    try editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, makeArrowEvent("ArrowDown"))
    try expectBlockCursorAt(editor, 2)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::$tryExitShadowRootToBlockCursor — shadow root → block cursor (#8736) > ArrowRight at end of shadow root exits to block cursor after it"
    ))
  func arrowRightExitsAfter() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try shadowText().select(6, 6) }
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    try expectBlockCursorAt(editor, 2)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::nested shadow root exit (#8736) > ArrowLeft at start of inner shadow root walks up and exits to block cursor"
    ))
  func nestedArrowLeftExits() throws {
    let editor = try createNestedShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try firstElement(firstElement(elementChild(getRoot(), 1))).select(0, 0)
    }
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, makeArrowEvent("ArrowLeft"))
    try expectBlockCursorAt(editor, 1)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::nested shadow root exit (#8736) > ArrowRight at end of inner shadow root walks up and exits to block cursor"
    ))
  func nestedArrowRightExits() throws {
    let editor = try createNestedShadowRootEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try firstText(firstElement(firstElement(elementChild(getRoot(), 1)))).select(6, 6)
    }
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    try expectBlockCursorAt(editor, 2)
  }

  // Structure (simulates layout-container > layout-items):
  //   root
  //     outerShadowRoot
  //       shadowRoot1 (paragraph > text)
  //       shadowRoot2 (paragraph > text)
  // ArrowLeft at start of shadowRoot2 should NOT exit to a block cursor
  // between the two shadow roots (their parent is also a shadow root).
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::no block cursor between sibling shadow roots inside a shadow root parent (#8736) > ArrowLeft at start of sibling shadow root does not produce block cursor"
    ))
  func siblingShadowRootsNoBlockCursor() throws {
    let editor = try buildEditor {
      let outer = try createTestShadowRootNode()
      let sr1 = try createTestShadowRootNode()
      try sr1.append(createParagraphNode().append(createTextNode("col1")))
      let sr2 = try createTestShadowRootNode()
      try sr2.append(createParagraphNode().append(createTextNode("col2")))
      try outer.append(sr1, sr2)
      try getRoot().clear().append(outer)
    }
    defer { editor.dispose() }
    // Place cursor at start of sr2's paragraph
    try editor.update(discrete: true) {
      try firstElement(elementChild(firstElement(getRoot()), 1)).select(0, 0)
    }
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, makeArrowEvent("ArrowLeft"))
    // $tryExitShadowRootToBlockCursor should NOT place a block cursor on the
    // outer shadow root (key = outer.getKey()).
    let afterAnchorKey = try editor.read { () throws in
      try #require(try getSelection() as? RangeSelection).anchor.key
    }
    let outerKey = try editor.read { try #require(try getRoot().getFirstChild()).getKey() }
    #expect(afterAnchorKey != outerKey)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorShadowRoot.test.ts::full round-trip: decorator → block cursor → shadow root → block cursor → decorator (#8736) > ArrowRight traversal: decorator → enter shadow root → exit → reach next decorator"
    ))
  func fullRoundTrip() throws {
    let editor = try createDecoratorShadowRootEditor()
    defer { editor.dispose() }
    // Start: NodeSelection on decorator[0]
    try selectNode(editor, at: 0)

    // Step 1: ArrowRight → block cursor at root:1
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    try editor.read { () throws in
      let s = try #require(try getSelection() as? RangeSelection)
      #expect(try s.anchor.key == getRoot().getKey())
      #expect(s.anchor.offset == 1)
    }

    // Step 2: ArrowRight → enter shadow root (selectStart → text:0)
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    try editor.read { () throws in
      let s = try #require(try getSelection() as? RangeSelection)
      #expect(try s.anchor.key == shadowText().getKey())
      #expect(s.anchor.offset == 0)
    }

    // Step 3: Place cursor at end of text (upstream's jsdom doesn't support
    // selection.modify so it is set directly)
    try editor.update(discrete: true) { try shadowText().select(6, 6) }

    // Step 4: ArrowRight → exit shadow root → block cursor at root:2
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, makeArrowEvent("ArrowRight"))
    try editor.read { () throws in
      let s = try #require(try getSelection() as? RangeSelection)
      #expect(try s.anchor.key == getRoot().getKey())
      #expect(s.anchor.offset == 2)
    }
  }
}

@Suite struct RichTextBlockCursorRootBoundaryTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#8886) > ArrowUp at block cursor before the top decorator does not leave the editor"
    ))
  func arrowUpAtTop() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    // Select the leading decorator (NodeSelection)
    try selectNode(editor, at: 0)

    // First ArrowUp collapses the NodeSelection to a block cursor at root:0
    try editor.dispatchCommand(KEY_ARROW_UP_COMMAND, makeArrowEvent("ArrowUp"))
    try expectBlockCursorAt(editor, 0)

    // Second ArrowUp must be handled (preventDefault) so the native selection
    // cannot escape above the editor, and the lexical selection stays put.
    let event = makeArrowEvent("ArrowUp")
    let handled = try editor.dispatchCommand(KEY_ARROW_UP_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#8886) > ArrowDown at block cursor after the bottom decorator does not leave the editor"
    ))
  func arrowDownAtBottom() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    // Select the trailing decorator (NodeSelection)
    try selectNode(editor, at: 2)

    // First ArrowDown collapses the NodeSelection to a block cursor at root:3
    try editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, makeArrowEvent("ArrowDown"))
    try expectBlockCursorAt(editor, 3)

    // Second ArrowDown must be handled (preventDefault) so the native
    // selection cannot escape below the editor.
    let event = makeArrowEvent("ArrowDown")
    let handled = try editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 3)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight at the block cursor after the last block stays put"
    ))
  func arrowRightAfterLastBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 3, 3)
    try expectBlockCursorAt(editor, 3)

    let event = makeArrowEvent("ArrowRight")
    let handled = try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 3)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowLeft at the block cursor before the first block stays put"
    ))
  func arrowLeftBeforeFirstBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 0, 0)
    try expectBlockCursorAt(editor, 0)

    let event = makeArrowEvent("ArrowLeft")
    let handled = try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight at the block cursor before the last block is not consumed"
    ))
  func arrowRightBeforeLastBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    // root:2 sits between the paragraph and the trailing decorator, so there
    // is still a block to move into.
    try selectRoot(editor, 2, 2)

    let event = makeArrowEvent("ArrowRight")
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, event)
    // The pre-existing decorator navigation still runs and selects the
    // trailing decorator rather than leaving the caret at the root.
    try editor.read { () throws in
      let s = try #require(try getSelection() as? NodeSelection)
      let nodes = try s.getNodes()
      let expected = try getRoot().getChildAtIndex(2)
      #expect(nodes.count == 1)
      #expect(nodes.first?.is(expected) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowRight at the block cursor after the last block stays put"
    ))
  func shiftArrowRightAfterLastBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    // Holding shift does not create anything to extend toward: there is no
    // block past the last one, so the outward key is consumed either way.
    try selectRoot(editor, 3, 3)

    let event = makeArrowEvent("ArrowRight", shiftKey: true)
    let handled = try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 3)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowLeft at the block cursor before the first block stays put"
    ))
  func shiftArrowLeftBeforeFirstBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 0, 0)

    let event = makeArrowEvent("ArrowLeft", shiftKey: true)
    let handled = try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, event)
    #expect(handled == true)
    #expect(event.defaultPrevented == true)
    try expectBlockCursorAt(editor, 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > Shift+ArrowLeft at the block cursor after the last block is not consumed"
    ))
  func shiftArrowLeftAfterLastBlock() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    // Inward, not outward: root:3 is the end of the root, but ArrowLeft moves
    // back toward the trailing decorator, so the edge check must not fire.
    try selectRoot(editor, 3, 3)

    let event = makeArrowEvent("ArrowLeft", shiftKey: true)
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, event)
    try editor.read { () throws in
      let s = try #require(try getSelection() as? RangeSelection)
      #expect(s.isCollapsed() == false)
      #expect(s.focus.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/RichTextBlockCursorRootBoundary.test.ts::block cursor root boundary navigation (#7999) > ArrowRight with a non-collapsed selection at the root edge is not consumed"
    ))
  func arrowRightNonCollapsed() throws {
    let editor = try createBoundaryEditor()
    defer { editor.dispose() }
    try selectRoot(editor, 0, 3)

    let event = makeArrowEvent("ArrowRight")
    let handled = try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, event)
    #expect(handled == false)
    #expect(event.defaultPrevented == false)
  }
}
