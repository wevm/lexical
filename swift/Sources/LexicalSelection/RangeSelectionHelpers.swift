import Lexical

// Ported from packages/lexical-selection/src/range-selection.ts.

/// `$copyBlockFormatIndent`.
public func copyBlockFormatIndent(_ srcNode: ElementNode, _ destNode: ElementNode) throws {
  let format = try srcNode.getFormatType()
  let indent = try srcNode.getIndent()
  if try format != destNode.getFormatType() {
    try destNode.setFormat(format)
  }
  if try indent != destNode.getIndent() {
    try destNode.setIndent(indent)
  }
}

/// `$isAtEdgeOfElement`: whether nothing lies between `point` and the edge
/// of `element` in `direction`.
public func isAtEdgeOfElement(
  _ point: Point, _ element: ElementNode, _ direction: CaretDirection
) throws -> Bool {
  var caret: PointCaret? = try caretFromPoint(point, direction)
  if let c = caret, try isExtendableTextPointCaret(c) {
    return false
  }
  while let c = caret {
    guard let parent = try c.getParentAtCaret(), try c.getNodeAtCaret() == nil else {
      return false
    }
    if element.is(parent) {
      return true
    }
    caret = try c.getParentCaret()
  }
  return false
}

private func isPointAtBlockEdge(
  _ point: Point, _ block: ElementNode, _ direction: CaretDirection
) throws -> Bool {
  if let node = try point.getNode() as? ElementNode, try node.isEmpty() {
    return false
  }
  return try isAtEdgeOfElement(point, block, direction)
}

/// `$setBlocksType`: replaces each block in the selection with one made by
/// `createElement`.
public func setBlocksType<T: ElementNode>(
  _ selection: (any BaseSelection)?, _ createElement: () throws -> T,
  _ afterCreateElement: (ElementNode, T) throws -> Void = { try copyBlockFormatIndent($0, $1) }
) throws {
  guard let selection else { return }
  let anchorAndFocus = selection.getStartEndPoints()
  var skipFocus = false
  var focusBlock: LexicalNode?
  var blockMap: [(NodeKey, ElementNode)] = []
  func add(_ node: ElementNode) {
    if !blockMap.contains(where: { $0.0 == node.getKey() }) {
      blockMap.append((node.getKey(), node))
    }
  }
  if let (anchor, focus) = anchorAndFocus {
    let anchorBlock = try findMatchingParent(anchor.getNode(), INTERNAL_isBlock)
    focusBlock = try findMatchingParent(focus.getNode(), INTERNAL_isBlock)
    let direction: CaretDirection = try selection.isBackward() ? .previous : .next
    if let fb = focusBlock as? ElementNode, !fb.is(anchorBlock) {
      skipFocus = try isPointAtBlockEdge(focus, fb, flipDirection(direction))
    }
    if let ab = anchorBlock as? ElementNode { add(ab) }
    if let fb = focusBlock as? ElementNode, !skipFocus { add(fb) }
  }
  for node in try selection.getNodes() {
    if let element = node as? ElementNode, try INTERNAL_isBlock(element) {
      if skipFocus && element.is(focusBlock) { continue }
      add(element)
    } else if anchorAndFocus == nil,
      let ancestor = try findMatchingParent(node, INTERNAL_isBlock) as? ElementNode
    {
      add(ancestor)
    }
  }
  for (_, prevNode) in blockMap {
    if try getSlotHost(prevNode) != nil { continue }
    if try isBlockContainer(prevNode) {
      var points: [Point] = []
      var seen: [ObjectIdentifier] = []
      for s in [selection, try getSelection()].compactMap({ $0 }) {
        guard let range = s as? RangeSelection, !seen.contains(ObjectIdentifier(range)) else {
          continue
        }
        seen.append(ObjectIdentifier(range))
        for point in [range.anchor, range.focus] where point.key == prevNode.getKey() {
          points.append(point)
        }
      }
      if !points.isEmpty, try prevNode.isEmpty() {
        let element = try createElement()
        try prevNode.append(element)
        for point in points {
          try point.set(element.getKey(), 0, .element)
        }
      }
      continue
    }
    let element = try createElement()
    try afterCreateElement(prevNode, element)
    try prevNode.replace(element, includeChildren: true)
  }
}

