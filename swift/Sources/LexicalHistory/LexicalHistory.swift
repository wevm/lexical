import Lexical
import LexicalExtension

// Port of packages/lexical-history/src/index.ts.

typealias MergeAction = Int
let HISTORY_MERGE: MergeAction = 0
let HISTORY_PUSH: MergeAction = 1
let DISCARD_HISTORY_CANDIDATE: MergeAction = 2

typealias ChangeType = Int
let OTHER: ChangeType = 0
let COMPOSING_CHARACTER: ChangeType = 1
let INSERT_CHARACTER_AFTER_SELECTION: ChangeType = 2
let DELETE_CHARACTER_BEFORE_SELECTION: ChangeType = 3
let DELETE_CHARACTER_AFTER_SELECTION: ChangeType = 4

/// `HistoryStateEntry`.
public struct HistoryStateEntry {
  public var editor: Editor
  public var editorState: EditorState

  public init(editor: Editor, editorState: EditorState) {
    self.editor = editor
    self.editorState = editorState
  }
}

/// `HistoryState`: the undo/redo history maintained by the history plugin:
/// the `current` entry plus the `undoStack` and `redoStack` of previous and
/// future `HistoryStateEntry`s. Create an empty one with
/// `createEmptyHistoryState` and pass it to the history plugin to share
/// history across editors. A class, because JS shares the object by
/// reference.
public final class HistoryState {
  public var current: HistoryStateEntry?
  public var redoStack: [HistoryStateEntry]
  public var undoStack: [HistoryStateEntry]

  public init(
    current: HistoryStateEntry? = nil, redoStack: [HistoryStateEntry] = [],
    undoStack: [HistoryStateEntry] = []
  ) {
    self.current = current
    self.redoStack = redoStack
    self.undoStack = undoStack
  }
}

typealias IntentionallyMarkedAsDirtyElement = Bool

func getDirtyNodes(
  _ editorState: EditorState, _ dirtyLeaves: JSSet<NodeKey>,
  _ dirtyElements: JSMap<NodeKey, IntentionallyMarkedAsDirtyElement>
) -> [LexicalNode] {
  let nodeMap = editorState._nodeMap
  var nodes: [LexicalNode] = []

  for dirtyLeafKey in dirtyLeaves {
    if let dirtyLeaf = nodeMap.get(dirtyLeafKey) {
      nodes.append(dirtyLeaf)
    }
  }

  for (dirtyElementKey, intentionallyMarkedAsDirty) in dirtyElements {
    if !intentionallyMarkedAsDirty {
      continue
    }

    if let dirtyElement = nodeMap.get(dirtyElementKey), !isRootNode(dirtyElement) {
      nodes.append(dirtyElement)
    }
  }

  return nodes
}

