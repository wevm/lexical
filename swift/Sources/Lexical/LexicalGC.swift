// Port of packages/lexical/src/LexicalGC.ts and LexicalNormalization.ts.

package func garbageCollectDetachedDecorators(_ editor: Editor, _ pendingEditorState: EditorState) {
  let currentDecorators = editor._decorators
  let pendingDecorators = editor._pendingDecorators
  var decorators = pendingDecorators ?? currentDecorators
  let nodeMap = pendingEditorState._nodeMap
  var changed = false
  for key in decorators.keys where !nodeMap.has(key) {
    decorators[key] = nil
    changed = true
  }
  if changed {
    editor._pendingDecorators = decorators
  }
}

func garbageCollectDetachedDeepChildNodes(
  _ node: LexicalNode, _ parentKey: NodeKey, _ prevNodeMap: NodeMap, _ nodeMap: NodeMap,
  _ nodeMapDelete: inout [NodeKey], _ dirtyNodes: DirtyNodes
) {
  if let element = node as? ElementNode {
    var childKey = element.__first
    while let key = childKey {
      guard let child = nodeMap.get(key) else {
        break
      }
      let isElement = child is ElementNode
      if child.__parent == parentKey && !(isElement && dirtyNodes.has(key)) {
        if isElement || hasSlots(child) {
          garbageCollectDetachedDeepChildNodes(
            child, key, prevNodeMap, nodeMap, &nodeMapDelete, dirtyNodes)
        }
        // If we have created a node and it was dereferenced, then also
        // remove it from out dirty nodes Set.
        if !prevNodeMap.has(key) {
          dirtyNodes.delete(key)
        }
        nodeMapDelete.append(key)
      }
      childKey = child.__next
    }
  }
  for slotKey in slotKeys(node) {
    guard let slotNode = nodeMap.get(slotKey), slotHostKeyField(slotNode) == parentKey else {
      continue
    }
    if slotNode is ElementNode || hasSlots(slotNode) {
      garbageCollectDetachedDeepChildNodes(
        slotNode, slotKey, prevNodeMap, nodeMap, &nodeMapDelete, dirtyNodes)
    }
    if !prevNodeMap.has(slotKey) {
      dirtyNodes.delete(slotKey)
    }
    nodeMapDelete.append(slotKey)
  }
}

/// The dirty leaves (a set) or the dirty elements (a map), which the GC
/// prunes alike.
package enum DirtyNodes {
  case leaves(JSSet<NodeKey>)
  case elements(JSMap<NodeKey, Bool>)

  func has(_ key: NodeKey) -> Bool {
    switch self {
    case .leaves(let set): set.has(key)
    case .elements(let map): map.has(key)
    }
  }

  func delete(_ key: NodeKey) {
    switch self {
    case .leaves(let set): set.delete(key)
    case .elements(let map): map.delete(key)
    }
  }
}

package func garbageCollectDetachedNodes(
  _ prevEditorState: EditorState, _ editorState: EditorState, _ dirtyLeaves: JSSet<NodeKey>,
  _ dirtyElements: JSMap<NodeKey, Bool>
) throws {
  let prevNodeMap = prevEditorState._nodeMap
  let nodeMap = editorState._nodeMap
  // Store dirtyElements in a queue for later deletion; deleting dirty
  // subtrees too early will hinder accessing .__next on child nodes
  var nodeMapDelete: [NodeKey] = []
  for (nodeKey, _) in dirtyElements {
    if let node = nodeMap.get(nodeKey), try !node.isAttached() {
      if node is ElementNode {
        garbageCollectDetachedDeepChildNodes(
          node, nodeKey, prevNodeMap, nodeMap, &nodeMapDelete, .elements(dirtyElements))
      }
      // If we have created a node and it was dereferenced, then also remove
      // it from out dirty nodes Set.
      if !prevNodeMap.has(nodeKey) {
        dirtyElements.delete(nodeKey)
      }
      nodeMapDelete.append(nodeKey)
    }
  }
  for nodeKey in dirtyLeaves {
    if let node = nodeMap.get(nodeKey), try !node.isAttached() {
      if hasSlots(node) {
        garbageCollectDetachedDeepChildNodes(
          node, nodeKey, prevNodeMap, nodeMap, &nodeMapDelete, .leaves(dirtyLeaves))
      }
      if !prevNodeMap.has(nodeKey) {
        dirtyLeaves.delete(nodeKey)
      }
      nodeMapDelete.append(nodeKey)
    }
  }
  let editor = try getActiveEditor()
  let cloneNotNeeded = editor._cloneNotNeeded
  for nodeKey in nodeMapDelete {
    nodeMap.delete(nodeKey)
    cloneNotNeeded.delete(nodeKey)
  }
  if let compositionKey = editor._compositionKey, !nodeMap.has(compositionKey) {
    editor._compositionKey = nil
  }
}

