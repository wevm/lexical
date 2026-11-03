import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts
// and SelectBlockSlots.test.ts. Upstream mounts a contenteditable root;
// here the editor is mounted on a `TestRootElement`.

private func selectAllKeyboardEvent() -> KeyboardEvent {
  KeyboardEvent("keydown", key: "a", ctrlKey: true)
}

private func mount(_ editor: Editor) -> () -> Void {
  do { try editor.setRootElement(TestRootElement()) } catch { Issue.record(error) }
  return {}
}

private func setUpEditor(cascadeSelection: Bool = false, disabled: Bool = false) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "select-block-test",
      dependencies: [
        configExtension(SelectBlockExtension) {
          $0.cascadeSelection = cascadeSelection
          $0.disabled = disabled
        }
      ], nodes: { [.klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          try getRoot().append(
            createParagraphNode().append(createTextNode("first")),
            createParagraphNode().append(createTextNode("second")))
        }),
      register: { editor, _, _ in mount(editor) }))
}

/// The nested editor includes RichTextExtension, so that these tests enforce
/// that the parent editor's listener runs in a higher priority bucket than
/// the nested editor's own SELECT_ALL_COMMAND handler.
private func setUpNestedEditor(
  _ parentEditor: Editor,
  _ initialEditorState: @escaping () throws -> Void = {
    try getRoot().append(createParagraphNode().append(createTextNode("caption")))
  }
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "nested-editor-test",
      dependencies: [
        RichTextExtension,
        configExtension(NestedEditorExtension) { $0.getParentEditor = { parentEditor } },
      ],
      editorConfig: InitialEditorConfig(
        namespace: "nested", initialEditorState: .update { _ in try initialEditorState() })))
}

private func secondParagraph() throws -> ElementNode {
  try #require(try getRoot().getChildAtIndex(1) as? ElementNode)
}

private func expectFullySelected(_ blockNode: ElementNode) throws {
  let selection = try #require(try getSelection() as? RangeSelection)
  #expect(try isBlockFullySelected(blockNode, selection))
}

private func points() throws -> [String] {
  let selection = try #require(try getSelection() as? RangeSelection)
  return [
    selection.anchor.key, "\(selection.anchor.offset)", selection.focus.key,
    "\(selection.focus.offset)",
  ]
}

private func selectSecondAt3(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    _ = try (secondParagraph().getFirstDescendant() as! TextNode).select(3, 3)
  }
}

