import Lexical
import LexicalSelection

// Ported from packages/lexical-utils/src/index.ts. The DOM helpers
// (`markSelection`, `positionNodeOnRange`, `selectionAlwaysOnDisplay`,
// `calculateZoomLevel`, `getScrollParent`, `mediaFileReader`) belong to the
// views.

/// `isMimeType`.
public func isMimeType(_ file: DataTransferFile, _ acceptableMimeTypes: [String]) -> Bool {
  acceptableMimeTypes.contains { file.type.hasPrefix($0) }
}

/// `DFSNode`.
public struct DFSNode {
  public let depth: Int
  public let node: LexicalNode
  public init(depth: Int, node: LexicalNode) {
    self.depth = depth
    self.node = node
  }
}

/// `$dfs`: the nodes from `startNode` (the root by default) to `endNode`,
/// depth first, with their depths.
public func dfs(_ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil) throws -> [DFSNode]
{
  try dfsIterator(startNode, endNode).toArray()
}

/// `$getAdjacentCaret`.
public func getAdjacentCaret(_ caret: NodeCaret?) throws -> SiblingCaret? {
  try caret?.getAdjacentCaret()
}

/// `$reverseDfs`.
public func reverseDfs(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
) throws
  -> [DFSNode]
{
  try reverseDfsIterator(startNode, endNode).toArray()
}

/// `$dfsIterator`.
public func dfsIterator(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
) throws
  -> StepwiseIterator<NodeCaret, DFSNode>
{
  try dfsCaretIterator(.next, startNode, endNode)
}

/// `$reverseDfsIterator`.
public func reverseDfsIterator(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
)
  throws -> StepwiseIterator<NodeCaret, DFSNode>
{
  try dfsCaretIterator(.previous, startNode, endNode)
}

/// `$dfsWithSlots`: `$dfs`, and after each slot host its slot subtrees.
public func dfsWithSlots(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
) throws
  -> [DFSNode]
{
  try dfsWithSlotsIterator(startNode, endNode).toArray()
}

/// `$dfsWithSlotsIterator`.
public func dfsWithSlotsIterator(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
)
  throws -> LazyIterator<DFSNode>
{
  let base = try dfsCaretIterator(.next, startNode, endNode)
  var pending: [DFSNode] = []
  return LazyIterator {
    if !pending.isEmpty { return pending.removeFirst() }
    guard let dfsNode = try base.next() else { return nil }
    if isSlotHost(dfsNode.node), !dfsNode.node.is(endNode) {
      for name in try getSlotNames(dfsNode.node) {
        if let slot = try getSlot(dfsNode.node, name) {
          try dfsSubtree(slot, dfsNode.depth + 1, &pending)
        }
      }
    }
    return dfsNode
  }
}

private func dfsSubtree(_ node: LexicalNode, _ depth: Int, _ out: inout [DFSNode]) throws {
  out.append(DFSNode(depth: depth, node: node))
  if isSlotHost(node) {
    for name in try getSlotNames(node) {
      if let slot = try getSlot(node, name) { try dfsSubtree(slot, depth + 1, &out) }
    }
  }
  if let element = node as? ElementNode {
    for child in try element.getChildren() { try dfsSubtree(child, depth + 1, &out) }
  }
}

/// `$reverseDfsWithSlots`.
public func reverseDfsWithSlots(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
)
  throws -> [DFSNode]
{
  try reverseDfsWithSlotsIterator(startNode, endNode).toArray()
}

/// `$reverseDfsWithSlotsIterator`: slot subtrees come after a host's
/// children, mirroring the forward order.
public func reverseDfsWithSlotsIterator(
  _ startNode: LexicalNode? = nil, _ endNode: LexicalNode? = nil
) throws -> LazyIterator<DFSNode> {
  let base = try dfsCaretIterator(.previous, startNode, endNode)
  var hosts: [DFSNode] = []
  var queue: [DFSNode] = []
  var done = false
  return LazyIterator {
    while queue.isEmpty {
      if done {
        guard let host = hosts.popLast() else { return nil }
        try reverseSlotsOf(host.node, host.depth + 1, &queue)
        continue
      }
      guard let dfsNode = try base.next() else {
        done = true
        continue
      }
      while let host = hosts.last, dfsNode.depth <= host.depth {
        hosts.removeLast()
        try reverseSlotsOf(host.node, host.depth + 1, &queue)
      }
      queue.append(dfsNode)
      if isSlotHost(dfsNode.node), try !getSlotNames(dfsNode.node).isEmpty,
        !dfsNode.node.is(endNode)
      {
        hosts.append(dfsNode)
      }
    }
    return queue.removeFirst()
  }
}

