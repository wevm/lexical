// Ported from packages/lexical/src/caret/LexicalCaretUtils.ts: the caret
// functions that edit the tree. The read-only ones live in LexicalCaret.swift.

/// `$recordDeleted`: records a deleted node while the editor records text
/// edits. The port does not record text edits yet, so this is a no-op.
package func recordDeleted(_ node: LexicalNode) {}

/// `$getAdjacentNodes`: the nodes after the caret in its direction.
public func getAdjacentNodes(_ initialCaret: NodeCaret) throws -> [LexicalNode] {
  try collectSiblingNodes(initialCaret.getNodeAtCaret(), initialCaret.direction)
}

private func getAnchorCandidates(
  _ anchor: PointCaret, _ rootMode: RootMode = .root
) throws
  -> [PointCaret]
{
  var carets: [PointCaret] = [anchor]
  var parent = try anchor.isChild ? anchor.getParentCaret(rootMode) : anchor.getSiblingCaret()
  while let current = parent {
    carets.append(try rewindSiblingCaret(current))
    parent = try current.getParentCaret(rootMode)
  }
  return carets
}

private func isCaretAttached(_ caret: PointCaret?) throws -> Bool {
  guard let caret else { return false }
  return try caret.origin.isAttached()
}

/// How `removeTextFromCaretRange` treats a text slice that covers its whole
/// node.
public enum RemoveTextSliceMode: Sendable {
  /// Removes the node.
  case removeEmptySlices
  /// Keeps the first such node, empty, as the caret's place.
  case preserveEmptyTextSliceCaret
}

