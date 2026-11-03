// Port of the model functions of packages/lexical/src/LexicalUtils.ts. The DOM
// helpers belong to the UI layers.

/// `resetRandomKey()`: restarts node keys at 1 on this thread.
public func resetRandomKey() {
  ThreadContext.current.keyCounter = 1
}

/// `generateRandomKey()`: the next node key (`"1"`, `"2"`, ...).
public func generateRandomKey() -> NodeKey {
  let context = ThreadContext.current
  let key = String(context.keyCounter)
  context.keyCounter += 1
  return key
}

package func getRegisteredNodeOrThrow(
  _ editor: Editor, _ nodeType: String
) throws
  -> RegisteredNode
{
  guard let registeredNode = getRegisteredNode(editor, nodeType) else {
    throw LexicalError("registeredNode: Type \(nodeType) not found")
  }
  return registeredNode
}

package func getRegisteredNode(_ editor: Editor, _ nodeType: String) -> RegisteredNode? {
  editor._nodes.get(nodeType)
}

/// `getTextDirection`: `rtl` or `ltr` from the first strong character, or
/// `nil`.
public func getTextDirection(_ text: String) -> ElementDirection? {
  if testRTL(text) {
    return "rtl"
  }
  if testLTR(text) {
    return "ltr"
  }
  return nil
}

/// `$isTokenOrTab`.
public func isTokenOrTab(_ node: TextNode) throws -> Bool {
  try node is TabNode || node.isToken()
}

/// `$isTokenOrSegmented`.
public func isTokenOrSegmented(_ node: TextNode) throws -> Bool {
  try isTokenOrTab(node) || node.isSegmented()
}

/// `toggleTextFormatType`: `format` with `type` toggled (or aligned with
/// `alignWithFormat`), clearing the formats it excludes.
public func toggleTextFormatType(
  _ format: Int, _ type: TextFormatType, _ alignWithFormat: Int?
)
  -> Int
{
  let activeFormat = TEXT_TYPE_TO_FORMAT[type] ?? 0
  if let alignWithFormat, (format & activeFormat) == (alignWithFormat & activeFormat) {
    return format
  }
  var newFormat = format ^ activeFormat
  switch type {
  case "subscript":
    newFormat &= ~IS_SUPERSCRIPT
  case "superscript":
    newFormat &= ~IS_SUBSCRIPT
  case "lowercase":
    newFormat &= ~IS_UPPERCASE
    newFormat &= ~IS_CAPITALIZE
  case "uppercase":
    newFormat &= ~IS_LOWERCASE
    newFormat &= ~IS_CAPITALIZE
  case "capitalize":
    newFormat &= ~IS_LOWERCASE
    newFormat &= ~IS_UPPERCASE
  default:
    break
  }
  return newFormat
}

/// `$isLeafNode`: a text node, line break, or decorator.
public func isLeafNode(_ node: LexicalNode?) -> Bool {
  node is TextNode || node is LineBreakNode || node is DecoratorNode
}

/// `$setNodeKey`: gives a new node a key and registers it in the active
/// editor state, or assigns `existingKey` (a clone).
package func setNodeKey(_ node: LexicalNode, _ existingKey: NodeKey?) throws {
  if let existingKey {
    try errorOnNodeKeyConstructorMismatch(node, existingKey)
    node.__key = existingKey
    return
  }
  try errorOnReadOnly()
  try errorOnInfiniteTransforms()
  let editor = try getActiveEditor()
  let editorState = try getActiveEditorState()
  let key = generateRandomKey()
  try errorOnFrozenNodeMap(editorState._nodeMap, "set")
  editorState._nodeMap.set(key, node)
  // TODO Split this function into leaf/element
  if node is ElementNode {
    editor._dirtyElements.set(key, true)
  } else {
    editor._dirtyLeaves.add(key)
  }
  editor._cloneNotNeeded.set(key, node)
  editor._dirtyType = editor._dirtyType == NO_DIRTY_NODES ? HAS_DIRTY_NODES : editor._dirtyType
  node.__key = key
}

