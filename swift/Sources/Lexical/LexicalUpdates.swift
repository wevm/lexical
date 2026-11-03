// Port of packages/lexical/src/LexicalUpdates.ts.

package func isDispatchingCommitListeners(_ editor: Editor) -> Bool {
  ThreadContext.current.editorsDispatchingCommitListeners.contains(ObjectIdentifier(editor))
}

package func isCurrentlyReadOnlyMode() -> Bool {
  let context = ThreadContext.current
  return context.isReadOnlyMode || (context.activeEditorState?._readOnly ?? false)
}

package func errorOnReadOnly() throws {
  if ThreadContext.current.isReadOnlyMode {
    throw LexicalError("Cannot use method in read-only mode.")
  }
}

package func errorOnInfiniteTransforms() throws {
  if ThreadContext.current.infiniteTransformCount > 99 {
    throw LexicalError(
      "One or more transforms are endlessly triggering additional transforms. May have encountered infinite recursion caused by transforms that have their preconditions too lose and/or conflict with each other."
    )
  }
}

package func getActiveEditorState() throws -> EditorState {
  guard let editorState = ThreadContext.current.activeEditorState else {
    throw LexicalError(
      "Unable to find an active editor state. State helpers or node methods can only be used synchronously during the callback of editor.update(), editor.read(), or editorState.read()."
    )
  }
  return editorState
}

package func getActiveEditor() throws -> Editor {
  guard let editor = ThreadContext.current.activeEditor else {
    throw LexicalError(
      "Unable to find an active editor. This method can only be used synchronously during the callback of editor.update(), editor.read(), or editor.getEditorState().read(..., {editor})."
    )
  }
  return editor
}

/// `$assumeActiveEditor`.
package func assumeActiveEditor(_ editor: Editor) throws {
  let context = ThreadContext.current
  if try getActiveEditorState() != nil && context.activeEditor == nil {
    context.activeEditor = editor
  }
  try devInvariant(
    context.activeEditor === editor,
    "The given editor argument does not match $getEditor() in this context. Use editor.getEditorState().read(..., {editor}) if this cross-editor call is intentional."
  )
}

/// `$fullReconcile`: reconcile the whole tree in this update.
package func fullReconcile() {
  ThreadContext.current.activeEditor?._dirtyType = FULL_RECONCILE
}

package func internalGetActiveEditor() -> Editor? {
  ThreadContext.current.activeEditor
}

package func internalGetActiveEditorState() -> EditorState? {
  ThreadContext.current.activeEditorState
}

package func applyTransforms(
  _ editor: Editor, _ node: LexicalNode, _ transformsCache: inout [String: [Transform]]
) throws {
  let type = node.__type
  let registeredNode = try getRegisteredNodeOrThrow(editor, type)
  let transforms: [Transform]
  if let cached = transformsCache[type] {
    transforms = cached
  } else {
    transforms = registeredNode.transformList
    transformsCache[type] = transforms
  }
  for transform in transforms {
    try transform.fn(node)
    if try !node.isAttached() {
      break
    }
  }
}

func isNodeValidForTransform(_ node: LexicalNode, _ compositionKey: NodeKey?) throws -> Bool {
  try node.__key != compositionKey && node.isAttached()
}

func normalizeAllDirtyTextNodes(_ editorState: EditorState, _ editor: Editor) throws {
  let dirtyLeaves = editor._dirtyLeaves
  let nodeMap = editorState._nodeMap
  for nodeKey in dirtyLeaves {
    if let node = nodeMap.get(nodeKey) as? TextNode, try node.isAttached(), try node.isSimpleText(),
      try !node.isUnmergeable()
    {
      try normalizeTextNode(node)
    }
  }
}

func addTags(_ editor: Editor, _ tags: [UpdateTag]) {
  for tag in tags {
    editor._updateTags.add(tag)
  }
}

