// Port of packages/lexical/src/LexicalSelection.ts: points, the range and
// node selections, and the helpers the core uses to keep them valid. Text
// insertion and deletion follow; DOM selection belongs to the UI layers.

/// A point's type: `'text'` or `'element'`.
public enum PointType: String, Sendable {
  case text
  case element
}

/// `Point`: a key, an offset (UTF-16 in text, a child index in an element)
/// and a type.
public final class Point {
  public private(set) var key: NodeKey
  public private(set) var offset: Int
  public private(set) var type: PointType
  weak var _selection: (any BaseSelection)?

  public init(_ key: NodeKey, _ offset: Int, _ type: PointType) {
    self.key = key
    self.offset = offset
    self.type = type
  }

  public func `is`(_ point: Point) -> Bool {
    key == point.key && offset == point.offset && type == point.type
  }

  public func isBefore(_ b: Point) throws -> Bool {
    if key == b.key {
      return offset < b.offset
    }
    let aCaret = try normalizeCaret(caretFromPoint(self, .next))
    let bCaret = try normalizeCaret(caretFromPoint(b, .next))
    return try comparePointCaretNext(aCaret, bCaret) < 0
  }

  public func getNode() throws -> LexicalNode {
    guard let node = try getNodeByKey(key) else {
      throw LexicalError("Point.getNode: node not found")
    }
    return node
  }

  public func set(
    _ key: NodeKey, _ offset: Int, _ type: PointType, _ onlyIfChanged: Bool = false
  ) throws {
    let selection = _selection
    let oldKey = self.key
    if onlyIfChanged && self.key == key && self.offset == offset && self.type == type {
      return
    }
    self.key = key
    self.offset = offset
    self.type = type
    let node = try getNodeByKey(key)
    try invariant(
      type == .text ? node is TextNode : node is ElementNode,
      "PointType.set: node with key \(key) is \(node?.__type ?? "[not found]") and can not be used for a \(type.rawValue) point"
    )
    if !isCurrentlyReadOnlyMode() {
      if try getCompositionKey() == oldKey {
        try setCompositionKey(key)
      }
      if let selection {
        selection.setCachedNodes(nil)
        (selection as? RangeSelection)?._cachedIsBackward = nil
        selection.dirty = true
      }
    }
  }
}

/// `$createPoint`.
public func createPoint(_ key: NodeKey, _ offset: Int, _ type: PointType) -> Point {
  Point(key, offset, type)
}

private func selectPointOnNode(_ point: Point, _ node: LexicalNode) throws {
  var key = node.__key
  var offset = point.offset
  var type = PointType.element
  if let text = node as? TextNode {
    type = .text
    let textContentLength = try text.getTextContentSize()
    if offset > textContentLength {
      offset = textContentLength
    }
  } else if !(node is ElementNode) {
    if let nextSibling = try node.getNextSibling() as? TextNode {
      key = nextSibling.__key
      offset = 0
      type = .text
    } else if let parentNode = try node.getParent() {
      key = parentNode.__key
      offset = try node.getIndexWithinParent() + 1
    }
  }
  try point.set(key, offset, type)
}

/// `$moveSelectionPointToEnd`.
public func moveSelectionPointToEnd(_ point: Point, _ node: LexicalNode) throws {
  if let element = node as? ElementNode {
    let lastNode = try element.getLastDescendant()
    if let lastNode, lastNode is ElementNode || lastNode is TextNode {
      try selectPointOnNode(point, lastNode)
    } else {
      try selectPointOnNode(point, node)
    }
  } else {
    try selectPointOnNode(point, node)
  }
}