func errorOnNodeKeyConstructorMismatch(_ node: LexicalNode, _ existingKey: NodeKey) throws {
  guard let editorState = internalGetActiveEditorState(),
    let existingNode = editorState._nodeMap.get(existingKey)
  else {
    return
  }
  if type(of: existingNode) != type(of: node) {
    throw LexicalError(
      "Lexical node with constructor \(type(of: node)) attempted to re-use key from node in active editor state with constructor \(type(of: existingNode)). Keys must not be re-used when the type is changed."
    )
  }
}

func internalMarkParentElementsAsDirty(
  _ parentKey: NodeKey, _ nodeMap: NodeMap, _ dirtyElements: JSMap<NodeKey, Bool>
) {
  var nextParentKey: NodeKey? = parentKey
  while let key = nextParentKey {
    if dirtyElements.has(key) {
      return
    }
    guard let node = nodeMap.get(key) else {
      break
    }
    dirtyElements.set(key, false)
    nextParentKey = node.__parent ?? slotHostKeyField(node)
  }
}

/// `$removeFromParent`.
package func removeFromParent(_ node: LexicalNode) throws {
  try detachNode(node.getParent() == nil ? node : node.getWritable())
}

package func internalMarkNodeAsDirty(_ node: LexicalNode) throws {
  try errorOnInfiniteTransforms()
  try invariant(
    !node.__ephemeral,
    "internalMarkNodeAsDirty: Ephemeral nodes must not be marked as dirty (key \(node.__key) type \(node.__type))"
  )
  let parent = node.__parent ?? slotHostKeyField(node)
  let editorState = try getActiveEditorState()
  let editor = try getActiveEditor()
  let nodeMap = editorState._nodeMap
  let dirtyElements = editor._dirtyElements
  if let parent {
    internalMarkParentElementsAsDirty(parent, nodeMap, dirtyElements)
  }
  let key = node.__key
  if editor._dirtyType == NO_DIRTY_NODES {
    editor._dirtyType = HAS_DIRTY_NODES
  }
  if node is ElementNode {
    dirtyElements.set(key, true)
  } else {
    editor._dirtyLeaves.add(key)
  }
}

package func internalMarkSiblingsAsDirty(_ node: LexicalNode) throws {
  if let previousNode = try node.getPreviousSibling() {
    try internalMarkNodeAsDirty(previousNode)
  }
  if let nextNode = try node.getNextSibling() {
    try internalMarkNodeAsDirty(nextNode)
  }
}

/// `$setCompositionKey`: the node IME composition is happening in.
public func setCompositionKey(_ compositionKey: NodeKey?) throws {
  try errorOnReadOnly()
  let editor = try getActiveEditor()
  let previousCompositionKey = editor._compositionKey
  if compositionKey != previousCompositionKey {
    editor._compositionKey = compositionKey
    editor._compositionMergeHeld = nil
    if let previousCompositionKey, let node = try getNodeByKey(previousCompositionKey) {
      _ = try node.getWritable()
    }
    if let compositionKey, let node = try getNodeByKey(compositionKey) {
      _ = try node.getWritable()
    }
  }
}

/// `$getCompositionKey`.
public func getCompositionKey() throws -> NodeKey? {
  if isCurrentlyReadOnlyMode() {
    return nil
  }
  return try getActiveEditor()._compositionKey
}

/// `$getNodeByKey`: the node with `key` in the active (or given) editor
/// state.
public func getNodeByKey(_ key: NodeKey, _ editorState: EditorState? = nil) throws -> LexicalNode? {
  try (editorState ?? getActiveEditorState())._nodeMap.get(key)
}

/// `$getNodeByKeyOrThrow`.
public func getNodeByKeyOrThrow(_ key: NodeKey) throws -> LexicalNode {
  guard let node = try getNodeByKey(key) else {
    throw LexicalError("Expected node with key \(key) to exist but it's not in the nodeMap.")
  }
  return node
}