/// Transforms run on every dirty node until none is left: dirty leaves
/// first, then dirty elements (the root last), repeatedly, since a transform
/// can dirty more nodes.
func applyAllTransforms(_ editorState: EditorState, _ editor: Editor) throws {
  let dirtyLeaves = editor._dirtyLeaves
  let dirtyElements = editor._dirtyElements
  let nodeMap = editorState._nodeMap
  let compositionKey = try getCompositionKey()
  var transformsCache: [String: [Transform]] = [:]
  var untransformedDirtyLeaves = dirtyLeaves
  var untransformedDirtyLeavesLength = untransformedDirtyLeaves.size
  var untransformedDirtyElements = dirtyElements
  var untransformedDirtyElementsLength = untransformedDirtyElements.size
  let context = ThreadContext.current
  while untransformedDirtyLeavesLength > 0 || untransformedDirtyElementsLength > 0 {
    if untransformedDirtyLeavesLength > 0 {
      // We leverage editor._dirtyLeaves to track the new dirty leaves after
      // the transforms
      editor._dirtyLeaves = JSSet()
      for nodeKey in untransformedDirtyLeaves {
        let node = nodeMap.get(nodeKey)
        if let text = node as? TextNode, try text.isAttached(), try text.isSimpleText(),
          try !text.isUnmergeable()
        {
          try normalizeTextNode(text)
        }
        if let node, try isNodeValidForTransform(node, compositionKey) {
          try applyTransforms(editor, node, &transformsCache)
        }
        dirtyLeaves.add(nodeKey)
      }
      untransformedDirtyLeaves = editor._dirtyLeaves
      untransformedDirtyLeavesLength = untransformedDirtyLeaves.size
      // We want to prioritize node transforms over element transforms
      if untransformedDirtyLeavesLength > 0 {
        context.infiniteTransformCount += 1
        continue
      }
    }
    // All dirty leaves have been processed. Let's do elements! We have new
    // dirty leaves, so we should reset the dirty elements.
    editor._dirtyLeaves = JSSet()
    editor._dirtyElements = JSMap()
    // The root is always considered intentionally dirty if any attached node
    // changed, and it has been deleted from the untransformedDirtyElements
    // to be processed last.
    let rootDirty = untransformedDirtyElements.delete("root")
    if rootDirty {
      untransformedDirtyElements.set("root", true)
    }
    for (nodeKey, intentionallyMarkedAsDirty) in untransformedDirtyElements {
      dirtyElements.set(nodeKey, intentionallyMarkedAsDirty)
      if !intentionallyMarkedAsDirty {
        continue
      }
      if let node = nodeMap.get(nodeKey), try isNodeValidForTransform(node, compositionKey) {
        try applyTransforms(editor, node, &transformsCache)
      }
    }
    untransformedDirtyLeaves = editor._dirtyLeaves
    untransformedDirtyLeavesLength = untransformedDirtyLeaves.size
    untransformedDirtyElements = editor._dirtyElements
    untransformedDirtyElementsLength = untransformedDirtyElements.size
    context.infiniteTransformCount += 1
  }
  editor._dirtyLeaves = dirtyLeaves
  editor._dirtyElements = dirtyElements
}

/// `$parseSerializedNode`: a node from its JSON, in either form.
public func parseSerializedNode(_ serializedNode: JSONObject) throws -> LexicalNode {
  try parseSerializedNodeImpl(serializedNode, getActiveEditor()._nodes)
}

func parseSerializedNodeImpl(
  _ serializedNode: JSONObject, _ registeredNodes: JSMap<String, RegisteredNode>
) throws -> LexicalNode {
  let type = serializedNode["type"]?.stringValue ?? ""
  guard let registeredNode = registeredNodes.get(type) else {
    throw LexicalError("parseEditorState: type \"\(type)\" + not found")
  }
  let nodeClass = registeredNode.klass
  if type != nodeClass.getType() {
    throw LexicalError("LexicalNode: Node \(nodeClass) does not implement .importJSON().")
  }
  let node = try nodeClass.importJSON(serializedNode)
  if let element = node as? ElementNode, case .array(let children)? = serializedNode["children"] {
    for child in children {
      guard case .object(let serializedChild) = child else {
        throw LexicalError("parseEditorState: a child is not a serialized node")
      }
      let childNode = try parseSerializedNodeImpl(serializedChild, registeredNodes)
      try element.append(childNode)
    }
  }
  if case .object(let slots)? = serializedNode["$slots"] {
    try invariant(
      isSlotHost(node),
      "$parseSerializedNode: node \(nodeClass) has slots but is not a valid slot host; only ElementNodes and DecoratorNodes can host slots."
    )
    for (name, value) in slots {
      guard case .object(let serializedSlot) = value else { continue }
      let slotNode = try parseSerializedNodeImpl(serializedSlot, registeredNodes)
      try setSlot(node, name, slotNode)
    }
  }
  return node
}