@Suite struct SelectBlockExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > selects the block first and then the whole document"
    ))
  func blockThenDocument() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try selectSecondAt3(editor)
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      try expectFullySelected(secondParagraph())
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try !isBlockFullySelected(getRoot(), selection))
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > repeated select all does not change the selection"
    ))
  func repeatedSelectAll() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try selectSecondAt3(editor)
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    let before = try editor.read { try points() }
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { () throws in
      #expect(try points() == before)
      try expectFullySelected(getRoot())
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a selection spanning multiple blocks expands to the whole document"
    ))
  func spanningSelection() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let firstText = try getRoot().getFirstDescendant() as! TextNode
      let secondText = try secondParagraph().getFirstDescendant() as! TextNode
      let selection = try firstText.select(2, 2)
      try selection.focus.set(secondText.getKey(), 3, .text)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a selection anchored on the root still selects all"
    ))
  func anchoredOnRoot() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // A partial selection with points directly on the root, which has no
      // top level element.
      _ = try getRoot().select(0, 1)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all works after the document changes while fully selected"
    ))
  func afterDocumentChanges() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try selectSecondAt3(editor)
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    // A remote or programmatic change that does not touch the selection.
    try editor.update(discrete: true) {
      try getRoot().append(createParagraphNode().append(createTextNode("third")))
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all preserves a manually created full selection"
    ))
  func preservesFullSelection() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let first = try getRoot().getFirstDescendant() as! TextNode
      let last = try getRoot().getLastDescendant() as! TextNode
      // A backward selection of the whole document.
      let selection = try last.select(6, 6)
      try selection.focus.set(first.getKey(), 0, .text)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      // The already fully selected document is not rewritten by selectAll.
      #expect(try selection.isBackward())
      try expectFullySelected(getRoot())
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > select all in a single-block document"
    ))
  func singleBlockDocument() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot()
      try root.clear().append(createParagraphNode().append(createTextNode("only")))
      _ = try (root.getFirstDescendant() as! TextNode).select(2, 2)
    }
    // Selecting the only block selects the whole document.
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
    // Pressing again is still handled and remains fully selected.
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a node selection of an inline decorator selects its block"
    ))
  func inlineDecoratorNodeSelection() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode()
      try secondParagraph().append(decorator)
      let nodeSelection = createNodeSelection()
      nodeSelection.add(decorator.getKey())
      try setSelection(nodeSelection)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(secondParagraph()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > a node selection of a top-level decorator selects all"
    ))
  func topLevelDecoratorNodeSelection() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode().setIsInline(false)
      try getRoot().append(decorator)
      let nodeSelection = createNodeSelection()
      nodeSelection.add(decorator.getKey())
      try setSelection(nodeSelection)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > an empty node selection defers to other handlers"
    ))
  func emptyNodeSelection() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try setSelection(createNodeSelection()) }
    #expect(try !editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > respects disabled config"
    ))
  func respectsDisabled() throws {
    let editor = try setUpEditor(disabled: true)
    defer { editor.dispose() }
    try selectSecondAt3(editor)
    #expect(try !editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension > enables and disables PreventSelectAllExtension together"
    ))
  func preventSelectAllTogether() throws {
    let editor = try setUpEditor(disabled: true)
    defer { editor.dispose() }
    let selectBlockStores = try getExtensionDependencyFromEditor(editor, SelectBlockExtension)
      .output
    let preventSelectAllStores = try getExtensionDependencyFromEditor(
      editor, PreventSelectAllExtension
    ).output
    #expect(preventSelectAllStores.disabled.value == true)
    selectBlockStores.disabled.value = false
    #expect(preventSelectAllStores.disabled.value == false)
    selectBlockStores.disabled.value = true
    #expect(preventSelectAllStores.disabled.value == true)
  }
}

@Suite struct SelectBlockNestedEditorTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > ignores commands bubbled from a nested editor when cascadeSelection is off"
    ))
  func ignoresBubbled() throws {
    let editor = try setUpEditor()
    defer { editor.dispose() }
    let nestedEditor = try setUpNestedEditor(editor)
    defer { nestedEditor.dispose() }
    // A stale node selection in the parent editor must not hijack the nested
    // editor's select all.
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode()
      try secondParagraph().append(decorator)
      let nodeSelection = createNodeSelection()
      nodeSelection.add(decorator.getKey())
      try setSelection(nodeSelection)
    }
    // The nested editor's own handler takes care of the command.
    #expect(try nestedEditor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try nestedEditor.read { try expectFullySelected(getRoot()) }
    try editor.read { () throws in #expect(try getSelection() is NodeSelection) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection ignores the command until the nested editor is fully selected"
    ))
  func cascadeUntilFullySelected() throws {
    let editor = try setUpEditor(cascadeSelection: true)
    defer { editor.dispose() }
    let nestedEditor = try setUpNestedEditor(editor)
    defer { nestedEditor.dispose() }
    try nestedEditor.update(discrete: true) {
      _ = try (getRoot().getFirstDescendant() as! TextNode).select(3, 3)
    }
    // The nested editor's own handler selects all of its content first.
    #expect(try nestedEditor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try nestedEditor.read { try expectFullySelected(getRoot()) }
    try editor.read { () throws in #expect(!(try getSelection() is RangeSelection)) }
    // The next press cascades to the parent editor.
    #expect(try nestedEditor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection works when dispatched from inside a nested editor update"
    ))
  func cascadeFromInsideUpdate() throws {
    let editor = try setUpEditor(cascadeSelection: true)
    defer { editor.dispose() }
    let nestedEditor = try setUpNestedEditor(editor)
    defer { nestedEditor.dispose() }
    var handled = false
    try nestedEditor.update(discrete: true) {
      _ = try selectAll()
      // Dispatch while this update is still in progress.
      handled = try nestedEditor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
      // The update continues after the dispatch returns.
      try getRoot().append(createParagraphNode().append(createTextNode("after")))
    }
    #expect(handled)
    try nestedEditor.read { () throws in
      let textContent = try getRoot().getTextContent()
      #expect(textContent.contains("caption"))
      #expect(textContent.contains("after"))
    }
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockExtension.test.ts::SelectBlockExtension with nested editors > cascadeSelection handles the command from an empty nested editor"
    ))
  func cascadeFromEmptyNestedEditor() throws {
    let editor = try setUpEditor(cascadeSelection: true)
    defer { editor.dispose() }
    let nestedEditor = try setUpNestedEditor(editor) {
      _ = try getRoot().append(createParagraphNode()).selectStart()
    }
    defer { nestedEditor.dispose() }
    // An empty nested editor is fully selected by its caret.
    #expect(try nestedEditor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }
}