package func cloneDecorators(_ editor: Editor) -> [NodeKey: Any] {
  let pendingDecorators = editor._decorators
  editor._pendingDecorators = pendingDecorators
  return pendingDecorators
}

package func getEditorStateTextContent(_ editorState: EditorState) throws -> String {
  try editorState.read { try getRoot().getTextContent() }
}

package func markNodesWithTypesAsDirty(_ editor: Editor, _ types: [String]) throws {
  // We only need to mark nodes dirty if they were in the previous state.
  // If they aren't, then they are by definition dirty already.
  let cachedMap = try getCachedTypeToNodeMap(editor.getEditorState())
  var dirtyNodeMaps: [NodeMap] = []
  for type in types {
    if let nodeMap = cachedMap.get(type) {
      dirtyNodeMaps.append(nodeMap)
    }
  }
  if dirtyNodeMaps.isEmpty {
    return
  }
  try editor.update(
    {
      for nodeMap in dirtyNodeMaps {
        for (nodeKey, _) in nodeMap {
          // We are only concerned with nodes that are still in the editor
          // state
          if let latest = try getNodeByKey(nodeKey) {
            try latest.markDirty()
          }
        }
      }
    }, editor._pendingEditorState == nil ? EditorUpdateOptions(tag: [HISTORY_MERGE_TAG]) : nil)
}

/// `$getRoot`: the root of the active editor state.
public func getRoot() throws -> RootNode {
  try internalGetRoot(getActiveEditorState())
}

package func internalGetRoot(_ editorState: EditorState) -> RootNode {
  editorState._nodeMap.get("root") as! RootNode
}

/// `$restoreEmptyContainerParagraph`: appends a paragraph to a root or
/// shadow root left empty when `removedChild` (a block) was removed.
@discardableResult
public func restoreEmptyContainerParagraph(
  _ container: LexicalNode?, _ removedChild: LexicalNode?
)
  throws -> ParagraphNode?
{
  guard let container = container as? ElementNode, try isRootOrShadowRoot(container),
    try container.isAttached()
  else {
    return nil
  }
  let isEmpty =
    try container is RootNode ? container.getChildrenSize() == 0 : container.isEmpty()
  if !isEmpty {
    return nil
  }
  if !(container is RootNode), try removedChild.map(INTERNAL_isBlock) != true {
    return nil
  }
  let paragraph = try createParagraphNode()
  try container.append(paragraph)
  return paragraph
}

/// `$setSelection`: makes `selection` the selection of the active editor
/// state.
public func setSelection(_ selection: BaseSelection?) throws {
  try errorOnReadOnly()
  let editorState = try getActiveEditorState()
  if let selection {
    selection.dirty = true
    selection.setCachedNodes(nil)
    if let range = selection as? RangeSelection, try getActiveEditor()._slotsUsed {
      try clampRangeSelectionToSlotFrame(range)
    }
  }
  editorState._selection = selection
}

/// `doesContainSurrogatePair`.
public func doesContainSurrogatePair(_ str: String) -> Bool {
  str.unicodeScalars.contains { $0.value > 0xFFFF }
}

package func getEditorsToPropagate(_ editor: Editor) -> [Editor] {
  var editorsToPropagate: [Editor] = []
  var currentEditor: Editor? = editor
  while let current = currentEditor {
    editorsToPropagate.append(current)
    currentEditor = current._parentEditor
  }
  return editorsToPropagate
}

/// `createUID()`, as JS computes it without a DOM: `s1`, `s2`, ... in base
/// 36.
package func createUID() -> String {
  let context = ThreadContext.current
  context.serverUIDCounter += 1
  return "s" + String(context.serverUIDCounter, radix: 36)
}

func getRootChildAncestor(_ node: LexicalNode) throws -> LexicalNode? {
  var current: LexicalNode? = node
  while let node = current {
    guard let parent = try node.getParent() else {
      return nil
    }
    if parent is RootNode {
      return node
    }
    current = parent
  }
  return nil
}