private func reverseSlotsOf(_ host: LexicalNode, _ childDepth: Int, _ out: inout [DFSNode]) throws {
  for name in try getSlotNames(host).reversed() {
    if let slot = try getSlot(host, name) { try reverseDfsSubtree(slot, childDepth, &out) }
  }
}

private func reverseDfsSubtree(_ node: LexicalNode, _ depth: Int, _ out: inout [DFSNode]) throws {
  out.append(DFSNode(depth: depth, node: node))
  if let element = node as? ElementNode {
    for child in try element.getChildren().reversed() {
      try reverseDfsSubtree(child, depth + 1, &out)
    }
  }
  if isSlotHost(node) { try reverseSlotsOf(node, depth + 1, &out) }
}

private func getEndCaret(
  _ startNode: LexicalNode, _ direction: CaretDirection
) throws
  -> NodeCaret?
{
  try getAdjacentSiblingOrParentSiblingCaret(getSiblingCaret(startNode, direction))?.0
}

private func dfsCaretIterator(
  _ direction: CaretDirection, _ startNode: LexicalNode?, _ endNode: LexicalNode?
) throws -> StepwiseIterator<NodeCaret, DFSNode> {
  let start = try startNode ?? getRoot()
  let startCaret: NodeCaret =
    (start as? ElementNode).map { getChildCaret($0, direction) }
    ?? getSiblingCaret(start, direction)
  var depth = try getDepth(start)
  let endCaret: NodeCaret?
  if let endNode {
    endCaret =
      try getAdjacentChildCaret(
        getChildCaretOrSelf(getSiblingCaret(endNode, direction) as SiblingCaret))
      ?? getEndCaret(endNode, direction)
  } else {
    endCaret = try getEndCaret(start, direction)
  }
  return StepwiseIterator(
    initial: startCaret, hasNext: { _ in true },
    step: { state in
      if state.isSameNodeCaret(endCaret) { return nil }
      if state.isChild { depth += 1 }
      guard let rval = try getAdjacentSiblingOrParentSiblingCaret(state),
        !rval.0.isSameNodeCaret(endCaret)
      else { return nil }
      depth += rval.1
      return rval.0
    },
    map: { state in DFSNode(depth: depth, node: state.origin) })
}

/// `$getNextSiblingOrParentSibling`.
public func getNextSiblingOrParentSibling(_ node: LexicalNode) throws -> (LexicalNode, Int)? {
  try getAdjacentSiblingOrParentSiblingCaret(getSiblingCaret(node, .next)).map {
    ($0.0.origin, $0.1)
  }
}

/// `$getDepth`: the number of ancestors, climbing slot hosts too.
public func getDepth(_ node: LexicalNode?) throws -> Int {
  var depth = -1
  var inner = node
  while let current = inner {
    depth += 1
    inner = try current.getParent() ?? getSlotHost(current)
  }
  return depth
}

/// `$getNextRightPreorderNode`.
public func getNextRightPreorderNode(_ startingNode: LexicalNode) throws -> LexicalNode? {
  let startCaret = getChildCaretOrSelf(getSiblingCaret(startingNode, .previous) as SiblingCaret)!
  return try getAdjacentSiblingOrParentSiblingCaret(startCaret, .root)?.0.origin
}

/// `$getNearestNodeOfType`.
public func getNearestNodeOfType<T: ElementNode>(_ node: LexicalNode, _ klass: T.Type) throws -> T?
{
  var parent: LexicalNode? = node
  while let current = parent {
    if let match = current as? T { return match }
    parent = try current.getParent()
  }
  return nil
}

/// `$getNearestBlockElementAncestorOrThrow`.
public func getNearestBlockElementAncestorOrThrow(_ startNode: LexicalNode) throws -> ElementNode {
  let blockNode = try findMatchingParent(startNode) { node in
    guard let element = node as? ElementNode else { return false }
    return try !element.isInline()
  }
  guard let block = blockNode as? ElementNode else {
    throw LexicalError("Expected node \(startNode.__key) to have closest block element node.")
  }
  return block
}

