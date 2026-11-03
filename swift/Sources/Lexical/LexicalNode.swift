// Port of packages/lexical/src/LexicalNode.ts.

public typealias NodeKey = String

/// The base of every node. Mirrors JS `LexicalNode`: nodes are immutable once
/// an update commits, and every method resolves the latest version of the
/// node through its key, inside `editor.update` or `editor.read`.
///
/// A subclass declares its type, schema, and transforms in `config()`, like
/// `$config()` in JS, and needs `required init(key:)`. `@LexicalNode`
/// generates both.
open class LexicalNode {
  /// The node's type. Assigned once, in `init`.
  public package(set) var __type: String = ""
  /// The key every version of this node shares.
  public package(set) var __key: NodeKey = ""
  package var __parent: NodeKey?
  package var __prev: NodeKey?
  package var __next: NodeKey?
  package var __state: NodeState?
  /// See `$markEphemeral`.
  package var __ephemeral = false

  // MARK: Static configuration

  /// The string type of this node, unique among the nodes registered on an
  /// editor: `static getType()`. Read from `config()` by default.
  open class func getType() -> String {
    getNodeClassRecord(self).ownNodeType ?? ""
  }

  /// The node's static configuration: `$config()`. A class that does not
  /// override it inherits its superclass's, type included, as in JS.
  open class func config() -> NodeConfig? { nil }

  /// `static importJSON(serializedNode)`. The default builds the node with
  /// `$create` and applies the schema, which is what JS synthesizes for a
  /// node with a `$config`.
  open class func importJSON(_ serializedNode: JSONObject) throws -> LexicalNode {
    if let importJSON = getNodeClassRecord(self).importJSON {
      return try importJSON(serializedNode)
    }
    let node = try create(self)
    return try node.updateFromJSON(serializedNode)
  }

  /// Creates a node with a fresh key in the active editor state, or with
  /// `key` (a clone).
  public required init(key: NodeKey? = nil) throws {
    self.__type = Swift.type(of: self).getType()
    try setNodeKey(self, key)
    if __type != "root" {
      try errorOnTypeKlassMismatch(__type, Swift.type(of: self))
    }
  }

  /// Copies the state of `prevNode`, the version this node clones (same
  /// key), or the node it copies (a new key). A subclass that stores more
  /// than its schema declares overrides this and calls `super` first.
  open func afterCloneFrom(_ prevNode: LexicalNode) throws {
    if __key == prevNode.__key {
      __parent = prevNode.__parent
      __next = prevNode.__next
      __prev = prevNode.__prev
      __state = prevNode.__state
    } else if let state = prevNode.__state {
      __state = state.getWritable(self)
    }
  }

  /// Called on a copy made by `$copyNode`, to reset the state that should not
  /// carry over.
  open func resetOnCopyNodeFrom(_ originalNode: LexicalNode) throws {
    if let state = __state {
      __state = state.getWritable(self).resetOnCopyNode()
    }
  }

  // MARK: Getters and traversers

  public func getType() -> String { __type }

  open func isInline() throws -> Bool {
    throw LexicalError(
      "LexicalNode: Node \(Swift.type(of: self)) does not implement .isInline().")
  }

  /// Whether the node is attached to the root (directly, or through a slot
  /// host).
  public func isAttached() throws -> Bool {
    var nodeKey: NodeKey? = __key
    while let key = nodeKey {
      if key == "root" {
        return true
      }
      guard let node = try getNodeByKey(key) else {
        break
      }
      nodeKey = try node.__parent ?? getSlotHostKey(node)
    }
    return false
  }

  public func isSelected(_ selection: BaseSelection? = nil) throws -> Bool {
    guard let targetSelection = try selection ?? getSelection() else {
      return false
    }
    let isSelected = try isKeyInSelectionNodes(targetSelection, __key)
    if !isSelected || self is TextNode {
      return isSelected
    }
    // For inline images inside of element nodes.
    // Without this change the image will be selected if the cursor is before or after it.
    if let targetSelection = targetSelection as? RangeSelection,
      targetSelection.anchor.type == .element, targetSelection.focus.type == .element
    {
      if targetSelection.isCollapsed() {
        return false
      }
      let parentNode = try getParent()
      if self is DecoratorNode, try isInline(), let parentNode {
        let firstPoint =
          try targetSelection.isBackward() ? targetSelection.focus : targetSelection.anchor
        if try parentNode.is(firstPoint.getNode()),
          try firstPoint.offset == parentNode.getChildrenSize(),
          try self.is(parentNode.getLastChild())
        {
          return false
        }
      }
    }
    return isSelected
  }

