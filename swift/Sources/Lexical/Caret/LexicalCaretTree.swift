// Port of packages/lexical/src/caret/LexicalCaretTree.ts. Text-edit
// recording (`_recordTextEdits`) belongs to the reconciler and is not ported
// yet.

private func linkSiblings(
  _ writableParent: ElementNode, _ writablePrevious: LexicalNode?, _ writableNext: LexicalNode?
) {
  let previousKey = writablePrevious?.__key
  let nextKey = writableNext?.__key
  if let writablePrevious {
    writablePrevious.__next = nextKey
  } else {
    writableParent.__first = nextKey
  }
  if let writableNext {
    writableNext.__prev = previousKey
  } else {
    writableParent.__last = previousKey
  }
}

private func recordNodeMove(_ node: LexicalNode, _ parent: ElementNode) throws {
  let editor = try getActiveEditor()
  if !editor._recordNodeMoves {
    return
  }
  let prevNodeMap = editor._editorState._nodeMap
  if prevNodeMap.has(node.__key) {
    let editorState = try getActiveEditorState()
    if editorState._movedNodes == nil {
      editorState._movedNodes = JSMap()
    }
    let id = ObjectIdentifier(prevNodeMap)
    var movedKeys = editorState._movedNodes!.get(id)
    if movedKeys == nil {
      movedKeys = JSSet()
      editorState._movedNodes!.set(id, movedKeys!)
    }
    movedKeys!.add(node.__key)
  }
}

/// `$insertNodeBetween`: links `writableNode` into `writableParent` between
/// two siblings. The caller adjusts `__size`.
package func insertNodeBetween(
  _ writableParent: ElementNode, _ writableNode: LexicalNode, _ writablePrevious: LexicalNode?,
  _ writableNext: LexicalNode?
) throws {
  let key = writableNode.__key
  if let writablePrevious {
    writablePrevious.__next = key
  } else {
    writableParent.__first = key
  }
  if let writableNext {
    writableNext.__prev = key
  } else {
    writableParent.__last = key
  }
  writableNode.__prev = writablePrevious?.__key
  writableNode.__next = writableNext?.__key
  writableNode.__parent = writableParent.__key
}

/// `$detachNode`: unlinks `writableNode` from its parent.
package func detachNode(_ writableNode: LexicalNode) throws {
  if let slotHostKey = try getSlotHostKey(writableNode) {
    throw LexicalError(
      "$removeFromParent: node \(writableNode.__key) is slotted into host \(slotHostKey); a slotted node and a child are mutually exclusive. Remove it from its slot first."
    )
  }
  guard let parent = try writableNode.getParent() else {
    return
  }
  let writableParent = try parent.getWritable()
  try recordNodeMove(writableNode, writableParent)
  let previous = try writableNode.getPreviousSibling()
  let next = try writableNode.getNextSibling()
  linkSiblings(writableParent, try previous?.getWritable(), try next?.getWritable())
  writableNode.__prev = nil
  writableNode.__next = nil
  writableNode.__parent = nil
  writableParent.__size -= 1
}

/// `$detachSiblingRange`: unlinks `count` siblings starting at `first`,
/// which follows `before`, and returns their keys.
package func detachSiblingRange(
  _ writableParent: ElementNode, _ first: LexicalNode?, _ count: Int, _ before: LexicalNode?
) throws -> [NodeKey] {
  var nodes: [LexicalNode] = []
  var node = first
  for _ in 0..<count {
    guard let current = node else {
      throw LexicalError("splice: sibling not found")
    }
    let next = try current.getNextSibling()
    nodes.append(try current.getWritable())
    node = next
  }
  let previous = try before?.getWritable()
  let next = try node?.getWritable()
  linkSiblings(writableParent, previous, next)
  writableParent.__size -= nodes.count
  return try nodes.map { writable in
    try recordNodeMove(writable, writableParent)
    writable.__prev = nil
    writable.__next = nil
    writable.__parent = nil
    return writable.__key
  }
}