// MARK: - Normalization

func canSimpleTextNodesBeMerged(_ node1: TextNode, _ node2: TextNode) -> Bool {
  let node1State = node1.__state
  let node2State = node2.__state
  return node1.__mode == node2.__mode && node1.__format == node2.__format
    && node1.__style == node2.__style
    && (node1State == nil || node1State === node2State
      || nodeStatesAreEquivalent(node1State, node2State))
}

func mergeTextNodes(_ node1: TextNode, _ node2: TextNode) throws -> TextNode {
  let writableNode1 = try node1.mergeWithSibling(node2)
  let normalizedNodes = try getActiveEditor()._normalizedNodes
  normalizedNodes.add(node1.__key)
  normalizedNodes.add(node2.__key)
  return writableNode1
}

/// `$normalizeTextNode`: removes an empty simple text node, or merges it
/// with a mergeable neighbor.
package func normalizeTextNode(_ textNode: TextNode) throws {
  var node = textNode
  if try node.__text == "" && node.isSimpleText() && !node.isUnmergeable() {
    try node.remove()
    return
  }
  let editor = try getActiveEditor()
  let compositionKey = editor._compositionKey
  let holdComposition =
    compositionKey != nil
    && (editor._compositionMergeHeld == compositionKey
      || editor._updateTags.has(DEFER_COMPOSITION_MERGE_TAG))
  func holdsMerge(_ node1: TextNode, _ node2: TextNode) -> Bool {
    if holdComposition && (node1.__key == compositionKey || node2.__key == compositionKey)
      && canSimpleTextNodesBeMerged(node1, node2)
    {
      editor._compositionMergeHeld = compositionKey
      return true
    }
    return false
  }

  // Backward
  while let previousNode = try node.getPreviousSibling() as? TextNode,
    try previousNode.isSimpleText(), try !previousNode.isUnmergeable()
  {
    if previousNode.__text == "" {
      try previousNode.remove()
    } else if holdsMerge(previousNode, node) {
      break
    } else if canSimpleTextNodesBeMerged(previousNode, node) {
      node = try mergeTextNodes(previousNode, node)
      break
    } else {
      break
    }
  }

  // Forward
  while let nextNode = try node.getNextSibling() as? TextNode, try nextNode.isSimpleText(),
    try !nextNode.isUnmergeable()
  {
    if nextNode.__text == "" {
      try nextNode.remove()
    } else if holdsMerge(node, nextNode) {
      break
    } else if canSimpleTextNodesBeMerged(node, nextNode) {
      node = try mergeTextNodes(node, nextNode)
      break
    } else {
      break
    }
  }
}

/// `$normalizeSelection`: moves element points onto text where they can be.
package func normalizeSelection(_ selection: RangeSelection) throws -> RangeSelection {
  try normalizePoint(selection.anchor)
  try normalizePoint(selection.focus)
  return selection
}

func normalizePoint(_ point: Point) throws {
  while point.type == .element {
    guard let node = try point.getNode() as? ElementNode else { return }
    let offset = point.offset
    let nextNode: LexicalNode?
    let nextOffsetAtEnd: Bool
    if try offset == node.getChildrenSize() {
      nextNode = try node.getChildAtIndex(offset - 1)
      nextOffsetAtEnd = true
    } else {
      nextNode = try node.getChildAtIndex(offset)
      nextOffsetAtEnd = false
    }
    if let text = nextNode as? TextNode {
      try point.set(text.__key, nextOffsetAtEnd ? text.getTextContentSize() : 0, .text, true)
      break
    }
    guard let element = nextNode as? ElementNode else {
      break
    }
    try point.set(element.__key, nextOffsetAtEnd ? element.getChildrenSize() : 0, .element, true)
  }
}
