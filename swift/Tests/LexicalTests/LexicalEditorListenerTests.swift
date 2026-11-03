import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalEditorListener.test.ts. The `vi.fn()`
// mocks become call logs. The `listener return values` tests build a rich
// text editor upstream only to have paragraphs; a core editor has them too.

private final class Root {}

private func describe(_ root: AnyObject?) -> String {
  root.map { "\(ObjectIdentifier($0).hashValue)" } ?? "nil"
}

@Suite struct LexicalEditorListenerTests {
  // MARK: registerRootListener

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > can return a function that is called when unregistered"
    ))
  func rootListenerCleanupOnUnregister() throws {
    let editor = try createTestEditor()
    var calls: [(AnyObject?, AnyObject?)] = []
    var callbackCalls = 0
    let unregister = editor.registerRootListener { next, prev in
      calls.append((next, prev))
      return { callbackCalls += 1 }
    }
    #expect(editor._rootListeners.size == 1)
    #expect(calls.count == 1)
    #expect(calls.last.map { $0.0 == nil && $0.1 == nil } == true)
    #expect(callbackCalls == 0)
    unregister()
    #expect(calls.count == 2)
    #expect(calls.last.map { $0.0 == nil && $0.1 == nil } == true)
    // Called once to unregister the original return value and then again
    // after the finalization (nil, …) call.
    #expect(callbackCalls == 2)
    #expect(editor._rootListeners.size == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > updates the function on each call"
    ))
  func rootListenerUpdatesCleanup() throws {
    let editor = try createTestEditor()
    var calls: [(AnyObject?, AnyObject?)] = []
    var callbackCalls = 0
    let unregister = editor.registerRootListener { next, prev in
      calls.append((next, prev))
      // Only the first call returns the function.
      return calls.count == 1 ? { callbackCalls += 1 } : nil
    }
    #expect(calls.count == 1)
    #expect(callbackCalls == 0)
    unregister()
    #expect(calls.count == 2)
    #expect(calls.last.map { $0.0 == nil && $0.1 == nil } == true)
    #expect(callbackCalls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerRootListener > works when the root element changes too"
    ))
  func rootListenerRootChanges() throws {
    let editor = try createTestEditor()
    var callbackCalls: [String] = []
    let initialRoot = Root()
    let nextRoot = Root()
    try editor.setRootElement(initialRoot)
    let unregister = editor.registerRootListener { next, prev in
      { callbackCalls.append("\(describe(next)),\(describe(prev))") }
    }
    #expect(callbackCalls.isEmpty)
    try editor.setRootElement(nextRoot)
    try editor.setRootElement(nil)
    try editor.setRootElement(initialRoot)
    let i = describe(initialRoot)
    let n = describe(nextRoot)
    // We haven't unregistered the call to initialRoot yet.
    #expect(callbackCalls == ["\(i),nil", "\(n),\(i)", "nil,\(n)"])
    unregister()
    #expect(
      callbackCalls == ["\(i),nil", "\(n),\(i)", "nil,\(n)", "\(i),nil", "nil,\(i)"])
  }

  // MARK: registerEditableListener

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > can return a function that is called when unregistered"
    ))
  func editableListenerCleanupOnUnregister() throws {
    let editor = try createTestEditor()
    var listenerCalls = 0
    var callbackCalls = 0
    let unregister = editor.registerEditableListener { _ -> ListenerCleanup? in
      listenerCalls += 1
      return { callbackCalls += 1 }
    }
    #expect(editor._editableListeners.size == 1)
    // Not called immediately on registration.
    #expect(listenerCalls == 0)
    try editor.setEditable(false)
    // Called on first change.
    #expect(listenerCalls == 1)
    #expect(callbackCalls == 0)
    unregister()
    // Called on unregister.
    #expect(listenerCalls == 1)
    #expect(callbackCalls == 1)
    #expect(editor._editableListeners.size == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > updates the function on each call"
    ))
  func editableListenerUpdatesCleanup() throws {
    let editor = try createTestEditor()
    var calls: [Bool] = []
    var callbackCalls = 0
    let unregister = editor.registerEditableListener { editable -> ListenerCleanup? in
      calls.append(editable)
      return calls.count == 1 ? { callbackCalls += 1 } : nil
    }
    #expect(calls.isEmpty)
    try editor.setEditable(false)
    #expect(calls == [false])
    #expect(callbackCalls == 0)
    try editor.setEditable(true)
    #expect(calls == [false, true])
    // Only the first call returns the function.
    #expect(callbackCalls == 1)
    unregister()
    #expect(callbackCalls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > registerEditableListener > works when editable state changes"
    ))
  func editableListenerStateChanges() throws {
    let editor = try createTestEditor()
    var listenerCalls = 0
    var callbackCalls: [Bool] = []
    let unregister = editor.registerEditableListener { editable -> ListenerCleanup? in
      listenerCalls += 1
      return { callbackCalls.append(editable) }
    }
    #expect(listenerCalls == 0)
    try editor.setEditable(false)
    #expect(listenerCalls == 1)
    #expect(callbackCalls.isEmpty)
    try editor.setEditable(true)
    #expect(listenerCalls == 2)
    // The previous callback is called when the state changes.
    #expect(callbackCalls == [false])
    try editor.setEditable(false)
    #expect(callbackCalls == [false, true])
    unregister()
    // The final callback is called on unregister.
    #expect(callbackCalls == [false, true, false])
  }

  // MARK: listener return values

  private func appendParagraph(_ text: String) throws {
    try getRoot().clear().append(try createParagraphNode().append(try createTextNode(text)))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > an update listener may return a non-function value"
    ))
  func updateListenerVoid() throws {
    let editor = try createTestEditor()
    var seen: [String] = []
    let unregister = editor.registerUpdateListener { _ in seen.append("x") }
    try editor.update(discrete: true) { try appendParagraph("a") }
    try editor.update(discrete: true) { try appendParagraph("b") }
    unregister()
    #expect(seen == ["x", "x"])
    #expect(editor._updateListeners.size == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > a text content listener may return a non-function value"
    ))
  func textContentListenerVoid() throws {
    let editor = try createTestEditor()
    var seen: [String] = []
    let unregister = editor.registerTextContentListener { seen.append($0) }
    try editor.update(discrete: true) { try appendParagraph("a") }
    try editor.update(discrete: true) { try appendParagraph("b") }
    unregister()
    #expect(seen == ["a", "b"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditorListener.test.ts::LexicalEditor listeners > listener return values > an update listener returning a function gets it called as cleanup"
    ))
  func updateListenerCleanup() throws {
    let editor = try createTestEditor()
    var cleanups = 0
    let unregister = editor.registerUpdateListener { _ -> ListenerCleanup? in { cleanups += 1 } }
    try editor.update(discrete: true) { try appendParagraph("a") }
    // The cleanup from the first call runs before the second call.
    #expect(cleanups == 0)
    try editor.update(discrete: true) { try appendParagraph("b") }
    #expect(cleanups == 1)
    unregister()
    #expect(cleanups == 2)
  }
}