/// `BaseSelection`: what range, node, and table selections share.
public protocol BaseSelection: AnyObject {
  var dirty: Bool { get set }
  func clone() -> any BaseSelection
  func getNodes() throws -> [LexicalNode]
  func getTextContent() throws -> String
  func extract() throws -> [LexicalNode]
  func insertText(_ text: String) throws
  func insertRawText(_ text: String) throws
  func insertNodes(_ nodes: [LexicalNode]) throws
  func `is`(_ selection: (any BaseSelection)?) -> Bool
  func getStartEndPoints() -> (Point, Point)?
  func isCollapsed() -> Bool
  func isBackward() throws -> Bool
  func getCachedNodes() -> [LexicalNode]?
  func setCachedNodes(_ nodes: [LexicalNode]?)
}

/// `NodeSelection`: a set of selected nodes, such as decorators.
public final class NodeSelection: BaseSelection {
  package var _nodes: JSSet<NodeKey>
  var _cachedNodes: [LexicalNode]?
  public var dirty = false

  public init(_ objects: JSSet<NodeKey>) {
    _nodes = objects
  }

  public func getCachedNodes() -> [LexicalNode]? { _cachedNodes }
  public func setCachedNodes(_ nodes: [LexicalNode]?) { _cachedNodes = nodes }

  public func `is`(_ selection: (any BaseSelection)?) -> Bool {
    guard let selection = selection as? NodeSelection else {
      return false
    }
    return _nodes.size == selection._nodes.size && _nodes.allSatisfy { selection._nodes.has($0) }
  }

  public func isCollapsed() -> Bool { false }
  public func isBackward() -> Bool { false }
  public func getStartEndPoints() -> (Point, Point)? { nil }

  public func add(_ key: NodeKey) {
    dirty = true
    _nodes.add(key)
    _cachedNodes = nil
  }

  public func delete(_ key: NodeKey) {
    dirty = true
    _nodes.delete(key)
    _cachedNodes = nil
  }

  public func clear() {
    dirty = true
    _nodes.clear()
    _cachedNodes = nil
  }

  public func has(_ key: NodeKey) -> Bool { _nodes.has(key) }

  public func clone() -> any BaseSelection { NodeSelection(_nodes.copy()) }

  public func extract() throws -> [LexicalNode] { try getNodes() }

  public func getNodes() throws -> [LexicalNode] {
    if let cachedNodes = _cachedNodes {
      return cachedNodes
    }
    var nodes: [LexicalNode] = []
    for object in _nodes {
      if let node = try getNodeByKey(object) {
        nodes.append(node)
      }
    }
    if !isCurrentlyReadOnlyMode() {
      _cachedNodes = nodes
    }
    return nodes
  }

  public func getTextContent() throws -> String {
    try getNodes().map { try $0.getTextContent() }.joined()
  }

  /// Removes the selected nodes, leaving the selection where the first was.
  public func deleteNodes() throws {
    let nodes = try getNodes().filter { try getSlotHostKey($0) == nil }
    if let first = nodes.first, try (getSelection() ?? getPreviousSelection()) === self {
      let firstCaret = getSiblingCaret(first, .next)
      try setSelectionFromCaretRange(getCaretRange(firstCaret, firstCaret))
    }
    for node in nodes {
      try node.remove()
    }
    try ensureRootHasParagraph()
  }
}

package func ensureRootHasParagraph() throws {
  let root = try getRoot()
  if try root.getChildrenSize() == 0, try getSelectionSlotFrame(getSelection()) == nil {
    let paragraph = try createParagraphNode()
    try root.append(paragraph)
    try paragraph.select()
  }
}

/// `$isRangeSelection`.
public func isRangeSelection(_ x: Any?) -> Bool { x is RangeSelection }

/// `$isNodeSelection`.
public func isNodeSelection(_ x: Any?) -> Bool { x is NodeSelection }

/// `RangeSelection`: an anchor and a focus, with the format and style
/// typed text takes.
public final class RangeSelection: BaseSelection {
  public var format: Int
  public var style: String
  public let anchor: Point
  public let focus: Point
  var _cachedNodes: [LexicalNode]?
  package var _cachedIsBackward: Bool?
  public var dirty = false

