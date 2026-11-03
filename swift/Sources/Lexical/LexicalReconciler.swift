// Port of packages/lexical/src/LexicalReconciler.ts: the model half.
//
// JS reconciles the editor state into the DOM after every commit while a
// root element is attached. Besides rendering, that walk is where the model
// learns what it cannot compute alone: which nodes were created, updated or
// destroyed (for mutation listeners), the root's cached text, each element's
// `__textFormat` / `__textStyle` (taken from its first text descendant),
// and the decorators to render. This file does that walk over the same
// trees, in the same order, with each keyed DOM element replaced by a
// `ReconciledNode` that holds the caches JS keeps on it
// (`__lexicalTextContent`, `__lexicalFirstTextKey`). The native views render
// from the committed state.
//
// The children fast paths of `$reconcileChildren` (suffix splicing and the
// same-size walk) are not ported: they skip DOM work and produce the same
// results as the general path, which this port always takes.

/// The stand-in for an element's keyed DOM: what the reconciler caches on
/// it between commits.
package final class ReconciledNode {
  /// `__lexicalTextContent`: the subtree's text (elements only).
  var textContent: String?
  /// `__lexicalFirstTextKey`: the first text descendant (elements only);
  /// `.some(nil)` when there is none, `nil` while unset.
  var firstTextKey: NodeKey??
  /// The slot container this node is mounted in, as `host/name`, for slot
  /// values.
  var slotContainer: String?
}

/// `setMutatedNode`.
package func setMutatedNode(
  _ mutatedNodes: MutatedNodes, _ editor: Editor, _ node: LexicalNode, _ mutation: NodeMutation
) throws {
  if editor._mutationListeners.size == 0 {
    return
  }
  let nodeType = node.__type
  let nodeKey = node.__key
  guard let registeredNode = editor._nodes.get(nodeType) else {
    throw LexicalError("Type \(nodeType) not in registeredNodes")
  }
  let klass = ObjectIdentifier(registeredNode.klass)
  let mutatedNodesByType: JSMap<NodeKey, NodeMutation>
  if let existing = mutatedNodes.get(klass) {
    mutatedNodesByType = existing
  } else {
    mutatedNodesByType = JSMap()
    mutatedNodes.set(klass, mutatedNodesByType)
  }
  let prevMutation = mutatedNodesByType.get(nodeKey)
  // If the node has already been "destroyed", yet we are re-making it, then
  // this means a move likely happened. We should change the mutation to be
  // that of "updated" instead.
  let isMove = prevMutation == "destroyed" && mutation == "created"
  if prevMutation == nil || isMove {
    mutatedNodesByType.set(nodeKey, isMove ? "updated" : mutation)
  }
}

private func readSlots(_ node: LexicalNode) -> SlotMap {
  (node as? any SlotNode)?.__slots ?? SlotMap()
}

private func isSlotChild(_ node: LexicalNode) -> Bool {
  slotHostKeyField(node) != nil
}

/// `$reconcileRoot`: reconciles `nextEditorState` against `prevEditorState`
/// and returns the mutated nodes. Runs in the pending update context.
package func reconcileRoot(
  _ prevEditorState: EditorState, _ nextEditorState: EditorState, _ editor: Editor,
  _ dirtyType: Int, _ dirtyElements: JSMap<NodeKey, Bool>, _ dirtyLeaves: JSSet<NodeKey>
) throws -> MutatedNodes {
  let reconciler = Reconciler(
    editor: editor, prevEditorState: prevEditorState, nextEditorState: nextEditorState,
    treatAllNodesAsDirty: dirtyType == FULL_RECONCILE, dirtyElements: dirtyElements,
    dirtyLeaves: dirtyLeaves)
  _ = try reconciler.reconcileNode("root")
  return reconciler.mutatedNodes
}

private final class Reconciler {
  let editor: Editor
  let treatAllNodesAsDirty: Bool
  let editorStateReadOnly: Bool
  let dirtyElements: JSMap<NodeKey, Bool>
  let dirtyLeaves: JSSet<NodeKey>
  let prevNodeMap: NodeMap
  let nextNodeMap: NodeMap
  let prevKeyToDOMMap: JSMap<NodeKey, ReconciledNode>
  let mutatedNodes = MutatedNodes()

