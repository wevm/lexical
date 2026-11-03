import Lexical
import Synchronization

// Ported from packages/lexical-selection/src/lexical-node.ts.

/// How `sliceSelectedTextNodeContent` changes a node: in place, or on an
/// ephemeral clone.
public enum SliceMutation: Sendable { case `self`, clone }

private func slices(_ range: CaretRange) throws -> [TextPointCaretSlice?] {
  let (a, b) = try range.getTextSlices()
  return [a, b]
}

/// `$sliceSelectedTextNodeContent`: `textNode` with its text cut to the
/// part `selection` covers.
@discardableResult
public func sliceSelectedTextNodeContent<T: TextNode>(
  _ selection: any BaseSelection, _ textNode: T, _ mutates: SliceMutation = .self
) throws -> T {
  var textNode = textNode
  guard let points = selection.getStartEndPoints(),
    points.0.key == textNode.__key || points.1.key == textNode.__key,
    try textNode.isSelected(selection), try !isTokenOrSegmented(textNode)
  else {
    return textNode
  }
  let (start, end) = try selection.isBackward() ? (points.1, points.0) : points
  var slice: TextPointCaretSlice?
  if let range = selection as? RangeSelection, start.type == .element || end.type == .element {
    slice = getTextPointCaretSliceForNode(try slices(caretRangeFromSelection(range)), textNode)
  }
  let startOffset: Int
  let endOffset: Int?
  if let slice {
    (startOffset, endOffset) = slice.getSliceIndices()
  } else {
    startOffset = textNode.__key == start.key ? start.offset : 0
    endOffset = textNode.__key == end.key ? end.offset : nil
  }
  let text = textNode.__text.jsSlice(startOffset, endOffset)
  if text != textNode.__text {
    if mutates == .clone {
      textNode = try cloneWithPropertiesEphemeral(textNode)
    }
    textNode.__text = text
  }
  return textNode
}

/// `$isAtNodeEnd`.
public func isAtNodeEnd(_ point: Point) throws -> Bool {
  if point.type == .text {
    return try point.offset == point.getNode().getTextContentSize()
  }
  guard let node = try point.getNode() as? ElementNode else {
    throw LexicalError("isAtNodeEnd: node must be a TextNode or ElementNode")
  }
  return try point.offset == node.getChildrenSize()
}

/// `$trimTextContentFromAnchor`: removes `delCount` characters of content
/// before `anchor`, as a character limit does.
public func trimTextContentFromAnchor(_ editor: Editor, _ anchor: Point, _ delCount: Int) throws {
  var currentNode: LexicalNode? = try anchor.getNode()
  var remaining = delCount
  if let element = currentNode as? ElementNode,
    let descendant = try element.getDescendantByIndex(anchor.offset)
  {
    currentNode = descendant
  }
  while remaining > 0, var node = currentNode {
    if let element = node as? ElementNode, let last = try element.getLastDescendant() {
      node = last
    }
    var nextNode: LexicalNode? = try node.getPreviousSibling()
    var additionalElementWhitespace = 0
    if nextNode == nil {
      var parent: LexicalNode? = try node.getParentOrThrow()
      var parentSibling = try parent?.getPreviousSibling()
      while parentSibling == nil {
        parent = try parent?.getParent()
        guard let p = parent else {
          nextNode = nil
          break
        }
        parentSibling = try p.getPreviousSibling()
      }
      if let parent {
        additionalElementWhitespace = try parent.isInline() ? 0 : 2
        nextNode = parentSibling
      }
    }
    var text = try node.getTextContent()
    if text.isEmpty, let element = node as? ElementNode, try !element.isInline() {
      text = "\n\n"
    }
    let currentNodeSize = text.utf16.count
    guard let textNode = node as? TextNode, remaining < currentNodeSize else {
      let parent = try node.getParent()
      try node.remove()
      if let parent, try parent.getChildrenSize() == 0, !(parent is RootNode) {
        try parent.remove()
      }
      remaining -= currentNodeSize + additionalElementWhitespace
      currentNode = nextNode
      continue
    }
    let key = textNode.getKey()
    let prevTextContent: String? = try editor.read(.latest) {
      if let prevNode = try getNodeByKey(key) as? TextNode, try prevNode.isSimpleText() {
        return try prevNode.getTextContent()
      }
      return nil
    }
    let offset = currentNodeSize - remaining
    let slicedText = text.jsSlice(0, offset)
    if let prevTextContent, prevTextContent != text {
      let prevSelection = try getPreviousSelection()
      var target: TextNode = textNode
      if try !textNode.isSimpleText() {
        let replacement = try createTextNode(prevTextContent)
        try textNode.replace(replacement)
        target = replacement
      } else {
        try textNode.setTextContent(prevTextContent)
      }
      if let prevSelection = prevSelection as? RangeSelection, prevSelection.isCollapsed() {
        let prevOffset = prevSelection.anchor.offset
        try target.select(prevOffset, prevOffset)
      }
    } else if try textNode.isSimpleText() {
      let isSelected = anchor.key == key
      var anchorOffset = anchor.offset
      if anchorOffset < remaining {
        anchorOffset = currentNodeSize
      }
      let splitStart = isSelected ? anchorOffset - remaining : 0
      let splitEnd = isSelected ? anchorOffset : offset
      let parts = try textNode.splitText(splitStart, splitEnd)
      if isSelected && splitStart == 0 {
        try parts[0].remove()
      } else {
        try parts[1].remove()
      }
    } else {
      try textNode.replace(try createTextNode(slicedText))
    }
    remaining = 0
  }
}

