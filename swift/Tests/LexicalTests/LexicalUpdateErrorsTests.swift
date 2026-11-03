import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalUpdateErrors.test.ts: an update that
// throws commits nothing of itself, with `onError` rethrowing or reporting.
// `await Promise.resolve()` is `drainMicrotasks()`.

enum ErrorMode: String, CaseIterable, Sendable, CustomTestStringConvertible {
  case `rethrows`, reports
  var testDescription: String { rawValue }
}

private struct Commit: Equatable {
  var dirtyText: [String]
  var next: String
  var prev: String
  var tags: [String]
}

private func appBug() -> LexicalError { LexicalError("app bug") }
private func message(_ error: any Error) -> String {
  (error as? LexicalError)?.message ?? "\(error)"
}

private func rootText() throws -> String { try getRoot().getTextContent() }
private func textOf(_ state: EditorState) throws -> String { try state.read(rootText) }
private func paragraph(_ text: String) throws -> ParagraphNode {
  try createParagraphNode().append(try createTextNode(text))
}
private func textOf(_ paragraph: LexicalNode) throws -> TextNode {
  try (paragraph as! ParagraphNode).getFirstChildOrThrow() as! TextNode
}
private func first() throws -> TextNode { try textOf(try getRoot().getFirstChildOrThrow()) }
private func last() throws -> TextNode { try textOf(try getRoot().getLastChildOrThrow()) }

private final class Setup {
  let editor: Editor
  var errors: [any Error] = []
  var commits: [Commit] = []

  init(_ mode: ErrorMode, configure: ((Editor) throws -> Void)? = nil) throws {
    var report: (any Error) -> Void = { _ in }
    editor = try createEditor(
      CreateEditorArgs(onError: { error in
        report(error)
        if mode == .`rethrows` { throw error }
      }))
    report = { [unowned self] in errors.append($0) }
    try configure?(editor)
    try editor.update(discrete: true) {
      try getRoot().append(try paragraph("one"), try paragraph("two"))
    }
    _ = editor.registerUpdateListener { [unowned self] payload in
      let state = payload.editorState
      commits.append(
        Commit(
          dirtyText: try state.read {
            payload.dirtyLeaves.array.map { key in
              (state._nodeMap.get(key) as? TextNode).map { $0.__text } ?? key
            }
          },
          next: try textOf(state), prev: try textOf(payload.prevEditorState),
          tags: payload.tags.array.sorted()))
    }
  }

  /// `expectNextUpdateAlone`: the next update commits only its own change.
  func expectNextUpdateAlone(
    _ before: String, sourceLocation: SourceLocation = #_sourceLocation
  ) throws {
    commits = []
    try editor.update(tag: "next", discrete: true) { _ = try last().setTextContent("two!") }
    #expect(
      commits == [
        Commit(
          dirtyText: ["two!"], next: String(before.dropLast(3)) + "two!", prev: before,
          tags: ["next"])
      ], sourceLocation: sourceLocation)
  }
}

/// `fail`: the run throws "app bug" when `onError` rethrows, and returns
/// otherwise.
private func fail(
  _ mode: ErrorMode, sourceLocation: SourceLocation = #_sourceLocation,
  _ run: () throws -> Void
) {
  do {
    try run()
    #expect(mode == .reports, "expected the run to throw", sourceLocation: sourceLocation)
  } catch {
    #expect(mode == .`rethrows`, "unexpected \(error)", sourceLocation: sourceLocation)
    #expect(message(error) == "app bug", sourceLocation: sourceLocation)
  }
}

/// Runs `run` and returns the message of what it threw.
private func thrownMessage(_ run: () throws -> Void) -> String? {
  do {
    try run()
    return nil
  } catch {
    return message(error)
  }
}