  // The sub-tree accumulators, reset at the start of each cycle.
  var subTreeTextContent = ""
  var subTreeTextFormat: Int?
  var subTreeTextStyle: String?
  var subTreeFirstTextKey: NodeKey?

  init(
    editor: Editor, prevEditorState: EditorState, nextEditorState: EditorState,
    treatAllNodesAsDirty: Bool, dirtyElements: JSMap<NodeKey, Bool>, dirtyLeaves: JSSet<NodeKey>
  ) {
    self.editor = editor
    self.treatAllNodesAsDirty = treatAllNodesAsDirty
    self.editorStateReadOnly = nextEditorState._readOnly
    self.dirtyElements = dirtyElements
    self.dirtyLeaves = dirtyLeaves
    self.prevNodeMap = prevEditorState._nodeMap
    self.nextNodeMap = nextEditorState._nodeMap
    self.prevKeyToDOMMap = JSMap(Array(editor._keyToDOMMap))
  }

  // MARK: First-text capture

  struct CaptureGuard {
    let firstTextKey: NodeKey?
    let format: Int?
    let style: String?
  }

  func beginCaptureGuard() -> CaptureGuard {
    CaptureGuard(
      firstTextKey: subTreeFirstTextKey, format: subTreeTextFormat, style: subTreeTextStyle)
  }

  func endCaptureGuard(_ saved: CaptureGuard) {
    if saved.firstTextKey != nil {
      subTreeTextFormat = saved.format
      subTreeTextStyle = saved.style
      subTreeFirstTextKey = saved.firstTextKey
    }
  }

  /// `$bubbleChildFirstText`.
  func bubbleChildFirstText(_ childKeyedDom: ReconciledNode) throws {
    if subTreeFirstTextKey != nil {
      return
    }
    guard let childFirstKey = childKeyedDom.firstTextKey else {
      throw LexicalError(
        "$bubbleChildFirstText: missing __lexicalFirstTextKey on element keyed DOM")
    }
    guard let childFirstKey, let textNode = nextNodeMap.get(childFirstKey) as? TextNode else {
      return
    }
    subTreeTextFormat = try textNode.getFormat()
    subTreeTextStyle = try textNode.getStyle()
    subTreeFirstTextKey = childFirstKey
  }

  func captureFirstText(_ node: LexicalNode?) throws {
    if let node = node as? TextNode, subTreeTextFormat == nil {
      subTreeTextFormat = try node.getFormat()
      subTreeTextStyle = try node.getStyle()
      subTreeFirstTextKey = node.__key
    }
  }

  // MARK: Keyed records

  func getElementByKeyOrThrow(_ key: NodeKey) throws -> ReconciledNode {
    guard let dom = editor._keyToDOMMap.get(key) else {
      throw LexicalError("Reconciliation: could not find DOM element for node key \(key)")
    }
    return dom
  }

  func getPrevElementByKeyOrThrow(_ key: NodeKey) throws -> ReconciledNode {
    guard let dom = prevKeyToDOMMap.get(key) else {
      throw LexicalError("Reconciliation: could not find DOM element for node key \(key)")
    }
    return dom
  }

  func isDirty(_ key: NodeKey) -> Bool {
    treatAllNodesAsDirty || dirtyLeaves.has(key) || dirtyElements.has(key)
  }

  // MARK: Destroy

  /// `$destroyNode`.
  func destroyNode(_ key: NodeKey) throws {
    let node = prevNodeMap.get(key)
    // A node moved across parents in the same transaction still exists in
    // the next node map; its new parent reuses it, and `reconcileNode` marks
    // it 'updated' instead.
    if nextNodeMap.has(key) {
      return
    }
    editor._keyToDOMMap.delete(key)
    if let node = node as? ElementNode {
      for child in try createChildrenArray(node, prevNodeMap) {
        try destroyNode(child)
      }
    }
    if let node {
      for slotKey in readSlots(node).values {
        try destroyNode(slotKey)
      }
      try setMutatedNode(mutatedNodes, editor, node, "destroyed")
    }
  }

  // MARK: Create