  public init(_ anchor: Point, _ focus: Point, _ format: Int, _ style: String) {
    self.anchor = anchor
    self.focus = focus
    self.format = format
    self.style = style
    anchor._selection = self
    focus._selection = self
  }

  public func getCachedNodes() -> [LexicalNode]? { _cachedNodes }
  public func setCachedNodes(_ nodes: [LexicalNode]?) { _cachedNodes = nodes }

  public func `is`(_ selection: (any BaseSelection)?) -> Bool {
    guard let selection = selection as? RangeSelection else {
      return false
    }
    return anchor.is(selection.anchor) && focus.is(selection.focus)
      && format == selection.format && style == selection.style
  }

  public func isCollapsed() -> Bool { anchor.is(focus) }

  public func getNodes() throws -> [LexicalNode] {
    if let cachedNodes = _cachedNodes {
      return cachedNodes
    }
    let range = try getCaretRangeInDirection(caretRangeFromSelection(self), .next)
    let nodes = try getNodesFromCaretRangeCompat(range)
    if isCollapsed() && nodes.count > 1 {
      throw LexicalError(
        "RangeSelection.getNodes() returned \(nodes.count) > 1 nodes in a collapsed selection")
    }
    if !isCurrentlyReadOnlyMode() {
      _cachedNodes = nodes
    }
    return nodes
  }

  @discardableResult
  public func setTextNodeRange(
    _ anchorNode: TextNode, _ anchorOffset: Int, _ focusNode: TextNode, _ focusOffset: Int
  ) throws -> RangeSelection {
    try anchor.set(anchorNode.__key, anchorOffset, .text)
    try focus.set(focusNode.__key, focusOffset, .text)
    return self
  }

  public func getTextContent() throws -> String {
    if isCollapsed() {
      return ""
    }
    let nodes = try getNodes()
    let (anchorSlice, focusSlice) = try caretRangeFromSelection(self).getTextSlices()
    var textContent = ""
    var prevWasElement = true
    for node in nodes {
      if let element = node as? ElementNode, try !element.isInline() {
        if !prevWasElement {
          textContent += "\n"
        }
        var slotText = ""
        for slotName in try getSlotNames(element) {
          if let slot = try getSlot(element, slotName) {
            slotText += try slot.getTextContent()
          }
        }
        if !slotText.isEmpty {
          textContent += slotText
          prevWasElement = false
        } else if try element.isEmpty() {
          prevWasElement = false
        } else {
          prevWasElement = true
        }
      } else {
        prevWasElement = false
        if node is TextNode {
          let slice = [anchorSlice, focusSlice].lazy.compactMap { $0 }
            .first { $0.caret.origin.is(node) }
          textContent += try slice?.getTextContent() ?? node.getTextContent()
        } else if node is DecoratorNode || node is LineBreakNode {
          textContent += try node.getTextContent()
        }
      }
    }
    return textContent
  }

  public func clone() -> any BaseSelection { cloneRange() }

  /// `clone()` with its concrete type.
  public func cloneRange() -> RangeSelection {
    RangeSelection(
      createPoint(anchor.key, anchor.offset, anchor.type),
      createPoint(focus.key, focus.offset, focus.type), format, style)
  }

  public func toggleFormat(_ format: TextFormatType) {
    self.format = toggleTextFormatType(self.format, format, nil)
    dirty = true
  }

  public func setFormat(_ format: Int) {
    self.format = format
    dirty = true
  }

  public func setStyle(_ style: String) {
    self.style = style
    dirty = true
  }

  public func hasFormat(_ type: TextFormatType) -> Bool {
    (format & (TEXT_TYPE_TO_FORMAT[type] ?? 0)) != 0
  }

  public func isBackward() throws -> Bool {
    if let cached = _cachedIsBackward {
      return cached
    }
    let isBackward = try focus.isBefore(anchor)
    if !isCurrentlyReadOnlyMode() {
      _cachedIsBackward = isBackward
    }
    return isBackward
  }

  public func getStartEndPoints() -> (Point, Point)? { (anchor, focus) }
}