  public func getKey() -> NodeKey { __key }

  public func getIndexWithinParent() throws -> Int {
    guard let parent = try getParent() else {
      return -1
    }
    var node = try parent.getFirstChild()
    var index = 0
    while let current = node {
      if self.is(current) {
        return index
      }
      index += 1
      node = try current.getNextSibling()
    }
    return -1
  }

  public func getParent() throws -> ElementNode? {
    guard let parent = try getLatest().__parent else {
      return nil
    }
    return try getNodeByKey(parent) as? ElementNode
  }

  public func getParentOrThrow() throws -> ElementNode {
    guard let parent = try getParent() else {
      throw LexicalError("Expected node \(__key) to have a parent.")
    }
    return parent
  }

  /// The ancestor that is a child of the root or of a shadow root: an
  /// `ElementNode`, or this node when it is a block `DecoratorNode`.
  open func getTopLevelElement() throws -> LexicalNode? {
    var node: LexicalNode? = self
    while let current = node {
      let parent = try current.getParent()
      // A slot value's host acts as a shadow root, so the slot boundary is
      // the top of the isolated scope and this node is its top-level element.
      if try isRootOrShadowRoot(parent) || getSlotHostKey(current) != nil {
        try invariant(
          current is ElementNode || (current === self && current is DecoratorNode),
          "Children of root nodes must be elements or decorators")
        return current
      }
      node = parent
    }
    return nil
  }

  open func getTopLevelElementOrThrow() throws -> LexicalNode {
    guard let parent = try getTopLevelElement() else {
      throw LexicalError("Expected node \(__key) to have a top parent element.")
    }
    return parent
  }

  public func getParents() throws -> [ElementNode] {
    var parents: [ElementNode] = []
    var node = try getParent()
    while let current = node {
      parents.append(current)
      node = try current.getParent()
    }
    return parents
  }

  public func getParentKeys() throws -> [NodeKey] {
    try getParents().map(\.__key)
  }

  public func getPreviousSibling() throws -> LexicalNode? {
    guard let prevKey = try getLatest().__prev else {
      return nil
    }
    return try getNodeByKey(prevKey)
  }

  public func getPreviousSiblings() throws -> [LexicalNode] {
    try collectSiblingNodes(getPreviousSibling(), .previous).reversed()
  }

  public func getNextSibling() throws -> LexicalNode? {
    guard let nextKey = try getLatest().__next else {
      return nil
    }
    return try getNodeByKey(nextKey)
  }

  public func getNextSiblings() throws -> [LexicalNode] {
    try collectSiblingNodes(getNextSibling(), .next)
  }

  /// Whether `object` is a version of this node (same key).
  public func `is`(_ object: LexicalNode?) -> Bool {
    guard let object else {
      return false
    }
    return __key == object.__key
  }

  public func isBefore(_ targetNode: LexicalNode) throws -> Bool {
    guard let compare = try getCommonAncestor(self, targetNode) else {
      return false
    }
    switch compare {
    case .descendant:
      return true
    case .branch:
      return try getCommonAncestorResultBranchOrder(compare) == -1
    case .same, .ancestor:
      return false
    }
  }

  public func isParentOf(_ targetNode: LexicalNode) throws -> Bool {
    try hasAncestor(targetNode, self)
  }