func getChangeType(
  _ prevEditorState: EditorState?, _ nextEditorState: EditorState,
  _ dirtyLeavesSet: JSSet<NodeKey>,
  _ dirtyElementsSet: JSMap<NodeKey, IntentionallyMarkedAsDirtyElement>, _ isComposing: Bool
) -> ChangeType {
  guard let prevEditorState,
    !(dirtyLeavesSet.size == 0 && dirtyElementsSet.size == 0 && !isComposing)
  else {
    return OTHER
  }

  if isComposing {
    return COMPOSING_CHARACTER
  }

  guard let nextSelection = nextEditorState._selection as? RangeSelection,
    let prevSelection = prevEditorState._selection as? RangeSelection,
    prevSelection.isCollapsed(), nextSelection.isCollapsed()
  else {
    return OTHER
  }

  let dirtyNodes = getDirtyNodes(nextEditorState, dirtyLeavesSet, dirtyElementsSet)

  if dirtyNodes.isEmpty {
    return OTHER
  }

  // Catching the case when inserting new text node into an element (e.g. first char in paragraph/list),
  // or after existing node.
  if dirtyNodes.count > 1 {
    let nextNodeMap = nextEditorState._nodeMap
    let nextAnchorNode = nextNodeMap.get(nextSelection.anchor.key)
    let prevAnchorNode = nextNodeMap.get(prevSelection.anchor.key)

    if let nextAnchorNode = nextAnchorNode as? TextNode, prevAnchorNode != nil,
      !prevEditorState._nodeMap.has(nextAnchorNode.__key),
      nextAnchorNode.__text.utf16.count == 1, nextSelection.anchor.offset == 1
    {
      return INSERT_CHARACTER_AFTER_SELECTION
    }

    return OTHER
  }

  let nextDirtyNode = dirtyNodes[0]

  let prevDirtyNode = prevEditorState._nodeMap.get(nextDirtyNode.__key)

  guard let prevDirtyNode = prevDirtyNode as? TextNode,
    let nextDirtyNode = nextDirtyNode as? TextNode,
    prevDirtyNode.__mode == nextDirtyNode.__mode
  else {
    return OTHER
  }

  let prevText = prevDirtyNode.__text
  let nextText = nextDirtyNode.__text

  if prevText == nextText {
    return OTHER
  }

  let nextAnchor = nextSelection.anchor
  let prevAnchor = prevSelection.anchor

  if nextAnchor.key != prevAnchor.key || nextAnchor.type != .text {
    return OTHER
  }

  let nextAnchorOffset = nextAnchor.offset
  let prevAnchorOffset = prevAnchor.offset
  let textDiff = nextText.utf16.count - prevText.utf16.count

  if textDiff == 1 && prevAnchorOffset == nextAnchorOffset - 1 {
    return INSERT_CHARACTER_AFTER_SELECTION
  }

  if textDiff == -1 && prevAnchorOffset == nextAnchorOffset + 1 {
    return DELETE_CHARACTER_BEFORE_SELECTION
  }

  if textDiff == -1 && prevAnchorOffset == nextAnchorOffset {
    return DELETE_CHARACTER_AFTER_SELECTION
  }

  return OTHER
}

func isTextNodeUnchanged(
  _ key: NodeKey, _ prevEditorState: EditorState, _ nextEditorState: EditorState
) throws -> Bool {
  let prevNode = prevEditorState._nodeMap.get(key)
  let nextNode = nextEditorState._nodeMap.get(key)

  let prevSelection = prevEditorState._selection as? RangeSelection
  let nextSelection = nextEditorState._selection as? RangeSelection
  var isDeletingLine = false
  if let prevSelection, let nextSelection {
    isDeletingLine =
      prevSelection.anchor.type == .element && prevSelection.focus.type == .element
      && nextSelection.anchor.type == .text && nextSelection.focus.type == .text
  }

  if !isDeletingLine, let prevNode = prevNode as? TextNode,
    let nextNode = nextNode as? TextNode, prevNode.__parent == nextNode.__parent
  {
    // This has the assumption that object key order won't change if the
    // content did not change, which should normally be safe given
    // the manner in which nodes and exportJSON are typically implemented.
    return try prevEditorState.read { try prevNode.exportJSON() }.stringify()
      == nextEditorState.read { try nextNode.exportJSON() }.stringify()
  }
  return false
}

/// `$serializeNode`.
func serializeNode(_ node: LexicalNode) throws -> String {
  let children: JSONValue =
    if let element = node as? ElementNode {
      .array(try element.getChildrenKeys().map { .string($0) })
    } else {
      .null
    }
  return JSONValue.array([.object(try node.exportJSON()), children]).stringify()
}

func areNodesUnchanged(
  _ keys: JSSet<NodeKey>, _ prevEditorState: EditorState, _ nextEditorState: EditorState
) throws -> Bool {
  for key in keys {
    let prevNode = prevEditorState._nodeMap.get(key)
    let nextNode = nextEditorState._nodeMap.get(key)
    if prevNode === nextNode {
      continue
    }
    guard let prevNode, let nextNode,
      try prevEditorState.read({ try serializeNode(prevNode) })
        == nextEditorState.read({ try serializeNode(nextNode) })
    else {
      return false
    }
  }
  return true
}