private func isPointAttached(_ point: Point) throws -> Bool {
  try point.getNode().isAttached()
}

private func removeParentEmptyElements(_ startingNode: ElementNode) throws {
  var node: ElementNode? = startingNode
  while let current = node, try !isBlockContainer(current) {
    let latest = try current.getLatest()
    let parentNode = try current.getParent()
    if try latest.getChildrenSize() == 0 {
      try current.remove(preserveEmptyParent: true)
    }
    node = parentNode
  }
}

private func wrappedBlock(
  _ node: LexicalNode, _ cache: inout [NodeKey: ElementNode?]
) throws -> ElementNode? {
  var walked: [NodeKey] = []
  var block: LexicalNode? = node
  var found: ElementNode??
  while found == nil {
    guard let current = block else {
      found = .some(nil)
      break
    }
    if let cached = cache[current.getKey()] {
      found = cached
    } else if try isBlockContainer(current) {
      found = current as? ElementNode
    } else if let element = current as? ElementNode, !INTERNAL_isInlineContent(element) {
      found = element
    } else {
      walked.append(current.getKey())
      block = try current.getParent()
    }
  }
  let result = found ?? nil
  for key in walked { cache[key] = .some(result) }
  return result
}

private func isBlockLeafInElement(
  _ node: LexicalNode, _ cache: inout [NodeKey: Bool]
) throws -> Bool {
  guard let parent = try node.getParent(), isLeafNode(node), !INTERNAL_isInlineContent(node),
    try !isBlockContainer(parent)
  else {
    return false
  }
  if let blocksOnly = cache[parent.getKey()] { return blocksOnly }
  let blocksOnly = try parent.getChildren().allSatisfy { !INTERNAL_isInlineContent($0) }
  cache[parent.getKey()] = blocksOnly
  return blocksOnly
}

/// An empty element that is not inline content.
private func isEmptyBlock(_ node: LexicalNode) throws -> Bool {
  guard let element = node as? ElementNode else { return false }
  return try element.getChildrenSize() == 0 && !INTERNAL_isInlineContent(element)
}

private func childIn(
  _ container: ElementNode, _ node: LexicalNode, _ cache: inout [NodeKey: LexicalNode]
) throws -> LexicalNode {
  var walked: [NodeKey] = []
  var child = node
  var found = cache[child.getKey()]
  while found == nil {
    let parent = try child.getParent()
    if parent == nil || parent!.is(container) {
      found = child
      break
    }
    walked.append(child.getKey())
    child = parent!
    found = cache[child.getKey()]
  }
  for key in walked { cache[key] = found }
  cache[node.getKey()] = found
  return found!
}

private func inlineRun(_ child: LexicalNode) throws -> [LexicalNode] {
  var first = child
  while let prev = try first.getPreviousSibling(), INTERNAL_isInlineContent(prev) {
    first = prev
  }
  var run: [LexicalNode] = []
  var it: LexicalNode? = first
  while let node = it, INTERNAL_isInlineContent(node) {
    run.append(node)
    it = try node.getNextSibling()
  }
  return run
}