// MARK: SelectBlockSlots.test.ts

/// A multi-block slot value: a shadow-root container.
private final class SlotContainerNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      SlotContainerNode.self, type: "select-block-slots-container", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

private func slotContainer(_ texts: [String]) throws -> SlotContainerNode {
  let container = try create(SlotContainerNode.self)
  for text in texts {
    try container.append(createParagraphNode().append(createTextNode(text)))
  }
  return container
}

// root > p("before"), host p("host body") {title: container(p("slot one"),
// p("slot two"))}, p("after").
private func setUpSlotEditor(
  disabled: Bool = false, slotTexts: [String] = ["slot one", "slot two"]
)
  throws -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: "select-block-slots-test",
      dependencies: [
        RichTextExtension,
        configExtension(SelectBlockExtension) {
          $0.cascadeSelection = false
          $0.disabled = disabled
        },
      ], nodes: { [.klass(SlotContainerNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          let host = try createParagraphNode().append(createTextNode("host body"))
          _ = try setSlot(host, "title", slotContainer(slotTexts))
          try getRoot().append(
            createParagraphNode().append(createTextNode("before")), host,
            createParagraphNode().append(createTextNode("after")))
        }),
      register: { editor, _, _ in mount(editor) }))
}

private func hostParagraph() throws -> ElementNode {
  try #require(try getRoot().getChildAtIndex(1) as? ElementNode)
}

private func slotValue() throws -> SlotContainerNode {
  try #require(try getSlot(hostParagraph(), "title") as? SlotContainerNode)
}

private func slotParagraph(_ index: Int) throws -> ElementNode {
  try #require(try slotValue().getChildAtIndex(index) as? ElementNode)
}

private func slotText(_ index: Int) throws -> TextNode {
  try #require(try slotParagraph(index).getFirstChild() as? TextNode)
}

private func expectSelectionInSlotFrame() throws {
  let selection = try #require(try getSelection() as? RangeSelection)
  let value = try slotValue()
  let anchorFrame = try #require(try getSlotFrame(selection.anchor.getNode()))
  let focusFrame = try #require(try getSlotFrame(selection.focus.getNode()))
  #expect(anchorFrame.is(value))
  #expect(focusFrame.is(value))
}

