import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalSelection
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-history/src/__tests__/unit/LexicalHistory.test.tsx.

private func createParagraphNodeWithText(_ text: String) throws -> ParagraphNode {
  try createParagraphNode().append(createTextNode(text))
}

@LexicalNode(type: "custom-text", extends: TextNode.self)
final class CustomTextNode: TextNode {
  @Field(try arrayValue(stringValue()), accessors: false) var __classes: [JSONValue] = []

  func addClass(_ className: String) throws {
    let writable = try getWritable()
    if !writable.__classes.contains(.string(className)) {
      writable.__classes.append(.string(className))
    }
  }
}

private func createCustomTextNode(_ text: String) throws -> CustomTextNode {
  try create(CustomTextNode.self).setTextContent(text)
}

/// The child editor a `ChildEditorNode` holds, never serialized.
nonisolated(unsafe) private let EditorKey = createState(
  "editor", parse: { _ -> Editor? in nil }, unparse: { _ in nil }, isEqual: { $0 === $1 })

final class ChildEditorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ChildEditorNode.self, type: "child-editor", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }

  func getOrResetEditor() throws -> Editor {
    if let prevEditor = try getState(self, EditorKey) {
      return prevEditor
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "ChildEditorNode", dependencies: [SharedHistoryExtension, NestedEditorExtension]))
    try setState(self, EditorKey, editor)
    return editor
  }
}

// Upstream's ChildEditorExtension also attaches each child editor to its
// decorator's DOM, which only the rendered HTML assertions need.
private let ChildEditorExtension = defineExtension(
  name: "@lexical/test/ChildEditor", nodes: { [.klass(ChildEditorNode.self)] })

/// Counterpart of upstream's `<TestComposer>` with `RichTextPlugin` and
/// `HistoryPlugin` (whose default delay is 1000 ms), mounted.
private func buildRichTextEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [RichTextExtension, configExtension(HistoryExtension) { $0.delay = 1000 }]))
  try editor.setRootElement(TestRootElement())
  drainMicrotasks()
  return editor
}

/// `await editor.dispatchCommand(...)`: the dispatch, then the microtask
/// that commits it.
private func dispatchAndFlush(_ editor: Editor, _ command: LexicalCommand<Void>) throws {
  try editor.dispatchCommand(command)
  drainMicrotasks()
}

private final class CanFlags {
  var canRedo: Bool
  var canUndo: Bool
  init(_ value: Bool) {
    canRedo = value
    canUndo = value
  }
}

private func registerCanFlags(_ editor: Editor, _ flags: CanFlags) {
  editor.registerCommand(
    CAN_REDO_COMMAND,
    { payload, _ in
      flags.canRedo = payload
      return false
    }, COMMAND_PRIORITY_CRITICAL)
  editor.registerCommand(
    CAN_UNDO_COMMAND,
    { payload, _ in
      flags.canUndo = payload
      return false
    }, COMMAND_PRIORITY_CRITICAL)
}

private func appendParagraph(_ text: String) throws {
  try getRoot().append(createParagraphNode().append(createTextNode(text)))
}

private func historyOutput(_ editor: Editor) throws -> HistoryExtensionOutput {
  try getExtensionDependencyFromEditor(editor, HistoryExtension).output
}