func normalizeSelectionForSelectAll(
  _ selection: RangeSelection, _ container: ElementNode
) throws
  -> RangeSelection
{
  let anchor = selection.anchor
  let focus = selection.focus
  let anchorKey = anchor.key
  let anchorOffset = anchor.offset
  let anchorType = anchor.type
  let focusKey = focus.key
  let focusOffset = focus.offset
  let focusType = focus.type
  _ = try normalizeSelection(selection)
  if !(container is RootNode) {
    return selection
  }
  // Selecting all of a document that is one shadow root (a table) keeps the
  // points on the root rather than descending into it.
  let anchorTop = try getRootChildAncestor(anchor.getNode())
  if let anchorTop = anchorTop as? ElementNode, anchorTop.isShadowRoot(),
    try anchorTop.is(getRootChildAncestor(focus.getNode()))
  {
    try anchor.set(anchorKey, anchorOffset, anchorType)
    try focus.set(focusKey, focusOffset, focusType)
  }
  return selection
}

/// `$selectAll`.
@discardableResult
public func selectAll(_ selection: RangeSelection? = nil) throws -> RangeSelection {
  let root = try getRoot()
  if let selection {
    let anchor = selection.anchor
    let focus = selection.focus
    let anchorNode = try anchor.getNode()
    if let rootNode = anchorNode as? RootNode {
      try anchor.set(rootNode.getKey(), 0, .element)
      try focus.set(rootNode.getKey(), rootNode.getChildrenSize(), .element)
      _ = try normalizeSelectionForSelectAll(selection, rootNode)
      return selection
    }
    let topParent = try anchorNode.getTopLevelElementOrThrow()
    guard let parent = try topParent.getParent() else {
      if let topElement = topParent as? ElementNode {
        try anchor.set(topElement.getKey(), 0, .element)
        try focus.set(topElement.getKey(), topElement.getChildrenSize(), .element)
        _ = try normalizeSelectionForSelectAll(selection, topElement)
      }
      return selection
    }
    try anchor.set(parent.getKey(), 0, .element)
    try focus.set(parent.getKey(), parent.getChildrenSize(), .element)
    _ = try normalizeSelectionForSelectAll(selection, parent)
    return selection
  }
  let newSelection = try root.select(0, root.getChildrenSize())
  try setSelection(normalizeSelectionForSelectAll(newSelection, root))
  return newSelection
}

/// `$nodesOfType`: the attached nodes of exactly `klass`'s type.
public func nodesOfType<T: LexicalNode>(_ klass: T.Type) throws -> [T] {
  let klassType = klass.getType()
  let editorState = try getActiveEditorState()
  if editorState._readOnly {
    let nodes = try getCachedTypeToNodeMap(editorState).get(klassType)
    return nodes?.valuesArray.compactMap { $0 as? T } ?? []
  }
  var nodesOfType: [T] = []
  for (_, node) in editorState._nodeMap {
    if let node = node as? T, node.__type == klassType, try node.isAttached() {
      nodesOfType.append(node)
    }
  }
  return nodesOfType
}

func resolveElement(
  _ element: ElementNode, _ isBackward: Bool, _ focusOffset: Int
) throws
  -> LexicalNode?
{
  let parent = try element.getParent()
  var offset = focusOffset
  var block = element
  if let parent {
    if isBackward && focusOffset == 0 {
      offset = try block.getIndexWithinParent()
      block = parent
    } else if try !isBackward && focusOffset == block.getChildrenSize() {
      offset = try block.getIndexWithinParent() + 1
      block = parent
    }
  }
  return try block.getChildAtIndex(isBackward ? offset - 1 : offset)
}