  public func getNodesBetween(_ targetNode: LexicalNode) throws -> [LexicalNode] {
    let forward = try isBefore(targetNode)
    var nodes: [LexicalNode] = []
    var node: LexicalNode? = self
    var entering = true
    var openElements = 0
    while let current = node {
      // Only ancestors above the starting point lack an enter event. Count
      // open elements to recognize their first visit without a visited Set.
      if entering || openElements == 0 {
        nodes.append(current)
      }
      if current.is(targetNode) {
        break
      }
      if !entering && openElements > 0 {
        openElements -= 1
      }
      var child: LexicalNode? = nil
      if entering, let element = current as? ElementNode {
        child = try forward ? element.getFirstChild() : element.getLastChild()
      }
      if child != nil {
        openElements += 1
      }
      let adjacent: LexicalNode? =
        try child ?? (forward ? current.getNextSibling() : current.getPreviousSibling())
      entering = adjacent != nil
      node = try adjacent ?? current.getParent()
    }
    return forward ? nodes : nodes.reversed()
  }

  open func isDirty() throws -> Bool {
    try getActiveEditor()._dirtyLeaves.has(__key)
  }

  /// The version of this node in the active editor state.
  public final func getLatest() throws -> Self {
    if __ephemeral {
      return self
    }
    guard let latest = try getNodeByKey(__key) as? Self else {
      throw LexicalError(staleNodeMessage(__key, __type))
    }
    return latest
  }

  /// A mutable version of this node, cloning it on its first write in an
  /// update.
  public final func getWritable() throws -> Self {
    if __ephemeral {
      return self
    }
    try errorOnReadOnly()
    // Before the clone, whose hooks may read: a node that changes ends an
    // export walk's memo (see INTERNAL_$withExportMemo).
    internalStopExportMemo()
    let editorState = try getActiveEditorState()
    let editor = try getActiveEditor()
    let key = __key
    let cloneNotNeeded = editor._cloneNotNeeded
    let selection = editorState._selection
    selection?.setCachedNodes(nil)
    if let writableNode = cloneNotNeeded.get(key) as? Self {
      // Transforms clear the dirty node set on each iteration to keep track
      // on newly dirty nodes.
      try internalMarkNodeAsDirty(writableNode)
      return writableNode
    }
    let nodeMap = editorState._nodeMap
    guard let latestNode = nodeMap.get(key) as? Self else {
      throw LexicalError(staleNodeMessage(key, __type))
    }
    let mutableNode = try cloneWithProperties(latestNode)
    cloneNotNeeded.set(key, mutableNode)
    nodeMap.set(key, mutableNode)
    try internalMarkNodeAsDirty(mutableNode)
    return mutableNode
  }

  open func getTextContent() throws -> String {
    try getSlotsTextContent(self)
  }

  /// The UTF-16 length of `getTextContent()`.
  open func getTextContentSize() throws -> Int {
    // Decorator slot hosts use this base impl: slot text is folded into
    // getTextContent, so .length is the size.
    try getTextContent().utf16.count
  }

  // MARK: Serialization

  /// The node's JSON: every property its schema declares, then its
  /// NodeState. With `compact`, the properties parsing would restore anyway
  /// are omitted. Override only for output a schema can not describe, and
  /// call `super`.
  open func exportJSON(_ compact: Bool = false) throws -> JSONObject {
    var json = try exportNodeJSONOnce(self, compact)
    if let state = __state {
      for (key, value) in try state.toJSON() {
        json[key] = value
      }
    }
    return json
  }

  /// Applies serialized JSON to this node: the whole schema, so a property
  /// the JSON omits is reset to its default. If overridden, call `super`.
  @discardableResult
  open func updateFromJSON(_ serializedNode: JSONObject) throws -> Self {
    try applyJSONSetters(updateStateFromJSON(self, serializedNode), serializedNode)
  }

  // MARK: Setters and mutators

  /// Removes this node from the EditorState. If it isn't re-inserted
  /// somewhere, the garbage collector cleans it up.
  open func remove(preserveEmptyParent: Bool = false) throws {
    try removeNode(self, restoreSelection: true, preserveEmptyParent: preserveEmptyParent)
  }