  /// `$createNode`. `slot` is the container the node is created into: `nil`
  /// where JS passes no DOM slot (a replacement), which never reuses a moved
  /// node's DOM; a slot value passes its `host/name` container.
  @discardableResult
  func createNode(_ key: NodeKey, slot: Slot?) throws -> ReconciledNode {
    guard let node = nextNodeMap.get(key) else {
      throw LexicalError("createNode: node does not exist in nodeMap")
    }
    // Cross-parent move: the same key existed in the previous tree under a
    // different parent (or slot host). Reuse its record and reconcile it.
    if let slot, let prevNode = prevNodeMap.get(key), let existingDOM = prevKeyToDOMMap.get(key) {
      let prevSlotHost = slotHostKeyField(prevNode)
      let nextSlotHost = slotHostKeyField(node)
      let modelMoved = prevNode.__parent != node.__parent || prevSlotHost != nextSlotHost
      let slotChildDomDetached =
        nextSlotHost != nil && existingDOM.slotContainer != slot.container
      if modelMoved || slotChildDomDetached {
        existingDOM.slotContainer = slot.container
        editor._keyToDOMMap.set(key, existingDOM)
        return try reconcileNode(key)
      }
    }
    let dom = ReconciledNode()
    dom.slotContainer = slot?.container
    editor._keyToDOMMap.set(key, dom)

    if let node = node as? ElementNode {
      let outerBefore = subTreeTextContent
      let childrenSize = node.__size
      let slots = readSlots(node)
      let slotTextContent = slots.isEmpty ? "" : try mountSlotChildren(node, slots)
      if childrenSize == 0 {
        dom.textContent = slotTextContent
        dom.firstTextKey = .some(nil)
        subTreeTextContent += slotTextContent
      } else {
        let children = try createChildrenArray(node, nextNodeMap)
        try createChildren(children, node, 0, childrenSize - 1)
        if slotTextContent != "" {
          let childText = dom.textContent ?? ""
          dom.textContent = slotTextContent + childText
          subTreeTextContent = outerBefore + slotTextContent + childText
        }
      }
      if try hasCustomTextContent(node) {
        let text = try node.getTextContent()
        dom.textContent = text
        subTreeTextContent = outerBefore + text
      }
    } else {
      let text = try node.getTextContent()
      if let node = node as? DecoratorNode {
        if let decorator = try node.decorate(editor) {
          reconcileDecorator(key, decorator)
        }
        let slots = readSlots(node)
        if !slots.isEmpty {
          _ = try mountSlotChildren(node, slots)
        }
      }
      subTreeTextContent += text
    }
    try setMutatedNode(mutatedNodes, editor, node, "created")
    return dom
  }

  /// A DOM slot: where a created node goes. `container` names a slot
  /// container (`host/name`); `nil` is the host's children.
  struct Slot {
    let container: String?
  }

  /// `$createChildren`.
  func createChildren(
    _ children: [NodeKey], _ element: ElementNode, _ startIndex: Int, _ endIndex: Int
  ) throws {
    let previousSubTreeTextContent = subTreeTextContent
    let outerSaved = beginCaptureGuard()
    subTreeTextContent = ""
    subTreeTextFormat = nil
    subTreeTextStyle = nil
    subTreeFirstTextKey = nil
    if startIndex <= endIndex {
      for index in startIndex...endIndex {
        let saved = beginCaptureGuard()
        try createNode(children[index], slot: Slot(container: nil))
        let node = nextNodeMap.get(children[index])
        if node is TextNode {
          try captureFirstText(node)
        } else if let node = node as? ElementNode, index < endIndex, try !node.isInline() {
          subTreeTextContent += DOUBLE_LINE_BREAK
        }
        endCaptureGuard(saved)
      }
    }
    guard let cacheDom = editor._keyToDOMMap.get(element.__key) else {
      throw LexicalError(
        "$createChildren: Element with key \(element.__key) missing from keyToDOMMap")
    }
    cacheDom.textContent = subTreeTextContent
    cacheDom.firstTextKey = .some(subTreeFirstTextKey)
    subTreeTextContent = previousSubTreeTextContent + subTreeTextContent
    endCaptureGuard(outerSaved)
  }

  // MARK: Slots