package func parseEditorState(
  _ serializedEditorState: JSONObject, _ editor: Editor, _ updateFn: (() throws -> Void)?
) throws -> EditorState {
  let editorState = createEmptyEditorState()
  let context = ThreadContext.current
  let previousActiveEditorState = context.activeEditorState
  let previousReadOnlyMode = context.isReadOnlyMode
  let previousActiveEditor = context.activeEditor
  let previousDirtyElements = editor._dirtyElements
  let previousDirtyLeaves = editor._dirtyLeaves
  let previousCloneNotNeeded = editor._cloneNotNeeded
  let previousDirtyType = editor._dirtyType
  editor._dirtyElements = JSMap()
  editor._dirtyLeaves = JSSet()
  editor._cloneNotNeeded = JSMap()
  editor._dirtyType = NO_DIRTY_NODES
  context.activeEditorState = editorState
  context.isReadOnlyMode = false
  context.activeEditor = editor
  defer {
    editor._dirtyElements = previousDirtyElements
    editor._dirtyLeaves = previousDirtyLeaves
    editor._cloneNotNeeded = previousCloneNotNeeded
    editor._dirtyType = previousDirtyType
    context.activeEditorState = previousActiveEditorState
    context.isReadOnlyMode = previousReadOnlyMode
    context.activeEditor = previousActiveEditor
  }
  do {
    let registeredNodes = editor._nodes
    guard case .object(let serializedNode)? = serializedEditorState["root"] else {
      throw LexicalError("parseEditorState: the serialized state has no root")
    }
    _ = try parseSerializedNodeImpl(serializedNode, registeredNodes)
    try updateFn?()
    // Make the editorState immutable
    editorState._readOnly = true
    editorState._parsed = true
  } catch {
    try editor._onError(error)
  }
  return editorState
}

/// Runs `callback` with `editorState` (and `editor`) active, read-only.
package func readEditorState<V>(
  _ editor: Editor?, _ editorState: EditorState, _ callback: () throws -> V
) throws -> V {
  let context = ThreadContext.current
  let previousActiveEditorState = context.activeEditorState
  let previousReadOnlyMode = context.isReadOnlyMode
  let previousActiveEditor = context.activeEditor
  context.activeEditorState = editorState
  context.isReadOnlyMode = true
  context.activeEditor = editor
  defer {
    context.activeEditorState = previousActiveEditorState
    context.isReadOnlyMode = previousReadOnlyMode
    context.activeEditor = previousActiveEditor
  }
  return try callback()
}

package func commitPendingUpdates(
  _ editor: Editor, _ recoveryEditorState: EditorState? = nil
)
  throws
{
  let context = ThreadContext.current
  let previouslyCommitting = context.isCommittingPendingUpdates
  context.isCommittingPendingUpdates = true
  defer { context.isCommittingPendingUpdates = previouslyCommitting }
  let notificationResult = try notifyPendingSelectionChange(editor)
  try commitPendingUpdatesImpl(editor, recoveryEditorState)
  if notificationResult {
    try editor._onWarn(
      LexicalError("Selection change listeners are endlessly changing the selection."))
  }
}