@Suite struct LexicalUpdateErrorsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that changes text, then throws (discrete: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that changes text, then throws (discrete: false)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a callback that changes text, then throws (discrete: true)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a callback that changes text, then throws (discrete: false)"
    ),
    arguments: ErrorMode.allCases, [true, false])
  func callbackChangesTextThenThrows(mode: ErrorMode, discrete: Bool) throws {
    let setup = try Setup(mode)
    fail(mode) {
      try setup.editor.update(tag: "failed", discrete: discrete) {
        _ = try first().setTextContent("CHANGED")
        throw appBug()
      }
    }
    #expect(setup.errors.map(message) == ["app bug"])
    #expect(try textOf(setup.editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a callback that throws before any change, then an update that changes nothing"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a callback that throws before any change, then an update that changes nothing"
    ),
    arguments: ErrorMode.allCases)
  func throwsBeforeAnyChange(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    fail(mode) { try setup.editor.update(discrete: true) { throw appBug() } }
    try setup.editor.update(tag: COLLABORATION_TAG, discrete: true) {}
    try setup.expectNextUpdateAlone("one\n\ntwo")
    #expect(setup.editor._pendingEditorState == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a transform that throws"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a transform that throws"
    ),
    arguments: ErrorMode.allCases)
  func transformThrows(mode: ErrorMode) throws {
    let setup = try Setup(mode) { editor in
      _ = try editor.registerNodeTransform(TextNode.self) { node in
        if try node.getTextContent() == "boom" { throw appBug() }
      }
    }
    fail(mode) {
      try setup.editor.update(discrete: true) { _ = try first().setTextContent("boom") }
    }
    #expect(try textOf(setup.editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a nested update that throws discards the update it is part of"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a nested update that throws discards the update it is part of"
    ),
    arguments: ErrorMode.allCases)
  func nestedUpdateThrows(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    fail(mode) {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("ONE")
        try editor.update {
          _ = try last().setTextContent("TWO")
          throw appBug()
        }
      }
    }
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > an update batched after another one discards both"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > an update batched after another one discards both"
    ),
    arguments: ErrorMode.allCases)
  func batchedUpdateDiscardsBoth(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    try editor.update { _ = try first().setTextContent("ONE") }
    fail(mode) {
      try editor.update {
        _ = try last().setTextContent("TWO")
        throw appBug()
      }
    }
    drainMicrotasks()
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > a selection change listener that throws"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > a selection change listener that throws"
    ),
    arguments: ErrorMode.allCases)
  func selectionChangeListenerThrows(mode: ErrorMode) throws {
    let setup = try Setup(mode) { editor in
      _ = editor.registerCommand(
        SELECTION_CHANGE_COMMAND,
        { _, _ in
          _ = try first().setTextContent("CHANGED")
          throw appBug()
        }, COMMAND_PRIORITY_EDITOR)
    }
    fail(mode) {
      try setup.editor.update(discrete: true) {
        let selection = createNodeSelection()
        selection.add(try last().getKey())
        try setSelection(selection)
      }
    }
    #expect(try textOf(setup.editor.getEditorState()) == "one\n\ntwo")
    #expect(setup.editor.getEditorState()._selection == nil)
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError rethrows > what the failed update committed before it threw stays"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update that throws, onError reports > what the failed update committed before it threw stays"
    ),
    arguments: ErrorMode.allCases)
  func committedBeforeThrowStays(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    let other = try editor.parseEditorState(try editor.getEditorState().toJSON())
    fail(mode) {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("ONE")
        try editor.setEditorState(other)
        throw appBug()
      }
    }
    #expect(setup.commits.map(\.next).contains("ONE\n\ntwo"))
    #expect(try textOf(editor.getEditorState()) == "ONE\n\ntwo")
    try setup.expectNextUpdateAlone("ONE\n\ntwo")
  }

  // MARK: error handling of an update that throws

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > a read in onError sees the committed state and commits nothing of the failed update"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > a read in onError sees the committed state and commits nothing of the failed update"
    ),
    arguments: ErrorMode.allCases)
  func readInOnError(mode: ErrorMode) throws {
    var reads: [String] = []
    var errors: [any Error] = []
    var editorRef: Editor?
    let editor = try createEditor(
      CreateEditorArgs(onError: { error in
        errors.append(error)
        reads.append(try editorRef!.read(rootText))
        if mode == .`rethrows` { throw error }
      }))
    editorRef = editor
    try editor.update(discrete: true) {
      try getRoot().append(try paragraph("one"), try paragraph("two"))
    }
    fail(mode) {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("FAILED")
        throw appBug()
      }
    }
    #expect(reads == ["one\n\ntwo"])
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
    editorRef = nil
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > the updates it queued are dropped with it"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > the updates it queued are dropped with it"
    ),
    arguments: ErrorMode.allCases)
  func queuedUpdatesDropped(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    fail(mode) {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("ONE")
        try editor.update { _ = try last().setTextContent("TWO") }
        throw appBug()
      }
    }
    drainMicrotasks()
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > the updates queued before it still run"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > the updates queued before it still run"
    ),
    arguments: ErrorMode.allCases)
  func updatesQueuedBeforeStillRun(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    _ = editor.registerUpdateListener { [unowned editor] payload in
      if payload.tags.has("queue") {
        try editor.update { throw appBug() }
        try editor.update { _ = try last().setTextContent("TWO") }
      }
    }
    fail(mode) {
      try editor.update(tag: "queue", discrete: true) { _ = try first().setTextContent("ONE") }
    }
    drainMicrotasks()
    #expect(try textOf(editor.getEditorState()) == "ONE\n\nTWO")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > an error thrown while the editor is restored is reported too"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > an error thrown while the editor is restored is reported too"
    ),
    arguments: ErrorMode.allCases)
  func errorWhileRestoringReported(mode: ErrorMode) throws {
    let setup = try Setup(mode)
    let editor = setup.editor
    var armed = true
    _ = editor.registerUpdateListener { _ in
      if armed {
        armed = false
        throw LexicalError("listener bug")
      }
    }
    let thrown = thrownMessage {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("FAILED")
        throw appBug()
      }
    }
    if mode == .`rethrows` {
      #expect(thrown == "app bug")
      #expect(setup.errors.map(message) == ["app bug", "listener bug"])
    } else {
      #expect(thrown == "listener bug")
      #expect(setup.errors.map(message) == ["app bug"])
    }
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > an update onError queues runs, also when restoring the editor throws"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > an update onError queues runs, also when restoring the editor throws"
    ),
    arguments: ErrorMode.allCases)
  func onErrorUpdateRuns(mode: ErrorMode) throws {
    var errors: [any Error] = []
    var editorRef: Editor?
    let editor = try createEditor(
      CreateEditorArgs(onError: { error in
        errors.append(error)
        if message(error) == "app bug" {
          try editorRef!.update(discrete: true) { _ = try first().setTextContent("repaired") }
        }
        if mode == .`rethrows` { throw error }
      }))
    editorRef = editor
    try editor.update(discrete: true) {
      try getRoot().append(try paragraph("one"), try paragraph("two"))
    }
    var armed = true
    _ = editor.registerUpdateListener { _ in
      if armed {
        armed = false
        throw LexicalError("listener bug")
      }
    }
    let thrown = thrownMessage {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("FAILED")
        throw appBug()
      }
    }
    #expect(thrown == (mode == .`rethrows` ? "app bug" : "listener bug"))
    drainMicrotasks()
    #expect(try textOf(editor.getEditorState()) == "repaired\n\ntwo")
    editorRef = nil
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError rethrows > a nested update sharing its pending state: reported once, discarded as a whole"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::error handling of an update that throws, onError reports > a nested update sharing its pending state: reported once, discarded as a whole"
    ),
    arguments: ErrorMode.allCases)
  func nestedSharedPendingState(mode: ErrorMode) throws {
    let setup = try Setup(mode) { editor in
      _ = editor.registerCommand(
        SELECTION_CHANGE_COMMAND, { _, _ in throw appBug() }, COMMAND_PRIORITY_EDITOR)
    }
    let editor = setup.editor
    let other = try createEditor(
      CreateEditorArgs(onError: { [unowned setup] in setup.errors.append($0) }))
    try other.update(discrete: true) { try getRoot().append(try paragraph("other")) }
    let otherState = try editor.parseEditorState(try other.getEditorState().toJSON())
    fail(mode) {
      try editor.update(discrete: true) {
        _ = try first().setTextContent("ONE")
        let selection = createNodeSelection()
        selection.add(try last().getKey())
        try setSelection(selection)
        try editor.setEditorState(otherState)
      }
    }
    #expect(setup.errors.map(message) == ["app bug"])
    #expect(try textOf(editor.getEditorState()) == "one\n\ntwo")
    try setup.expectNextUpdateAlone("one\n\ntwo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateErrors.test.ts::an update listener that throws > does not undo the commit, whatever onError does"
    ))
  func updateListenerThrowDoesNotUndo() throws {
    for mode in ErrorMode.allCases {
      let setup = try Setup(mode)
      let editor = setup.editor
      let remove = editor.registerUpdateListener { _ in throw LexicalError("listener bug") }
      #expect(
        thrownMessage {
          try editor.update(discrete: true) { _ = try first().setTextContent("ONE") }
        } == "listener bug")
      remove()
      #expect(setup.errors.isEmpty)
      #expect(editor._pendingEditorState == nil)
      #expect(try textOf(editor.getEditorState()) == "ONE\n\ntwo")
    }
  }
}