@Suite struct LexicalHistoryTests {
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory after clearing"
    ))
  func afterClearing() throws {
    let editor = try buildRichTextEditor()
    defer { editor.dispose() }
    let flags = CanFlags(true)
    registerCanFlags(editor, flags)
    try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
    #expect(flags.canRedo == false)
    #expect(flags.canUndo == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory.Redo after Quote Node"
    ))
  func redoAfterQuoteNode() throws {
    let editor = try buildRichTextEditor()
    defer { editor.dispose() }
    try update(editor) {
      let root = try getRoot()
      let paragraph1 = try createParagraphNodeWithText("AAA")
      let paragraph2 = try createParagraphNodeWithText("BBB")
      // The editor has one child that is an empty paragraph Node.
      try root.getChildAtIndex(0)?.replace(paragraph1)
      try root.append(paragraph2)
    }
    let initialJSONState = try editor.getEditorState().toJSON()

    try update(editor) {
      let root = try getRoot()
      let selection = createRangeSelection()
      let firstTextNode = try root.getAllTextNodes()[0]
      try selection.anchor.set(firstTextNode.getKey(), 0, .text)
      try selection.focus.set(firstTextNode.getKey(), 3, .text)
      try setSelection(selection)
      try setBlocksType(selection) { try createQuoteNode() }
    }

    let after = try editor.getEditorState().toJSON()
    let children = try #require(after["root"]?.objectValue?["children"]?.arrayValue)
    #expect(children.count == 2)
    let quote = try #require(children[0].objectValue)
    #expect(quote["type"] == .string("quote"))
    let quoteChildren = try #require(quote["children"]?.arrayValue)
    #expect(quoteChildren.count == 1)
    #expect(quoteChildren[0].objectValue?["type"] == .string("text"))
    #expect(quoteChildren[0].objectValue?["text"] == .string("AAA"))

    try update(editor) { try editor.dispatchCommand(UNDO_COMMAND) }

    #expect(initialJSONState.stringify() == (try editor.getEditorState().toJSON().stringify()))
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory in sequence: change, undo, redo, undo, change"
    ))
  func inSequence() throws {
    let editor = try buildRichTextEditor()
    defer { editor.dispose() }
    let flags = CanFlags(false)
    registerCanFlags(editor, flags)

    // focus (needs the focus to initialize): what `editor.focus()` does to
    // the model when there is no selection.
    try editor.update(tag: FOCUS_TAG) { try getRoot().selectEnd() }
    drainMicrotasks()
    #expect(flags.canRedo == false)
    #expect(flags.canUndo == false)

    // change
    try update(editor) { try getRoot().append(createParagraphNodeWithText("foo")) }
    #expect(flags.canRedo == false)
    #expect(flags.canUndo == true)

    // undo
    try update(editor) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(flags.canRedo == true)
    #expect(flags.canUndo == false)

    // redo
    try update(editor) { try editor.dispatchCommand(REDO_COMMAND) }
    #expect(flags.canRedo == false)
    #expect(flags.canUndo == true)

    // undo
    try update(editor) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(flags.canRedo == true)
    #expect(flags.canUndo == false)

    // change
    try update(editor) { try getRoot().append(createParagraphNodeWithText("foo")) }
    #expect(flags.canRedo == false)
    #expect(flags.canUndo == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > undoStack selection points to the same editor"
    ))
  func undoStackSelectionSameEditor() throws {
    let editor_ = try createTestEditor()
    let sharedHistory = createEmptyHistoryState()
    _ = try registerHistory(editor_, sharedHistory, 1000)
    try update(editor_) { try getRoot().append(createParagraphNode()) }
    try update(editor_) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      let nodeSelection = createNodeSelection()
      nodeSelection.add(paragraph.getKey())
      try setSelection(nodeSelection)
    }
    let nestedEditor = try createTestEditor()
    try nestedEditor.update(tag: HISTORY_MERGE_TAG) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectEnd()
    }
    drainMicrotasks()
    nestedEditor._parentEditor = editor_
    _ = try registerHistory(nestedEditor, sharedHistory, 1000)

    try update(nestedEditor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.selectEnd()
    }

    #expect(sharedHistory.undoStack.count == 2)
    try dispatchAndFlush(editor_, UNDO_COMMAND)
    #expect(editor_.getEditorState()._selection is NodeSelection)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > Changes to TextNode leaf are detected properly #6409"
    ))
  func textNodeLeafChanges() throws {
    let editor = try createTestEditor(nodes: [.klass(CustomTextNode.self)])
    let sharedHistory = createEmptyHistoryState()
    _ = try registerHistory(editor, sharedHistory, 0)
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(createCustomTextNode("Initial text")))
    }
    #expect(sharedHistory.undoStack.isEmpty)

    try editor.update(discrete: true) {
      // Mark dirty with no changes
      for node in try getRoot().getAllTextNodes() {
        _ = try node.getWritable()
      }
      // Restore the editor state and ensure the history did not change
      try restoreEditorState(editor, editor.getEditorState())
    }
    #expect(sharedHistory.undoStack.isEmpty)

    try editor.update(discrete: true) {
      // Mark dirty with text change
      for node in try getRoot().getAllTextNodes() where node is CustomTextNode {
        try node.setTextContent(node.getTextContent() + "!")
      }
    }
    #expect(sharedHistory.undoStack.count == 1)

    try editor.update(discrete: true) {
      // Mark dirty with only a change to the class
      for node in try getRoot().getAllTextNodes() {
        if let node = node as? CustomTextNode {
          try node.addClass("updated")
        }
      }
    }
    #expect(sharedHistory.undoStack.count == 2)
  }
}

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [configExtension(HistoryExtension) { $0.delay = 0 }])
  )
}