private func getNodesFromCaretRangeCompat(_ range: CaretRange) throws -> [LexicalNode] {
  var nodes: [LexicalNode] = []
  let (beforeSlice, afterSlice) = try range.getTextSlices()
  if let beforeSlice {
    nodes.append(beforeSlice.caret.origin)
  }
  var seenAncestors = Set<NodeKey>()
  var openElements = 0
  let carets = try range.iterNodeCarets()
  while let caret = try carets.next() {
    let origin = caret.origin
    if caret.isChild {
      if nodes.isEmpty {
        seenAncestors.insert(origin.__key)
      } else {
        openElements += 1
        nodes.append(origin)
      }
    } else if !(origin is ElementNode) || openElements == 0 {
      nodes.append(origin)
    } else {
      openElements -= 1
    }
  }
  if let afterSlice {
    nodes.append(afterSlice.caret.origin)
  }
  if range.focus.isSibling, range.focus.origin is ElementNode,
    try range.focus.getNodeAtCaret() == nil
  {
    var reverseCaret: NodeCaret? = getChildCaret(range.focus.origin, .previous)
    while let caret = reverseCaret, caret.isChild, seenAncestors.contains(caret.origin.__key),
      try !(caret.origin as! ElementNode).isEmpty(), caret.origin.is(nodes.last)
    {
      seenAncestors.remove(caret.origin.__key)
      nodes.removeLast()
      reverseCaret = try getAdjacentChildCaret(caret)
    }
  }
  while nodes.count > 1 {
    if let lastIncludedNode = nodes.last as? ElementNode {
      if try openElements > 0 || lastIncludedNode.isEmpty()
        || seenAncestors.contains(lastIncludedNode.__key)
      {
        // Keep it.
      } else {
        nodes.removeLast()
        continue
      }
    }
    break
  }
  if nodes.isEmpty && range.isCollapsed() {
    let normCaret = try normalizeCaret(range.anchor)
    let flippedNormCaret = try normalizeCaret(range.anchor.getFlipped())
    func candidate(_ caret: PointCaret) throws -> LexicalNode? {
      try caret.isText ? caret.origin : caret.getNodeAtCaret()
    }
    let node =
      try candidate(normCaret) ?? candidate(flippedNormCaret)
      ?? (range.anchor.getNodeAtCaret() != nil ? normCaret.origin : flippedNormCaret.origin)
    nodes.append(node)
  }
  return nodes
}

/// `$setSelectionFromCaretRange`.
@discardableResult
public func setSelectionFromCaretRange(_ caretRange: CaretRange) throws -> RangeSelection {
  let selection = try getSelection() as? RangeSelection ?? createRangeSelection()
  try updateRangeSelectionFromCaretRange(selection, caretRange)
  try setSelection(selection)
  return selection
}

/// `$updateRangeSelectionFromCaretRange`.
public func updateRangeSelectionFromCaretRange(
  _ selection: RangeSelection, _ range: CaretRange
)
  throws
{
  try setPointFromCaret(selection.anchor, range.anchor)
  try setPointFromCaret(selection.focus, range.focus)
}

/// `$getPointSlotFrame`.
public func getPointSlotFrame(_ point: Point) throws -> LexicalNode? {
  try getNodeByKey(point.key).flatMap(getSlotFrame)
}

/// `$isSameFrame`.
public func isSameFrame(_ a: LexicalNode?, _ b: LexicalNode?) -> Bool {
  guard let a else { return b == nil }
  return a.is(b)
}

private func setPointToFrameEdge(_ point: Point, _ frame: LexicalNode, _ atEnd: Bool) throws {
  if let element = frame as? ElementNode {
    try point.set(frame.getKey(), atEnd ? element.getChildrenSize() : 0, .element)
  } else {
    try point.set(frame.getKey(), atEnd ? frame.getTextContentSize() : 0, .text)
  }
}

