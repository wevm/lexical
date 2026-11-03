import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalSelectionChangeCommand.test.ts. Upstream
// builds its editors with `buildEditorFromExtensions()` and mounts a
// contenteditable; here a `Root` stands in for it, and `isConnected` false
// for a root that is not in the document. Expected DOM text is asserted as
// the committed model's text.

private final class Root: LexicalRootElement {
  var isConnected = true
}

/// `buildEditorFromExtensions()`: errors and warnings are thrown unless a
/// test replaces `_onError` or `_onWarn`.
private func buildEditor() throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(namespace: "", nodes: defaultTestNodes, onError: { throw $0 }, theme: [:]))
}

/// `mountEditor(editor)`: a connected root.
@discardableResult
private func mountEditor(_ editor: Editor) throws -> Root {
  let root = Root()
  try editor.setRootElement(root)
  return root
}

/// The `original` paragraph with the caret at 1, which most tests start from.
private func setUpOriginal(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let text = try createTextNode("original")
    try getRoot().clear().append(try createParagraphNode().append(text))
    _ = try text.select(1, 1)
  }
}

private func firstText() throws -> TextNode {
  try getRoot().getAllTextNodes()[0]
}

private func committedText(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getTextContent() }
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

/// Registers `listener` for `SELECTION_CHANGE_COMMAND` at low priority.
@discardableResult
private func onSelectionChange(
  _ editor: Editor, _ listener: @escaping () throws -> Bool
)
  -> () -> Void
{
  editor.registerCommand(SELECTION_CHANGE_COMMAND, { _, _ in try listener() }, COMMAND_PRIORITY_LOW)
}

private func selectFirstChildNode() throws {
  let selection = createNodeSelection()
  selection.add(try getRoot().getFirstChildOrThrow().getKey())
  try setSelection(selection)
}

/// `expect(a).toBe(b)` for selections: the same object, or both `nil`.
private func same(_ a: (any BaseSelection)?, _ b: (any BaseSelection)?) -> Bool {
  (a as AnyObject?) === (b as AnyObject?)
}