private func buildEditorWithHistory(_ historyState: HistoryState) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        configExtension(HistoryExtension) {
          $0.createInitialHistoryState = { _ in historyState }
          $0.delay = 0
        }
      ]))
}

// Two updates are required to populate the undoStack: the first update sets
// `historyState.current`, the second pushes the previous `current` onto the
// stack and dispatches CAN_UNDO_COMMAND.
private func makeEditorWithOneUndoEntry() throws -> Editor {
  let editor = try buildEditor()
  try editor.update(discrete: true) { try appendParagraph("first") }
  try editor.update(discrete: true) { try appendParagraph("second") }
  return editor
}

@Suite struct HistoryExtensionSignalsTests {
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals start as false"
    ))
  func signalsStartFalse() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == false)
    #expect(output.canRedo.peek() == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canUndo becomes true after a push, canRedo stays false"
    ))
  func canUndoAfterPush() throws {
    let editor = try makeEditorWithOneUndoEntry()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == true)
    #expect(output.canRedo.peek() == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo becomes true after undo, canUndo goes false"
    ))
  func canRedoAfterUndo() throws {
    let editor = try makeEditorWithOneUndoEntry()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(output.canUndo.peek() == false)
    #expect(output.canRedo.peek() == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo clears after redo, canUndo returns true"
    ))
  func canRedoClearsAfterRedo() throws {
    let editor = try makeEditorWithOneUndoEntry()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    try editor.dispatchCommand(UNDO_COMMAND)
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(output.canUndo.peek() == true)
    #expect(output.canRedo.peek() == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals reset to false after CLEAR_HISTORY_COMMAND"
    ))
  func signalsResetAfterClear() throws {
    let editor = try makeEditorWithOneUndoEntry()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == true)
    try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
    #expect(output.canUndo.peek() == false)
    #expect(output.canRedo.peek() == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo clears when a new edit is made after undo"
    ))
  func canRedoClearsOnNewEdit() throws {
    let editor = try makeEditorWithOneUndoEntry()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    // Wrap UNDO dispatch in editor.update so that the HISTORIC_TAG from
    // undo's setEditorState does not leak into the subsequent edit.
    try editor.update(discrete: true) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(output.canRedo.peek() == true)
    try editor.update(discrete: true) { try appendParagraph("third") }
    #expect(output.canRedo.peek() == false)
    #expect(output.canUndo.peek() == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canUndo is true immediately when initialized with a non-empty undoStack"
    ))
  func canUndoWithPopulatedUndoStack() throws {
    let donor = try makeEditorWithOneUndoEntry()
    defer { donor.dispose() }
    let donorHistory = try historyOutput(donor).historyState.peek()
    #expect(donorHistory.undoStack.count > 0)

    let editor = try buildEditorWithHistory(donorHistory)
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == true)
    #expect(output.canRedo.peek() == false)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo is true immediately when initialized with a non-empty redoStack"
    ))
  func canRedoWithPopulatedRedoStack() throws {
    let donor = try makeEditorWithOneUndoEntry()
    defer { donor.dispose() }
    try donor.dispatchCommand(UNDO_COMMAND)
    let donorHistory = try historyOutput(donor).historyState.peek()
    #expect(donorHistory.redoStack.count > 0)

    let editor = try buildEditorWithHistory(donorHistory)
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == false)
    #expect(output.canRedo.peek() == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals update when historyState signal is reassigned to a populated state"
    ))
  func signalsUpdateOnReassign() throws {
    // Simulates what SharedHistoryExtension does when it inherits parent state.
    let editor = try buildEditor()
    defer { editor.dispose() }
    let output = try historyOutput(editor)
    #expect(output.canUndo.peek() == false)

    let populated = createEmptyHistoryState()
    populated.undoStack.append(
      HistoryStateEntry(editor: editor, editorState: editor.getEditorState()))

    try output.historyState.set(populated)
    #expect(output.canUndo.peek() == true)
    #expect(output.canRedo.peek() == false)
  }
}