private func getSlotHostInFrame(
  _ frame: LexicalNode, _ target: LexicalNode?
) throws
  -> LexicalNode?
{
  var host = try getSlotHost(frame)
  while let current = host {
    let hostFrame = try getSlotFrame(current)
    if isSameFrame(hostFrame, target) {
      return try hostFrame != nil || current.isAttached() ? current : nil
    }
    host = try hostFrame.flatMap(getSlotHost)
  }
  return nil
}

/// `$setPointAtSlotBoundary` in `'selection'` mode; deletion mode follows
/// with the deletion port.
private func setPointAtSlotBoundary(
  _ point: Point, _ anchorNode: LexicalNode?, _ anchorFrame: LexicalNode?,
  _ focusFrame: LexicalNode?, _ resolveFocusAfterAnchor: () throws -> Bool
) throws -> Bool {
  let host = try focusFrame.flatMap { try getSlotHostInFrame($0, anchorFrame) }
  if host == nil && anchorFrame == nil {
    return false
  }
  let focusAfterAnchor = try resolveFocusAfterAnchor()
  let hostParent = try host?.getParent()
  let parent: ElementNode
  let offset: Int
  if let host, let hostParent,
    try anchorNode == nil || !(host.is(anchorNode!) || host.isParentOf(anchorNode!))
  {
    parent = hostParent
    offset = try host.getIndexWithinParent() + (focusAfterAnchor ? 1 : 0)
  } else {
    let edge = host ?? anchorFrame
    guard let element = edge as? ElementNode else {
      if let edge {
        try setPointToFrameEdge(point, edge, focusAfterAnchor)
      }
      return true
    }
    parent = element
    offset = try focusAfterAnchor ? element.getChildrenSize() : 0
  }
  try point.set(parent.getKey(), offset, .element)
  return true
}

/// `$getCommonAncestorAcrossSlots` result, as in JS.
public enum CommonAncestorAcrossSlotsResult {
  case same(commonAncestor: LexicalNode)
  case ancestor(commonAncestor: LexicalNode, b: LexicalNode)
  case descendant(commonAncestor: LexicalNode, a: LexicalNode)
  case branch(commonAncestor: LexicalNode, a: LexicalNode, b: LexicalNode)
}

private func getParentAcrossSlots(_ node: LexicalNode) throws -> LexicalNode? {
  try node.getParent() ?? getSlotHost(node)
}

/// `$getCommonAncestorAcrossSlots`.
public func getCommonAncestorAcrossSlots(
  _ a: LexicalNode, _ b: LexicalNode
) throws
  -> CommonAncestorAcrossSlotsResult?
{
  var aPath: [NodeKey: LexicalNode?] = [:]
  var child: LexicalNode? = nil
  var node: LexicalNode? = a
  while let current = node {
    aPath[current.getKey()] = .some(child)
    child = current
    node = try getParentAcrossSlots(current)
  }
  child = nil
  node = b
  while let current = node {
    if let aChild = aPath[current.getKey()] {
      let commonAncestor = try current.getLatest()
      switch (aChild, child) {
      case (nil, nil): return .same(commonAncestor: commonAncestor)
      case (nil, let b?): return .ancestor(commonAncestor: commonAncestor, b: b)
      case (let a?, nil): return .descendant(commonAncestor: commonAncestor, a: a)
      case (let a?, let b?): return .branch(commonAncestor: commonAncestor, a: a, b: b)
      }
    }
    child = current
    node = try getParentAcrossSlots(current)
  }
  return nil
}