  /// `$mountSlotChildren`: returns the slots' text.
  func mountSlotChildren(_ node: LexicalNode, _ slots: SlotMap) throws -> String {
    let previousSubTreeTextContent = subTreeTextContent
    let outerSaved = beginCaptureGuard()
    var totalText = ""
    for (name, slotKey) in slots {
      subTreeTextContent = ""
      let saved = beginCaptureGuard()
      try createNode(slotKey, slot: Slot(container: "\(node.__key)/\(name)"))
      endCaptureGuard(saved)
      totalText += subTreeTextContent
    }
    endCaptureGuard(outerSaved)
    subTreeTextContent = previousSubTreeTextContent
    return totalText
  }

  /// `$reconcileSlotChildren`: returns the slots' text.
  func reconcileSlotChildren(_ prevNode: LexicalNode, _ nextNode: LexicalNode) throws -> String {
    let prevSlots = readSlots(prevNode)
    let nextSlots = readSlots(nextNode)
    for (name, prevSlotKey) in prevSlots where nextSlots.get(name) == nil {
      try destroyNode(prevSlotKey)
    }
    let previousSubTreeTextContent = subTreeTextContent
    let outerSaved = beginCaptureGuard()
    var totalText = ""
    for (name, nextSlotKey) in nextSlots {
      let prevSlotKey = prevSlots.get(name)
      let container = "\(nextNode.__key)/\(name)"
      subTreeTextContent = ""
      let saved = beginCaptureGuard()
      if prevSlotKey == nil || prevKeyToDOMMap.get(prevSlotKey!) == nil {
        try createNode(nextSlotKey, slot: Slot(container: container))
      } else if prevSlotKey == nextSlotKey {
        try reconcileNode(nextSlotKey)
      } else {
        try destroyNode(prevSlotKey!)
        try createNode(nextSlotKey, slot: Slot(container: container))
      }
      endCaptureGuard(saved)
      totalText += subTreeTextContent
    }
    endCaptureGuard(outerSaved)
    subTreeTextContent = previousSubTreeTextContent
    return totalText
  }

  // MARK: Reconcile

  /// `$hasCustomTextContent`: JS compares the node's `getTextContent` with
  /// `ElementNode.prototype.getTextContent`. Swift cannot ask whether a
  /// method is overridden, so every element other than the root takes its
  /// text from `getTextContent()`, which for an element that does not
  /// override it is the text the walk built.
  func hasCustomTextContent(_ node: ElementNode) throws -> Bool {
    !(node is RootNode)
  }

  func reconcileDecorator(_ key: NodeKey, _ decorator: Any) {
    var pendingDecorators = editor._pendingDecorators
    if pendingDecorators == nil {
      if let current = editor._decorators[key], identical(current, decorator) {
        return
      }
      pendingDecorators = cloneDecorators(editor)
    }
    pendingDecorators![key] = decorator
    editor._pendingDecorators = pendingDecorators
  }

  /// `===` on decorator values: object identity, and for values, never.
  func identical(_ a: Any, _ b: Any) -> Bool {
    guard type(of: a) is AnyClass, type(of: b) is AnyClass else {
      return false
    }
    return (a as AnyObject) === (b as AnyObject)
  }

  func reconcileTextFormat(_ element: ElementNode) throws {
    if let subTreeTextFormat, subTreeTextFormat != element.__textFormat, !editorStateReadOnly {
      try element.setTextFormat(subTreeTextFormat)
    }
  }

  func reconcileTextStyle(_ element: ElementNode) throws {
    if let subTreeTextStyle, subTreeTextStyle != element.__textStyle, !editorStateReadOnly {
      try element.setTextStyle(subTreeTextStyle)
    }
  }

  /// `$reconcileChildrenWithDirection`.
  func reconcileChildrenWithDirection(
    _ prevElement: ElementNode, _ nextElement: ElementNode, derivesTextFields: Bool
  ) throws {
    subTreeTextFormat = nil
    subTreeTextStyle = nil
    subTreeFirstTextKey = nil
    try reconcileChildren(prevElement, nextElement)
    if try !isRootOrShadowRoot(nextElement), derivesTextFields {
      try reconcileTextFormat(nextElement)
      try reconcileTextStyle(nextElement)
    }
  }