/// `registerNestedElementResolver`: lifts a `targetNode` nested directly in
/// another out of it, splitting the outer one.
public func registerNestedElementResolver<N: ElementNode>(
  _ editor: Editor, _ targetNode: N.Type, _ cloneNode: @escaping (N) throws -> N,
  _ handleOverlap: @escaping (N, N) throws -> Void
) throws -> () -> Void {
  func findMatch(_ node: N) throws -> (child: ElementNode, parent: N)? {
    for child in try node.getChildren() where child is N { return nil }
    var parentNode: ElementNode? = node
    var childNode: ElementNode = node
    while let current = parentNode {
      childNode = current
      parentNode = try current.getParent()
      if let parent = parentNode as? N { return (childNode, parent) }
    }
    return nil
  }
  return try editor.registerNodeTransform(targetNode) { node in
    guard let (child, parent) = try findMatch(node), child.is(node) else { return }
    try handleOverlap(parent, node)
    let nextSiblings = try child.getNextSiblings()
    try parent.insertAfter(child)
    if !nextSiblings.isEmpty {
      let newParent = try cloneNode(parent)
      try child.insertAfter(newParent)
      for sibling in nextSiblings { try newParent.append(sibling) }
    }
    if try !parent.canBeEmpty() && parent.getChildrenSize() == 0 {
      try parent.remove()
    }
  }
}

/// `$restoreEditorState`: replaces the pending state's nodes with copies
/// of `editorState`'s, and its selection.
public func restoreEditorState(_ editor: Editor, _ editorState: EditorState) throws {
  let nodeMap = NodeMap()
  for (key, node) in editorState._nodeMap {
    nodeMap.set(key, try cloneWithProperties(node))
  }
  editor._pendingEditorState?._nodeMap = nodeMap
  fullReconcile()
  try setSelection(editorState._selection?.clone())
}

private func hasContentAfter(_ caret: PointCaret) throws -> Bool {
  if try isExtendableTextPointCaret(caret) { return true }
  var c: NodeCaret? = caret.isText ? caret.getSiblingCaret() : caret
  while let current = c {
    if try current.getNodeAtCaret() != nil { return true }
    let parent = try current.isChild ? current.origin : current.origin.getParent()
    if try isBlockContainer(parent) { return false }
    c = try current.getParentCaret(.shadowRoot)
  }
  return false
}

/// `$insertNodeToNearestRoot`: inserts `node` at the selection, splitting
/// up to the nearest root, and selects after it.
@discardableResult
public func insertNodeToNearestRoot<T: LexicalNode>(_ node: T) throws -> T {
  let selection = try getSelection() ?? getPreviousSelection()
  var initialCaret: PointCaret?
  if let range = selection as? RangeSelection {
    initialCaret = try caretFromPoint(range.focus, .next)
  } else {
    if let selection, let lastNode = try selection.getNodes().last {
      initialCaret = getSiblingCaret(lastNode, .next) as SiblingCaret
    }
    if initialCaret == nil {
      initialCaret = try getChildCaret(getRoot(), .previous).getFlipped().insert(
        createParagraphNode())
    }
  }
  let caret = initialCaret!
  let insertCaret = try insertNodeToNearestRootAtCaret(
    node, caret,
    hasContentAfter(caret)
      ? SplitAtPointCaretNextOptions(shouldSplit: { _, edge in edge != .last }) : nil)
  let adjacent = try getAdjacentChildCaret(insertCaret)
  let selectionCaret = try adjacent.map { $0.isChild ? try normalizeCaret($0) : insertCaret }
  try setSelectionFromCaretRange(getCollapsedCaretRange(selectionCaret ?? insertCaret))
  return try node.getLatest()
}

/// `$insertNodeIntoLeaf`: inserts `node` at the selection without
/// splitting blocks.
public func insertNodeIntoLeaf(_ node: LexicalNode) throws {
  let selection = try getSelection()
  guard let range = selection as? RangeSelection else {
    try selection?.insertNodes([node])
    return
  }
  var insertCaret = try getCaretRangeInDirection(
    removeTextFromCaretRange(caretRangeFromSelection(range)), .next
  ).anchor
  if insertCaret.isText {
    guard let nextAnchor = try splitAtPointCaretNext(insertCaret) else { return }
    insertCaret = nextAnchor
  }
  let focus = try insertCaret.getFlipped()
  try focus.insert(node)
  try setSelectionFromCaretRange(getCaretRange(focus, focus))
}