func commitPendingUpdatesImpl(_ editor: Editor, _ recoveryEditorState: EditorState?) throws {
  guard let pendingEditorState = editor._pendingEditorState else {
    if !editor._updating && !editor._deferred.isEmpty {
      try triggerDeferredUpdateCallbacks(editor, editor._deferred, clearEditorDeferred: true)
    }
    return
  }
  let context = ThreadContext.current
  let currentEditorState = editor._editorState
  let needsUpdate = editor._dirtyType != NO_DIRTY_NODES
  let previouslyUpdating = editor._updating
  let mutatedNodes: MutatedNodes? = nil

  editor._pendingEditorState = nil
  editor._editorState = pendingEditorState

  // The reconciler runs here once a view is attached (see the UI layers).

  if !pendingEditorState._readOnly {
    pendingEditorState._readOnly = true
  }
  let dirtyLeaves = editor._dirtyLeaves
  let dirtyElements = editor._dirtyElements
  let normalizedNodes = editor._normalizedNodes
  let tags = editor._updateTags
  if needsUpdate {
    editor._dirtyType = NO_DIRTY_NODES
    editor._cloneNotNeeded.clear()
    editor._dirtyLeaves = JSSet()
    editor._dirtyElements = JSMap()
    editor._normalizedNodes = JSSet()
  }
  editor._updateTags = JSSet()
  let deferred = editor._deferred
  if !previouslyUpdating {
    editor._deferred = []
  }
  garbageCollectDetachedDecorators(editor, pendingEditorState)

  let id = ObjectIdentifier(editor)
  let previouslyDispatching = context.editorsDispatchingCommitListeners.contains(id)
  context.editorsDispatchingCommitListeners.insert(id)
  defer {
    if !previouslyDispatching {
      context.editorsDispatchingCommitListeners.remove(id)
    }
  }
  if let mutatedNodes {
    try triggerMutationListeners(editor, mutatedNodes, tags, dirtyLeaves, currentEditorState)
  }
  if let pendingDecorators = editor._pendingDecorators {
    editor._decorators = pendingDecorators
    editor._pendingDecorators = nil
    try triggerListeners(editor._decoratorListeners, editor, true) { try $0(pendingDecorators) }
  }
  try triggerTextContentListeners(
    editor, recoveryEditorState ?? currentEditorState, pendingEditorState)
  let payload = UpdateListenerPayload(
    dirtyElements: dirtyElements, dirtyLeaves: dirtyLeaves, editorState: pendingEditorState,
    mutatedNodes: mutatedNodes, normalizedNodes: normalizedNodes,
    prevEditorState: recoveryEditorState ?? currentEditorState, tags: tags)
  try triggerListeners(editor._updateListeners, editor, true) { try $0(payload) }
  if !previouslyUpdating {
    try triggerDeferredUpdateCallbacks(editor, deferred, clearEditorDeferred: false)
  }
  try triggerEnqueuedUpdates(editor)
}

func triggerTextContentListeners(
  _ editor: Editor, _ currentEditorState: EditorState, _ pendingEditorState: EditorState
) throws {
  if editor._textContentListeners.size == 0 {
    return
  }
  let currentTextContent = try getEditorStateTextContent(currentEditorState)
  let latestTextContent = try getEditorStateTextContent(pendingEditorState)
  if currentTextContent != latestTextContent {
    try triggerListeners(editor._textContentListeners, editor, true) { try $0(latestTextContent) }
  }
}

func triggerMutationListeners(
  _ editor: Editor, _ mutatedNodes: MutatedNodes, _ updateTags: JSSet<UpdateTag>,
  _ dirtyLeaves: JSSet<NodeKey>, _ prevEditorState: EditorState
) throws {
  let listeners = Array(editor._mutationListeners)
  for (_, (listener, klassSet)) in listeners {
    for klass in klassSet {
      if let mutatedNodesByType = mutatedNodes.get(klass) {
        try listener(
          mutatedNodesByType,
          MutationListenerPayload(
            updateTags: updateTags, dirtyLeaves: dirtyLeaves, prevEditorState: prevEditorState))
      }
    }
  }
}

func triggerEditableListeners(_ editor: Editor, _ editable: Bool) throws {
  try triggerListeners(editor._editableListeners, editor, true) { try $0(editable) }
}

/// `triggerListeners`: calls each listener registered when it starts, unless
/// it has been unregistered since.
package func triggerListeners<L>(
  _ listenerMap: ListenerMap<L>, _ editor: Editor, _ isCurrentlyEnqueuingUpdates: Bool,
  _ call: (L) throws -> Void
) throws {
  let previouslyUpdating = editor._updating
  editor._updating = isCurrentlyEnqueuingUpdates
  defer { editor._updating = previouslyUpdating }
  let listeners = Array(listenerMap.entries)
  for (id, entry) in listeners {
    if let unregister = entry.unregister {
      entry.unregister = nil
      unregister()
    }
    try call(entry.listener)
    if !listenerMap.entries.has(id) {
      continue
    }
  }
}

func hasSelectionChanged(_ editor: Editor, _ selection: BaseSelection?) -> Bool {
  let previous = editor._lastNotifiedSelection
  guard let selection else {
    return previous != nil
  }
  return !selection.is(previous)
}

/// `$dispatchSelectionChangeCommand`.
package func dispatchSelectionChangeCommand(
  _ editor: Editor, _ selection: BaseSelection?, force: Bool = false
) throws {
  if !force && !hasSelectionChanged(editor, selection) {
    return
  }
  try editor.dispatchCommand(SELECTION_CHANGE_COMMAND)
}