func createMergeActionGetter(
  _ editor: Editor, _ readDelay: @escaping () -> Double, _ dateNow: @escaping () -> Double
) -> (
  _ prevEditorState: EditorState?, _ nextEditorState: EditorState,
  _ currentHistoryEntry: HistoryStateEntry?, _ dirtyLeaves: JSSet<NodeKey>,
  _ dirtyElements: JSMap<NodeKey, IntentionallyMarkedAsDirtyElement>, _ tags: JSSet<UpdateTag>
) throws -> MergeAction {
  var prevChangeTime = dateNow()
  var prevChangeType = OTHER
  var compositionStartTime = prevChangeTime
  var compositionStartChangeType = OTHER
  var compositionStartState: EditorState? = nil
  // Include every update in the composition, not only its final deletion.
  let compositionDirtyKeys = JSSet<NodeKey>()

  return {
    [unowned editor]
    prevEditorState, nextEditorState, currentHistoryEntry, dirtyLeaves,
    dirtyElements, tags in
    var prevEditorState = prevEditorState
    let changeTime = dateNow()

    if tags.has(COMPOSITION_START_TAG) {
      compositionStartTime = prevChangeTime
      compositionStartChangeType = prevChangeType
      compositionStartState = prevEditorState
      compositionDirtyKeys.clear()
    }
    if compositionStartState != nil {
      for key in dirtyLeaves {
        compositionDirtyKeys.add(key)
      }
      for key in dirtyElements.keysArray {
        compositionDirtyKeys.add(key)
      }
    }

    // If applying changes from history stack there's no need
    // to run history logic again, as history entries already calculated
    if tags.has(HISTORIC_TAG) {
      prevChangeType = OTHER
      prevChangeTime = changeTime
      compositionStartState = nil
      compositionDirtyKeys.clear()
      return DISCARD_HISTORY_CANDIDATE
    }

    let isCompositionEnd = tags.has(COMPOSITION_END_TAG)
    if isCompositionEnd, let compositionStartState {
      prevChangeTime = compositionStartTime
      prevChangeType = compositionStartChangeType
      prevEditorState = compositionStartState
    }

    // A paste or cut is never a continuation of the surrounding typing, so
    // classify it as OTHER. This both prevents it from merging into the
    // preceding keystrokes and, because it becomes `prevChangeType`, stops the
    // following keystrokes from merging into it — giving the clipboard
    // operation its own undo entry (see #8609). Centralizing this here means any
    // update carrying these tags gets an undo boundary, regardless of which
    // handler produced it.
    let changeType =
      tags.has(PASTE_TAG) || tags.has(CUT_TAG)
      ? OTHER
      : getChangeType(
        prevEditorState, nextEditorState, dirtyLeaves, dirtyElements, editor.isComposing())

    var isUnchangedComposition = false
    let mergeAction: MergeAction = try {
      let isSameEditor = currentHistoryEntry == nil || currentHistoryEntry!.editor === editor
      let shouldPushHistory = tags.has(HISTORY_PUSH_TAG)
      let shouldMergeHistory = !shouldPushHistory && isSameEditor && tags.has(HISTORY_MERGE_TAG)

      if isCompositionEnd, let compositionStartState, isSameEditor, !shouldPushHistory,
        try areNodesUnchanged(compositionDirtyKeys, compositionStartState, nextEditorState)
      {
        isUnchangedComposition = true
        return HISTORY_MERGE
      }

      if shouldMergeHistory {
        return HISTORY_MERGE
      }

      if changeType == COMPOSING_CHARACTER {
        return DISCARD_HISTORY_CANDIDATE
      }

      guard let prevEditorState else {
        return HISTORY_PUSH
      }

      let selection = nextEditorState._selection
      let hasDirtyNodes = dirtyLeaves.size > 0 || dirtyElements.size > 0

      if !hasDirtyNodes {
        if selection != nil {
          return HISTORY_MERGE
        }

        return DISCARD_HISTORY_CANDIDATE
      }

      let delay = readDelay()
      if shouldPushHistory == false && changeType != OTHER && changeType == prevChangeType
        && changeTime < prevChangeTime + delay && isSameEditor
      {
        return HISTORY_MERGE
      }

      // A single node might have been marked as dirty, but not have changed
      // due to some node transform reverting the change. An explicit
      // HISTORY_PUSH_TAG always asks for its own entry, so it must not be
      // merged away here even when the only dirty leaf is an unchanged
      // TextNode (see #9217).
      if !shouldPushHistory && dirtyLeaves.size == 1 {
        let dirtyLeafKey = dirtyLeaves.array[0]
        if try isTextNodeUnchanged(dirtyLeafKey, prevEditorState, nextEditorState) {
          return HISTORY_MERGE
        }
      }

      return HISTORY_PUSH
    }()

    if isCompositionEnd {
      compositionStartState = nil
      compositionDirtyKeys.clear()
    }
    if !isUnchangedComposition {
      prevChangeTime = changeTime
      prevChangeType = changeType
    }

    return mergeAction
  }
}