/// `$wrapNodeInElement`.
@discardableResult
public func wrapNodeInElement<T: ElementNode>(
  _ node: LexicalNode, _ createElementNode: () throws -> T
) throws -> T {
  let elementNode = try createElementNode()
  try node.replace(elementNode)
  try elementNode.append(node)
  return elementNode
}

/// `objectKlassEquals`: whether `object`'s class has the name of
/// `objectClass`, as JS compares constructors across realms.
public func objectKlassEquals<T>(_ object: Any?, _ objectClass: T.Type) -> Bool {
  guard let object else { return false }
  return String(describing: type(of: object)) == String(describing: objectClass)
}

/// `eventFiles`: whether the event carries files, the files, and whether it
/// also carries HTML or plain text.
public func eventFiles(_ event: DOMEvent) -> (Bool, [DataTransferFile], Bool) {
  let dataTransfer: DataTransfer?
  if let drag = event as? DragEvent {
    dataTransfer = drag.dataTransfer
  } else if let clipboard = event as? ClipboardEvent {
    dataTransfer = clipboard.clipboardData
  } else {
    dataTransfer = nil
  }
  guard let dataTransfer else { return (false, [], false) }
  let types = dataTransfer.types
  return (
    types.contains("Files"), dataTransfer.files,
    types.contains("text/html") || types.contains("text/plain")
  )
}

/// `eventFiles` for a paste payload.
public func eventFiles(_ event: PasteCommandType) -> (Bool, [DataTransferFile], Bool) {
  eventFiles(event.event)
}

/// `$filter`.
public func filter<T>(
  _ nodes: [LexicalNode], _ filterFn: (LexicalNode) throws -> T?
) rethrows
  -> [T]
{
  try nodes.compactMap(filterFn)
}

/// `$handleIndentAndOutdent`: calls `indentOrOutdent` once for each
/// selected block that can indent.
public func handleIndentAndOutdent(_ indentOrOutdent: (ElementNode) throws -> Void) throws -> Bool {
  guard let selection = try getSelection() as? RangeSelection else { return false }
  var alreadyHandled = Set<NodeKey>()
  for node in try selection.getNodes() {
    if alreadyHandled.contains(node.getKey()) { continue }
    let parentBlock = try findMatchingParent(node) { parentNode in
      guard let element = parentNode as? ElementNode else { return false }
      return try !element.isInline()
    }
    guard let block = parentBlock as? ElementNode else { continue }
    if block.canIndent(), alreadyHandled.insert(block.getKey()).inserted {
      try indentOrOutdent(block)
    }
  }
  return !alreadyHandled.isEmpty
}

/// `$insertFirst`.
public func insertFirst(_ parent: ElementNode, _ node: LexicalNode) throws {
  try getChildCaret(parent, .next).insert(node)
}

/// `$isEditorIsNestedEditor`.
public func isEditorIsNestedEditor(_ editor: Editor) -> Bool { editor._parentEditor != nil }

/// `$unwrapAndFilterDescendants`: removes the descendants of `root` that
/// fail `predicate`, keeping their matching descendants in their place.
@discardableResult
public func unwrapAndFilterDescendants(
  _ root: ElementNode, _ predicate: @escaping (LexicalNode) throws -> Bool
) throws -> Bool {
  try unwrapAndFilterDescendantsImpl(root, predicate, nil)
}

private func unwrapAndFilterDescendantsImpl(
  _ root: ElementNode, _ predicate: @escaping (LexicalNode) throws -> Bool,
  _ onSuccess: ((LexicalNode) throws -> Void)?
) throws -> Bool {
  var didMutate = false
  try lastToFirstIterator(root).forEach { node in
    if try predicate(node) {
      try onSuccess?(node)
      return
    }
    didMutate = true
    if let element = node as? ElementNode {
      _ = try unwrapAndFilterDescendantsImpl(
        element, predicate, onSuccess ?? { child in try element.insertAfter(child) })
    }
    try node.remove()
  }
  return didMutate
}

/// `$descendantsMatching`: the topmost descendants that match
/// `predicate`, in order.
public func descendantsMatching(
  _ children: [LexicalNode], _ predicate: (LexicalNode) throws -> Bool
) throws -> [LexicalNode] {
  var result: [LexicalNode] = []
  var stack = Array(children.reversed())
  while let child = stack.popLast() {
    if try predicate(child) {
      result.append(child)
    } else if let element = child as? ElementNode {
      try lastToFirstIterator(element).forEach { stack.append($0) }
    }
  }
  return result
}