func notifyPendingSelectionChange(_ editor: Editor) throws -> Bool {
  let context = ThreadContext.current
  let id = ObjectIdentifier(editor)
  if context.editorsWithPendingSelectionChange.contains(id) {
    return false
  }
  context.editorsWithPendingSelectionChange.insert(id)
  defer { context.editorsWithPendingSelectionChange.remove(id) }
  var count = 0
  while let pending = editor._pendingEditorState {
    let selection = pending._selection
    if !hasSelectionChanged(editor, selection) {
      return false
    }
    // A headless editor (or one without a connected view) has no native
    // selection to notify about for a range selection.
    let skipNotification =
      (selection == nil || selection is RangeSelection)
      && (editor._headless || editor._rootElement == nil)
    if skipNotification || count == 100 {
      editor._lastNotifiedSelection = selection?.clone()
      return !skipNotification
    }
    try beginUpdate(
      editor, { try editor.dispatchCommand(SELECTION_CHANGE_COMMAND) }, nil, skipCommit: true)
    count += 1
  }
  return false
}

package func triggerCommandListeners<P>(
  _ editor: Editor, _ type: LexicalCommand<P>, _ payload: P, _ fromEditor: Editor
) throws -> Bool {
  let context = ThreadContext.current
  let editors = getEditorsToPropagate(editor)
  var updatingParentEditor: Editor? = nil
  if !context.isCommittingPendingUpdates {
    for e in editors where !e._updating {
      e._cascadeCount = 0
    }
  }
  if type === SELECTION_CHANGE_COMMAND {
    if context.activeEditor !== editor || context.isReadOnlyMode {
      var handled = false
      try updateEditorSync(
        editor,
        {
          handled = try triggerCommandListeners(editor, type, payload, fromEditor)
        })
      return handled
    }
    editor._lastNotifiedSelection = try getActiveEditorState()._selection?.clone()
  }
  let id = ObjectIdentifier(type)
  for i in stride(from: 4, through: 0, by: -1) {
    for (e, currentEditor) in editors.enumerated() {
      if e > 0 && currentEditor._updating {
        updatingParentEditor = currentEditor
        break
      }
      if let listenerInPriorityOrder = currentEditor._commands[id] {
        let listenersSet = listenerInPriorityOrder[i]
        if listenersSet.size > 0 {
          var returnVal = false
          try updateEditorSync(
            currentEditor,
            {
              for listener in listenersSet.array {
                if try listener.listener(payload, fromEditor) {
                  returnVal = true
                  return
                }
              }
            })
          if returnVal {
            return returnVal
          }
        }
      }
    }
  }
  if let updatingParentEditor {
    try updatingParentEditor.update({
      _ = try triggerCommandListeners(updatingParentEditor, type, payload, fromEditor)
    })
  }
  return false
}

func scheduleCascadeReset(_ editor: Editor) {
  let context = ThreadContext.current
  let id = ObjectIdentifier(editor)
  if context.editorsWithPendingCascadeReset.contains(id) {
    return
  }
  context.editorsWithPendingCascadeReset.insert(id)
  // `setTimeout(..., 0)`: after the microtasks of this turn.
  scheduleMicroTask { [weak editor] in
    ThreadContext.current.editorsWithPendingCascadeReset.remove(id)
    editor?._cascadeCount = 0
  }
}

func triggerEnqueuedUpdates(_ editor: Editor) throws {
  let queuedUpdates = editor._updates
  if queuedUpdates.isEmpty {
    editor._cascadeCount = 0
    return
  }
  scheduleCascadeReset(editor)
  let cascadeCount = editor._cascadeCount
  editor._cascadeCount += 1
  if cascadeCount > 99 {
    editor._updates = []
    editor._cascadeCount = 0
    try editor._onWarn(
      LexicalError(
        "One or more update listeners are endlessly enqueueing more updates. May have encountered infinite recursion caused by update listeners that trigger additional updates without a stop condition. Editor namespace: \(editor._config.namespace)"
      ))
    return
  }
  let queuedUpdate = editor._updates.removeFirst()
  try beginUpdate(editor, queuedUpdate.updateFn, queuedUpdate.options)
}