  /// Replaces this node with `replaceWith`, which is moved here.
  @discardableResult
  open func replace<N: LexicalNode>(_ replaceWith: N, includeChildren: Bool = false) throws -> N {
    try errorOnReadOnly()
    var selection = try getSelection()
    if let current = selection {
      selection = current.clone()
    }
    try errorOnInsertTextNodeOnRoot(self, replaceWith)
    let latest = try getLatest()
    let toReplaceKey = __key
    if let slotHost = try getSlotHost(latest) {
      throw LexicalError(
        "replace: node \(toReplaceKey) (type \(latest.getType())) is slotted into host \(slotHost.getKey()) (type \(slotHost.getType())); a slot value cannot be replaced through the tree API. Use $setSlot on its host to assign a replacement."
      )
    }
    let key = replaceWith.__key
    let writableReplaceWith = try replaceWith.getWritable()
    let writableParent = try getParentOrThrow().getWritable()
    try errorOnSlotCycleChild(writableParent, writableReplaceWith)
    let size = writableParent.__size
    // Detaching repairs the cloned selection's old-parent offsets (#6031).
    let replaceWithOldParent = try writableReplaceWith.getParent()
    _ = try detachNodeWithSelection(writableReplaceWith, selection as? RangeSelection)
    let prevSibling = try latest.getPreviousSibling()
    let nextSibling = try latest.getNextSibling()
    try removeNode(latest, restoreSelection: false, preserveEmptyParent: true)
    try insertNodeBetween(
      writableParent, writableReplaceWith, prevSibling?.getWritable(), nextSibling?.getWritable())
    // `size` was read before replaceWith was detached. When replaceWith was
    // already a child of this same parent, two children collapse into one.
    writableParent.__size =
      replaceWithOldParent?.is(writableParent) == true ? size - 1 : size
    var prevSizeBeforeChildrenTransfer = 0
    if includeChildren {
      guard let this = self as? ElementNode,
        let writableElement = writableReplaceWith as? ElementNode
      else {
        throw LexicalError("includeChildren should only be true for ElementNodes")
      }
      prevSizeBeforeChildrenTransfer = try writableElement.getChildrenSize()
      try writableElement.splice(prevSizeBeforeChildrenTransfer, 0, this.getChildren())
    }
    if let selection = selection as? RangeSelection {
      try setSelection(selection)
      // Children transfer maps element offsets into the replacement's
      // existing children. Other points retain replace's move-to-end
      // behavior.
      for point in [selection.anchor, selection.focus] where point.key == toReplaceKey {
        if includeChildren && point.type == .element {
          try point.set(
            writableReplaceWith.__key, prevSizeBeforeChildrenTransfer + point.offset, .element)
        } else {
          try moveSelectionPointToEnd(point, writableReplaceWith)
        }
      }
    }
    if try getCompositionKey() == toReplaceKey {
      try setCompositionKey(key)
    }
    return writableReplaceWith
  }

  /// Inserts `nodeToInsert` after this node, moving it from wherever it is.
  @discardableResult
  open func insertAfter(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  ) throws
    -> LexicalNode
  {
    try insertSibling(self, .next, nodeToInsert, restoreSelection)
  }

  /// Inserts `nodeToInsert` before this node, moving it from wherever it is.
  @discardableResult
  open func insertBefore(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  ) throws
    -> LexicalNode
  {
    try insertSibling(self, .previous, nodeToInsert, restoreSelection)
  }

  /// Whether this node must have a parent of the type
  /// `createParentElementNode` makes, for example a list item.
  open func isParentRequired() -> Bool { false }

  /// The parent `isParentRequired` asks for.
  open func createParentElementNode() throws -> ElementNode {
    try createParagraphNode()
  }

  @discardableResult
  open func selectStart() throws -> RangeSelection {
    try selectPrevious()
  }

  @discardableResult
  open func selectEnd() throws -> RangeSelection {
    try selectNext(0, 0)
  }

  @discardableResult
  open func selectPrevious(
    _ anchorOffset: Int? = nil, _ focusOffset: Int? = nil
  ) throws
    -> RangeSelection
  {
    try selectAdjacentNode(getSiblingCaret(self, .previous), anchorOffset, focusOffset)
  }

  @discardableResult
  open func selectNext(
    _ anchorOffset: Int? = nil, _ focusOffset: Int? = nil
  ) throws
    -> RangeSelection
  {
    try selectAdjacentNode(getSiblingCaret(self, .next), anchorOffset, focusOffset)
  }