/// `$detachNodeWithSelection`: detaches `node`, shifting `selection`'s
/// element points on its parent. Returns the points that were just after
/// the node.
package func detachNodeWithSelection(
  _ node: LexicalNode, _ selection: RangeSelection?
) throws
  -> [Point]?
{
  let parent = try selection == nil ? nil : node.getParent()
  var index = -1
  if let selection, let parent, try selectionTouchesElement(selection, parent) {
    index = try node.getIndexWithinParent()
  }
  var points: [Point]? = nil
  if let selection, let parent, index != -1 {
    points = [selection.anchor, selection.focus].filter {
      $0.type == .element && $0.key == parent.__key && $0.offset == index + 1
    }
    // Cache the direction before the points move.
    _ = try selection.isBackward()
  }
  try detachNode(node)
  if let selection, let parent, index != -1 {
    try updateElementSelectionOnCreateDeleteNode(selection, parent, index, -1)
  }
  selection?._cachedIsBackward = nil
  return points
}

/// `$insertSibling`: moves `node` next to `origin`.
@discardableResult
package func insertSibling(
  _ origin: LexicalNode, _ direction: CaretDirection, _ node: LexicalNode,
  _ restoreSelection: Bool
) throws -> LexicalNode {
  try errorOnReadOnly()
  let isNext = direction == .next
  try errorOnInsertTextNodeOnRoot(origin, node)
  let writableOrigin = try origin.getWritable()
  let writableNode = try node.getWritable()
  try errorOnSlotCycleChild(origin.getParentOrThrow(), writableNode)
  let currentSelection = try getSelection()
  let selection = restoreSelection ? currentSelection as? RangeSelection : nil
  let points = try detachNodeWithSelection(writableNode, selection)
  let parent = try origin.getParentOrThrow().getWritable()
  var index = -1
  if let selection,
    try (isNext && points?.isEmpty == false) || selectionTouchesElement(selection, parent)
  {
    index = try origin.getIndexWithinParent() + (isNext ? 1 : 0)
  }
  let sibling = try isNext ? origin.getNextSibling() : origin.getPreviousSibling()
  let writableSibling = try sibling?.getWritable()
  try insertNodeBetween(
    parent, writableNode, isNext ? writableOrigin : writableSibling,
    isNext ? writableSibling : writableOrigin)
  parent.__size += 1
  if let selection, index != -1 {
    try updateElementSelectionOnCreateDeleteNode(selection, parent, index)
    if isNext, let points {
      for point in points {
        try point.set(parent.__key, index + 1, .element)
      }
    }
  }
  return node
}

/// `$selectAdjacentNode`: selects the position on the far side of `caret`.
@discardableResult
package func selectAdjacentNode(
  _ caret: SiblingCaret, _ anchorOffset: Int? = nil, _ focusOffset: Int? = nil
) throws -> RangeSelection {
  try errorOnReadOnly()
  let origin = caret.origin
  let isNext = caret.direction == .next
  if let slotHost = try getSlotHost(origin) {
    return try isNext
      ? slotHost.selectNext(anchorOffset, focusOffset)
      : slotHost.selectPrevious(anchorOffset, focusOffset)
  }
  let sibling = try caret.getNodeAtCaret()
  let parent = try origin.getParentOrThrow()
  guard let sibling else {
    return try isNext ? parent.select() : parent.select(0, 0)
  }
  if let element = sibling as? ElementNode {
    return try isNext ? element.select(0, 0) : element.select()
  }
  if let text = sibling as? TextNode {
    return try text.select(anchorOffset, focusOffset)
  }
  let index = try sibling.getIndexWithinParent() + (isNext ? 0 : 1)
  return try parent.select(index, index)
}

/// `$collectSiblingNodes`: `start` and its siblings in `direction`.
public func collectSiblingNodes(
  _ start: LexicalNode?, _ direction: CaretDirection
) throws
  -> [LexicalNode]
{
  var siblings: [LexicalNode] = []
  var node = start
  while let current = node {
    siblings.append(current)
    node = try direction == .next ? current.getNextSibling() : current.getPreviousSibling()
  }
  return siblings
}