/// Build the entry that reverses `historyStateEntry` and belongs on the
/// opposite stack.
///
/// With a single editor this is always `current`, since `current` tracks the
/// live state of that editor. With a shared `HistoryState` the stacks
/// interleave entries from several editors, so `current` may belong to an
/// editor that is *not* about to change — pushing it would record a no-op and
/// lose the state we are about to overwrite. In that case read the live state
/// off the entry's own editor instead.
func getInverseEntry(
  _ historyStateEntry: HistoryStateEntry, _ current: HistoryStateEntry?
) -> HistoryStateEntry? {
  if let current, current.editor === historyStateEntry.editor {
    return current
  }
  let editor = historyStateEntry.editor
  let editorState = editor.getEditorState()
  // An empty EditorState can not be restored (setEditorState throws), so
  // there is nothing to reverse to.
  return editorState.isEmpty() ? nil : HistoryStateEntry(editor: editor, editorState: editorState)
}

/// Build the entry that an update from `editor` pushes onto the undo stack.
///
/// `current` is the state to restore when this update is undone, but with a
/// shared `HistoryState` it may belong to a different editor — one that
/// is not changing here, so restoring it would be a no-op and the state that is
/// about to be overwritten would never make it onto the stack. Record this
/// editor's own pre-update state in that case.
func getUndoEntry(
  _ editor: Editor, _ prevEditorState: EditorState, _ current: HistoryStateEntry?
) -> HistoryStateEntry? {
  guard let current else {
    return nil
  }
  if current.editor === editor {
    return current
  }
  // An empty EditorState can not be restored (setEditorState throws). Skipping
  // it mirrors the way the first update of an editor is not undoable.
  return prevEditorState.isEmpty()
    ? nil : HistoryStateEntry(editor: editor, editorState: prevEditorState)
}

func redo(
  _ editor: Editor, _ historyState: HistoryState,
  _ onChange: ((HistoryState) throws -> Void)? = nil
) throws {
  if !historyState.redoStack.isEmpty {
    let current = historyState.current
    let historyStateEntry = historyState.redoStack.popLast()

    if let historyStateEntry {
      if let inverseEntry = getInverseEntry(historyStateEntry, current) {
        historyState.undoStack.append(inverseEntry)
        try editor.dispatchCommand(CAN_UNDO_COMMAND, true)
      }
    }

    if historyState.redoStack.isEmpty {
      try editor.dispatchCommand(CAN_REDO_COMMAND, false)
    }

    historyState.current = historyStateEntry

    if let onChange {
      try onChange(historyState)
    }

    if let historyStateEntry {
      try historyStateEntry.editor.setEditorState(historyStateEntry.editorState, tag: HISTORIC_TAG)
    }
  }
}

func undo(
  _ editor: Editor, _ historyState: HistoryState,
  _ onChange: ((HistoryState) throws -> Void)? = nil
) throws {
  if !historyState.undoStack.isEmpty {
    let current = historyState.current
    let historyStateEntry = historyState.undoStack.popLast()

    if let historyStateEntry {
      if let inverseEntry = getInverseEntry(historyStateEntry, current) {
        historyState.redoStack.append(inverseEntry)
        try editor.dispatchCommand(CAN_REDO_COMMAND, true)
      }
    }

    if historyState.undoStack.isEmpty {
      try editor.dispatchCommand(CAN_UNDO_COMMAND, false)
    }

    historyState.current = historyStateEntry

    if let onChange {
      try onChange(historyState)
    }

    if let historyStateEntry {
      try historyStateEntry.editor.setEditorState(historyStateEntry.editorState, tag: HISTORIC_TAG)
    }
  }
}

