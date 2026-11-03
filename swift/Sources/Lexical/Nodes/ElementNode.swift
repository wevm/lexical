// Port of packages/lexical/src/nodes/LexicalElementNode.ts.

/// `ElementFormatType`: `''`, `left`, `start`, `center`, `right`, `end`, or
/// `justify`.
public typealias ElementFormatType = String

/// `'ltr' | 'rtl'`, or `nil`.
public typealias ElementDirection = String

/// A node with children, held as a doubly linked list of keys.
open class ElementNode: LexicalNode, SlotNode {
  package var __first: NodeKey?
  package var __last: NodeKey?
  package var __size = 0
  package var __format = 0
  package var __style = ""
  package var __indent = 0
  package var __dir: ElementDirection?
  package var __textFormat = 0
  package var __textStyle = ""
  package var __slotHost: NodeKey?
  /// What the node-structure normalization last recorded about this
  /// version's rule inputs (JS keeps it in a `WeakMap` keyed by version);
  /// never cloned.
  package var __structureRuleInputs: Int?
  package var __slots: SlotMap?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ElementNode.self, extends: LexicalNode.self,
      json: [
        SchemaField(
          "direction", enumValue([.null, "ltr", "rtl"]), of: ElementNode.self,
          get: { try $0.getDirection().map(JSONValue.string) ?? .null },
          set: { try $0.setDirection($1?.stringValue) }),
        SchemaField(
          "format", enumValue(["", "left", "start", "center", "right", "end", "justify"]),
          of: ElementNode.self,
          get: { try .string($0.getFormatType()) },
          set: { try $0.setFormat($1?.stringValue ?? "") }),
        SchemaField(
          "indent", numberValue(0, integer: true, min: 0), of: ElementNode.self,
          get: { try .int($0.getIndent()) },
          set: { try $0.setIndent(Int($1?.numberValue ?? 0)) }),
        SchemaField(
          "textFormat", numberValue(), of: ElementNode.self,
          get: { try $0.getSerializedTextFormat().map(JSONValue.int) },
          set: { try $0.setTextFormat(Int($1?.numberValue ?? 0)) }),
        SchemaField(
          "textStyle", stringValue(), of: ElementNode.self,
          get: { try $0.getSerializedTextStyle().map(JSONValue.string) },
          set: { try $0.setTextStyle($1?.stringValue ?? "") }),
      ],
      transform: { try normalizeShadowRootChildren($0 as! ElementNode) })
  }

  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prevNode = prevNode as! ElementNode
    if __key == prevNode.__key {
      __first = prevNode.__first
      __last = prevNode.__last
      __size = prevNode.__size
      __slotHost = prevNode.__slotHost
      try invariant(
        __slotHost == nil || __parent == nil,
        "ElementNode: node \(__key) is both slotted into host \(__slotHost ?? "null") and a child of parent \(__parent ?? "null"); __slotHost and __parent are mutually exclusive"
      )
      __slots = prevNode.__slots
    }
    __style = prevNode.__style
    // afterCloneElementNode
    __dir = prevNode.__dir
    __format = prevNode.__format
    __indent = prevNode.__indent
    __textFormat = prevNode.__textFormat
    __textStyle = prevNode.__textStyle
  }

  public func getFormat() throws -> Int {
    try getLatest().__format
  }

  open func getFormatType() throws -> ElementFormatType {
    ELEMENT_FORMAT_TO_TYPE[try getFormat()] ?? ""
  }

  open func getStyle() throws -> String {
    try getLatest().__style
  }

  open func getIndent() throws -> Int {
    try getLatest().__indent
  }

  public func getChildren() throws -> [LexicalNode] {
    try collectSiblingNodes(getFirstChild(), .next)
  }

  public func getChildrenKeys() throws -> [NodeKey] {
    var children: [NodeKey] = []
    var child = try getFirstChild()
    while let current = child {
      children.append(current.__key)
      child = try current.getNextSibling()
    }
    return children
  }

  public func getChildrenSize() throws -> Int {
    try getLatest().__size
  }

  open func isEmpty() throws -> Bool {
    try getChildrenSize() == 0 && getSlotNames(self).isEmpty
  }

  open override func isDirty() throws -> Bool {
    try getActiveEditor()._dirtyElements.has(__key)
  }

  public func isLastChild() throws -> Bool {
    let latest = try getLatest()
    let parentLastChild = try getParentOrThrow().getLastChild()
    return parentLastChild?.is(latest) ?? false
  }

  public func getAllTextNodes() throws -> [TextNode] {
    var textNodes: [TextNode] = []
    for name in try getSlotNames(self) {
      if let slot = try getSlot(self, name) as? ElementNode {
        textNodes += try slot.getAllTextNodes()
      }
    }
    var child = try getFirstChild()
    while let current = child {
      if let text = current as? TextNode {
        textNodes.append(text)
      }
      if let element = current as? ElementNode {
        textNodes += try element.getAllTextNodes()
      }
      child = try current.getNextSibling()
    }
    return textNodes
  }

  public func getFirstDescendant() throws -> LexicalNode? {
    var node = try getFirstChild()
    while let element = node as? ElementNode {
      guard let child = try element.getFirstChild() else {
        break
      }
      node = child
    }
    return node
  }

  public func getLastDescendant() throws -> LexicalNode? {
    var node = try getLastChild()
    while let element = node as? ElementNode {
      guard let child = try element.getLastChild() else {
        break
      }
      node = child
    }
    return node
  }

  public func getDescendantByIndex(_ index: Int) throws -> LexicalNode? {
    let atEnd = try index >= getChildrenSize()
    let child = try atEnd ? getLastChild() : getChildAtIndex(index)
    if let element = child as? ElementNode {
      return try (atEnd ? element.getLastDescendant() : element.getFirstDescendant()) ?? child
    }
    return child
  }

  public func getFirstChild() throws -> LexicalNode? {
    guard let firstKey = try getLatest().__first else {
      return nil
    }
    return try getNodeByKey(firstKey)
  }

  public func getFirstChildOrThrow() throws -> LexicalNode {
    guard let firstChild = try getFirstChild() else {
      throw LexicalError("Expected node \(__key) to have a first child.")
    }
    return firstChild
  }

  public func getLastChild() throws -> LexicalNode? {
    guard let lastKey = try getLatest().__last else {
      return nil
    }
    return try getNodeByKey(lastKey)
  }

  public func getLastChildOrThrow() throws -> LexicalNode {
    guard let lastChild = try getLastChild() else {
      throw LexicalError("Expected node \(__key) to have a last child.")
    }
    return lastChild
  }

  public func getChildAtIndex(_ index: Int) throws -> LexicalNode? {
    let size = try getChildrenSize()
    var node: LexicalNode?
    // `index < size / 2`, in JS's floating-point division.
    if Double(index) < Double(size) / 2 {
      node = try getFirstChild()
      var i = 0
      while let current = node, i <= index {
        if i == index {
          return current
        }
        node = try current.getNextSibling()
        i += 1
      }
      return nil
    }
    node = try getLastChild()
    var i = size - 1
    while let current = node, i >= index {
      if i == index {
        return current
      }
      node = try current.getPreviousSibling()
      i -= 1
    }
    return nil
  }

  open override func getTextContent() throws -> String {
    var textContent = try getSlotsTextContent(self)
    let children = try getChildren()
    for (i, child) in children.enumerated() {
      textContent += try child.getTextContent()
      if child is ElementNode, i != children.count - 1, try !child.isInline() {
        textContent += DOUBLE_LINE_BREAK
      }
    }
    return textContent
  }

  open override func getTextContentSize() throws -> Int {
    var textContentSize = try getSlotsTextContentSize(self)
    let children = try getChildren()
    for (i, child) in children.enumerated() {
      textContentSize += try child.getTextContentSize()
      if child is ElementNode, i != children.count - 1, try !child.isInline() {
        textContentSize += DOUBLE_LINE_BREAK.utf16.count
      }
    }
    return textContentSize
  }

  open func getDirection() throws -> ElementDirection? {
    try getLatest().__dir
  }

  open func getTextFormat() throws -> Int {
    try getLatest().__textFormat
  }

  public func hasFormat(_ type: ElementFormatType) throws -> Bool {
    if type != "" {
      let formatFlag = ELEMENT_TYPE_TO_FORMAT[type] ?? 0
      return try (getFormat() & formatFlag) != 0
    }
    return false
  }

  public func hasTextFormat(_ type: TextFormatType) throws -> Bool {
    let formatFlag = TEXT_TYPE_TO_FORMAT[type] ?? 0
    return try (getTextFormat() & formatFlag) != 0
  }

  /// The text format a node would have after toggling `type`, matching
  /// `alignWithFormat` when given.
  public func getFormatFlags(_ type: TextFormatType, _ alignWithFormat: Int?) throws -> Int {
    toggleTextFormatType(try getLatest().__textFormat, type, alignWithFormat)
  }

  open func getTextStyle() throws -> String {
    try getLatest().__textStyle
  }

  // Mutators

  @discardableResult
  open func select(
    _ _anchorOffset: Int? = nil, _ _focusOffset: Int? = nil
  ) throws
    -> RangeSelection
  {
    try errorOnReadOnly()
    let selection = try getSelection()
    var anchorOffset = _anchorOffset
    var focusOffset = _focusOffset
    let childrenCount = try getChildrenSize()
    if try !canBeEmpty() {
      if _anchorOffset == 0 && _focusOffset == 0 {
        let firstChild = try getFirstChild()
        if let text = firstChild as? TextNode {
          return try text.select(0, 0)
        }
        if let element = firstChild as? ElementNode {
          return try element.select(0, 0)
        }
      } else if (_anchorOffset == nil || _anchorOffset == childrenCount)
        && (_focusOffset == nil || _focusOffset == childrenCount)
      {
        let lastChild = try getLastChild()
        if let text = lastChild as? TextNode {
          return try text.select()
        }
        if let element = lastChild as? ElementNode {
          return try element.select()
        }
      }
    }
    if anchorOffset == nil {
      anchorOffset = childrenCount
    }
    if focusOffset == nil {
      focusOffset = childrenCount
    }
    let key = __key
    guard let selection = selection as? RangeSelection else {
      return try internalMakeRangeSelection(
        key, anchorOffset!, key, focusOffset!, .element, .element)
    }
    try selection.anchor.set(key, anchorOffset!, .element)
    try selection.focus.set(key, focusOffset!, .element)
    selection.dirty = true
    return selection
  }

  @discardableResult
  open override func selectStart() throws -> RangeSelection {
    if let firstNode = try getFirstDescendant() {
      return try firstNode.selectStart()
    }
    return try select()
  }

  @discardableResult
  open override func selectEnd() throws -> RangeSelection {
    if let lastNode = try getLastDescendant() {
      return try lastNode.selectEnd()
    }
    return try select()
  }

  @discardableResult
  public func clear() throws -> Self {
    let writableSelf = try getWritable()
    for child in try getChildren() {
      try child.remove()
    }
    return writableSelf
  }

  @discardableResult
  public func append(_ nodesToAppend: LexicalNode...) throws -> Self {
    try splice(getChildrenSize(), 0, nodesToAppend)
  }

  @discardableResult
  public func append(contentsOf nodesToAppend: [LexicalNode]) throws -> Self {
    try splice(getChildrenSize(), 0, nodesToAppend)
  }

  @discardableResult
  open func setDirection(_ direction: ElementDirection?) throws -> Self {
    let latest = try getWritable()
    latest.__dir = direction
    return latest
  }

  @discardableResult
  open func setFormat(_ type: ElementFormatType) throws -> Self {
    let latest = try getWritable()
    latest.__format = type != "" ? ELEMENT_TYPE_TO_FORMAT[type] ?? 0 : 0
    return latest
  }

  @discardableResult
  open func setStyle(_ style: String) throws -> Self {
    let latest = try getWritable()
    latest.__style = style
    return latest
  }

  @discardableResult
  open func setTextFormat(_ type: Int) throws -> Self {
    let latest = try getWritable()
    latest.__textFormat = type
    return latest
  }

  @discardableResult
  open func setTextStyle(_ style: String) throws -> Self {
    let latest = try getWritable()
    latest.__textStyle = style
    return latest
  }

  @discardableResult
  open func setIndent(_ indentLevel: Int) throws -> Self {
    let latest = try getWritable()
    latest.__indent = indentLevel
    return latest
  }

  /// Removes `deleteCount` children at `start` and inserts `nodesToInsert`
  /// there, moving each from wherever it is.
  @discardableResult
  open func splice(
    _ start: Int, _ deleteCount: Int, _ nodesToInsert: [LexicalNode]
  ) throws
    -> Self
  {
    try elementSplice(start, deleteCount, nodesToInsert)
  }

  /// `ElementNode.prototype.splice.call(node, ...)`: ElementNode's own
  /// splice, which an override cannot change.
  @discardableResult
  package final func elementSplice(
    _ start: Int, _ deleteCount: Int, _ nodesToInsert: [LexicalNode]
  ) throws
    -> Self
  {
    try invariant(
      !__ephemeral,
      "ElementNode.splice: Ephemeral nodes can not mutate their children (key \(__key) type \(__type))"
    )
    let oldSize = try getChildrenSize()
    let writableSelf = try getWritable()
    try invariant(
      start + deleteCount <= oldSize,
      "ElementNode.splice: start + deleteCount > oldSize (\(start) + \(deleteCount) > \(oldSize))"
    )
    for nodeToInsert in nodesToInsert {
      try errorOnSlotCycleChild(writableSelf, nodeToInsert)
    }
    let writableSelfKey = writableSelf.__key
    var nodesToInsertKeys: [NodeKey] = []
    var nodesToRemoveKeys: [NodeKey] = []
    var nodeAfterRange = try getChildAtIndex(start + deleteCount)
    var nodeBeforeRange: LexicalNode? = nil
    var newSize = oldSize - deleteCount + nodesToInsert.count

    if start != 0 {
      if start == oldSize {
        nodeBeforeRange = try getLastChild()
      } else if let node = try getChildAtIndex(start) {
        nodeBeforeRange = try node.getPreviousSibling()
      }
    }

    if deleteCount > 0 {
      nodesToRemoveKeys = try detachSiblingRange(
        writableSelf,
        nodeBeforeRange == nil ? getFirstChild() : nodeBeforeRange!.getNextSibling(),
        deleteCount, nodeBeforeRange)
    }

    var writablePrevNode: LexicalNode? =
      try !nodesToInsert.isEmpty && nodeBeforeRange != nil
      ? nodeBeforeRange!.getWritable() : nil
    for nodeToInsert in nodesToInsert {
      if let prev = writablePrevNode, nodeToInsert.is(prev) {
        nodeBeforeRange = try prev.getPreviousSibling()
        writablePrevNode = try nodeBeforeRange?.getWritable()
      }
      if let after = nodeAfterRange, nodeToInsert.is(after) {
        nodeAfterRange = try after.getNextSibling()
      }
      let writableNodeToInsert = try nodeToInsert.getWritable()
      if writableNodeToInsert.__parent == writableSelfKey {
        newSize -= 1
      }
      try detachNode(writableNodeToInsert)
      let nodeKeyToInsert = nodeToInsert.__key
      try insertNodeBetween(
        writableSelf, writableNodeToInsert, writablePrevNode, nodeAfterRange?.getWritable())
      if nodeToInsert.__key == writableSelfKey {
        throw LexicalError("append: attempting to append self")
      }
      nodesToInsertKeys.append(nodeKeyToInsert)
      writablePrevNode = writableNodeToInsert
    }

    writableSelf.__size = newSize

    // In case of deletion we need to adjust selection, copy it first and
    // then re-apply.
    if !nodesToRemoveKeys.isEmpty {
      if let selection = try getSelection() as? RangeSelection {
        let nodesToRemoveKeySet = Set(nodesToRemoveKeys)
        let nodesToInsertKeySet = Set(nodesToInsertKeys)
        for point in [selection.anchor, selection.focus] {
          if try isPointRemoved(point, nodesToRemoveKeySet, nodesToInsertKeySet) {
            try moveSelectionPointToSibling(
              point, point.getNode(), self, nodeBeforeRange, nodeAfterRange)
          }
        }
        // Cleanup if node can't be empty.
        if newSize == 0, try !canBeEmpty(), try !isRootOrShadowRoot(self) {
          try remove()
        }
      }
    }

    return writableSelf
  }

  /// Whether the element's own `textFormat` and `textStyle` are serialized:
  /// only when it has no text to take them from.
  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let output = try super.exportDOM(editor)
    if let element = output.element as? DOMElement {
      let indent = try getIndent()
      if indent > 0 {
        // padding-inline-start is not widely supported in email HTML; the
        // data attribute is the authoritative round-trip signal.
        element.style.setProperty("padding-inline-start", "\(indent * 40)px")
        element.setAttribute("data-lexical-indent", String(indent))
      }
      if let direction = try getDirection(), !direction.isEmpty {
        element.dir = direction
      }
    }
    return DOMExportOutput(element: output.element)
  }

  open func shouldSerializeTextStyles() throws -> Bool {
    if try isRootOrShadowRoot(self) {
      return false
    }
    return try !hasTextChild(self, deep: isInline())
  }

  /// `getSerializedTextFormat`: the `textFormat` to serialize, if any.
  open func getSerializedTextFormat() throws -> Int? {
    let textFormat = try getTextFormat()
    return try textFormat != 0 && shouldSerializeTextStyles() ? textFormat : nil
  }

  /// `getSerializedTextStyle`: the `textStyle` to serialize, if any.
  open func getSerializedTextStyle() throws -> String? {
    let textStyle = try getTextStyle()
    return try textStyle != "" && shouldSerializeTextStyles() ? textStyle : nil
  }

  // These are intended to be extends for specific element heuristics.

  open func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws
    -> LexicalNode?
  {
    nil
  }

  open func canIndent() -> Bool { true }

  /// The behavior of the node during backwards deletion (backspace) when
  /// the selection is at its start (offset 0). Return `true` when you make a
  /// change, which stops the collapse; otherwise, for an inline node with no
  /// previous sibling, it is asked of the parent.
  open func collapseAtStart(_ selection: RangeSelection) throws -> Bool { false }

  open func excludeFromCopy(_ destination: String? = nil) throws -> Bool { false }

  open func canReplaceWith(_ replacement: LexicalNode) throws -> Bool { true }

  open func canInsertAfter(_ node: LexicalNode) throws -> Bool { true }

  open func canBeEmpty() throws -> Bool { true }

  open func canInsertTextBefore() throws -> Bool { true }

  open func canInsertTextAfter() throws -> Bool { true }

  /// `false`, or the `inline` a class in its config chain declares (JS
  /// synthesizes an `isInline()` returning it for the declaring class).
  open override func isInline() throws -> Bool {
    getNodeClassRecord(Swift.type(of: self)).inline ?? false
  }

  /// A shadow root is an element that should behave like the root for its
  /// children: a table cell, for example.
  open func isShadowRoot() -> Bool { false }

  open func canMergeWith(_ node: ElementNode) throws -> Bool { false }

  open func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    false
  }

  /// Whether an empty element should merge with what is inserted into it,
  /// rather than being replaced.
  open func canMergeWhenEmpty() -> Bool { false }
}