  /// Marks the node dirty, so transforms and the reconciler visit it.
  public func markDirty() throws {
    _ = try getWritable()
  }
}

func staleNodeMessage(_ key: NodeKey, _ type: String) -> String {
  "Lexical node does not exist in active editor state (key \(key), type \(type)). The reference was probably kept from another read or update, for example in a closure, event handler or component state, and the node has since been removed or is not part of this editor state. Before using a node reference that may be stale, check node.isAttached() or look the node up again with $getNodeByKey(node.getKey()), inside the read or update where it is used."
}

/// `$removeNode`.
package func removeNode(
  _ nodeToRemove: LexicalNode, restoreSelection: Bool, preserveEmptyParent: Bool = false
) throws {
  try errorOnReadOnly()
  let key = nodeToRemove.__key
  guard let parent = try nodeToRemove.getParent() else {
    if let slotHostKey = try getSlotHostKey(nodeToRemove) {
      throw LexicalError(
        "$removeNode: node \(key) is slotted into host \(slotHostKey); use removeSlot on the host instead of remove()."
      )
    }
    return
  }
  let selection = try maybeMoveChildrenSelectionToParent(nodeToRemove)
  var selectionMoved = false
  if let selection = selection as? RangeSelection, restoreSelection {
    let anchor = selection.anchor
    let focus = selection.focus
    if anchor.key == key {
      try moveSelectionPointToSibling(
        anchor, nodeToRemove, parent, nodeToRemove.getPreviousSibling(),
        nodeToRemove.getNextSibling())
      selectionMoved = true
    }
    if focus.key == key {
      try moveSelectionPointToSibling(
        focus, nodeToRemove, parent, nodeToRemove.getPreviousSibling(),
        nodeToRemove.getNextSibling())
      selectionMoved = true
    }
  } else if selection is NodeSelection, restoreSelection, try nodeToRemove.isSelected() {
    try nodeToRemove.selectPrevious()
  }

  _ = try detachNodeWithSelection(
    nodeToRemove.getWritable(),
    restoreSelection && !selectionMoved ? selection as? RangeSelection : nil)

  if !preserveEmptyParent, try !isRootOrShadowRoot(parent), try !parent.canBeEmpty(),
    try parent.isEmpty()
  {
    try removeNode(parent, restoreSelection: restoreSelection)
  }
  if restoreSelection, selection != nil, parent is RootNode, try parent.isEmpty() {
    try parent.selectEnd()
  }
}

func errorOnTypeKlassMismatch(_ type: String, _ klass: LexicalNode.Type) throws {
  guard let registeredNode = try getRegisteredNode(getActiveEditor(), type) else {
    throw LexicalError(
      "Create node: Attempted to create node \(klass) that was not configured to be used on the editor."
    )
  }
  let editorKlass = registeredNode.klass
  if editorKlass != klass {
    throw LexicalError(
      "Create node: Type \(type) in node \(klass) does not match registered node \(editorKlass) with the same type"
    )
  }
}

/// `insertRangeAfter`.
public func insertRangeAfter(
  _ node: LexicalNode, _ firstToInsert: LexicalNode, _ lastToInsert: LexicalNode? = nil
) throws {
  let lastToInsert2 = try lastToInsert ?? firstToInsert.getParentOrThrow().getLastChild()!
  var current = firstToInsert
  var nodesToInsert = [firstToInsert]
  while current !== lastToInsert2 {
    guard let next = try current.getNextSibling() else {
      throw LexicalError(
        "insertRangeAfter: lastToInsert must be a later sibling of firstToInsert")
    }
    current = next
    nodesToInsert.append(current)
  }
  var currentNode = node
  for nodeToInsert in nodesToInsert {
    currentNode = try currentNode.insertAfter(nodeToInsert)
  }
}

/// `$isEphemeral`.
public func isEphemeral(_ node: LexicalNode) -> Bool { node.__ephemeral }

/// `$markEphemeral`: the node's `getLatest` and `getWritable` return itself.
/// It must not be added to an EditorState.
@discardableResult
public func markEphemeral<T: LexicalNode>(_ node: T) -> T {
  node.__ephemeral = true
  return node
}