@Suite struct SelectBlockSlotsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > select all inside a slot selects exactly the in-slot block"
    ))
  func inSlotBlock() throws {
    let editor = try setUpSlotEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { _ = try slotText(0).select(2, 2) }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      try expectFullySelected(slotParagraph(0))
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try !isBlockFullySelected(slotValue(), selection))
      try expectSelectionInSlotFrame()
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > repeated select all expands block, then slot, then document"
    ))
  func blockSlotDocument() throws {
    let editor = try setUpSlotEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { _ = try slotText(0).select(2, 2) }
    // 1st press: the caret's in-slot block only.
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { () throws in
      try expectFullySelected(slotParagraph(0))
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try !isBlockFullySelected(slotValue(), selection))
    }
    // 2nd press: the whole slot frame, still inside it.
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      try expectFullySelected(slotValue())
      try expectFullySelected(slotParagraph(0))
      try expectFullySelected(slotParagraph(1))
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try !isBlockFullySelected(getRoot(), selection))
      try expectSelectionInSlotFrame()
    }
    // 3rd press: the whole document.
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      try expectFullySelected(getRoot())
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try getSlotFrame(selection.anchor.getNode()) == nil)
      #expect(try getSlotFrame(selection.focus.getNode()) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > a one-block slot escalates block, then document, with no dead press"
    ))
  func oneBlockSlot() throws {
    let editor = try setUpSlotEditor(slotTexts: ["only block"])
    defer { editor.dispose() }
    try editor.update(discrete: true) { _ = try slotText(0).select(2, 2) }
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { try expectFullySelected(slotParagraph(0)) }
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > $isBlockFullySelected is slot-frame aware instead of throwing"
    ))
  func slotFrameAware() throws {
    let editor = try setUpSlotEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      let inSlotCaret = try slotText(0).select(2, 2)
      #expect(try !isBlockFullySelected(getRoot(), inSlotCaret))
      let spanning = try slotText(0).select(0, slotText(0).getTextContentSize())
      #expect(try isBlockFullySelected(slotParagraph(0), spanning))
      #expect(try !isBlockFullySelected(slotParagraph(1), spanning))
      let outsideText = try #require(try getRoot().getFirstDescendant() as? TextNode)
      let outside = try outsideText.select(0, outsideText.getTextContentSize())
      #expect(try !isBlockFullySelected(slotParagraph(0), outside))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > a block-shaped slot value escalates value, then document"
    ))
  func blockShapedSlotValue() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "select-block-line-slot-test",
        dependencies: [
          RichTextExtension,
          configExtension(SelectBlockExtension) {
            $0.cascadeSelection = false
            $0.disabled = false
          },
        ],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let host = try createParagraphNode().append(createTextNode("host body"))
            let line = try createParagraphNode().append(createTextNode("the line"))
            _ = try setSlot(host, "title", line)
            try getRoot().append(createParagraphNode().append(createTextNode("before")), host)
          }),
        register: { editor, _, _ in mount(editor) }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let host = try #require(try getRoot().getChildAtIndex(1))
      let line = try #require(try getSlot(host, "title") as? ElementNode)
      _ = try (line.getFirstChild() as! TextNode).select(3, 3)
    }
    // 1st press: the line itself (it is the scope's only block).
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try selection.getTextContent() == "the line")
      #expect(try getSlotFrame(selection.anchor.getNode()) != nil)
      #expect(try !isBlockFullySelected(getRoot(), selection))
    }
    // 2nd press: straight to the document.
    try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent())
    try editor.read { try expectFullySelected(getRoot()) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/SelectBlockSlots.test.ts::SelectBlockExtension with named slots > with SelectBlockExtension disabled, the rich-text default scopes select all to the slot"
    ))
  func disabledRichTextDefault() throws {
    let editor = try setUpSlotEditor(disabled: true)
    defer { editor.dispose() }
    try editor.update(discrete: true) { _ = try slotText(0).select(2, 2) }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { () throws in
      try expectFullySelected(slotValue())
      try expectFullySelected(slotParagraph(0))
      try expectFullySelected(slotParagraph(1))
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try !isBlockFullySelected(getRoot(), selection))
      try expectSelectionInSlotFrame()
    }
    // Outside a slot the default keeps the whole-document semantics.
    try editor.update(discrete: true) {
      _ = try (getRoot().getFirstDescendant() as! TextNode).select(3, 3)
    }
    #expect(try editor.dispatchCommand(SELECT_ALL_COMMAND, selectAllKeyboardEvent()))
    try editor.read { try expectFullySelected(getRoot()) }
  }
}