private let addNodeStyleWarned = Mutex(false)

/// `$addNodeStyle`: a deprecated no-op that warns once.
public func addNodeStyle(_ node: TextNode) {
  let warn = addNodeStyleWarned.withLock { warned in
    defer { warned = true }
    return !warned
  }
  if warn {
    consoleWarn("$addNodeStyle is a deprecated no-op and calls should be removed")
  }
}

/// What a style patch is applied to.
public enum StylePatchTarget {
  case text(TextNode)
  case selection(RangeSelection)
  case element(ElementNode)
}

/// A value of a style patch: a value, `nil` to remove the property, or a
/// function of the current value.
public enum StylePatchValue: ExpressibleByStringLiteral, ExpressibleByNilLiteral {
  case value(String)
  case remove
  case compute((String?, StylePatchTarget) throws -> String)

  public init(stringLiteral value: String) { self = .value(value) }
  public init(nilLiteral: ()) { self = .remove }
}

/// A style patch, in the order its properties are applied.
public typealias StylePatch = KeyValuePairs<String, StylePatchValue>

private func patchStyle(_ target: StylePatchTarget, _ patch: StylePatch) throws {
  let prevCSS: String
  switch target {
  case .selection(let selection):
    try invariant(
      selection.isCollapsed(),
      "$patchStyle must only be called with a TextNode, ElementNode, or collapsed RangeSelection")
    prevCSS = selection.style
  case .text(let node): prevCSS = try node.getStyle()
  case .element(let node): prevCSS = try node.getTextStyle()
  }
  let prevStyles = getStyleObjectFromCSS(prevCSS)
  var newStyles = prevStyles
  for (key, value) in patch {
    switch value {
    case .compute(let fn): newStyles[key] = try fn(prevStyles[key], target)
    case .remove: newStyles[key] = nil
    case .value(let value): newStyles[key] = value
    }
  }
  let newCSSText = getCSSFromStyleObject(newStyles)
  switch target {
  case .selection(let selection): selection.setStyle(newCSSText)
  case .text(let node): try node.setStyle(newCSSText)
  case .element(let node): try node.setTextStyle(newCSSText)
  }
}

/// `$patchStyleText`: applies `patch` to the style of the selected text,
/// to a collapsed selection's style, and to selected empty elements' text
/// style.
public func patchStyleText(_ selection: any BaseSelection, _ patch: StylePatch) throws {
  var patchedElementKeys = Set<NodeKey>()
  if let range = selection as? RangeSelection, range.isCollapsed() {
    try patchStyle(.selection(range), patch)
    if let emptyNode = try range.anchor.getNode() as? ElementNode, try emptyNode.isEmpty() {
      patchedElementKeys.insert(emptyNode.getKey())
      try patchStyle(.element(emptyNode), patch)
    }
  }
  try forEachSelectedTextNodeInSelection(selection) { try patchStyle(.text($0), patch) }
  for node in try selection.getNodes() {
    guard let element = node as? ElementNode, try element.canBeEmpty(),
      try element.getChildrenSize() == 0
    else { continue }
    if patchedElementKeys.insert(element.getKey()).inserted {
      try patchStyle(.element(element), patch)
    }
  }
}

/// `$forEachSelectedTextNode`: calls `fn` with each selected text node,
/// split to the selected part.
public func forEachSelectedTextNode(_ fn: (TextNode) throws -> Void) throws {
  try forEachSelectedTextNodeInSelection(try getSelection(), fn)
}

private func forEachSelectedTextNodeInSelection(
  _ selection: (any BaseSelection)?, _ fn: (TextNode) throws -> Void
) throws {
  guard let selection else { return }
  let range = selection as? RangeSelection
  let textSlices = try range.map { try slices(caretRangeFromSelection($0)) } ?? []
  for node in try selection.getNodes() {
    guard let textNode = node as? TextNode, textNode.canHaveFormat() else { continue }
    let slice = getTextPointCaretSliceForNode(textSlices, textNode)
    if try slice.map({ $0.distance == 0 }) ?? (textNode.getTextContentSize() == 0) {
      continue
    }
    if let slice, try !isTokenOrSegmented(textNode) {
      try fn(try splitTextPointCaretSlice(slice, range)!)
    } else {
      try fn(textNode)
    }
  }
  if let range, range.anchor.type == .text, range.focus.type == .text,
    range.anchor.key == range.focus.key
  {
    try ensureForwardRangeSelection(range)
  }
}

/// `$ensureForwardRangeSelection`: swaps a backward selection's points.
public func ensureForwardRangeSelection(_ selection: RangeSelection) throws {
  if try selection.isBackward() {
    let anchor = selection.anchor
    let focus = selection.focus
    let (key, offset, type) = (anchor.key, anchor.offset, anchor.type)
    try anchor.set(focus.key, focus.offset, focus.type)
    try focus.set(key, offset, type)
  }
}