/// `$getAdjacentNode`.
public func getAdjacentNode(_ focus: Point, _ isBackward: Bool) throws -> LexicalNode? {
  let focusOffset = focus.offset
  if focus.type == .element {
    guard let block = try focus.getNode() as? ElementNode else { return nil }
    return try resolveElement(block, isBackward, focusOffset)
  }
  let focusNode = try focus.getNode()
  if try (isBackward && focusOffset == 0)
    || (!isBackward && focusOffset == focusNode.getTextContentSize())
  {
    let possibleNode =
      try isBackward ? focusNode.getPreviousSibling() : focusNode.getNextSibling()
    if possibleNode == nil {
      return try resolveElement(
        focusNode.getParentOrThrow(), isBackward,
        focusNode.getIndexWithinParent() + (isBackward ? 0 : 1))
    }
    return possibleNode
  }
  return nil
}

/// `dispatchCommand(editor, command, payload)`.
@discardableResult
public func dispatchCommand<P>(
  _ editor: Editor, _ command: LexicalCommand<P>, _ payload: P
) throws
  -> Bool
{
  try triggerCommandListeners(editor, command, payload, editor)
}

/// `$hasUpdateTag`.
public func hasUpdateTag(_ tag: UpdateTag) throws -> Bool {
  try getActiveEditor()._updateTags.has(tag)
}

/// `$addUpdateTag`.
public func addUpdateTag(_ tag: UpdateTag) throws {
  try errorOnReadOnly()
  try getActiveEditor()._updateTags.add(tag)
}

/// `$onUpdate`: runs `updateFn` after this update commits.
public func onUpdate(_ updateFn: @escaping () throws -> Void) throws {
  try errorOnReadOnly()
  try getActiveEditor()._deferred.append(updateFn)
}

/// `$maybeMoveChildrenSelectionToParent`.
package func maybeMoveChildrenSelectionToParent(
  _ parentNode: LexicalNode
) throws
  -> BaseSelection?
{
  let selection = try getSelection()
  guard let range = selection as? RangeSelection, parentNode is ElementNode else {
    return selection
  }
  let anchor = range.anchor
  let focus = range.focus
  let anchorNode = try anchor.getNode()
  let focusNode = try focus.getNode()
  if try hasAncestor(anchorNode, parentNode) {
    try anchor.set(parentNode.__key, 0, .element)
  }
  if try hasAncestor(focusNode, parentNode) {
    try focus.set(parentNode.__key, 0, .element)
  }
  return selection
}

/// `$hasAncestor`.
public func hasAncestor(_ child: LexicalNode, _ targetNode: LexicalNode) throws -> Bool {
  var parent = try child.getParent()
  while let current = parent {
    if current.is(targetNode) {
      return true
    }
    parent = try current.getParent()
  }
  return false
}

/// `$isInlineElementOrDecoratorNode`.
public func isInlineElementOrDecoratorNode(_ node: LexicalNode) throws -> Bool {
  try (node is ElementNode && node.isInline()) || (node is DecoratorNode && node.isInline())
}

/// `$getNearestRootOrShadowRoot`.
public func getNearestRootOrShadowRoot(_ node: LexicalNode) throws -> ElementNode {
  var current: LexicalNode = try node.getLatest()
  while true {
    if try getSlotHostKey(current) != nil, let element = current as? ElementNode {
      return element
    }
    let parent = try current.getParentOrThrow()
    if try isRootOrShadowRoot(parent) {
      return parent
    }
    current = parent
  }
}

/// `$isShadowRootNode`.
public func isShadowRootNode(_ node: LexicalNode?) -> Bool {
  (node as? ElementNode)?.isShadowRoot() ?? false
}

/// `$isRootOrShadowRoot`.
public func isRootOrShadowRoot(_ node: LexicalNode?) throws -> Bool {
  node is RootNode || isShadowRootNode(node)
}

/// `$copyNode`: a copy of `node` with a new key and no position.
public func copyNode<T: LexicalNode>(_ node: T, skipReset: Bool = false) throws -> T {
  let copy = try type(of: node).init(key: node.__key)
  try setNodeKey(copy, nil)
  try copy.afterCloneFrom(node)
  if !skipReset {
    try copy.resetOnCopyNodeFrom(node)
  }
  return copy
}