/// `clearEditorDeferred` is JS's `editor._deferred === deferred`: whether
/// `deferred` is the editor's own queue rather than one already taken off it.
func triggerDeferredUpdateCallbacks(
  _ editor: Editor, _ deferred: [() throws -> Void], clearEditorDeferred: Bool
) throws {
  if clearEditorDeferred {
    editor._deferred = []
  }
  if !deferred.isEmpty {
    let previouslyUpdating = editor._updating
    editor._updating = true
    defer { editor._updating = previouslyUpdating }
    for callback in deferred {
      try callback()
    }
  }
}

func processNestedUpdates(_ editor: Editor, _ initialSkipTransforms: Bool = false) throws -> Bool {
  var skipTransforms = initialSkipTransforms
  // Updates might grow as we process them, we so we'll need to handle each
  // update as we go until the updates array is empty.
  while !editor._updates.isEmpty {
    let queuedUpdate = editor._updates.removeFirst()
    let nextUpdateFn = queuedUpdate.updateFn
    let options = queuedUpdate.options
    let pendingEditorState = editor._pendingEditorState
    if let options {
      if options.skipTransforms {
        skipTransforms = true
      }
      if options.discrete {
        try invariant(
          pendingEditorState != nil,
          "Unexpected empty pending editor state on discrete nested update")
        pendingEditorState!._flushSync = true
      }
      if let onUpdate = options.onUpdate {
        editor._deferred.append(onUpdate)
      }
      addTags(editor, options.tag)
    }
    if pendingEditorState == nil {
      try beginUpdate(editor, nextUpdateFn, options)
    } else {
      try nextUpdateFn()
    }
  }
  return skipTransforms
}

/// `$flushSyncAfterUpdate`: commit this update synchronously.
package func flushSyncAfterUpdate() throws {
  let editorState = try getActiveEditorState()
  try errorOnReadOnly()
  editorState._flushSync = true
}

func discardPendingUpdate(_ editor: Editor, _ queuedBefore: Set<ObjectIdentifier>?) {
  let committedEditorState = editor._editorState
  editor._pendingEditorState = committedEditorState
  editor._lastNotifiedSelection = committedEditorState._selection?.clone()
  editor._dirtyType = FULL_RECONCILE
  editor._cloneNotNeeded.clear()
  editor._dirtyLeaves = JSSet()
  editor._dirtyElements.clear()
  // Updates queued before the failed one stay queued; the ones it queued go.
  editor._updates.removeAll { !(queuedBefore?.contains(ObjectIdentifier($0)) ?? false) }
}

