// Port of packages/lexical/src/caret/LexicalCaret.ts and the point helpers
// of caret/LexicalCaretUtils.ts.
//
// JS has one class per caret type and direction. Here one `PointCaret`
// value carries its kind, which the JS `type` field names; `SiblingCaret`,
// `ChildCaret`, `TextPointCaret` and `NodeCaret` are aliases of it, and the
// kind is checked where JS relies on the static type.

/// `CaretDirection`: `'next'` or `'previous'`.
public enum CaretDirection: String, Sendable {
  case next
  case previous

  /// `flipDirection`.
  public var flipped: CaretDirection { self == .next ? .previous : .next }
}

/// `flipDirection`.
public func flipDirection(_ direction: CaretDirection) -> CaretDirection { direction.flipped }

/// `RootMode`: whether a caret stops at the root only, or at shadow roots
/// too.
public enum RootMode: Sendable {
  case root
  case shadowRoot
}

/// The JS `type` of a caret.
public enum CaretKind: Equatable, Sendable {
  /// Beside `origin`.
  case sibling
  /// Inside `origin`, an element, at its start (`next`) or end (`previous`).
  case child
  /// At `offset` in `origin`, a text node.
  case text(offset: Int)
}

/// A caret: a position relative to `origin`, facing `direction`.
public struct PointCaret {
  public let origin: LexicalNode
  public let direction: CaretDirection
  public let kind: CaretKind

  init(_ origin: LexicalNode, _ direction: CaretDirection, _ kind: CaretKind) {
    self.origin = origin
    self.direction = direction
    self.kind = kind
  }

  /// The offset of a text point caret; 0 otherwise.
  public var offset: Int {
    if case .text(let offset) = kind { offset } else { 0 }
  }

  public var isText: Bool { if case .text = kind { true } else { false } }
  public var isSibling: Bool { kind == .sibling }
  public var isChild: Bool { kind == .child }

  /// The node on the far side of the caret.
  public func getNodeAtCaret() throws -> LexicalNode? {
    switch kind {
    case .child:
      let element = origin as! ElementNode
      return try direction == .next ? element.getFirstChild() : element.getLastChild()
    case .sibling, .text:
      return try direction == .next ? origin.getNextSibling() : origin.getPreviousSibling()
    }
  }

  /// The element the caret is in.
  public func getParentAtCaret() throws -> ElementNode? {
    if kind == .child {
      return origin as? ElementNode
    }
    return try origin.getParent()
  }

  /// The sibling caret of the node at the caret.
  public func getAdjacentCaret() throws -> SiblingCaret? {
    try Lexical.getSiblingCaret(getNodeAtCaret(), direction)
  }

  /// The sibling caret of `origin`.
  public func getSiblingCaret() -> SiblingCaret {
    kind == .sibling ? self : PointCaret(origin, direction, .sibling)
  }

  /// The child caret of an element origin; this caret when it is one.
  public func getChildCaret() -> ChildCaret? {
    switch kind {
    case .child: self
    case .sibling: origin is ElementNode ? PointCaret(origin, direction, .child) : nil
    case .text: nil
    }
  }

  /// The sibling caret of the parent, unless the parent is the root (or a
  /// shadow root with `.shadowRoot`), or a slot frame.
  public func getParentCaret(_ mode: RootMode = .root) throws -> SiblingCaret? {
    try Lexical.getSiblingCaret(filterByMode(getParentAtCaret(), mode), direction)
  }

  /// The same position facing the other way.
  public func getFlipped() throws -> PointCaret {
    let dir = direction.flipped
    switch kind {
    case .text(let offset):
      return PointCaret(origin, dir, .text(offset: offset))
    case .child:
      return try Lexical.getSiblingCaret(getNodeAtCaret(), dir) ?? PointCaret(origin, dir, .child)
    case .sibling:
      return try Lexical.getSiblingCaret(getNodeAtCaret(), dir)
        ?? PointCaret(origin.getParentOrThrow(), dir, .child)
    }
  }