/// `$wrapNodes`: wraps the selected blocks' content in elements made by
/// `createElement`, all inside `wrappingElement` when given.
public func wrapNodes(
  _ selection: any BaseSelection, _ createElement: () throws -> ElementNode,
  _ wrappingElement: ElementNode? = nil
) throws {
  let anchor = selection.getStartEndPoints()?.0
  let nodes = try selection.getNodes()
  let nodesLength = nodes.count
  if let anchor, let slotFrame = try getSlotFrame(anchor.getNode()),
    try !isBlockContainer(slotFrame)
  {
    return
  }
  let anchorIsEmptyElement =
    try anchor?.type == .element && (anchor?.getNode() as? ElementNode)?.getChildrenSize() == 0
  if let anchor, nodesLength == 0 || (nodesLength == 1 && anchorIsEmptyElement) {
    var cache: [NodeKey: ElementNode?] = [:]
    guard let target = try wrappedBlock(anchor.getNode(), &cache) else { return }
    let children = try target.getChildren()
    var element = try createElement()
    let intoContainer = try isBlockContainer(target)
    if !intoContainer {
      try element.setFormat(target.getFormatType())
      try element.setIndent(target.getIndent())
    }
    for child in children { try element.append(child) }
    if let wrappingElement {
      element = try wrappingElement.append(element)
    }
    if intoContainer {
      try target.append(element)
      try element.selectStart()
    } else {
      try target.replace(element)
    }
    return
  }
  var topLevelNode: LexicalNode?
  var descendants: [LexicalNode] = []
  for node in nodes {
    if try isBlockContainer(node) {
      try wrapNodesImpl(selection, descendants, descendants.count, createElement, wrappingElement)
      descendants = []
      topLevelNode = node
    } else if try topLevelNode == nil || hasAncestor(node, topLevelNode!) {
      descendants.append(node)
    } else {
      try wrapNodesImpl(selection, descendants, descendants.count, createElement, wrappingElement)
      descendants = [node]
    }
  }
  try wrapNodesImpl(selection, descendants, descendants.count, createElement, wrappingElement)
}

private final class WrapUnit {
  let block: ElementNode
  let key: NodeKey
  let run: [LexicalNode]?
  init(_ block: ElementNode, _ key: NodeKey, _ run: [LexicalNode]?) {
    self.block = block
    self.key = key
    self.run = run
  }
}