/// `$comparePointCaretNextAcrossSlots`.
public func comparePointCaretNextAcrossSlots(_ a: PointCaret, _ b: PointCaret) throws -> Int? {
  guard let compare = try getCommonAncestorAcrossSlots(a.origin, b.origin) else {
    return nil
  }
  switch compare {
  case .same:
    if a.isText && b.isText {
      return a.offset < b.offset ? -1 : a.offset > b.offset ? 1 : 0
    }
    if a.kind == b.kind { return 0 }
    return a.isText ? -1 : b.isText ? 1 : a.isChild ? -1 : 1
  case .ancestor(_, let nodeB):
    return try a.isChild && getSlotHostKey(nodeB) == nil ? -1 : 1
  case .descendant(_, let nodeA):
    return try b.isChild && getSlotHostKey(nodeA) == nil ? 1 : -1
  case .branch(let commonAncestor, let nodeA, let nodeB):
    let aSlotKey = try getSlotHostKey(nodeA) != nil ? nodeA.getKey() : nil
    let bSlotKey = try getSlotHostKey(nodeB) != nil ? nodeB.getKey() : nil
    if let aSlotKey, let bSlotKey {
      for key in try getSlotMap(commonAncestor).values where key == aSlotKey || key == bSlotKey {
        return key == aSlotKey ? -1 : 1
      }
    }
    if aSlotKey != nil { return -1 }
    if bSlotKey != nil { return 1 }
    return try nodeA.isBefore(nodeB) ? -1 : 1
  }
}

/// `$clampRangeSelectionToSlotFrame`: keeps the focus in the anchor's slot
/// frame.
@discardableResult
package func clampRangeSelectionToSlotFrame(_ selection: RangeSelection) throws -> Bool {
  let anchorPoint = selection.anchor
  let focusPoint = selection.focus
  let anchorFrame = try getPointSlotFrame(anchorPoint)
  let focusFrame = try getPointSlotFrame(focusPoint)
  let clamped =
    try !isSameFrame(anchorFrame, focusFrame)
    && setPointAtSlotBoundary(
      focusPoint, getNodeByKey(anchorPoint.key), anchorFrame, focusFrame,
      {
        try comparePointCaretNextAcrossSlots(
          caretFromPoint(selection.anchor, .next), caretFromPoint(selection.focus, .next)) != 1
      })
  if clamped {
    selection.dirty = true
  }
  return clamped
}

/// `$isBlockElementNode`.
public func isBlockElementNode(_ node: LexicalNode?) throws -> Bool {
  guard let element = node as? ElementNode else { return false }
  return try !element.isInline()
}

/// `$internalMakeRangeSelection`: a new dirty range selection, made the
/// active state's.
@discardableResult
package func internalMakeRangeSelection(
  _ anchorKey: NodeKey, _ anchorOffset: Int, _ focusKey: NodeKey, _ focusOffset: Int,
  _ anchorType: PointType, _ focusType: PointType
) throws -> RangeSelection {
  let editorState = try getActiveEditorState()
  let selection = RangeSelection(
    createPoint(anchorKey, anchorOffset, anchorType),
    createPoint(focusKey, focusOffset, focusType), 0, "")
  selection.dirty = true
  editorState._selection = selection
  return selection
}

/// `$createRangeSelection`: a collapsed selection at the start of the root.
public func createRangeSelection() -> RangeSelection {
  RangeSelection(createPoint("root", 0, .element), createPoint("root", 0, .element), 0, "")
}

/// `$createNodeSelection`.
public func createNodeSelection() -> NodeSelection {
  NodeSelection(JSSet())
}

/// `$internalRefreshSelectionFormatAndStyle`.
package func internalRefreshSelectionFormatAndStyle(
  _ selection: RangeSelection, _ previousAnchorKey: NodeKey
) throws {
  let anchor = selection.anchor
  if anchor.key == previousAnchorKey {
    return
  }
  let anchorNode = try anchor.getNode()
  var format = 0
  var style = ""
  if let text = anchorNode as? TextNode {
    format = try text.getFormat()
    style = try text.getStyle()
  } else if let element = anchorNode as? ElementNode {
    format = try element.getTextFormat()
    style = try element.getTextStyle()
  }
  if selection.format != format || selection.style != style {
    selection.format = format
    selection.style = style
    selection.dirty = true
  }
}