/// `$applyNodeReplacement`: what the editor's replacement for `node`'s class
/// makes of it.
public func applyNodeReplacement<N: LexicalNode>(_ node: N) throws -> N {
  let editor = try getActiveEditor()
  let nodeType = node.getType()
  guard let registeredNode = getRegisteredNode(editor, nodeType) else {
    throw LexicalError(
      "$applyNodeReplacement node \(type(of: node)) with type \(nodeType) must be registered to the editor. You can do this by passing the node class via the \"nodes\" array in the editor config."
    )
  }
  guard let replace = registeredNode.replace else {
    return node
  }
  let replacementNode = try replace(node)
  let replacementNodeKlass = type(of: replacementNode)
  if let replaceWithKlass = registeredNode.replaceWithKlass {
    try invariant(
      isSubclass(replacementNodeKlass, of: replaceWithKlass),
      "$applyNodeReplacement failed. Expected replacement node to be an instance of \(replaceWithKlass) with type \(replaceWithKlass.getType()) but returned \(replacementNodeKlass) with type \(replacementNodeKlass.getType()) from original node \(type(of: node)) with type \(nodeType)"
    )
  } else {
    try invariant(
      isSubclass(replacementNodeKlass, of: type(of: node))
        && replacementNodeKlass != type(of: node),
      "$applyNodeReplacement failed. Ensure replacement node \(replacementNodeKlass) with type \(replacementNodeKlass.getType()) is a subclass of the original node \(type(of: node)) with type \(nodeType)."
    )
  }
  try invariant(
    replacementNode.__key != node.__key,
    "$applyNodeReplacement failed. Ensure that the key argument is *not* used in your replace function (from node \(type(of: node)) with type \(nodeType) to node \(replacementNodeKlass) with type \(replacementNodeKlass.getType())), Node keys must never be re-used except by the static clone method."
  )
  guard let result = replacementNode as? N else {
    throw LexicalError("$applyNodeReplacement failed: \(replacementNodeKlass) is not a \(N.self)")
  }
  return result
}

package func errorOnInsertTextNodeOnRoot(_ node: LexicalNode, _ insertNode: LexicalNode) throws {
  let parentNode = try node.getParent()
  if parentNode is RootNode && !(insertNode is ElementNode) && !(insertNode is DecoratorNode) {
    throw LexicalError("Only element or decorator nodes can be inserted in to the root node")
  }
}

/// `$splitNode`: splits `node` at child `offset` up to its root or shadow
/// root, returning the left and right trees.
public func splitNode(_ node: ElementNode, _ offset: Int) throws -> (ElementNode?, ElementNode) {
  let startNode: LexicalNode = try node.getChildAtIndex(offset) ?? node
  try invariant(try !isRootOrShadowRoot(node), "Can not call $splitNode() on root element")

  func recurse(_ currentNode: LexicalNode) throws -> (ElementNode, ElementNode, LexicalNode) {
    let parent = try currentNode.getParentOrThrow()
    let isParentRoot = try isRootOrShadowRoot(parent)
    // The node we start split from (leaf) is moved, but its recursive
    // parents are copied to create separate tree
    let nodeToMove =
      try currentNode === startNode && !isParentRoot
      ? currentNode : copyNode(currentNode)
    if isParentRoot {
      guard let current = currentNode as? ElementNode, let moved = nodeToMove as? ElementNode
      else {
        throw LexicalError("Children of a root must be ElementNode")
      }
      try current.insertAfter(moved)
      return (current, moved, moved)
    }
    let (leftTree, rightTree, newParent) = try recurse(parent)
    let nextSiblings = try currentNode.getNextSiblings()
    try (newParent as! ElementNode).append(contentsOf: [nodeToMove] + nextSiblings)
    return (leftTree, rightTree, nodeToMove)
  }

  let (leftTree, rightTree, _) = try recurse(startNode)
  return (leftTree, rightTree)
}