  /// This caret with the latest version of `origin`.
  public func getLatest() throws -> PointCaret {
    let latest = try origin.getLatest()
    return latest === origin ? self : PointCaret(latest, direction, kind)
  }

  /// Whether `other` has the same origin, direction and kind (a text caret
  /// and a sibling caret of one text node count as the same).
  public func isSameNodeCaret(_ other: PointCaret?) -> Bool {
    guard let other, direction == other.direction, origin.is(other.origin) else {
      return false
    }
    switch kind {
    case .child: return other.kind == .child
    case .sibling, .text: return other.kind != .child
    }
  }

  /// Whether `other` is the same caret.
  public func isSamePointCaret(_ other: PointCaret?) -> Bool {
    guard let other else { return false }
    return direction == other.direction && origin.is(other.origin) && kind == other.kind
  }

  /// Removes the node at the caret.
  @discardableResult
  public func remove() throws -> PointCaret {
    try getNodeAtCaret()?.remove()
    return self
  }

  /// Inserts `node` at the caret.
  @discardableResult
  public func insert(_ node: LexicalNode) throws -> PointCaret {
    switch kind {
    case .child:
      let element = origin as! ElementNode
      try element.splice(direction == .next ? 0 : element.getChildrenSize(), 0, [node])
    case .sibling, .text:
      if direction == .next {
        try origin.insertAfter(node)
      } else {
        try origin.insertBefore(node)
      }
    }
    return self
  }

  /// Replaces the node at the caret with `node`, or inserts it.
  @discardableResult
  public func replaceOrInsert(
    _ node: LexicalNode, includeChildren: Bool = false
  ) throws
    -> PointCaret
  {
    let target = try getNodeAtCaret()
    if node.is(origin) || node.is(target) {
      // Already in place.
    } else if let target {
      try target.replace(node, includeChildren: includeChildren)
    } else {
      try insert(node)
    }
    return self
  }

  /// Removes `deleteCount` nodes after the caret and inserts `nodes`.
  @discardableResult
  public func splice(
    _ deleteCount: Int, _ nodes: [LexicalNode], _ nodesDirection: CaretDirection = .next
  ) throws -> PointCaret {
    let nodeIter = nodesDirection == direction ? nodes : nodes.reversed()
    var caret = self
    let parent = try getParentAtCaret()
    let nodesToRemove = JSMap<NodeKey, LexicalNode>()
    var removeCaret = try caret.getAdjacentCaret()
    while let current = removeCaret, nodesToRemove.size < deleteCount {
      let writableNode = try current.origin.getWritable()
      nodesToRemove.set(writableNode.getKey(), writableNode)
      removeCaret = try current.getAdjacentCaret()
    }
    for node in nodeIter {
      if nodesToRemove.size > 0 {
        guard let target = try caret.getNodeAtCaret() else {
          throw LexicalError(
            "NodeCaret.splice: Underflow of expected nodesToRemove during splice (keys: \(nodesToRemove.keysArray.joined(separator: " ")))"
          )
        }
        nodesToRemove.delete(target.getKey())
        nodesToRemove.delete(node.getKey())
        if target.is(node) || caret.origin.is(node) {
          // Already in place.
        } else {
          if let nodeParent = try node.getParent(), nodeParent.is(parent) {
            try node.remove()
          }
          try target.replace(node)
        }
      } else {
        try caret.insert(node)
      }
      caret = PointCaret(node, direction, .sibling)
    }
    for node in nodesToRemove.valuesArray {
      try node.remove()
    }
    return self
  }

  /// The sibling carets from this one in `direction`, as iterating a JS
  /// caret does.
  public func siblings() throws -> [SiblingCaret] {
    var result: [SiblingCaret] = []
    var caret = try getAdjacentCaret()
    while let current = caret {
      result.append(current)
      caret = try current.getAdjacentCaret()
    }
    return result
  }
}