func beginUpdate(
  _ editor: Editor, _ updateFn: @escaping () throws -> Void, _ options: EditorUpdateOptions?,
  skipCommit: Bool = false
) throws {
  let context = ThreadContext.current
  let updateTags = editor._updateTags
  var skipTransforms = false
  var discrete = false
  if let options {
    if let onUpdate = options.onUpdate {
      editor._deferred.append(onUpdate)
    }
    addTags(editor, options.tag)
    skipTransforms = options.skipTransforms
    discrete = options.discrete
  }
  let currentEditorState = editor._editorState
  var pendingEditorState = editor._pendingEditorState
  var editorStateWasCloned = false
  if pendingEditorState == nil || pendingEditorState!._readOnly {
    pendingEditorState = cloneEditorState(pendingEditorState ?? currentEditorState)
    editor._pendingEditorState = pendingEditorState
    editorStateWasCloned = true
  }
  let pending = pendingEditorState!
  pending._flushSync = discrete

  let queuedBefore =
    editor._updates.isEmpty ? nil : Set(editor._updates.map(ObjectIdentifier.init))
  let previousActiveEditorState = context.activeEditorState
  let previousReadOnlyMode = context.isReadOnlyMode
  let previousActiveEditor = context.activeEditor
  let previouslyUpdating = editor._updating
  let previouslyNotifiedSelection = editor._lastNotifiedSelection
  context.activeEditorState = pending
  context.isReadOnlyMode = false
  editor._updating = true
  context.activeEditor = editor

  func restore() {
    context.activeEditorState = previousActiveEditorState
    context.isReadOnlyMode = previousReadOnlyMode
    context.activeEditor = previousActiveEditor
    editor._updating = previouslyUpdating
    context.infiniteTransformCount = 0
  }

  do {
    if editorStateWasCloned {
      // Without a view, there is no native selection to read: the selection
      // carries over from the current state.
      if let selection = currentEditorState._selection {
        pending._selection = selection.clone()
      }
    }
    let startingCompositionKey = editor._compositionKey
    try updateFn()
    skipTransforms = try processNestedUpdates(editor, skipTransforms)
    try applySelectionTransforms(pending, editor)
    if editor._dirtyType != NO_DIRTY_NODES {
      if skipTransforms {
        try normalizeAllDirtyTextNodes(pending, editor)
      } else {
        try applyAllTransforms(pending, editor)
      }
      _ = try processNestedUpdates(editor)
      try garbageCollectDetachedNodes(
        currentEditorState, pending, editor._dirtyLeaves, editor._dirtyElements)
    }
    let endingCompositionKey = editor._compositionKey
    if startingCompositionKey != endingCompositionKey {
      pending._flushSync = true
    }
    let pendingSelection = pending._selection
    if let pendingSelection = pendingSelection as? RangeSelection {
      if editor._slotsUsed {
        try clampRangeSelectionToSlotFrame(pendingSelection)
      }
      let pendingNodeMap = pending._nodeMap
      if pendingNodeMap.get(pendingSelection.anchor.key) == nil
        || pendingNodeMap.get(pendingSelection.focus.key) == nil
      {
        throw LexicalError(
          "updateEditor: selection has been lost because the previously selected nodes have been removed and selection wasn't moved to another node. Ensure selection changes after removing/replacing a selected node."
        )
      }
    } else if let nodeSelection = pendingSelection as? NodeSelection {
      // TODO: we should also validate node selection?
      if nodeSelection._nodes.size == 0 {
        pending._selection = nil
      }
    }
  } catch {
    // A nested update that throws rethrows to the update it runs in, which
    // owns the pending state.
    if !editorStateWasCloned && previousActiveEditorState === pending {
      restore()
      throw error
    }
    restore()
    discardPendingUpdate(editor, queuedBefore)
    var rethrown: Error? = nil
    do {
      try editor._onError(error)
    } catch {
      rethrown = error
    }
    do {
      try commitPendingUpdates(editor)
    } catch {
      scheduleMicroTask { try? triggerEnqueuedUpdates(editor) }
      if rethrown == nil {
        throw error
      }
      try? editor._onError(error)
    }
    if let rethrown {
      throw rethrown
    }
    return
  }
  restore()

  if skipCommit {
    return
  }

  let shouldUpdate =
    editor._dirtyType != NO_DIRTY_NODES || !editor._deferred.isEmpty
    || editorStateHasDirtySelection(pending, editor)

  if shouldUpdate {
    if pending._flushSync {
      pending._flushSync = false
      try commitPendingUpdates(editor)
    } else if editorStateWasCloned {
      scheduleMicroTask { [weak editor] in
        guard let editor else { return }
        do {
          try commitPendingUpdates(editor)
        } catch {
          try? editor._onError(error)
        }
      }
    }
  } else {
    pending._flushSync = false
    if editorStateWasCloned {
      updateTags.clear()
      editor._deferred = []
      editor._pendingEditorState = nil
      editor._lastNotifiedSelection = previouslyNotifiedSelection
    }
  }
}

package func updateEditorSync(
  _ editor: Editor, _ updateFn: @escaping () throws -> Void, _ options: EditorUpdateOptions? = nil
) throws {
  let context = ThreadContext.current
  if context.activeEditor === editor && options == nil {
    if isCurrentlyReadOnlyMode() {
      // An update dispatched from a read-only context runs as a fresh
      // writable update so it still applies.
      try beginUpdate(editor, updateFn, options)
    } else {
      try updateFn()
    }
  } else {
    try beginUpdate(editor, updateFn, options)
  }
}

package func updateEditor(
  _ editor: Editor, _ updateFn: @escaping () throws -> Void, _ options: EditorUpdateOptions?
) throws {
  if editor._updating {
    editor._updates.append(QueuedUpdate(updateFn, options))
  } else {
    try beginUpdate(editor, updateFn, options)
  }
}

/// An update waiting for the one in progress: compared by identity, as JS
/// compares the queued tuples.
package final class QueuedUpdate {
  let updateFn: () throws -> Void
  let options: EditorUpdateOptions?
  init(_ updateFn: @escaping () throws -> Void, _ options: EditorUpdateOptions?) {
    self.updateFn = updateFn
    self.options = options
  }
}