/// `INTERNAL_$isBlock`: a block decorator, or a non-inline element that
/// holds inline content (or nothing).
package func INTERNAL_isBlock(_ node: LexicalNode) throws -> Bool {
  if node is DecoratorNode, try !node.isInline() {
    return true
  }
  guard let element = node as? ElementNode, try !isRootOrShadowRoot(element) else {
    return false
  }
  let firstChild = try element.getFirstChild()
  let isLeafElement: Bool
  if let firstChild {
    isLeafElement =
      try firstChild is LineBreakNode || firstChild is TextNode || firstChild.isInline()
  } else {
    isLeafElement = true
  }
  return try !element.isInline() && element.canBeEmpty() && isLeafElement
}

/// `$getEditor`: the active editor.
public func getEditor() throws -> Editor {
  try getActiveEditor()
}

/// `$isKeyInSelectionNodes`.
package func isKeyInSelectionNodes(_ selection: BaseSelection, _ key: NodeKey) throws -> Bool {
  try selection.getNodes().contains { $0.__key == key }
}

/// `$getTextInsideCache`: whether each element has text inside, cached for
/// a frozen state (or for one export walk).
package func getTextInsideCache() throws -> JSMap<NodeKey, Bool>? {
  var editorState = try getActiveEditorState()
  if !editorState._readOnly {
    let editor = internalGetActiveEditor()
    guard let editor, editor._pendingEditorState === editorState,
      editor._dirtyType == NO_DIRTY_NODES, editor._editorState._readOnly
    else {
      if let memo = exportMemo, memo.editorState === editorState {
        return memo.holds
      }
      return nil
    }
    editorState = editor._editorState
  }
  if editorState._textInsideCache == nil {
    editorState._textInsideCache = JSMap()
  }
  return editorState._textInsideCache
}

package struct ExportMemo {
  let editorState: EditorState
  let holds: JSMap<NodeKey, Bool>
}

private var exportMemo: ExportMemo? {
  get { ThreadContext.current.exportMemo }
  set { ThreadContext.current.exportMemo = newValue }
}

/// `INTERNAL_$withExportMemo`: memoizes `$hasTextInside` for one export
/// walk of a pending state, until a node changes.
package func INTERNAL_withExportMemo<T>(_ f: () throws -> T) throws -> T {
  let context = ThreadContext.current
  if context.exportWalk {
    return try f()
  }
  let editorState = internalGetActiveEditorState()
  context.exportWalk = true
  if let editorState, !editorState._readOnly {
    context.exportMemo = ExportMemo(editorState: editorState, holds: JSMap())
  } else {
    context.exportMemo = nil
  }
  defer {
    context.exportWalk = false
    context.exportMemo = nil
  }
  return try f()
}

/// A node that changes ends an export walk's memo.
package func internalStopExportMemo() {
  let context = ThreadContext.current
  if context.exportMemo != nil {
    context.exportMemo = nil
  }
}

public typealias TypeToNodeMap = JSMap<String, NodeMap>

/// `getCachedTypeToNodeMap`: the nodes of a frozen state by type.
package func getCachedTypeToNodeMap(_ editorState: EditorState) throws -> TypeToNodeMap {
  // If this is a new Editor it may have a writable this._editorState with
  // only a 'root' entry.
  if !editorState._readOnly && editorState.isEmpty() {
    return TypeToNodeMap()
  }
  try invariant(editorState._readOnly, "getCachedTypeToNodeMap called with a writable EditorState")
  if let cached = editorState._typeToNodeMap {
    return cached
  }
  let typeToNodeMap = TypeToNodeMap()
  for (nodeKey, node) in editorState._nodeMap {
    let nodeType = node.__type
    var nodeMap = typeToNodeMap.get(nodeType)
    if nodeMap == nil {
      nodeMap = NodeMap()
      typeToNodeMap.set(nodeType, nodeMap!)
    }
    nodeMap!.set(nodeKey, node)
  }
  editorState._typeToNodeMap = typeToNodeMap
  return typeToNodeMap
}