public typealias SiblingCaret = PointCaret
public typealias ChildCaret = PointCaret
public typealias TextPointCaret = PointCaret
public typealias NodeCaret = PointCaret

private func filterByMode(_ node: ElementNode?, _ mode: RootMode) throws -> ElementNode? {
  guard let node else {
    return nil
  }
  switch mode {
  case .root where node is RootNode: return nil
  case .shadowRoot where try isRootOrShadowRoot(node): return nil
  default: break
  }
  return try getSlotHostKey(node) == nil ? node : nil
}

/// `$getSiblingCaret`.
public func getSiblingCaret(_ origin: LexicalNode, _ direction: CaretDirection) -> SiblingCaret {
  PointCaret(origin, direction, .sibling)
}

/// `$getSiblingCaret` for an optional origin.
public func getSiblingCaret(
  _ origin: LexicalNode?, _ direction: CaretDirection
)
  -> SiblingCaret?
{
  origin.map { PointCaret($0, direction, .sibling) }
}

/// `$getChildCaret`: `nil` unless `origin` is an element.
public func getChildCaret(_ origin: LexicalNode?, _ direction: CaretDirection) -> ChildCaret? {
  origin is ElementNode ? PointCaret(origin!, direction, .child) : nil
}

/// `$getChildCaret` for an element.
public func getChildCaret(_ origin: ElementNode, _ direction: CaretDirection) -> ChildCaret {
  PointCaret(origin, direction, .child)
}

/// The offset of a text caret: a number, or the end of the text in a
/// direction.
public enum TextOffset: Sendable, ExpressibleByIntegerLiteral {
  case offset(Int)
  case direction(CaretDirection)

  public init(integerLiteral value: Int) { self = .offset(value) }
}

/// `$getTextPointCaret`.
public func getTextPointCaret(
  _ origin: TextNode, _ direction: CaretDirection, _ offset: TextOffset
) throws -> TextPointCaret {
  PointCaret(origin, direction, .text(offset: try getTextNodeOffset(origin, offset)))
}

/// `$getTextPointCaret` with a numeric offset.
public func getTextPointCaret(
  _ origin: TextNode, _ direction: CaretDirection, _ offset: Int
)
  throws -> TextPointCaret
{
  try getTextPointCaret(origin, direction, .offset(offset))
}

/// `$getTextNodeOffset`: `offset` as a number; out-of-range offsets throw,
/// or with `clamp` are clamped.
public func getTextNodeOffset(
  _ origin: TextNode, _ offset: TextOffset, clamp: Bool = false
)
  throws -> Int
{
  let size = try origin.getTextContentSize()
  var numericOffset: Int
  switch offset {
  case .direction(.next): numericOffset = size
  case .direction(.previous): numericOffset = 0
  case .offset(let value): numericOffset = value
  }
  if numericOffset < 0 || numericOffset > size {
    let description: String
    switch offset {
    case .direction(let direction): description = direction.rawValue
    case .offset(let value): description = String(value)
    }
    try devInvariant(
      clamp,
      "$getTextNodeOffset: invalid offset \(description) for size \(size) at key \(origin.getKey())"
    )
    numericOffset = numericOffset < 0 ? 0 : size
  }
  return numericOffset
}

/// `$getChildCaretOrSelf`.
public func getChildCaretOrSelf(_ caret: PointCaret?) -> PointCaret? {
  caret?.getChildCaret() ?? caret
}

/// `$getAdjacentChildCaret`.
public func getAdjacentChildCaret(_ caret: NodeCaret?) throws -> NodeCaret? {
  guard let caret else { return nil }
  return getChildCaretOrSelf(try caret.getAdjacentCaret())
}

/// `TextPointCaretSlice`: `distance` code units of text from a text caret.
public struct TextPointCaretSlice {
  public let caret: TextPointCaret
  public let distance: Int