private func buildEditorWithMaxDepth(_ maxDepth: Int?) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        configExtension(HistoryExtension) {
          $0.delay = 0
          $0.set(\.maxDepth, maxDepth)
        }
      ]))
}

// Each discrete update is a separate history event when delay is 0. The
// first update populates historyState.current; the second pushes it onto
// undoStack; from then on every additional update adds one entry. So `n`
// updates produce `n - 1` undoStack entries.
private func pushNHistoryEvents(_ editor: Editor, _ n: Int) throws {
  for i in 0..<n {
    try editor.update(discrete: true) { try appendParagraph("p\(i)") }
  }
}

@Suite struct HistoryExtensionMaxDepthTests {
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > defaults to null (unbounded) and matches the historical behavior"
    ))
  func defaultsToUnbounded() throws {
    let editor = try buildEditorWithMaxDepth(nil)
    defer { editor.dispose() }
    try pushNHistoryEvents(editor, 25)
    let output = try historyOutput(editor)
    #expect(output.maxDepth.peek() == nil)
    #expect(output.historyState.peek().undoStack.count == 24)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > caps the undoStack to maxDepth via FIFO eviction"
    ))
  func capsUndoStack() throws {
    let editor = try buildEditorWithMaxDepth(5)
    defer { editor.dispose() }
    try pushNHistoryEvents(editor, 20)
    let output = try historyOutput(editor)
    #expect(output.historyState.peek().undoStack.count == 5)
    // canUndo stays true once any entry exists.
    #expect(output.canUndo.peek() == true)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > reactively respects a maxDepth signal update for future events"
    ))
  func reactiveMaxDepth() throws {
    let editor = try buildEditorWithMaxDepth(nil)
    defer { editor.dispose() }
    try pushNHistoryEvents(editor, 8)  // 7 entries
    let output = try historyOutput(editor)
    #expect(output.historyState.peek().undoStack.count == 7)

    // Lowering maxDepth does not retroactively trim — only future pushes
    // observe the new cap, at which point the stack settles to that length.
    try output.maxDepth.set(3)
    #expect(output.historyState.peek().undoStack.count == 7)

    try pushNHistoryEvents(editor, 5)  // 5 more events trigger trimming
    #expect(output.historyState.peek().undoStack.count == 3)
  }
}

private func setParagraphText(_ text: String) throws {
  try getRoot().clear().append(createParagraphNodeWithText(text))
}

private func getText(_ target: Editor) throws -> String {
  try target.getEditorState().read { try getRoot().getTextContent() }
}