/// `$wrapNodesImpl`.
public func wrapNodesImpl(
  _ selection: any BaseSelection, _ nodes: [LexicalNode], _ nodesLength: Int,
  _ createElement: () throws -> ElementNode, _ wrappingElement: ElementNode? = nil
) throws {
  if nodes.isEmpty { return }
  var firstNode = nodes[0]
  for (i, node) in nodes.enumerated() where i < nodesLength {
    if try (isLeafNode(node) && node.getParent() != nil) || isEmptyBlock(node) {
      firstNode = node
      break
    }
  }
  var elementMapping: [NodeKey: ElementNode] = [:]
  var elements: [ElementNode] = []
  var blockOf: [NodeKey: ElementNode?] = [:]
  var childOf: [NodeKey: LexicalNode] = [:]
  var blockLeafParents: [NodeKey: Bool] = [:]
  guard let firstNodeBlock = try wrappedBlock(firstNode, &blockOf) else { return }
  var target: LexicalNode = firstNodeBlock
  if try isBlockContainer(firstNodeBlock) {
    let child = try childIn(firstNodeBlock, firstNode, &childOf)
    target = INTERNAL_isInlineContent(child) ? try inlineRun(child)[0] : child
  } else if try isBlockLeafInElement(firstNode, &blockLeafParents) {
    target = firstNode
  }
  var targetIsPrevSibling = false
  while true {
    if let prevSibling = try target.getPreviousSibling() {
      target = prevSibling
      targetIsPrevSibling = true
      break
    }
    target = try target.getParentOrThrow()
    if try isBlockContainer(target) { break }
  }
  var emptyElements = Set<NodeKey>()
  for node in nodes.prefix(nodesLength) where try isEmptyBlock(node) {
    emptyElements.insert(node.getKey())
  }
  var movedNodes = Set<NodeKey>()
  var removals: [() throws -> Void] = []
  var units: [WrapUnit?] = []
  var runOf: [NodeKey: WrapUnit] = [:]
  for node in nodes.prefix(nodesLength) {
    let nodeParent = try node.getParent()
    if let nodeParent, try isBlockLeafInElement(node, &blockLeafParents) {
      units.append(WrapUnit(nodeParent, node.getKey(), [node]))
      continue
    }
    let block = try nodeParent.flatMap { try wrappedBlock($0, &blockOf) }
    guard let block, try isBlockContainer(block) else {
      units.append(block.map { WrapUnit($0, $0.getKey(), nil) })
      continue
    }
    let child = try childIn(block, node, &childOf)
    let inline = INTERNAL_isInlineContent(child)
    if !inline && !isLeafNode(child) {
      units.append(nil)
      continue
    }
    var unit = runOf[child.getKey()]
    if unit == nil {
      let run = inline ? try inlineRun(child) : [child]
      let newUnit = WrapUnit(block, run[0].getKey(), run)
      for n in run { runOf[n.getKey()] = newUnit }
      unit = newUnit
    }
    units.append(unit)
  }
  for i in 0..<nodesLength {
    let node = nodes[i]
    let unit = units[i]
    if let unit, isLeafNode(node), !movedNodes.contains(node.getKey()) {
      let parent = unit.block
      let parentKey = unit.key
      if elementMapping[parentKey] == nil {
        let targetElement = try createElement()
        if unit.run == nil {
          try targetElement.setFormat(parent.getFormatType())
          try targetElement.setIndent(parent.getIndent())
        }
        elements.append(targetElement)
        elementMapping[parentKey] = targetElement
        let children = try unit.run ?? parent.getChildren()
        try targetElement.splice(targetElement.getChildrenSize(), 0, children)
        for child in children {
          movedNodes.insert(child.getKey())
          if let element = child as? ElementNode {
            for key in try element.getChildrenKeys() { movedNodes.insert(key) }
          }
        }
        removals.append { try removeParentEmptyElements(parent) }
      }
    } else if emptyElements.contains(node.getKey()) {
      guard let element = node as? ElementNode else {
        throw LexicalError("Expected node in emptyElements to be an ElementNode")
      }
      let targetElement = try createElement()
      try targetElement.setFormat(element.getFormatType())
      try targetElement.setIndent(element.getIndent())
      elements.append(targetElement)
      removals.append { try element.remove(preserveEmptyParent: true) }
    }
  }
  if let wrappingElement {
    for element in elements { try wrappingElement.append(element) }
  }
  var lastElement: ElementNode?
  if let rootTarget = target as? ElementNode, try isBlockContainer(rootTarget) {
    if targetIsPrevSibling {
      if let wrappingElement {
        try rootTarget.insertAfter(wrappingElement)
      } else {
        for element in elements.reversed() { try rootTarget.insertAfter(element) }
      }
    } else if let firstChild = try rootTarget.getFirstChild() {
      if let wrappingElement {
        try firstChild.insertBefore(wrappingElement)
      } else {
        for element in elements {
          try firstChild.insertBefore(element)
          lastElement = element
        }
      }
    } else if let wrappingElement {
      try rootTarget.append(wrappingElement)
    } else {
      for element in elements {
        try rootTarget.append(element)
        lastElement = element
      }
    }
  } else if let wrappingElement {
    try target.insertAfter(wrappingElement)
  } else {
    for element in elements.reversed() {
      try target.insertAfter(element)
      lastElement = element
    }
  }
  for removal in removals { try removal() }
  let prevSelection = try getPreviousSelection()
  if let prev = prevSelection as? RangeSelection, try isPointAttached(prev.anchor),
    try isPointAttached(prev.focus)
  {
    try setSelection(prev.clone())
  } else if let lastElement {
    try lastElement.selectEnd()
  } else {
    selection.dirty = true
  }
}

private func getComputedStyle(_ selection: RangeSelection) throws -> ComputedStyle? {
  let anchorNode = try selection.anchor.getNode()
  if let element = anchorNode as? ElementNode {
    return try getComputedStyleForElement(element)
  }
  return try getComputedStyleForParent(anchorNode)
}

private func isEditorVerticalOrientation(_ selection: RangeSelection) throws -> Bool {
  try getComputedStyle(selection)?.writingMode == "vertical-rl"
}

/// `$isParentElementRTL`.
public func isParentElementRTL(_ selection: RangeSelection) throws -> Bool {
  try getComputedStyle(selection)?.direction == "rtl"
}