  public func getSliceIndices() -> (Int, Int) {
    let offset = caret.offset
    let offsetB = offset + distance
    return offsetB < offset ? (offsetB, offset) : (offset, offsetB)
  }

  public func getTextContent() throws -> String {
    let (startIndex, endIndex) = getSliceIndices()
    return try caret.origin.getTextContent().jsSlice(startIndex, endIndex)
  }

  public func getTextContentSize() -> Int { abs(distance) }

  /// Removes the slice's text, returning the caret where it was.
  public func removeTextSlice() throws -> TextPointCaret {
    let (indexStart, indexEnd) = getSliceIndices()
    return try getTextPointCaret(
      setTextContentSpliced(caret.origin as! TextNode, indexStart, indexEnd - indexStart, ""),
      caret.direction, indexStart)
  }
}

/// `$getTextPointCaretSlice`.
public func getTextPointCaretSlice(
  _ caret: TextPointCaret, _ distance: Int
)
  -> TextPointCaretSlice
{
  TextPointCaretSlice(caret: caret, distance: distance)
}

/// `$setTextContentSpliced`: replaces `delCount` code units at `offset`.
@discardableResult
public func setTextContentSpliced<T: TextNode>(
  _ node: T, _ offset: Int, _ delCount: Int, _ text: String
) throws -> T {
  let content = try node.getTextContent()
  return try node.setTextContent(
    content.jsSlice(0, offset) + text + content.jsSlice(offset + delCount))
}

private func getSliceFromTextPointCaret(
  _ caret: TextPointCaret, isFocus: Bool
) throws
  -> TextPointCaretSlice
{
  let offsetB = try getTextNodeOffset(
    caret.origin as! TextNode, .direction(isFocus ? caret.direction.flipped : caret.direction))
  return getTextPointCaretSlice(caret, offsetB - caret.offset)
}

/// `CaretRange`: the carets between `anchor` and `focus`, which face
/// `direction`.
public final class CaretRange {
  public let direction: CaretDirection
  public var anchor: PointCaret
  public var focus: PointCaret

  init(_ anchor: PointCaret, _ focus: PointCaret, _ direction: CaretDirection) {
    self.anchor = anchor
    self.focus = focus
    self.direction = direction
  }

  public func getLatest() throws -> CaretRange {
    let anchor = try self.anchor.getLatest()
    let focus = try self.focus.getLatest()
    return anchor.origin === self.anchor.origin && focus.origin === self.focus.origin
      ? self : CaretRange(anchor, focus, direction)
  }

  public func isCollapsed() -> Bool {
    anchor.isSamePointCaret(focus)
  }

  /// The text slices at either end: one slice when both ends are in one
  /// text node.
  public func getTextSlices() throws -> (TextPointCaretSlice?, TextPointCaretSlice?) {
    let anchor = try self.anchor.getLatest()
    let focus = try self.focus.getLatest()
    if anchor.isText, focus.isText, anchor.isSameNodeCaret(focus) {
      return (getTextPointCaretSlice(anchor, focus.offset - anchor.offset), nil)
    }
    return (
      try anchor.isText ? getSliceFromTextPointCaret(anchor, isFocus: false) : nil,
      try focus.isText ? getSliceFromTextPointCaret(focus, isFocus: true) : nil
    )
  }

  /// The node carets strictly between anchor and focus, in document order
  /// for `direction`.
  public func iterNodeCarets(
    _ rootMode: RootMode = .root
  ) throws -> StepwiseIterator<NodeCaret, NodeCaret> {
    let anchor = try self.anchor.isText ? self.anchor.getSiblingCaret() : self.anchor.getLatest()
    let focus = try self.focus.getLatest()
    let isTextFocus = focus.isText
    let step: (NodeCaret) throws -> NodeCaret? = { state in
      if state.isSameNodeCaret(focus) {
        return nil
      }
      return try getAdjacentChildCaret(state) ?? state.getParentCaret(rootMode)
    }
    return StepwiseIterator(
      initial: anchor.isSameNodeCaret(focus) ? nil : try step(anchor),
      hasNext: { !(isTextFocus && focus.isSameNodeCaret($0)) },
      step: step, map: { $0 })
  }
}