/// `$getSelection`: the selection of the active editor state.
public func getSelection() throws -> (any BaseSelection)? {
  try getActiveEditorState()._selection
}

/// `$getPreviousSelection`: the selection of the committed state.
public func getPreviousSelection() throws -> (any BaseSelection)? {
  try getActiveEditor()._editorState._selection
}

/// `$selectionTouchesElement`.
package func selectionTouchesElement(
  _ selection: RangeSelection, _ parentNode: LexicalNode
)
  throws -> Bool
{
  let parentKey = parentNode.__key
  return selection.anchor.key == parentKey || selection.focus.key == parentKey
}

/// `$updateElementSelectionOnCreateDeleteNode`: shifts element points on
/// `parentNode` past a child added (`times` > 0) or removed at
/// `nodeOffset`.
package func updateElementSelectionOnCreateDeleteNode(
  _ selection: RangeSelection, _ parentNode: LexicalNode, _ nodeOffset: Int, _ times: Int = 1
) throws {
  if try !selectionTouchesElement(selection, parentNode) {
    return
  }
  for point in [selection.anchor, selection.focus] {
    if point.key == parentNode.__key
      && ((nodeOffset <= point.offset && times > 0) || (nodeOffset < point.offset && times < 0))
    {
      try point.set(parentNode.__key, max(0, point.offset + times), .element)
    }
    if let node = try point.getNode() as? ElementNode {
      let size = try node.getChildrenSize()
      let atEnd = point.offset >= size
      if let child = try node.getChildAtIndex(atEnd ? size - 1 : point.offset) as? TextNode {
        try point.set(child.__key, atEnd ? child.getTextContentSize() : 0, .text)
      }
    }
  }
}

/// `applySelectionTransforms`: lets the text nodes at the ends of the
/// pending selection adjust it.
package func applySelectionTransforms(_ nextEditorState: EditorState, _ editor: Editor) throws {
  let prevSelection = editor.getEditorState()._selection
  guard let nextSelection = nextEditorState._selection as? RangeSelection else {
    return
  }
  var anchorNode: LexicalNode? = nil
  if nextSelection.anchor.type == .text {
    let node = try nextSelection.anchor.getNode()
    anchorNode = node
    try (node as? TextNode)?.selectionTransform(prevSelection, nextSelection)
  }
  if nextSelection.focus.type == .text {
    let focusNode = try nextSelection.focus.getNode()
    if anchorNode !== focusNode {
      try (focusNode as? TextNode)?.selectionTransform(prevSelection, nextSelection)
    }
  }
}

/// `moveSelectionPointToSibling`: moves `point` off `node`, which is being
/// removed from `parent`.
package func moveSelectionPointToSibling(
  _ point: Point, _ node: LexicalNode, _ parent: ElementNode, _ prevSibling: LexicalNode?,
  _ nextSibling: LexicalNode?
) throws {
  let sibling = prevSibling ?? nextSibling
  if let text = sibling as? TextNode {
    try point.set(text.__key, prevSibling == nil ? 0 : text.getTextContentSize(), .text)
  } else if let element = sibling as? ElementNode {
    try point.set(element.__key, prevSibling == nil ? 0 : element.getChildrenSize(), .element)
  } else {
    let offset = try node.getIndexWithinParent()
    try point.set(parent.__key, offset == -1 ? parent.getChildrenSize() : offset, .element)
  }
}

/// `adjustPointOffsetForMergedSibling`.
package func adjustPointOffsetForMergedSibling(
  _ point: Point, _ isBefore: Bool, _ key: NodeKey, _ target: TextNode, _ textLength: Int
) throws {
  if point.type == .text {
    try point.set(key, point.offset + (isBefore ? 0 : textLength), .text)
  } else if try point.offset > target.getIndexWithinParent() {
    try point.set(point.key, point.offset - 1, .element)
  }
}

/// `$getTextContent`: the text of the selection, or empty.
public func getTextContent() throws -> String {
  try getSelection()?.getTextContent() ?? ""
}