  /// `$reconcileChildren`, general path.
  func reconcileChildren(_ prevElement: ElementNode, _ nextElement: ElementNode) throws {
    let previousSubTreeTextContent = subTreeTextContent
    let prevChildrenSize = prevElement.__size
    let nextChildrenSize = nextElement.__size
    subTreeTextContent = ""
    guard let cacheDom = editor._keyToDOMMap.get(nextElement.__key) else {
      throw LexicalError(
        "$reconcileChildren: Element with key \(nextElement.__key) missing from keyToDOMMap")
    }
    if prevChildrenSize == 1 && nextChildrenSize == 1 {
      let prevFirstChildKey = prevElement.__first!
      let nextFirstChildKey = nextElement.__first!
      if prevFirstChildKey == nextFirstChildKey {
        try reconcileNode(prevFirstChildKey)
      } else {
        _ = try getPrevElementByKeyOrThrow(prevFirstChildKey)
        try createNode(nextFirstChildKey, slot: nil)
        try destroyNode(prevFirstChildKey)
      }
      try captureFirstText(nextNodeMap.get(nextFirstChildKey))
    } else {
      let prevChildren = try createChildrenArray(prevElement, prevNodeMap)
      let nextChildren = try createChildrenArray(nextElement, nextNodeMap)
      try invariant(
        prevChildren.count == prevChildrenSize,
        "$reconcileChildren: prevChildren.length !== prevChildrenSize")
      try invariant(
        nextChildren.count == nextChildrenSize,
        "$reconcileChildren: nextChildren.length !== nextChildrenSize")
      if prevChildrenSize == 0 {
        if nextChildrenSize != 0 {
          try createChildren(nextChildren, nextElement, 0, nextChildrenSize - 1)
        }
      } else if nextChildrenSize == 0 {
        for child in prevChildren {
          try destroyNode(child)
        }
      } else {
        try reconcileNodeChildren(nextElement, prevChildren, nextChildren)
      }
    }
    cacheDom.textContent = subTreeTextContent
    cacheDom.firstTextKey = .some(subTreeFirstTextKey)
    subTreeTextContent = previousSubTreeTextContent + subTreeTextContent
  }

  /// `$reconcileNodeChildren`.
  func reconcileNodeChildren(
    _ nextElement: ElementNode, _ prevChildren: [NodeKey], _ nextChildren: [NodeKey]
  ) throws {
    let prevEndIndex = prevChildren.count - 1
    let nextEndIndex = nextChildren.count - 1
    var prevChildrenSet: Set<NodeKey>?
    var nextChildrenSet: Set<NodeKey>?
    var prevIndex = 0
    var nextIndex = 0
    while prevIndex <= prevEndIndex && nextIndex <= nextEndIndex {
      let prevKey = prevChildren[prevIndex]
      let nextKey = nextChildren[nextIndex]
      let saved = beginCaptureGuard()
      if prevKey == nextKey {
        try reconcileNode(nextKey)
        prevIndex += 1
        nextIndex += 1
      } else {
        if nextChildrenSet == nil {
          nextChildrenSet = Set(nextChildren[nextIndex...])
        }
        if prevChildrenSet == nil {
          prevChildrenSet = Set(prevChildren[prevIndex...])
        } else if !prevChildrenSet!.contains(prevKey) {
          // continue if prevKey has already been moved
          prevIndex += 1
          endCaptureGuard(saved)
          continue
        }
        if !nextChildrenSet!.contains(prevKey) {
          // Remove prev and continue
          _ = try getPrevElementByKeyOrThrow(prevKey)
          try destroyNode(prevKey)
          prevIndex += 1
          prevChildrenSet!.remove(prevKey)
          endCaptureGuard(saved)
          continue
        }
        if !prevChildrenSet!.contains(nextKey) {
          // Create next
          try createNode(nextKey, slot: Slot(container: nil))
          nextIndex += 1
        } else {
          // Move next
          _ = try getElementByKeyOrThrow(nextKey)
          try reconcileNode(nextKey)
          prevIndex += 1
          nextIndex += 1
        }
      }
      let node = nextNodeMap.get(nextKey)
      if node is TextNode {
        try captureFirstText(node)
      } else if let node = node as? ElementNode, nextIndex <= nextEndIndex, try !node.isInline() {
        subTreeTextContent += DOUBLE_LINE_BREAK
      }
      endCaptureGuard(saved)
    }
    let appendNewChildren = prevIndex > prevEndIndex
    let removeOldChildren = nextIndex > nextEndIndex
    if appendNewChildren && !removeOldChildren {
      try createChildren(nextChildren, nextElement, nextIndex, nextEndIndex)
    } else if removeOldChildren && !appendNewChildren {
      for index in prevIndex...prevEndIndex {
        try destroyNode(prevChildren[index])
      }
    }
  }