/// `$getCaretRange`.
public func getCaretRange(_ anchor: PointCaret, _ focus: PointCaret) throws -> CaretRange {
  try invariant(
    anchor.direction == focus.direction,
    "$getCaretRange: anchor and focus must be in the same direction")
  return CaretRange(anchor, focus, anchor.direction)
}

/// `$getCollapsedCaretRange`.
public func getCollapsedCaretRange(_ anchor: PointCaret) throws -> CaretRange {
  try getCaretRange(anchor, anchor)
}

/// `$getCaretInDirection`.
public func getCaretInDirection(
  _ caret: PointCaret, _ direction: CaretDirection
) throws
  -> PointCaret
{
  try caret.direction == direction ? caret : caret.getFlipped()
}

/// `$getCaretRangeInDirection`.
public func getCaretRangeInDirection(
  _ range: CaretRange, _ direction: CaretDirection
) throws
  -> CaretRange
{
  if range.direction == direction {
    return range
  }
  return try getCaretRange(
    getCaretInDirection(range.focus, direction), getCaretInDirection(range.anchor, direction))
}

/// `$extendCaretToRange`: from `anchor` to the end of the document in its
/// direction.
public func extendCaretToRange(_ anchor: PointCaret) throws -> CaretRange {
  try getCaretRange(
    anchor,
    getCaretInDirection(getChildCaret(getRoot(), anchor.direction.flipped), anchor.direction))
}

private func compareNumber(_ a: Int, _ b: Int) -> Int { a < b ? -1 : a > b ? 1 : 0 }

private func compareSameOriginCarets(_ a: PointCaret, _ b: PointCaret) -> Int {
  if a.isText && b.isText {
    return compareNumber(a.offset, b.offset)
  }
  if a.kind == b.kind {
    return 0
  }
  return a.isText ? -1 : b.isText ? 1 : a.isChild ? -1 : 1
}

/// `$comparePointCaretNext`: the document order of two `next` carets.
public func comparePointCaretNext(_ a: PointCaret, _ b: PointCaret) throws -> Int {
  guard let compare = try getCommonAncestor(a.origin, b.origin) else {
    throw LexicalError(
      "$comparePointCaretNext: a (key \(a.origin.getKey())) and b (key \(b.origin.getKey())) do not have a common ancestor"
    )
  }
  switch compare {
  case .same: return compareSameOriginCarets(a, b)
  case .ancestor: return a.isChild ? -1 : 1
  case .descendant: return b.isChild ? 1 : -1
  case .branch: return try getCommonAncestorResultBranchOrder(compare)
  }
}

/// `CommonAncestorResult`: how two nodes relate in the tree.
public enum CommonAncestorResult {
  /// The nodes are the same.
  case same(commonAncestor: LexicalNode)
  /// The first node is an ancestor of the second.
  case ancestor(commonAncestor: ElementNode)
  /// The first node is a descendant of the second.
  case descendant(commonAncestor: ElementNode)
  /// `a` and `b` are the children of `commonAncestor` on the paths to the
  /// two nodes.
  case branch(commonAncestor: ElementNode, a: LexicalNode, b: LexicalNode)

  public var commonAncestor: LexicalNode {
    switch self {
    case .same(let node): node
    case .ancestor(let node), .descendant(let node), .branch(let node, _, _): node
    }
  }
}