@Suite(.serialized) struct LexicalSelectionChangeCommandTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child replace across selection notifications (normalize: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child replace across selection notifications (normalize: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child reorder across selection notifications (normalize: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reconciles a same-size child reorder across selection notifications (normalize: true)"
    ),
    arguments: [("replace", false), ("replace", true), ("reorder", false), ("reorder", true)])
  func reconcilesSameSizeChild(operation: String, normalize: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      for text in ["first", "second", "third", "fourth"] {
        try root.append(try createParagraphNode().append(try createTextNode(text)))
      }
      _ = try root.selectStart()
    }
    let previous = editor.getEditorState()
    var calls = 0
    if normalize {
      onSelectionChange(editor) {
        calls += 1
        let selection = try rangeSelection()
        if !selection.hasFormat("bold") { selection.toggleFormat("bold") }
        return false
      }
    }
    try editor.update(discrete: true) {
      let root = try getRoot()
      let third = try #require(try root.getChildAtIndex(2))
      if operation == "replace" {
        _ = try third.replace(try createParagraphNode().append(try createTextNode("replacement")))
      } else {
        _ = try third.insertBefore(try root.getLastChildOrThrow())
      }
      _ = try root.selectEnd()
    }
    let expected =
      operation == "replace"
      ? ["first", "second", "replacement", "fourth"] : ["first", "second", "fourth", "third"]
    #expect(
      try editor.read { try getRoot().getChildren().map { try $0.getTextContent() } } == expected)
    #expect(
      try previous.read { try getRoot().getChildren().map { try $0.getTextContent() } }
        == ["first", "second", "third", "fourth"])
    if normalize { #expect(calls == 2) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > explicit selection commands retain a fresh update-cascade budget"
    ))
  func explicitCommandsRetainFreshBudget() throws {
    let editor = try buildEditor()
    let warnings = Calls()
    let errors = Calls()
    editor._onWarn = { warnings.record($0) }
    editor._onError = { errors.record($0) }
    onSelectionChange(editor) {
      try editor.update { try getRoot().markDirty() }
      return false
    }
    let unregister = editor.registerUpdateListener { _ in try editor.update {} }
    for _ in 0..<150 {
      try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
      // Flush commits without allowing the macrotask budget reset to run.
      for _ in 0..<4 { drainMicrotasksOnly() }
    }
    unregister()
    #expect(warnings.count == 0)
    #expect(errors.count == 0)
    drainMicrotasks()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > does not warn when the last allowed notification clears a rootless selection"
    ))
  func lastNotificationClearsRootlessSelection() throws {
    let editor = try buildEditor()
    let warnings = Calls()
    let onWarn = editor._onWarn
    editor._onWarn = {
      warnings.record($0)
      try onWarn($0)
    }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      if calls == 100 {
        try setSelection(nil)
      } else {
        let root = try getRoot()
        let selection = createNodeSelection()
        selection.add(
          (calls % 2 == 1 ? try root.getLastChildOrThrow() : try root.getFirstChildOrThrow())
            .getKey())
        try setSelection(selection)
      }
      return false
    }
    try editor.update(discrete: true) {
      let first = try createParagraphNode()
      try getRoot().clear().append(first, try createParagraphNode())
      let selection = createNodeSelection()
      selection.add(first.getKey())
      try setSelection(selection)
    }
    #expect(calls == 100)
    #expect(warnings.count == 0)
    #expect(try editor.read { try getSelection() } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > counts explicit dispatches before commit (outside update: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > counts explicit dispatches before commit (outside update: true)"
    ),
    arguments: [false, true])
  func countsExplicitDispatches(outside: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try setUpOriginal(editor)
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.update {
      _ = try firstText().select(3, 3)
      if !outside { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
    }
    if outside { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
    try editor.read {}
    #expect(calls == 1)
    // Explicit commands still run even when the selection is unchanged.
    try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
    try editor.read {}
    #expect(calls == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when a selection listener throws (explicit dispatch: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when a selection listener throws (explicit dispatch: true)"
    ),
    arguments: [false, true])
  func abortsWhenListenerThrows(explicit: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try setUpOriginal(editor)
    let failure = LexicalError("selection listener failed")
    let errors = Calls()
    let warnings = Calls()
    editor._onError = { errors.record($0) }
    let onWarn = editor._onWarn
    editor._onWarn = {
      warnings.record($0)
      try onWarn($0)
    }
    let unregister = onSelectionChange(editor) {
      let text = try firstText()
      if try text.getTextContent() == "edited!" {
        _ = try text.setTextContent("listener change")
        try getRoot().append(try createParagraphNode().append(try createTextNode("discard")))
        _ = try getRoot().selectEnd()
        throw failure
      }
      return false
    }
    try editor.update(discrete: true) {
      let text = try firstText()
      _ = try text.setTextContent("edited!")
      _ = try text.select(3, 3)
      if explicit { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
    }
    #expect(errors.count == 1)
    #expect("\(errors.errors.first.map { "\($0)" } ?? "")" == "\(failure)")
    #expect(warnings.count == 0)
    #expect(try committedText(editor) == "original")
    try editor.read { () throws in
      #expect(try rangeSelection().anchor.offset == 1)
    }
    unregister()
    try editor.update(discrete: true) { _ = try firstText().select(4, 4) }
    #expect(errors.count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: false, retry selection: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: false, retry selection: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: true, retry selection: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > restores the notification baseline after rollback (explicit: true, retry selection: true)"
    ),
    arguments: [(false, false), (false, true), (true, false), (true, true)])
  func restoresBaselineAfterRollback(explicit: Bool, retrySelection: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try setUpOriginal(editor)
    let errors = Calls()
    editor._onError = { errors.record($0) }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      if calls == 1 { throw LexicalError("selection listener failed") }
      return false
    }
    try editor.update(discrete: true) {
      _ = try firstText().select(3, 3)
      if explicit { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }
    }
    #expect(errors.count == 1)
    #expect(calls == 1)
    calls = 0
    try editor.update(
      {
        if retrySelection { _ = try firstText().select(3, 3) }
      }, EditorUpdateOptions(onUpdate: {}, discrete: true))
    #expect(calls == (retrySelection ? 1 : 0))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when listener work fails in a transform"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > aborts the update when listener work fails in a nested update"
    ),
    arguments: ["transform", "nested update"])
  func abortsWhenListenerWorkFails(origin: String) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try setUpOriginal(editor)
    let errors = Calls()
    editor._onError = { errors.record($0) }
    _ = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "transform failure" {
        _ = try (node as! TextNode).setTextContent("must be discarded")
        throw LexicalError("listener work failed")
      }
    }
    try editor.read {}
    onSelectionChange(editor) {
      let text = try firstText()
      _ = try text.setTextContent("transform failure")
      if origin == "nested update" {
        try editor.update {
          try text.remove()
          throw LexicalError("listener work failed")
        }
      }
      return false
    }
    try editor.update(discrete: true) {
      let text = try firstText()
      _ = try text.setTextContent("edited!")
      _ = try text.select(3, 3)
    }
    #expect(errors.count == 1)
    #expect(errors.errors.map { "\($0)" } == ["\(LexicalError("listener work failed"))"])
    #expect(try committedText(editor) == "original")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > does not replay skipped range changes when a detached root reconnects"
    ))
  func doesNotReplaySkippedRangeChanges() throws {
    let editor = try buildEditor()
    let root = try mountEditor(editor)
    try setUpOriginal(editor)
    root.isConnected = false
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.update(discrete: true) { _ = try firstText().select(3, 3) }
    #expect(calls == 0)
    root.isConnected = true
    try editor.update(tag: SKIP_DOM_SELECTION_TAG, discrete: true) {
      try getRoot().append(try createParagraphNode())
    }
    #expect(calls == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > caps notifications without discarding edits (throwing warning: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > caps notifications without discarding edits (throwing warning: true)"
    ),
    arguments: [false, true])
  func capsNotifications(throws throwsWarning: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try setUpOriginal(editor)
    let errors = Calls()
    let warnings = Calls()
    editor._onError = { errors.record($0) }
    editor._onWarn = {
      warnings.record($0)
      if throwsWarning { throw $0 }
    }
    var updates = 0
    editor.registerUpdateListener { _ in updates += 1 }
    var onUpdateCalls = 0
    var calls = 0
    let unregister = onSelectionChange(editor) {
      calls += 1
      let offset = try rangeSelection().anchor.offset == 1 ? 2 : 1
      _ = try firstText().select(offset, offset)
      return false
    }
    // A discrete commit lets this test catch a throwing onWarn at the call
    // site.
    let update = {
      try editor.update(
        {
          let text = try firstText()
          _ = try text.setTextContent("TYPED original")
          _ = try text.select(2, 2)
        }, EditorUpdateOptions(onUpdate: { onUpdateCalls += 1 }, discrete: true))
    }
    if throwsWarning {
      #expect {
        try update()
      } throws: {
        "\($0)".contains("Selection change listeners are endlessly changing the selection.")
      }
    } else {
      try update()
    }
    #expect(calls == 100)
    #expect(warnings.count == 1)
    #expect(errors.count == 0)
    #expect(try committedText(editor) == "TYPED original")
    #expect(updates == 1)
    #expect(onUpdateCalls == 1)
    unregister()
    try editor.update(discrete: true) { _ = try firstText().select(3, 3) }
    try editor.read { () throws in
      #expect(try rangeSelection().anchor.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a mounted editor"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a rootless editor"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > node selections notify before commit in a detached editor"
    ),
    arguments: ["mounted", "rootless", "detached"])
  func nodeSelectionsNotifyBeforeCommit(kind: String) throws {
    let editor = try buildEditor()
    if kind == "mounted" {
      try mountEditor(editor)
    } else if kind == "detached" {
      let root = Root()
      root.isConnected = false
      try editor.setRootElement(root)
    }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("original")))
    }
    let previous = editor.getEditorState()
    var calls = 0
    var committedTextDuringListener: [String] = []
    onSelectionChange(editor) {
      calls += 1
      #expect(editor.getEditorState() === previous)
      #expect(same(try getPreviousSelection(), previous._selection))
      #expect(try getRoot().getTextContent() == "CHANGED")
      committedTextDuringListener.append(
        try previous.read { try getRoot().getTextContent() })
      return false
    }
    try editor.update(discrete: true) {
      let text = try firstText()
      _ = try text.setTextContent("CHANGED")
      let selection = createNodeSelection()
      selection.add(text.getKey())
      try setSelection(selection)
    }
    #expect(calls == 1)
    #expect(committedTextDuringListener == ["original"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > listener edits and transforms are reconciled in the original commit"
    ))
  func listenerEditsJoinOriginalCommit() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello")
      try getRoot().clear().append(try createParagraphNode().append(text))
      _ = try text.select(1, 1)
    }
    var updates = 0
    editor.registerUpdateListener { _ in updates += 1 }
    _ = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "Changed" {
        _ = try (node as! TextNode).setTextContent("Transformed")
      }
    }
    try editor.read {}
    updates = 0
    let previousState = editor.getEditorState()
    let unregister = onSelectionChange(editor) {
      #expect(editor.getEditorState() === previousState)
      #expect(try previousState.read { try getRoot().getTextContent() } == "Hello")
      _ = try rangeSelection()
      #expect(same(try getPreviousSelection(), previousState._selection))
      _ = try firstText().setTextContent("Changed")
      return false
    }
    try editor.update(discrete: true) { _ = try firstText().select(2, 2) }
    unregister()
    #expect(try committedText(editor) == "Transformed")
    #expect(updates == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves callbacks added by a command during commit listeners"
    ))
  func preservesCallbacksAddedDuringCommitListeners() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("initial")))
    }
    let command = createCommand(payload: Void.self)
    var callbacks: [String] = []
    let committedText = { try editor.getEditorState().read { try getRoot().getTextContent() } }
    editor.registerCommand(
      command,
      { _, _ in
        _ = try firstText().setTextContent("second")
        try onUpdate { callbacks.append("second: \(try committedText())") }
        return true
      }, COMMAND_PRIORITY_LOW)
    var unregister: () -> Void = {}
    unregister = editor.registerUpdateListener { _ in
      unregister()
      try editor.dispatchCommand(command)
    }
    try editor.update(discrete: true) {
      _ = try firstText().setTextContent("first")
      try onUpdate { callbacks.append("first: \(try committedText())") }
    }
    drainMicrotasks()
    #expect(callbacks == ["first: first", "second: second"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves existing notification behavior for a rootless editor"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves existing notification behavior for a detached editor"
    ),
    arguments: ["rootless", "detached"])
  func preservesNotificationBehavior(rootKind: String) throws {
    let editor = try buildEditor()
    if rootKind == "detached" {
      let root = Root()
      root.isConnected = false
      try editor.setRootElement(root)
    }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().clear().append(try createParagraphNode().append(text))
      _ = try text.select(2, 5)
    }
    try editor.update(discrete: true) { try setSelection(nil) }
    #expect(calls == 0)
    // Non-range selection notifications already work without a connected
    // root.
    try editor.update(discrete: true) { try selectFirstChildNode() }
    #expect(calls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > tracks non-range notifications while the root is detached"
    ))
  func tracksNonRangeWhileDetached() throws {
    let editor = try buildEditor()
    let root = try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().clear().append(try createParagraphNode().append(text))
      _ = try text.select(2, 5)
    }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    root.isConnected = false
    try editor.update(discrete: true) { try selectFirstChildNode() }
    #expect(calls == 1)
    root.isConnected = true
    try editor.update(discrete: true) { _ = try firstText().select(2, 5) }
    #expect(calls == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reports committed selection changes without requiring DOM events"
    ))
  func reportsCommittedSelectionChanges() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(2, 9)
    }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.update(discrete: true) { try rangeSelection().removeText() }
    #expect(calls == 1)
    try editor.update(discrete: true) { try rangeSelection().insertText("y") }
    #expect(calls == 2)
    try editor.update(discrete: true) { _ = try getRoot().selectEnd() }
    #expect(calls == 3)
    try editor.update(discrete: true) { try rangeSelection().toggleFormat("bold") }
    #expect(calls == 4)
    try editor.update(discrete: true) { try rangeSelection().setStyle("color: red") }
    #expect(calls == 5)
    // Dirtying the same selection or changing content without moving it is
    // not a selection change.
    try editor.update(discrete: true) { try setSelection(try getSelection()!.clone()) }
    try editor.update(discrete: true) { try getRoot().append(try createParagraphNode()) }
    #expect(calls == 5)
    try editor.update(discrete: true) { try selectFirstChildNode() }
    #expect(calls == 6)
    try editor.update(discrete: true) { try setSelection(nil) }
    #expect(calls == 7)
    try editor.update(discrete: true) { try setSelection(nil) }
    #expect(calls == 7)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > reports selection changes from setEditorState"
    ))
  func reportsSelectionChangesFromSetEditorState() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(2, 9)
    }
    let previousState = editor.getEditorState()
    try editor.update(discrete: true) { _ = try getRoot().selectEnd() }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.setEditorState(previousState)
    try editor.read {}
    #expect(calls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > listeners see the committed selection when DOM synchronization is skipped"
    ))
  func listenersSeeSelectionWhenDOMSyncSkipped() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().clear().append(try createParagraphNode().append(text))
      _ = try text.select(1, 1)
    }
    var calls = 0
    let unregister = onSelectionChange(editor) {
      calls += 1
      let selection = try rangeSelection()
      #expect([selection.anchor.offset, selection.focus.offset] == [3, 7])
      let previous = try #require(try getPreviousSelection() as? RangeSelection)
      #expect([previous.anchor.offset, previous.focus.offset] == [1, 1])
      return false
    }
    try editor.update(tag: SKIP_DOM_SELECTION_TAG, discrete: true) {
      _ = try firstText().select(3, 7)
    }
    #expect(calls == 1)
    unregister()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > notifies a selection change made by a command listener"
    ))
  func notifiesChangeMadeByListener() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(1, 1)
    }
    var offsets: [[Int]] = []
    onSelectionChange(editor) {
      let selection = try rangeSelection()
      let previous = try #require(try getPreviousSelection() as? RangeSelection)
      offsets.append([previous.anchor.offset, selection.anchor.offset])
      if selection.anchor.offset == 3 {
        _ = try firstText().select(4, 4)
      }
      return false
    }
    try editor.update(discrete: true) { _ = try firstText().select(3, 3) }
    try editor.read {}
    #expect(offsets == [[1, 3], [1, 4]])
    try editor.read { () throws in
      #expect(try getSelection()?.is(try getPreviousSelection()) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > notifies once for batched changes and skips a batch with no net change"
    ))
  func notifiesOnceForBatchedChanges() throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(1, 1)
    }
    var calls = 0
    onSelectionChange(editor) {
      calls += 1
      return false
    }
    try editor.update { _ = try firstText().select(2, 2) }
    try editor.update { _ = try firstText().select(3, 3) }
    try editor.read {}
    #expect(calls == 1)
    try editor.update { _ = try firstText().select(4, 4) }
    try editor.update { _ = try firstText().select(3, 3) }
    try editor.read {}
    #expect(calls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: move (discrete: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: clear (discrete: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: replace (discrete: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: content only (discrete: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: move (discrete: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: clear (discrete: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: replace (discrete: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSelectionChangeCommand.test.ts::SELECTION_CHANGE_COMMAND > preserves mutation-listener updates: content only (discrete: true)"
    ),
    arguments: [
      ("move", false), ("clear", false), ("replace", false), ("content only", false),
      ("move", true), ("clear", true), ("replace", true), ("content only", true),
    ])
  func preservesMutationListenerUpdates(change: String, discrete: Bool) throws {
    let editor = try buildEditor()
    try mountEditor(editor)
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello world")
      try getRoot().clear().append(try createParagraphNode().append(text))
      _ = try text.select(1, 1)
    }
    var selections: [BaseSelection?] = []
    let unregisterCommand = onSelectionChange(editor) {
      selections.append(try getSelection()?.clone())
      return false
    }
    var unregisterMutation: (() -> Void)?
    unregisterMutation = try editor.registerMutationListener(
      TextNode.self,
      { _, _ in
        unregisterMutation?()
        try editor.update(discrete: discrete) {
          if change == "clear" {
            try setSelection(nil)
          } else if change == "replace" {
            let text = try createTextNode("Replacement")
            try getRoot().clear().append(try createParagraphNode().append(text))
            _ = try text.select(7, 7)
          } else if change == "move" {
            _ = try firstText().select(7, 7)
          } else {
            try firstText().setTextContent("Hello world!?")
          }
        }
      }, skipInitialization: true)
    try editor.update(discrete: true) {
      let text = try firstText()
      try text.setTextContent("Hello world!")
      _ = try text.select(2, 2)
    }
    try editor.read { () throws in
      let selection = try getSelection()
      if change == "clear" {
        #expect(selection == nil)
      } else {
        let range = try #require(selection as? RangeSelection)
        let offset = change == "content only" ? 2 : 7
        #expect([range.anchor.offset, range.focus.offset] == [offset, offset])
      }
      #expect(
        try getRoot().getTextContent()
          == (change == "replace"
            ? "Replacement" : change == "content only" ? "Hello world!?" : "Hello world!"))
      // Mutation listeners run after the first selection has committed.
      // Their changes belong to a second update and notify before its commit.
      #expect(selections.count == (change == "content only" ? 1 : 2))
      if let selection {
        #expect(try selections.last.flatMap { $0 }.map { try selection.is($0) } == true)
      } else {
        #expect(selections.last.map { $0 == nil } == true)
      }
    }
    unregisterCommand()
  }
}