  /// `$reconcileNode`.
  @discardableResult
  func reconcileNode(_ key: NodeKey) throws -> ReconciledNode {
    guard let prevNode = prevNodeMap.get(key), var nextNode = nextNodeMap.get(key) else {
      throw LexicalError("reconcileNode: prevNode or nextNode does not exist in nodeMap")
    }
    let isDirty = isDirty(key)
    let dom = try getElementByKeyOrThrow(key)

    // If the node key points to the same instance in both states and isn't
    // dirty, we just update the text content cache.
    if prevNode === nextNode && !isDirty {
      let text: String
      if prevNode is ElementNode {
        guard let previousSubTreeTextContent = dom.textContent else {
          throw LexicalError(
            "reconcileNode: missing __lexicalTextContent on non-dirty element of type \(prevNode.__type)"
          )
        }
        text = previousSubTreeTextContent
        try bubbleChildFirstText(dom)
      } else {
        text = try prevNode.getTextContent()
      }
      subTreeTextContent += text
      return dom
    }
    // If the node key doesn't point to the same instance in both maps, it
    // was cloned. If it's also dirty, we mark it as mutated.
    if prevNode !== nextNode && isDirty {
      try setMutatedNode(mutatedNodes, editor, nextNode, "updated")
    }

    if let prevElement = prevNode as? ElementNode {
      guard let nextElement = nextNode as? ElementNode else {
        throw LexicalError("Node with key \(key) changed from ElementNode to !ElementNode")
      }
      let slotTextContent =
        isDirty && (!readSlots(nextElement).isEmpty || !readSlots(prevElement).isEmpty)
        ? try reconcileSlotChildren(prevElement, nextElement) : ""
      if isDirty {
        let outerBefore = subTreeTextContent
        try reconcileChildrenWithDirection(
          prevElement, nextElement,
          derivesTextFields: prevNode !== nextNode || dirtyElements.has(key))
        if slotTextContent != "" {
          let childText = dom.textContent ?? ""
          dom.textContent = slotTextContent + childText
          subTreeTextContent = outerBefore + slotTextContent + childText
        }
        if try hasCustomTextContent(nextElement) {
          let text = try nextElement.getTextContent()
          dom.textContent = text
          subTreeTextContent = outerBefore + text
        }
      } else {
        guard let previousSubTreeTextContent = dom.textContent else {
          throw LexicalError(
            "reconcileNode: missing __lexicalTextContent on cloned non-dirty element of type \(prevNode.__type)"
          )
        }
        subTreeTextContent += previousSubTreeTextContent
        try bubbleChildFirstText(dom)
      }
    } else {
      let text = try nextNode.getTextContent()
      if let decoratorNode = nextNode as? DecoratorNode {
        if let decorator = try decoratorNode.decorate(editor) {
          reconcileDecorator(key, decorator)
        }
        if isDirty && (!readSlots(nextNode).isEmpty || !readSlots(prevNode).isEmpty) {
          _ = try reconcileSlotChildren(prevNode, nextNode)
        }
      }
      subTreeTextContent += text
    }

    if !editorStateReadOnly, let root = nextNode as? RootNode {
      // Re-fetch the latest root: a child reconcile can clone the root
      // mid-cycle.
      let latestRoot = try root.getLatest()
      if latestRoot.__cachedText != subTreeTextContent {
        let nextRootNode = try latestRoot.getWritable()
        nextRootNode.__cachedText = subTreeTextContent
        nextNode = nextRootNode
      }
    }
    _ = nextNode
    return dom
  }
}