/// `$getCommonAncestorResultBranchOrder`: -1 when `a` comes before `b`.
public func getCommonAncestorResultBranchOrder(_ compare: CommonAncestorResult) throws -> Int {
  guard case .branch(_, let a, let b) = compare else {
    throw LexicalError("$getCommonAncestorResultBranchOrder: expected a branch result")
  }
  let aKey = a.__key
  let bKey = b.__key
  var na: LexicalNode? = a
  var nb: LexicalNode? = b
  while let currentA = na, let currentB = nb {
    if currentA.__key == bKey {
      return -1
    } else if currentB.__key == aKey {
      return 1
    }
    na = try currentA.getNextSibling()
    nb = try currentB.getNextSibling()
  }
  return na == nil ? 1 : -1
}

private func initialElementTuple(_ node: LexicalNode) throws -> (ElementNode?, LexicalNode?) {
  if let element = node as? ElementNode {
    return (try element.getLatest(), nil)
  }
  return (try node.getParent(), try node.getLatest())
}

/// `$getCommonAncestor`: how `a` and `b` relate, or `nil` when they are in
/// different trees.
public func getCommonAncestor(_ a: LexicalNode, _ b: LexicalNode) throws -> CommonAncestorResult? {
  if a.is(b) {
    return .same(commonAncestor: a)
  }
  // Keyed by node key: every lookup goes through getLatest/getParent, so a
  // key names one node object here, as the JS Map's object keys do.
  var aMap: [NodeKey: LexicalNode?] = [:]
  var (parent, child) = try initialElementTuple(a)
  while let current = parent {
    aMap[current.__key] = .some(child)
    child = current
    parent = try current.getParent()
  }
  (parent, child) = try initialElementTuple(b)
  while let current = parent {
    if let aChild = aMap[current.__key] {
      guard let aChild else {
        try invariant(a.is(current), "$originComparison: ancestor logic error")
        return .ancestor(commonAncestor: current)
      }
      guard let child else {
        try invariant(b.is(current), "$originComparison: descendant logic error")
        return .descendant(commonAncestor: current)
      }
      try invariant(
        (aChild is ElementNode || a.is(aChild)) && (child is ElementNode || b.is(child))
          && current.is(aChild.getParent()) && current.is(child.getParent()),
        "$originComparison: branch logic error")
      return .branch(commonAncestor: current, a: aChild, b: child)
    }
    child = current
    parent = try current.getParent()
  }
  return nil
}

// MARK: LexicalCaretUtils

/// `$caretFromPoint`.
public func caretFromPoint(_ point: Point, _ direction: CaretDirection) throws -> PointCaret {
  let node = try getNodeByKeyOrThrow(point.key)
  if point.type == .text {
    guard let text = node as? TextNode else {
      throw LexicalError(
        "$caretFromPoint: Node with type \(node.getType()) and key \(point.key) that does not inherit from TextNode encountered for text point"
      )
    }
    return try getTextPointCaret(text, direction, point.offset)
  }
  guard let element = node as? ElementNode else {
    throw LexicalError(
      "$caretFromPoint: Node with type \(node.getType()) and key \(point.key) that does not inherit from ElementNode encountered for element point"
    )
  }
  return try getChildCaretAtIndex(element, point.offset, direction)
}

/// `$setPointFromCaret`.
public func setPointFromCaret(_ point: Point, _ caret: PointCaret) throws {
  let origin = caret.origin
  let isNext = caret.direction == .next
  switch caret.kind {
  case .text(let offset):
    try point.set(origin.getKey(), offset, .text)
  case .sibling:
    if let text = origin as? TextNode {
      try point.set(
        origin.getKey(), getTextNodeOffset(text, .direction(caret.direction)), .text)
    } else {
      try point.set(
        origin.getParentOrThrow().getKey(), origin.getIndexWithinParent() + (isNext ? 1 : 0),
        .element)
    }
  case .child:
    let element = origin as! ElementNode
    try point.set(origin.getKey(), isNext ? 0 : element.getChildrenSize(), .element)
  }
}