/// `$descendantsMatching` with a type: the topmost descendants of type `T`.
public func descendantsMatching<T: LexicalNode>(
  _ children: [LexicalNode], _ type: T.Type
) throws
  -> [T]
{
  try descendantsMatching(children) { $0 is T }.map { $0 as! T }
}

/// `$firstToLastIterator`: the children of `node`, re-reading each next
/// sibling as it goes, so children may be removed while iterating.
public func firstToLastIterator(_ node: ElementNode) -> StepwiseIterator<SiblingCaret, LexicalNode>
{
  childIterator(getChildCaret(node, .next))
}

/// `$lastToFirstIterator`.
public func lastToFirstIterator(_ node: ElementNode) -> StepwiseIterator<SiblingCaret, LexicalNode>
{
  childIterator(getChildCaret(node, .previous))
}

private func childIterator(_ startCaret: NodeCaret) -> StepwiseIterator<SiblingCaret, LexicalNode> {
  var seen = Set<NodeKey>()
  return StepwiseIterator(
    initial: try? startCaret.getAdjacentCaret(), hasNext: { $0.isSibling },
    step: { try $0.getAdjacentCaret() },
    map: { caret in
      let origin = try caret.origin.getLatest()
      let key = origin.getKey()
      if !seen.insert(key).inserted {
        throw LexicalError(
          "$childIterator: Cycle detected, node with key \(key) has already been traversed")
      }
      return origin
    })
}

/// `$unwrapNode`: replaces `node` with its children.
public func unwrapNode(_ node: ElementNode) throws {
  try rewindSiblingCaret(getSiblingCaret(node, .next)).splice(1, node.getChildren())
}

/// `makeStateWrapper`: `$get` and `$set` bound to `stateConfig`.
public struct StateConfigWrapper<V> {
  public let stateConfig: StateConfig<V>
  public func get(_ node: LexicalNode) throws -> V { try getState(node, stateConfig) }
  @discardableResult
  public func set<T: LexicalNode>(_ node: T, _ value: V) throws -> T {
    try setState(node, stateConfig, value)
  }
  @discardableResult
  public func set<T: LexicalNode>(_ node: T, _ updater: (V) throws -> V) throws -> T {
    try setState(node, stateConfig, updater)
  }
}

/// `makeStateWrapper`.
public func makeStateWrapper<V>(_ stateConfig: StateConfig<V>) -> StateConfigWrapper<V> {
  StateConfigWrapper(stateConfig: stateConfig)
}

/// `$onEscapeUp`: in a container at the start of its parent, inserts a
/// paragraph before it.
public func onEscapeUp(
  _ isContainerNode: (LexicalNode) throws -> Bool, _ event: KeyboardEvent? = nil
) throws -> Bool {
  guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
    let container = try findMatchingParent(selection.anchor.getNode(), isContainerNode)
      as? ElementNode,
    let parent = try container.getParent(), try parent.getFirstChild() === container,
    try isAtStartOfNode(selection.anchor, container)
  else { return false }
  try container.insertBefore(createParagraphNode()).selectEnd()
  event?.preventDefault()
  return true
}

/// `$onEscapeDown`: in a container at the end of its parent, inserts a
/// paragraph after it.
public func onEscapeDown(
  _ isContainerNode: (LexicalNode) throws -> Bool, _ event: KeyboardEvent? = nil
) throws -> Bool {
  guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
    let container = try findMatchingParent(selection.anchor.getNode(), isContainerNode)
      as? ElementNode,
    let parent = try container.getParent(), try parent.getLastChild() === container,
    try isAtEndOfNode(selection.anchor, container)
  else { return false }
  try container.insertAfter(createParagraphNode()).selectEnd()
  event?.preventDefault()
  return true
}

/// `$isAtStartOfNode`.
public func isAtStartOfNode(_ point: Point, _ node: ElementNode) throws -> Bool {
  try isAtEdgeOfElement(point, node, .previous)
}

/// `$isAtEndOfNode`.
public func isAtEndOfNode(_ point: Point, _ node: ElementNode) throws -> Bool {
  try isAtEdgeOfElement(point, node, .next)
}