/// `$shouldOverrideDefaultCharacterSelection`: whether moving by a
/// character must be done by Lexical rather than the platform: next to a
/// decorator or block, or at an unmergeable text boundary.
public func shouldOverrideDefaultCharacterSelection(
  _ selection: RangeSelection, _ isBackward: Bool
) throws -> Bool {
  var adjustedIsBackward = try isEditorVerticalOrientation(selection) ? !isBackward : isBackward
  if try isParentElementRTL(selection) {
    adjustedIsBackward = !adjustedIsBackward
  }
  let focusCaret = try caretFromPoint(selection.focus, adjustedIsBackward ? .previous : .next)
  if try isExtendableTextPointCaret(focusCaret) {
    return false
  }
  if focusCaret.isText, let origin = focusCaret.origin as? TextNode, !isTabNode(origin),
    try origin.isUnmergeable()
  {
    let sibling = try focusCaret.getNodeAtCaret()
    if sibling is TextNode && !isTabNode(sibling) {
      return true
    }
  }
  let carets = try extendCaretToRange(focusCaret).iterNodeCarets()
  while let nextCaret = try carets.next() {
    if nextCaret.isChild {
      return try !nextCaret.origin.isInline()
    } else if nextCaret.origin is ElementNode {
      continue
    } else if nextCaret.origin is DecoratorNode {
      return true
    }
    break
  }
  return false
}

/// `$moveCaretSelection`.
public func moveCaretSelection(
  _ selection: RangeSelection, _ isHoldingShift: Bool, _ isBackward: Bool,
  _ granularity: SelectionGranularity
) throws {
  try selection.modify(isHoldingShift ? .extend : .move, isBackward, granularity)
}

/// `$moveCharacter`: moves by a character in the visual direction.
public func moveCharacter(
  _ selection: RangeSelection, _ isHoldingShift: Bool, _ isBackward: Bool
) throws {
  let isRTL = try isParentElementRTL(selection)
  let isVertical = try isEditorVerticalOrientation(selection)
  let adjustedIsBackward = isVertical || isRTL ? !isBackward : isBackward
  try moveCaretSelection(selection, isHoldingShift, adjustedIsBackward, .character)
}

private func getNodeStyleValueForProperty(
  _ node: TextNode, _ styleProperty: String, _ defaultValue: String
) throws -> String {
  let value = getStyleObjectFromCSS(try node.getStyle())[styleProperty] ?? ""
  return value.isEmpty ? defaultValue : value
}

/// `$getSelectionStyleValueForProperty`: the property's value in the
/// selected text, `""` when the text disagrees.
public func getSelectionStyleValueForProperty(
  _ selection: any BaseSelection, _ styleProperty: String, _ defaultValue: String = ""
) throws -> String {
  var styleValue: String?
  let nodes = try selection.getNodes()
  var startNode: LexicalNode?
  var endNode: LexicalNode?
  if let range = selection as? RangeSelection {
    if range.isCollapsed() && !range.style.isEmpty,
      let value = getStyleObjectFromCSS(range.style)[styleProperty]
    {
      return value
    }
    let isBackward = try range.isBackward()
    let firstNode = try isBackward ? range.focus.getNode() : range.anchor.getNode()
    let lastNode = try isBackward ? range.anchor.getNode() : range.focus.getNode()
    let startOffset = isBackward ? range.focus.offset : range.anchor.offset
    let endOffset = isBackward ? range.anchor.offset : range.focus.offset
    if let text = firstNode as? TextNode, try startOffset == text.getTextContentSize() {
      startNode = text
    }
    if endOffset == 0 {
      endNode = lastNode
    }
  }
  for (i, node) in nodes.enumerated() {
    guard let text = node as? TextNode, !text.is(i == 0 ? startNode : endNode) else { continue }
    let nodeStyleValue = try getNodeStyleValueForProperty(text, styleProperty, defaultValue)
    if styleValue == nil {
      styleValue = nodeStyleValue
    } else if styleValue != nodeStyleValue {
      styleValue = ""
      break
    }
  }
  return styleValue ?? defaultValue
}