/// `$caretRangeFromSelection`.
public func caretRangeFromSelection(_ selection: RangeSelection) throws -> CaretRange {
  let anchorCaret = try caretFromPoint(selection.anchor, .next)
  let focusCaret = try caretFromPoint(selection.focus, .next)
  let direction: CaretDirection =
    try comparePointCaretNext(anchorCaret, focusCaret) <= 0 ? .next : .previous
  return try getCaretRange(
    getCaretInDirection(anchorCaret, direction), getCaretInDirection(focusCaret, direction))
}

/// `$rewindSiblingCaret`.
public func rewindSiblingCaret(_ caret: SiblingCaret) throws -> NodeCaret {
  let rewindOrigin = try getSiblingCaret(caret.origin, caret.direction.flipped).getNodeAtCaret()
  if let rewindOrigin {
    return getSiblingCaret(rewindOrigin, caret.direction)
  }
  return try getChildCaret(caret.origin.getParentOrThrow(), caret.direction)
}

private func getDeepestChildOrSelf(_ initialCaret: PointCaret) throws -> PointCaret {
  var caret = initialCaret
  while caret.isChild {
    guard let adjacent = try getAdjacentChildCaret(caret), adjacent.isChild else {
      break
    }
    caret = adjacent
  }
  return caret
}

/// `$normalizeCaret`: the deepest equivalent caret, a text caret when one
/// is adjacent.
public func normalizeCaret(_ initialCaret: PointCaret) throws -> PointCaret {
  let caret = try getDeepestChildOrSelf(initialCaret.getLatest())
  let direction = caret.direction
  if let text = caret.origin as? TextNode {
    return try caret.isText ? caret : getTextPointCaret(text, direction, .direction(direction))
  }
  if let adj = try caret.getAdjacentCaret(), let text = adj.origin as? TextNode {
    return try getTextPointCaret(text, direction, .direction(direction.flipped))
  }
  return caret
}

/// `$isExtendableTextPointCaret`: a text caret with text in its direction.
public func isExtendableTextPointCaret(_ caret: PointCaret) throws -> Bool {
  guard caret.isText, let text = caret.origin as? TextNode else {
    return false
  }
  return try caret.offset != getTextNodeOffset(text, .direction(caret.direction))
}

/// `$getChildCaretAtIndex`.
public func getChildCaretAtIndex(
  _ parent: ElementNode, _ index: Int, _ direction: CaretDirection
) throws -> NodeCaret {
  let size = try parent.getChildrenSize()
  let offset = index > 0 ? min(index, size) : 0
  let originIndex = offset - (direction == .next ? 1 : 0)
  let origin =
    try originIndex < 0 || originIndex >= size ? nil : parent.getChildAtIndex(originIndex)
  if let origin {
    return getSiblingCaret(origin, direction)
  }
  return getChildCaret(parent, direction)
}

/// `$isBlockFullySelected`: whether the range covers all of `blockNode`.
public func isBlockFullySelected(
  _ blockNode: ElementNode, _ selection: RangeSelection
) throws
  -> Bool
{
  try isBlockFullySelected(blockNode, caretRangeFromSelection(selection))
}

/// `$isBlockFullySelected` for a caret range.
public func isBlockFullySelected(
  _ blockNode: ElementNode, _ caretRange: CaretRange
) throws
  -> Bool
{
  let range = try getCaretRangeInDirection(caretRange, .next)
  // A range inside a slot frame never covers a block outside it.
  let anchorFrame = try getSlotFrame(range.anchor.origin)
  let blockFrame = try getSlotFrame(blockNode.getLatest())
  if anchorFrame == nil ? blockFrame != nil : !anchorFrame!.is(blockFrame) {
    return false
  }
  let blockStart = try normalizeCaret(getChildCaret(blockNode, .next))
  let blockEnd = try getCaretInDirection(normalizeCaret(getChildCaret(blockNode, .previous)), .next)
  return try comparePointCaretNext(range.anchor, blockStart) <= 0
    && comparePointCaretNext(range.focus, blockEnd) >= 0
}