/// `$removeTextFromCaretRange`: removes the range's content and returns
/// the collapsed range where it was.
public func removeTextFromCaretRange(
  _ initialRange: CaretRange, _ sliceMode: RemoveTextSliceMode = .removeEmptySlices
) throws -> CaretRange {
  if initialRange.isCollapsed() {
    return initialRange
  }
  let rootMode = RootMode.root
  let nextDirection = CaretDirection.next
  var sliceState = sliceMode
  let range = try getCaretRangeInDirection(initialRange, nextDirection)
  var rangeContainer: LexicalNode? = range.anchor.origin
  while let container = rangeContainer, try !isRootOrShadowRoot(container) {
    rangeContainer = try container.getParent()
  }
  let rangeContainerChild = try (rangeContainer as? ElementNode)?.getFirstChild()
  var anchorCandidates = try getAnchorCandidates(range.anchor, rootMode)
  var focusCandidates = try getAnchorCandidates(range.focus.getFlipped(), rootMode)
  var seenStart = Set<NodeKey>()
  var removedNodes: [LexicalNode] = []
  for caret in try range.iterNodeCarets(rootMode) {
    if caret.isChild {
      seenStart.insert(caret.origin.getKey())
    } else if caret.isSibling {
      let origin = caret.origin
      if !(origin is ElementNode) || seenStart.contains(origin.getKey()) {
        removedNodes.append(origin)
      }
    }
  }
  // An insertion-ordered set, as JS iterates its Set.
  let removedParents = JSMap<NodeKey, ElementNode>()
  for node in removedNodes {
    if let parent = try node.getParent(), !seenStart.contains(parent.getKey()) {
      removedParents.set(parent.getKey(), parent)
    }
    recordDeleted(node)
    try removeFromParent(node)
  }
  for parent in removedParents.valuesArray {
    if try !parent.canBeEmpty() && !isRootOrShadowRoot(parent) && parent.isEmpty()
      && parent.isAttached()
    {
      try parent.remove()
    }
  }
  let (anchorSlice, focusSlice) = try range.getTextSlices()
  for slice in [anchorSlice, focusSlice] {
    guard let slice else { continue }
    let origin = slice.caret.origin as! TextNode
    let contentSize = try origin.getTextContentSize()
    let caretBefore = try rewindSiblingCaret(getSiblingCaret(origin, nextDirection))
    let mode = try origin.getMode()
    if (abs(slice.distance) == contentSize && sliceState == .removeEmptySlices)
      || (mode == "token" && slice.distance != 0)
    {
      recordDeleted(origin)
      try caretBefore.remove()
    } else if slice.distance != 0 {
      sliceState = .removeEmptySlices
      var nextCaret = try slice.removeTextSlice()
      let sliceOrigin = slice.caret.origin
      if mode == "segmented" {
        let src = nextCaret.origin as! TextNode
        let plainTextNode = try createTextNode(src.getTextContent())
          .setStyle(src.getStyle())
          .setFormat(src.getFormat())
          .setDetail(src.getDetail())
        try carryNodeState(src, plainTextNode)
        try caretBefore.replaceOrInsert(plainTextNode)
        nextCaret = try getTextPointCaret(plainTextNode, nextDirection, nextCaret.offset)
      }
      if sliceOrigin.is(anchorCandidates[0].origin) {
        anchorCandidates[0] = nextCaret
      }
      if sliceOrigin.is(focusCandidates[0].origin) {
        focusCandidates[0] = try nextCaret.getFlipped()
      }
    }
  }
  let anchorCandidate = try getAttachedCaret(anchorCandidates)
  let focusCandidate = try getAttachedCaret(focusCandidates)
  if let (anchorBlock, focusBlock) = try getBlockMergeTargets(
    anchorCandidate, focusCandidate, seenStart)
  {
    try getChildCaret(anchorBlock, .previous).splice(0, focusBlock.getChildren())
    var parent = try focusBlock.getParent()
    try focusBlock.remove(preserveEmptyParent: true)
    while let element = parent, try element.isEmpty() {
      parent = try element.getParent()
      try element.remove(preserveEmptyParent: true)
    }
  } else if let focusCandidate {
    let focusBlock = try getBlockFromCaret(focusCandidate)
    let focusBlockParent = try focusBlock?.getParent()
    let topmostShadowRoot = try focusBlock?.getParents().last(where: { isShadowRootNode($0) })
    if let focusBlock, let focusBlockParent, !(focusBlockParent is RootNode),
      try focusBlock.isEmpty(), seenStart.contains(focusBlock.getKey()),
      try getSlotNames(focusBlock).isEmpty,
      topmostShadowRoot == nil || seenStart.contains(topmostShadowRoot!.getKey())
    {
      try focusBlock.remove(preserveEmptyParent: true)
      var parent: ElementNode? = focusBlockParent
      while let element = parent, !(element is RootNode), try element.isEmpty() {
        let grandparent = try element.getParent()
        if let grandparent, grandparent is RootNode, try grandparent.getChildrenSize() <= 1,
          try element.canBeEmpty()
        {
          break
        }
        parent = grandparent
        try element.remove(preserveEmptyParent: true)
      }
    }
  }
  if try restoreEmptyContainerParagraph(rangeContainer, rangeContainerChild) == nil,
    let rangeContainer, try !rangeContainer.isAttached()
  {
    try restoreEmptyContainerParagraph(getRoot(), nil)
  }
  if let bestCandidate = try getAttachedCaret(
    [anchorCandidate, focusCandidate] + anchorCandidates + focusCandidates)
  {
    let anchor = try getCaretInDirection(bestCandidate, initialRange.direction)
    return try getCollapsedCaretRange(anchor)
  }
  throw LexicalError(
    "$removeTextFromCaretRange: selection was lost, could not find a new anchor given candidates with keys: [\(anchorCandidates.map { "\"\($0.origin.__key)\"" }.joined(separator: ","))]"
  )
}

private func getAttachedCaret(_ candidates: [PointCaret?]) throws -> PointCaret? {
  for candidate in candidates where try isCaretAttached(candidate) {
    return try normalizeCaret(candidate!)
  }
  return nil
}

private func getBlockFromCaret(_ caret: PointCaret) throws -> ElementNode? {
  if caret.isChild {
    if let origin = caret.origin as? ElementNode, try INTERNAL_isBlock(origin) {
      return origin
    }
  } else if let parent = try caret.getParentAtCaret(), try INTERNAL_isBlock(parent) {
    return parent
  }
  return nil
}

private func getBlockMergeTargets(
  _ anchor: PointCaret?, _ focus: PointCaret?, _ seenStart: Set<NodeKey>
) throws -> (ElementNode, ElementNode)? {
  guard let anchorParent = try anchor?.getParentAtCaret(),
    let focusParent = try focus?.getParentAtCaret(),
    let common = try getCommonAncestor(anchorParent, focusParent),
    case .branch = common
  else {
    return nil
  }
  func getBlock(_ parent: ElementNode, _ selectedOnly: Bool) throws -> ElementNode? {
    var block: ElementNode?
    var node: ElementNode? = parent
    while let current = node, !current.is(common.commonAncestor) {
      if try isRootOrShadowRoot(current) {
        return nil
      }
      if try (!selectedOnly || seenStart.contains(current.__key)) && INTERNAL_isBlock(current) {
        block = current
      }
      node = try current.getParent()
    }
    return block
  }
  guard let anchorBlock = try getBlock(anchorParent, false),
    let focusBlock = try getBlock(focusParent, true),
    try getSlotNames(focusBlock).isEmpty
  else {
    return nil
  }
  return (anchorBlock, focusBlock)
}