func clearHistory(
  _ historyState: HistoryState, _ onChange: ((HistoryState) throws -> Void)? = nil
)
  throws
{
  historyState.undoStack = []
  historyState.redoStack = []
  historyState.current = nil
  if let onChange {
    try onChange(historyState)
  }
}

private let clockOrigin = ContinuousClock.now

/// The default clock of `registerHistory` and `HistoryConfig.now`: the
/// milliseconds elapsed on a monotonic clock, where JS uses `Date.now()`.
/// Only differences between two readings matter.
public func historyDateNow() -> Double {
  let (seconds, attoseconds) = (ContinuousClock.now - clockOrigin).components
  return Double(seconds) * 1000 + Double(attoseconds) / 1e15
}

/// `registerHistory`: registers the listeners that manage the undo/redo
/// history stack and the related editor commands. Returns the clean-up that
/// unregisters them.
/// - Parameters:
///   - delay: The time (in milliseconds) the editor should delay generating
///     a new history stack, instead of merging the current changes with the
///     current stack.
///   - dateNow: The clock used for delay-based merging.
///   - onHistoryStateChange: Called once on registration and again any time
///     `historyState` is mutated (push, pop, clear, etc.), but not when a
///     candidate update is discarded without changing the stacks.
///   - maxDepth: The maximum number of entries the undo stack may hold; the
///     oldest are dropped first. `nil` keeps the stack unbounded.
public func registerHistory(
  _ editor: Editor, _ historyState: HistoryState, _ delay: Double,
  _ dateNow: @escaping () -> Double = historyDateNow,
  _ onHistoryStateChange: ((HistoryState) throws -> Void)? = nil, _ maxDepth: Int? = nil
) throws -> () -> Void {
  try registerHistory(
    editor, historyState, readDelay: { delay }, dateNow, onHistoryStateChange,
    readMaxDepth: { maxDepth })
}

/// `registerHistory` with `delay` and `maxDepth` read from signals, so they
/// can be reconfigured while it is registered.
public func registerHistory(
  _ editor: Editor, _ historyState: HistoryState, _ delay: ReadonlySignal<Double>,
  _ dateNow: @escaping () -> Double = historyDateNow,
  _ onHistoryStateChange: ((HistoryState) throws -> Void)? = nil,
  _ maxDepth: ReadonlySignal<Int?>? = nil
) throws -> () -> Void {
  try registerHistory(
    editor, historyState, readDelay: { delay.peek() }, dateNow, onHistoryStateChange,
    readMaxDepth: { maxDepth?.peek() ?? nil })
}