/// `$normalizeShadowRootChildren`: an inline child of the root or a shadow
/// root is wrapped in the parent element it asks for, unless the editor
/// enforces node structure.
func normalizeShadowRootChildren(_ node: ElementNode) throws {
  if try isRootOrShadowRoot(node), try !INTERNAL_isNodeStructureEnforced(getActiveEditor()) {
    var block: ElementNode? = nil
    for child in try node.getChildren() {
      if try child.isInline() {
        let target = try block ?? child.replace(child.createParentElementNode())
        block = try target.append(child)
      } else {
        block = nil
      }
    }
  }
}

public func isElementNode(_ node: LexicalNode?) -> Bool { node is ElementNode }

func hasTextChild(_ element: ElementNode, deep: Bool) throws -> Bool {
  if deep {
    return try hasTextInside(element, getTextInsideCache())
  }
  var child = try element.getFirstChild()
  while let current = child {
    if current is TextNode {
      return true
    }
    child = try current.getNextSibling()
  }
  return false
}

func hasTextInside(_ element: ElementNode, _ cache: JSMap<NodeKey, Bool>?) throws -> Bool {
  let key = element.getKey()
  if let cached = cache?.get(key) {
    return cached
  }
  var found = false
  var child = try element.getFirstChild()
  while let current = child, !found {
    if current is TextNode {
      found = true
    } else if let childElement = current as? ElementNode {
      found = try hasTextInside(childElement, cache)
    }
    child = try current.getNextSibling()
  }
  cache?.set(key, found)
  return found
}

func isPointRemoved(
  _ point: Point, _ nodesToRemoveKeySet: Set<NodeKey>, _ nodesToInsertKeySet: Set<NodeKey>
) throws -> Bool {
  var node: LexicalNode? = try point.getNode()
  while let current = node {
    let nodeKey = current.__key
    if nodesToRemoveKeySet.contains(nodeKey) && !nodesToInsertKeySet.contains(nodeKey) {
      return true
    }
    node = try current.getParent()
  }
  return false
}