@Suite struct SharedHistoryStateTests {
  // https://github.com/meta/lexical/issues/8623
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::shared HistoryState across editors > undo and redo apply to the editor that changed"
    ))
  func undoRedoApplyToChangedEditor() throws {
    let parentEditor = try createTestEditor()
    let nestedEditor = try createTestEditor()
    // Give both editors content before registering history so that the edits
    // below are changes rather than initialization.
    for target in [parentEditor, nestedEditor] {
      try target.update(discrete: true) { try setParagraphText("initial") }
    }
    let sharedHistory = createEmptyHistoryState()
    _ = try registerHistory(parentEditor, sharedHistory, 0)
    _ = try registerHistory(nestedEditor, sharedHistory, 0)

    try parentEditor.update(discrete: true) { try setParagraphText("parent 1") }
    try parentEditor.update(discrete: true) { try setParagraphText("parent 2") }
    try nestedEditor.update(discrete: true) { try setParagraphText("nested 1") }
    #expect(sharedHistory.undoStack.count == 2)
    #expect(sharedHistory.undoStack[0].editor === parentEditor)
    #expect(sharedHistory.undoStack[1].editor === nestedEditor)

    // The most recent change was made in the nested editor, so that is the
    // editor the undo has to apply to.
    try dispatchAndFlush(nestedEditor, UNDO_COMMAND)
    #expect(try getText(nestedEditor) == "initial")
    #expect(try getText(parentEditor) == "parent 2")

    // The next entry belongs to the parent editor.
    try dispatchAndFlush(nestedEditor, UNDO_COMMAND)
    #expect(try getText(nestedEditor) == "initial")
    #expect(try getText(parentEditor) == "parent 1")
    #expect(sharedHistory.undoStack.isEmpty)

    try dispatchAndFlush(nestedEditor, REDO_COMMAND)
    #expect(try getText(nestedEditor) == "initial")
    #expect(try getText(parentEditor) == "parent 2")

    try dispatchAndFlush(nestedEditor, REDO_COMMAND)
    #expect(try getText(nestedEditor) == "nested 1")
    #expect(try getText(parentEditor) == "parent 2")
    #expect(sharedHistory.redoStack.isEmpty)
  }
}

private final class ArtificialClock {
  var step = -1
  func now() -> Double {
    step += 1
    return 1000 * Double(step)
  }
}

@Suite struct SharedHistoryExtensionTests {
  // Upstream asserts the rendered HTML of the parent and child editors; the
  // port asserts the same content in the model.
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::SharedHistoryExtension > can create a parent editor"
    ))
  func canCreateParentEditor() throws {
    let clock = ArtificialClock()
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "parent",
        dependencies: [
          configExtension(HistoryExtension) {
            $0.delay = 0
            $0.now = clock.now
          },
          ChildEditorExtension,
        ]))
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update(discrete: true) {
      try selectAll().insertNodes([createTextNode("parent editor")])
    }
    let getChildEditor = { () throws -> Editor in
      let child = try #require(try getRoot().getChildAtIndex(1) as? ChildEditorNode)
      return try child.getOrResetEditor()
    }
    let childText = { () throws -> String in
      try editor.read { try getChildEditor().read { try getRoot().getTextContent() } }
    }
    #expect(try editor.read { try getRoot().getTextContent() } == "parent editor")

    try editor.update(discrete: true) {
      let child = try create(ChildEditorNode.self)
      try child.getOrResetEditor().update(discrete: true) {
        try selectAll().insertText("Child editor")
        try setSelection(nil)
      }
      try getRoot().selectEnd().insertNodes([child])
    }
    // insertNodes at the end of the paragraph no longer leaves a stray empty
    // paragraph after the block decorator (#9095).
    #expect(try editor.read { try getRoot().getChildrenSize() } == 2)
    #expect(try childText() == "Child editor")

    try editor.read {
      try getChildEditor().update(discrete: true) {
        try getRoot().selectEnd().insertText(". Updated!")
        try setSelection(nil)
      }
    }
    #expect(try childText() == "Child editor. Updated!")
    let childEditor = try editor.read { try getChildEditor() }
    #expect(
      try historyOutput(childEditor).historyState.peek()
        === historyOutput(editor).historyState.peek())

    // The last change was made in the child editor, so a single undo reverts
    // it and leaves the parent alone.
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try childText() == "Child editor")
    #expect(try editor.read { try getRoot().getChildrenSize() } == 2)

    try editor.update(discrete: true) { try editor.dispatchCommand(UNDO_COMMAND) }
    #expect(try editor.read { try getRoot().getChildrenSize() } == 1)
    #expect(try editor.read { try getRoot().getTextContent() } == "parent editor")
  }
}