func registerHistory(
  _ editor: Editor, _ historyState: HistoryState, readDelay: @escaping () -> Double,
  _ dateNow: @escaping () -> Double, _ onHistoryStateChange: ((HistoryState) throws -> Void)?,
  readMaxDepth: @escaping () -> Int?
) throws -> () -> Void {
  let getMergeAction = createMergeActionGetter(editor, readDelay, dateNow)

  let notifyChange = {
    if let onHistoryStateChange {
      try onHistoryStateChange(historyState)
    }
  }

  let applyChange = { [unowned editor] (payload: UpdateListenerPayload) throws in
    let editorState = payload.editorState
    let prevEditorState = payload.prevEditorState
    let current = historyState.current
    let currentEditorState = current?.editorState

    if current != nil && editorState === currentEditorState {
      return
    }

    let mergeAction = try getMergeAction(
      prevEditorState, editorState, current, payload.dirtyLeaves, payload.dirtyElements,
      payload.tags)

    if mergeAction == HISTORY_PUSH {
      if !historyState.redoStack.isEmpty {
        historyState.redoStack = []
        try editor.dispatchCommand(CAN_REDO_COMMAND, false)
      }

      if let undoEntry = getUndoEntry(editor, prevEditorState, current) {
        historyState.undoStack.append(undoEntry)
        if let cap = readMaxDepth(), historyState.undoStack.count > cap {
          // FIFO-evict the oldest entries so the stack stays at `cap`.
          historyState.undoStack.removeFirst(historyState.undoStack.count - cap)
        }
        try editor.dispatchCommand(CAN_UNDO_COMMAND, true)
      }
    } else if mergeAction == DISCARD_HISTORY_CANDIDATE {
      return
    }

    // Else we merge
    historyState.current = HistoryStateEntry(editor: editor, editorState: editorState)
    try notifyChange()
  }

  // Allow consumers (e.g. HistoryExtension) to read pre-populated stacks
  // immediately, without waiting for the first edit.
  try notifyChange()

  return mergeRegister(
    editor.registerCommand(
      UNDO_COMMAND,
      { _, editor in
        try undo(editor, historyState, onHistoryStateChange)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      REDO_COMMAND,
      { _, editor in
        try redo(editor, historyState, onHistoryStateChange)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      CLEAR_EDITOR_COMMAND,
      { _, _ in
        try clearHistory(historyState, onHistoryStateChange)
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      CLEAR_HISTORY_COMMAND,
      { _, editor in
        try clearHistory(historyState, onHistoryStateChange)
        try editor.dispatchCommand(CAN_REDO_COMMAND, false)
        try editor.dispatchCommand(CAN_UNDO_COMMAND, false)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerUpdateListener(applyChange)
  )
}

/// `createEmptyHistoryState`.
public func createEmptyHistoryState() -> HistoryState {
  HistoryState()
}

/// `HistoryConfig`.
public struct HistoryConfig {
  /// The time (in milliseconds) the editor should delay generating a new
  /// history stack, instead of merging the current changes with the current
  /// stack. The default is 300ms.
  public var delay: Double
  /// The initial history state, by default `createEmptyHistoryState`.
  public var createInitialHistoryState: (Editor) -> HistoryState
  /// Whether history is disabled. JS disables it by default where there is
  /// no `window` (server rendering); Swift always has an editing surface,
  /// so it defaults to `false`.
  public var disabled: Bool
  /// The clock, by default `historyDateNow` (JS `Date.now`).
  public var now: () -> Double
  /// The maximum number of entries the undo stack may hold. When the cap is
  /// exceeded the oldest entries are dropped (FIFO) so the stack stays at
  /// this length. `nil` (the default) keeps the stack unbounded.
  public var maxDepth: Int?

  public init(
    delay: Double = 300,
    createInitialHistoryState: @escaping (Editor) -> HistoryState = { _ in
      createEmptyHistoryState()
    },
    disabled: Bool = false, now: @escaping () -> Double = historyDateNow, maxDepth: Int? = nil
  ) {
    self.delay = delay
    self.createInitialHistoryState = createInitialHistoryState
    self.disabled = disabled
    self.now = now
    self.maxDepth = maxDepth
  }
}

/// The writable signals `HistoryExtension` creates during the init phase
/// (`HistoryExtensionInit`).
public struct HistoryExtensionInit {
  public let canRedo: Signal<Bool>
  public let canUndo: Signal<Bool>
}

/// `HistoryExtensionOutput`: the output signals of `HistoryExtension`.
///
/// The config-derived signals (`delay`, `disabled`, `historyState`,
/// `maxDepth`, `now`) are writable so that peers such as
/// `SharedHistoryExtension` can redirect them at runtime. `canUndo` and
/// `canRedo` are derived from the current `HistoryState` and kept in sync.
public struct HistoryExtensionOutput {
  /// Whether the redo stack has an entry.
  public let canRedo: ReadonlySignal<Bool>
  /// Whether the undo stack has an entry.
  public let canUndo: ReadonlySignal<Bool>
  /// The merge delay in milliseconds forwarded to `registerHistory`.
  public let delay: Signal<Double>
  /// When `true` the history listener is not registered.
  public let disabled: Signal<Bool>
  /// The active `HistoryState`.
  public let historyState: Signal<HistoryState>
  /// The cap of the undo stack (`nil` for none). Changes apply to the next
  /// history event; the current stack is not trimmed retroactively.
  public let maxDepth: Signal<Int?>
  /// The clock forwarded to `registerHistory`.
  public let now: Signal<() -> Double>
}

/// `HistoryExtension`: registers the listeners that manage the undo/redo
/// history stack and the related editor commands.
public let HistoryExtension = defineExtension(
  name: "@lexical/history/History", config: safeCast(HistoryConfig()),
  init: { _, _, _ in HistoryExtensionInit(canRedo: signal(false), canUndo: signal(false)) },
  build: { editor, config, state in
    let initResult = try state.getInitResult()
    return HistoryExtensionOutput(
      canRedo: initResult.canRedo, canUndo: initResult.canUndo, delay: signal(config.delay),
      disabled: signal(config.disabled),
      historyState: signal(config.createInitialHistoryState(editor)),
      maxDepth: signal(config.maxDepth), now: signal(config.now))
  },
  register: { editor, _, state in
    let initResult = try state.getInitResult()
    let canUndo = initResult.canUndo
    let canRedo = initResult.canRedo
    let stores = state.getOutput()
    // Single update path: passing `nil` resets both signals (used for the
    // disabled state); passing a HistoryState derives them from its stacks
    // (used by registerHistory on init and after every mutation). The batch
    // ensures subscribers to both signals are only notified once per change.
    let syncFromHistoryState = { (historyState: HistoryState?) throws in
      try batch {
        try canUndo.set(historyState != nil && !historyState!.undoStack.isEmpty)
        try canRedo.set(historyState != nil && !historyState!.redoStack.isEmpty)
      }
    }
    return try effect { () -> (() -> Void)? in
      if stores.disabled.value {
        try syncFromHistoryState(nil)
        return nil
      }
      return try registerHistory(
        editor, stores.historyState.value, stores.delay, { stores.now.peek()() },
        syncFromHistoryState, stores.maxDepth)
    }
  })

func getHistoryPeer(
  _ editor: Editor?
) throws -> LexicalExtensionDependency<HistoryConfig, HistoryExtensionOutput, HistoryExtensionInit>?
{
  guard let editor else { return nil }
  return try getPeerDependencyFromEditor(
    editor, HistoryExtension.name, as: type(of: HistoryExtension))
}

/// `SharedHistoryConfig`.
public struct SharedHistoryConfig {
  /// Whether shared history is disabled.
  public var disabled: Bool
  /// The parent editor to use; by default the editor's parent
  /// (`config.parentEditor`, which `NestedEditorExtension` sets).
  public var parentEditor: Editor?

  public init(disabled: Bool = false, parentEditor: Editor? = nil) {
    self.disabled = disabled
    self.parentEditor = parentEditor
  }
}

/// `SharedHistoryExtension`: registers the history listeners with the
/// parent editor's history, when the parent editor has `HistoryExtension`.
///
/// JS also re-links the two after `HMRExtension` restores either editor's
/// history; that extension is not ported (DESIGN.md).
public let SharedHistoryExtension = defineExtension(
  name: "@lexical/history/SharedHistory", config: safeCast(SharedHistoryConfig()),
  dependencies: [configExtension(HistoryExtension) { $0.disabled = true }],
  build: { editor, config, _ in
    namedSignals(
      SharedHistoryConfig(
        disabled: config.disabled, parentEditor: config.parentEditor ?? editor._parentEditor))
  },
  register: { _, _, state in
    try effect {
      let output = state.getOutput()
      let disabled = output.disabled
      let parentEditor = output.parentEditor
      if !disabled.value {
        let output = try state.getDependency(HistoryExtension).output
        guard let parentPeer = try getHistoryPeer(parentEditor.value) else {
          return
        }
        let parentOutput = parentPeer.output
        try batch {
          try output.delay.set(parentOutput.delay.value)
          try output.historyState.set(parentOutput.historyState.value)
          try output.now.set(parentOutput.now.value)
          // The cap must come from the parent too: the child pushes onto the
          // parent's shared undoStack, so applying the child's own (default
          // nil) maxDepth would silently void the limit the app configured.
          try output.maxDepth.set(parentOutput.maxDepth.value)
          // Note that toggling the parent history will force this to be changed
          try output.disabled.set(parentOutput.disabled.value)
        }
      }
    }
  })