/// `$getAdjacentSiblingOrParentSiblingCaret`: the next caret in document
/// order and the change in depth to reach it.
public func getAdjacentSiblingOrParentSiblingCaret(
  _ startCaret: NodeCaret, _ rootMode: RootMode = .root
) throws -> (NodeCaret, Int)? {
  var depthDiff = 0
  var caret = startCaret
  var nextCaret = try getAdjacentChildCaret(caret)
  while nextCaret == nil {
    depthDiff -= 1
    guard let parentCaret = try caret.getParentCaret(rootMode) else {
      return nil
    }
    caret = parentCaret
    nextCaret = try getAdjacentChildCaret(caret)
  }
  return nextCaret.map { ($0, depthDiff) }
}

/// `$splitTextPointCaret`: splits the text at the caret, unless it is at
/// an end, and returns the sibling caret between the halves.
public func splitTextPointCaret(_ textPointCaret: TextPointCaret) throws -> NodeCaret {
  let origin = textPointCaret.origin as! TextNode
  let offset = textPointCaret.offset
  let direction = textPointCaret.direction
  if try offset == getTextNodeOffset(origin, .direction(direction)) {
    return textPointCaret.getSiblingCaret()
  } else if try offset == getTextNodeOffset(origin, .direction(direction.flipped)) {
    return try rewindSiblingCaret(textPointCaret.getSiblingCaret())
  }
  guard let textNode = try origin.splitText(offset).first else {
    throw LexicalError("$splitTextPointCaret: splitText must return at least one TextNode")
  }
  return try getCaretInDirection(getSiblingCaret(textNode, .next), direction)
}

/// `$getTextPointCaretSliceForNode`.
public func getTextPointCaretSliceForNode(
  _ slices: [TextPointCaretSlice?], _ node: LexicalNode
) -> TextPointCaretSlice? {
  for slice in slices {
    if let slice, slice.caret.origin.is(node) {
      return slice
    }
  }
  return nil
}

/// `$splitTextPointCaretSlice`: splits the slice's text into its own node,
/// moving `selection`'s points that were in it.
@discardableResult
public func splitTextPointCaretSlice(
  _ slice: TextPointCaretSlice, _ selection: RangeSelection? = nil
) throws -> TextNode? {
  let origin = slice.caret.origin as! TextNode
  let (start, end) = slice.getSliceIndices()
  if start == end {
    return nil
  }
  if try start == 0 && end == origin.getTextContentSize() {
    return origin
  }
  let parent = try origin.getParent()
  var index = -1
  if let selection, let parent, try selectionTouchesElement(selection, parent) {
    index = try origin.getIndexWithinParent()
  }
  let points: [(Point, Int?)] =
    selection.map { selection in
      [selection.anchor, selection.focus].map { point in
        let offset: Int? =
          point.key == origin.__key && point.type == .text
          ? point.offset - start
          : parent != nil && point.key == parent!.__key && point.offset == index ? 0 : nil
        return (point, offset)
      }
    } ?? []
  let splitNodes = try origin.splitText(start, end)
  let node = splitNodes[start == 0 ? 0 : 1]
  if let parent, let selection, selection !== (try getSelection() as? RangeSelection) {
    try updateElementSelectionOnCreateDeleteNode(
      selection, parent, index, splitNodes.count - 1)
  }
  for (point, offset) in points {
    if let offset {
      try point.set(node.__key, offset, .text)
    }
  }
  return node
}

/// `SplitAtPointCaretNextOptions`.
public struct SplitAtPointCaretNextOptions {
  /// Makes the empty element that takes the split-off siblings.
  public var copyElementNode: (ElementNode) throws -> ElementNode
  /// Splits a text caret.
  public var splitTextPointCaretNext: (TextPointCaret) throws -> NodeCaret
  /// Where splitting stops.
  public var rootMode: RootMode
  /// Whether to split `node` at its `first` or `last` edge.
  public var shouldSplit: (ElementNode, SplitEdge) throws -> Bool
  /// Removes an empty element instead of leaving it behind.
  public var removeEmptyDestination: Bool

