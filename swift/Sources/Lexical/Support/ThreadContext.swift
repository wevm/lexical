#if canImport(Darwin)
  import Darwin
#elseif canImport(Glibc)
  import Glibc
#elseif canImport(Musl)
  import Musl
#endif

/// The module-scope variables of the JS implementation, one set per thread.
///
/// JS keeps the active editor, the active editor state, and a few counters in
/// module scope, which is safe there because JS runs one thread. An `Editor`
/// is bound to the thread that created it (DESIGN.md, Q27), so the port keeps
/// those variables per thread, which preserves the JS semantics exactly:
/// updates save, set, and restore them around their callbacks.
package final class ThreadContext {
  // LexicalUpdates.ts
  package var activeEditorState: EditorState?
  package var activeEditor: Editor?
  package var isReadOnlyMode = false
  package var isAttemptingToRecoverFromReconcilerError = false
  package var isCommittingPendingUpdates = false
  package var editorsWithPendingCascadeReset = Set<ObjectIdentifier>()
  package var editorsWithPendingSelectionChange = Set<ObjectIdentifier>()
  package var editorsDispatchingCommitListeners = Set<ObjectIdentifier>()
  package var infiniteTransformCount = 0
  // LexicalUtils.ts
  package var keyCounter = 1
  package var serverUIDCounter = 0
  package var exportWalk = false
  package var exportMemo: ExportMemo?
  // LexicalSerializedExport.ts
  package var compactExport = false
  // LexicalTextNode.ts
  package var textEditSuspended: NodeKey?
  // `console.warn` (see Warnings.swift).
  package var warningHandler: ((String) -> Void)?
  /// `withUncaughtErrorHandler`.
  package var uncaughtErrorHandler: ((any Error) -> Void)?
  // The microtask queue (see Microtasks.swift).
  package var microtasks: [() -> Void] = []
  /// `setTimeout(fn, 0)` tasks, run by `drainMicrotasks` after the microtasks.
  package var macrotasks: [() -> Void] = []
  package var drainingMicrotasks = false
  /// The module state of the signals in LexicalExtension
  /// (`@preact/signals-core`).
  package var signalsState: AnyObject?
  /// The active render and import context of LexicalHTML
  /// (`ContextRecord.ts`'s `activeContext`).
  package var htmlContext: AnyObject?
  /// The export in progress of LexicalMarkdown (`MarkdownExport.ts`'s
  /// `bracketContext`).
  package var markdownBracketContext: AnyObject?
  /// The list columns of the markdown import in progress
  /// (`MarkdownTransformers.ts`'s `importListColumns` and
  /// `importJoinsLooseLists`).
  package var markdownImportListColumns: [Int]?
  package var markdownImportJoinsLooseLists = false
  /// `withIsApple`: overrides `IS_APPLE` on this thread.
  package var isAppleOverride: Bool?
  /// Runs before each `getParent()` of an element on this thread: a test's
  /// stand-in for patching `ElementNode.prototype.getParent` in JS.
  package var elementGetParentHook: (() throws -> Void)?
  /// The document of `getDocument()`.
  package lazy var defaultDocument = DOMDocument()

  init() {}

  /// The context of the calling thread.
  @inline(__always)
  package static var current: ThreadContext {
    if let pointer = pthread_getspecific(key) {
      return Unmanaged<ThreadContext>.fromOpaque(pointer).takeUnretainedValue()
    }
    let context = ThreadContext()
    pthread_setspecific(key, Unmanaged.passRetained(context).toOpaque())
    return context
  }

  private nonisolated(unsafe) static let key: pthread_key_t = {
    var key = pthread_key_t()
    pthread_key_create(&key) { pointer in
      #if canImport(Darwin)
        Unmanaged<ThreadContext>.fromOpaque(pointer).release()
      #else
        if let pointer { Unmanaged<ThreadContext>.fromOpaque(pointer).release() }
      #endif
    }
    return key
  }()
}