/// `$cloneWithProperties`: a mutable version of `latestNode` (same key),
/// which `getWritable` puts in the pending state.
package func cloneWithProperties<T: LexicalNode>(_ latestNode: T) throws -> T {
  let mutableNode = try type(of: latestNode).init(key: latestNode.__key)
  try mutableNode.afterCloneFrom(latestNode)
  try invariant(
    mutableNode.__key == latestNode.__key,
    "$cloneWithProperties: \(type(of: latestNode)).clone(node) (with type '\(latestNode.__type)') did not return a node with the same key, make sure to specify node.__key as the last argument to the constructor"
  )
  try invariant(
    mutableNode.__parent == latestNode.__parent && mutableNode.__next == latestNode.__next
      && mutableNode.__prev == latestNode.__prev,
    "$cloneWithProperties: \(type(of: latestNode)).clone(node) (with type '\(latestNode.__type)') overrode afterCloneFrom but did not call super.afterCloneFrom(prevNode)"
  )
  return mutableNode
}

/// `$cloneWithPropertiesEphemeral`.
public func cloneWithPropertiesEphemeral<T: LexicalNode>(_ latestNode: T) throws -> T {
  markEphemeral(try cloneWithProperties(latestNode))
}

/// `$create(klass)`: a node of `klass`, or of the class the editor replaces
/// it with.
public func create<T: LexicalNode>(_ klass: T.Type) throws -> T {
  let editor = try getEditor()
  try errorOnReadOnly()
  let registeredNode = try editor.resolveRegisteredNodeAfterReplacements(
    editor.getRegisteredNode(klass))
  let node = try registeredNode.klass.init()
  let result = registeredNode.replace == nil ? node : try applyNodeReplacement(node)
  guard let typed = result as? T else {
    throw LexicalError("$create: \(type(of: result)) is not a \(T.self)")
  }
  return typed
}

/// `$findMatchingParent`: the nearest of `startingNode` and its ancestors,
/// below the root, for which `findFn` holds.
public func findMatchingParent(
  _ startingNode: LexicalNode, _ findFn: (LexicalNode) throws -> Bool
) throws -> LexicalNode? {
  var curr: LexicalNode? = startingNode
  while let node = curr, !(node is RootNode) {
    if try findFn(node) {
      return node
    }
    curr = try node.getParent()
  }
  return nil
}

/// `$createChildrenArray`.
package func createChildrenArray(_ element: ElementNode, _ nodeMap: NodeMap?) throws -> [NodeKey] {
  var children: [NodeKey] = []
  var nodeKey = element.__first
  while let key = nodeKey {
    let node = try nodeMap == nil ? getNodeByKey(key) : nodeMap!.get(key)
    guard let node else {
      throw LexicalError("$createChildrenArray: node does not exist in nodeMap")
    }
    children.append(key)
    nodeKey = node.__next
  }
  return children
}

/// `getRegisteredSubtypeMap`: each registered type mapped to itself and the
/// types of the registered classes that extend it.
public func getRegisteredSubtypeMap(_ nodes: [LexicalNode.Type]) -> JSMap<String, JSSet<String>> {
  let subtypes = JSMap<String, JSSet<String>>()
  let klassByType = JSMap<String, LexicalNode.Type>()
  for klass in nodes {
    if let ownNodeType = getStaticNodeConfig(klass).ownNodeType {
      klassByType.set(ownNodeType, klass)
      subtypes.set(ownNodeType, JSSet())
    }
  }
  for (type, klass) in klassByType {
    for link in iterStaticNodeConfigChain(klass) {
      if let ownNodeType = link.ownNodeType, let bucket = subtypes.get(ownNodeType) {
        bucket.add(type)
      }
    }
  }
  return subtypes
}

/// `mergeRegister`: one clean-up that calls each of `funcs` in reverse
/// order, once; later calls do nothing.
public func mergeRegister(_ funcs: [() -> Void]) -> () -> Void {
  var funcs = funcs
  return {
    for fn in funcs.reversed() {
      fn()
    }
    funcs = []
  }
}

public func mergeRegister(_ funcs: (() -> Void)...) -> () -> Void {
  mergeRegister(funcs)
}