  /// The `edge` argument of `shouldSplit`.
  public enum SplitEdge: Sendable {
    case first
    case last
  }

  public init(
    copyElementNode: @escaping (ElementNode) throws -> ElementNode = { try copyNode($0) },
    splitTextPointCaretNext: @escaping (TextPointCaret) throws -> NodeCaret =
      splitTextPointCaret,
    rootMode: RootMode = .shadowRoot,
    shouldSplit: @escaping (ElementNode, SplitEdge) throws -> Bool = { _, _ in true },
    removeEmptyDestination: Bool = false
  ) {
    self.copyElementNode = copyElementNode
    self.splitTextPointCaretNext = splitTextPointCaretNext
    self.rootMode = rootMode
    self.shouldSplit = shouldSplit
    self.removeEmptyDestination = removeEmptyDestination
  }
}

/// `$splitAtPointCaretNext`: splits the parent at the caret, returning the
/// caret after the split-off part, or `nil` at the root.
public func splitAtPointCaretNext(
  _ pointCaret: PointCaret, _ options: SplitAtPointCaretNextOptions = .init()
) throws -> NodeCaret? {
  if pointCaret.isText {
    return try options.splitTextPointCaretNext(pointCaret)
  }
  let parentCaret = try pointCaret.getParentCaret(options.rootMode)
  if let parentCaret {
    let origin = parentCaret.origin as! ElementNode
    if pointCaret.isChild {
      let beforeParentCaret = try rewindSiblingCaret(parentCaret)
      if try options.removeEmptyDestination && origin.isEmpty() {
        try origin.remove()
        return beforeParentCaret
      }
      if try !(origin.canBeEmpty() && options.shouldSplit(origin, .first)) {
        return beforeParentCaret
      }
    }
    let siblings = try getAdjacentNodes(pointCaret)
    if try !siblings.isEmpty
      || (!options.removeEmptyDestination && origin.canBeEmpty()
        && options.shouldSplit(origin, .last))
    {
      try parentCaret.insert(options.copyElementNode(origin).splice(0, 0, siblings))
    }
  }
  return parentCaret
}

/// `$insertNodeToNearestRootAtCaret`: splits up to the nearest root (or
/// block container) and inserts `node` there, wrapped in a paragraph when
/// it is inline.
@discardableResult
public func insertNodeToNearestRootAtCaret(
  _ node: LexicalNode, _ caret: PointCaret, _ options: SplitAtPointCaretNextOptions? = nil
) throws -> NodeCaret {
  var insertCaret = try getCaretInDirection(caret, .next)
  if insertCaret.isText {
    if insertCaret.offset == 0 {
      insertCaret = try getSiblingCaret(insertCaret.origin, .previous).getFlipped()
    } else if try insertCaret.offset == insertCaret.origin.getTextContentSize() {
      insertCaret = getSiblingCaret(insertCaret.origin, .next)
    }
  }
  if insertCaret.origin.is(node) {
    try invariant(
      insertCaret.isSibling,
      "$insertNodeToNearestRootAtCaret node \(node.getKey()) of type \(node.getType()) can not be inserted into itself"
    )
    insertCaret = try rewindSiblingCaret(insertCaret)
  }
  if try node.is(insertCaret.getNodeAtCaret()) || node.is(insertCaret.getFlipped().getNodeAtCaret())
  {
    try node.remove(preserveEmptyParent: true)
  }
  let stopsAtContainers = options == nil || options!.rootMode == .shadowRoot
  var nextCaret: PointCaret? = insertCaret
  while let current = nextCaret {
    insertCaret = current
    if stopsAtContainers, !insertCaret.isText,
      try isBlockContainer(
        insertCaret.isChild ? insertCaret.origin : insertCaret.origin.getParent())
    {
      break
    }
    nextCaret = try splitAtPointCaretNext(current, options ?? .init())
  }
  try invariant(
    !insertCaret.isText,
    "$insertNodeToNearestRootAtCaret: An unattached TextNode can not be split")
  try insertCaret.insert(node.isInline() ? createParagraphNode().append(node) : node)
  return try getCaretInDirection(getSiblingCaret(node.getLatest(), .next), caret.direction)
}
