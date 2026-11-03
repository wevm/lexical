// Ported from packages/lexical/src/LexicalSelection.ts: the RangeSelection
// methods and helpers that edit the document (inserting text and nodes,
// removing text, splitting blocks, formatting). Deletion by character, word
// and line moves the selection through the platform's text layout, which
// the port does not have yet.

/// Marks a block that holds code, standing in for the JS test
/// `'__language' in node` that `insertNodes` uses to keep pasted content
/// inside a code block.
public protocol CodeLanguageNode: ElementNode {}

/// `InlineFormattableNode`: a node whose format is a text format bitmask.
/// `TextNode` is one.
public protocol InlineFormattableNode: LexicalNode {
  func getFormat() throws -> Int
  @discardableResult func setFormat(_ format: Int) throws -> Self
}

extension TextNode: InlineFormattableNode {}

/// `$isInlineFormattable`.
public func isInlineFormattable(_ node: LexicalNode?) -> Bool {
  node is InlineFormattableNode
}

/// `$recordTextInserted`: records an inserted text node while the editor
/// records text edits. The port does not record text edits yet.
package func recordTextInserted(_ node: TextNode) {}

private func transferStartingElementPointToTextPoint(
  _ start: Point, _ end: Point, _ format: Int, _ style: String
) throws {
  let element = try start.getNode() as! ElementNode
  let placementNode = try element.getChildAtIndex(start.offset)
  let textNode = try createTextNode()
  try textNode.setFormat(format)
  try textNode.setStyle(style)
  if let paragraph = placementNode as? ParagraphNode {
    try paragraph.splice(0, 0, [textNode])
  } else if let placementNode {
    let target: LexicalNode =
      try isRootOrShadowRoot(element) ? createParagraphNode().append(textNode) : textNode
    try placementNode.insertBefore(target)
  } else if try isRootOrShadowRoot(element) {
    if let lastChild = try element.getLastChild() as? ElementNode, try !lastChild.isInline(),
      try lastChild.isEmpty()
    {
      try lastChild.append(textNode)
    } else {
      try element.append(createParagraphNode().append(textNode))
    }
  } else {
    try element.append(textNode)
  }
  if start.is(end) {
    try end.set(textNode.__key, 0, .text)
  }
  try start.set(textNode.__key, 0, .text)
}

/// `$prepareCompositionTarget`: makes the anchor a text point that a
/// composition can write into, as inserting empty text does.
public func prepareCompositionTarget(_ selection: RangeSelection) throws {
  try insertText(selection, "", preparingComposition: true)
}

private func insertText(
  _ selection: RangeSelection, _ text: String, preparingComposition: Bool
) throws {
  var format = selection.format
  var style = selection.style
  if !selection.isCollapsed() {
    let firstPoint =
      try selection.focus.isBefore(selection.anchor) ? selection.focus : selection.anchor
    if let firstNode = try firstPoint.getNode() as? TextNode {
      format = try firstNode.getFormat()
      style = try firstNode.getStyle()
    }
    try selection.removeText()
    selection.format = format
    selection.style = style
    if text.isEmpty && !preparingComposition {
      return
    }
    if try getCompositionKey() == nil {
      if selection.anchor.type == .element {
        try transferStartingElementPointToTextPoint(
          selection.anchor, selection.focus, format, style)
      }
      try insertTextAtPoint(selection, text, format, style)
      return
    }
  }
  if selection.anchor.type == .element {
    try transferStartingElementPointToTextPoint(selection.anchor, selection.focus, format, style)
  }
  guard let anchorNode = try selection.anchor.getNode() as? TextNode else {
    throw LexicalError("insertText: anchor is not a text node")
  }
  let offset = selection.anchor.offset
  let anchorParent = try anchorNode.getParentOrThrow()
  let anchorSize = try anchorNode.getTextContentSize()
  let needsRedirect =
    try isTokenOrSegmented(anchorNode)
    || (offset == 0
      && (!anchorNode.canInsertTextBefore()
        || (!anchorParent.canInsertTextBefore() && anchorNode.__prev == nil)))
    || (offset == anchorSize
      && (!anchorNode.canInsertTextAfter()
        || (!anchorParent.canInsertTextAfter() && anchorNode.__next == nil)))
  if needsRedirect {
    if try anchorNode.isSegmented() && offset != 0 && offset != anchorSize {
      if try getCompositionKey() != nil {
        try anchorNode.setMode("normal").setFormat(format).setStyle(style)
        try getActiveEditor()._composedSegmentedKey = anchorNode.getKey()
      } else {
        let replacement = try createTextNode(anchorNode.getTextContent())
        try replacement.setFormat(format)
        try replacement.setStyle(style)
        try replacement.setDetail(anchorNode.getDetail())
        try carryNodeState(anchorNode, replacement)
        let isCurrentSelection = try getSelection() === selection
        try anchorNode.replace(replacement)
        try selection.setTextNodeRange(replacement, offset, replacement, offset)
        if try isCurrentSelection && getSelection() !== selection {
          try setSelection(selection)
        }
      }
      if !text.isEmpty || preparingComposition {
        try insertText(selection, text, preparingComposition: preparingComposition)
      }
      return
    }
    if text.isEmpty && !preparingComposition {
      return
    }
    if offset == 0 || offset == anchorSize {
      let before = offset == 0
      let direction: CaretDirection = before ? .previous : .next
      let sibling = try getSiblingCaret(anchorNode, direction).getNodeAtCaret()
      let target: TextNode
      if let sibling = sibling as? TextNode,
        try before ? sibling.canInsertTextAfter() : sibling.canInsertTextBefore(),
        try !isTokenOrSegmented(sibling)
      {
        target = sibling
      } else {
        target = try createTextNode().setFormat(format).setStyle(style)
        let canInsert =
          try before ? anchorParent.canInsertTextBefore() : anchorParent.canInsertTextAfter()
        try getSiblingCaret(canInsert ? anchorNode : anchorParent, direction).insert(target)
      }
      let targetOffset: Int? = before ? nil : 0
      try target.select(targetOffset, targetOffset)
      try insertText(selection, text, preparingComposition: preparingComposition)
      return
    }
    let newNode = try createTextNode(text)
    try newNode.setFormat(format)
    try newNode.setStyle(style)
    recordDeleted(anchorNode)
    recordTextInserted(newNode)
    try anchorNode.replace(newNode)
    try newNode.select()
    return
  }
  if text.isEmpty && !preparingComposition {
    return
  }
  let atStartOfInline = try anchorParent.isInline() && offset == 0 && anchorNode.__prev == nil
  let atEndOfInline =
    try anchorParent.isInline() && offset == anchorSize && anchorNode.__next == nil
  let formatDiffers = try anchorNode.getFormat() != format || anchorNode.getStyle() != style
  if atStartOfInline || atEndOfInline || formatDiffers {
    if try anchorNode.getTextContent().isEmpty && !atStartOfInline && !atEndOfInline {
      try anchorNode.setFormat(format)
      try anchorNode.setStyle(style)
    } else {
      try insertTextAtPoint(selection, text, format, style)
      return
    }
  }
  try anchorNode.spliceText(offset, 0, text, moveSelection: true)
  if try anchorNode.isComposing() && selection.anchor.type == .text {
    try selection.anchor.set(
      selection.anchor.key, selection.anchor.offset - text.utf16.count, selection.anchor.type)
  }
}

private func insertTextAtPoint(
  _ selection: RangeSelection, _ text: String, _ format: Int, _ style: String
) throws {
  guard let anchorNode = try selection.anchor.getNode() as? TextNode else {
    throw LexicalError("insertText: anchor is not a text node")
  }
  let offset = selection.anchor.offset
  let textNode = try createTextNode(text).setFormat(format).setStyle(style)
  recordTextInserted(textNode)
  let parent = try anchorNode.getParentOrThrow()
  let before = offset == 0
  let atBoundary = try before || offset == anchorNode.getTextContentSize()
  if try atBoundary && parent.isInline()
    && (before ? anchorNode.__prev : anchorNode.__next) == nil
  {
    try getSiblingCaret(parent, before ? .previous : .next).insert(textNode)
  } else {
    let origin = try atBoundary ? anchorNode : anchorNode.splitText(offset)[0]
    if before {
      try origin.insertBefore(textNode, restoreSelection: false)
    } else {
      try origin.insertAfter(textNode, restoreSelection: false)
    }
  }
  if try anchorNode.getTextContent().isEmpty && anchorNode.isAttached() {
    try anchorNode.remove()
  }
  try textNode.selectEnd()
  if try textNode.isComposing() && selection.anchor.type == .text {
    try selection.anchor.set(
      selection.anchor.key, selection.anchor.offset - text.utf16.count, selection.anchor.type)
  }
}

private func getNodeBeforePoint(_ point: Point) throws -> LexicalNode? {
  let node = try point.getNode()
  if point.offset > 0 {
    return try point.type == .element
      ? (node as? ElementNode)?.getChildAtIndex(point.offset - 1) : nil
  }
  var child: LexicalNode? = node
  while let current = child, try !INTERNAL_isBlock(current) && !isRootOrShadowRoot(current) {
    if let previousSibling = try current.getPreviousSibling() {
      return previousSibling
    }
    child = try current.getParent()
  }
  return nil
}

private func isPointAfterEmptyLine(_ point: Point) throws -> Bool {
  guard let nodeBefore = try getNodeBeforePoint(point), nodeBefore is LineBreakNode else {
    return false
  }
  return try nodeBefore.getPreviousSibling() is LineBreakNode
}

extension RangeSelection {
  /// Inserts `text`, with tabs and line breaks as their nodes.
  public func insertRawText(_ text: String) throws {
    try insertNodes(generateNodesFromRawText(text))
  }

  /// Replaces the selection with `text`, in the selection's format and
  /// style.
  public func insertText(_ text: String) throws {
    try Lexical.insertText(self, text, preparingComposition: false)
  }

  /// Removes the selected content and collapses the selection where it was.
  public func removeText() throws {
    let isCurrentSelection = try getSelection() === self
    let previousAnchorKey = anchor.key
    let newRange = try removeTextFromCaretRange(caretRangeFromSelection(self))
    try updateRangeSelectionFromCaretRange(self, newRange)
    if isCollapsed() {
      try internalRefreshSelectionFormatAndStyle(self, previousAnchorKey)
    }
    if try isCurrentSelection && getSelection() !== self {
      try setSelection(self)
    }
  }

  /// Toggles `formatType` on the selected text, aligned with
  /// `alignWithFormat` when given.
  public func formatText(_ formatType: TextFormatType, _ alignWithFormat: Int? = nil) throws {
    try Lexical.formatText(self, formatType, alignWithFormat)
  }

  /// Replaces the selection with `nodes`, splitting blocks as needed, and
  /// selects the end of what was inserted.
  public func insertNodes(_ nodes: [LexicalNode]) throws {
    if nodes.isEmpty {
      return
    }
    if !isCollapsed() {
      try removeText()
    }
    let anchorNode = try anchor.getNode()
    if anchor.type == .element, let anchorElement = anchorNode as? ElementNode,
      anchorElement.isShadowRoot(), try getSlotHostKey(anchorElement) != nil
    {
      var firstChild = try anchorElement.getFirstChild()
      if firstChild == nil {
        firstChild = try anchorElement.append(createParagraphNode()).getFirstChild()
      }
      if let child = firstChild, !(child is ElementNode) {
        let seed = try createParagraphNode()
        try child.insertBefore(seed)
        firstChild = seed
      }
      if let firstChild {
        try firstChild.selectStart()
        guard let redirected = try getSelection() as? RangeSelection else {
          throw LexicalError("Expected RangeSelection after redirecting into slot subtree")
        }
        return try redirected.insertNodes(nodes)
      }
    }
    if anchor.type == .element, try isBlockContainer(anchorNode) {
      let anchorElement = anchorNode as! ElementNode
      let blocksParent = try wrapInlineNodes(nodes)
      let nodeToSelect = try blocksParent.getLastDescendant()
      try anchorElement.splice(anchor.offset, 0, blocksParent.getChildren())
      try nodeToSelect?.selectEnd()
      return
    }
    let firstPoint = try isBackward() ? focus : anchor
    var firstNode = try firstPoint.getNode()
    var firstBlock = try findMatchingParent(firstNode, INTERNAL_isBlock)
    let last = nodes[nodes.count - 1]
    if let codeBlock = firstBlock as? CodeLanguageNode {
      if nodes[0] is CodeLanguageNode {
        try insertText(nodes[0].getTextContent())
      } else {
        let (_, index) = try removeTextAndSplitBlock(self)
        try codeBlock.splice(index, 0, nodes)
        try last.selectEnd()
      }
      return
    }
    func notInline(_ node: LexicalNode) throws -> Bool {
      try (node is ElementNode || node is DecoratorNode) && !node.isInline()
    }
    if try !nodes.contains(where: notInline) {
      guard let firstBlockElement = firstBlock as? ElementNode else {
        throw LexicalError(
          "Expected node \(String(describing: type(of: firstNode))) of type \(firstNode.getType()) to have a block ElementNode ancestor"
        )
      }
      let (container, index) = try removeTextAndSplitBlock(self, stopAtUnsplittableNode: true)
      let insertionParent = container as? ElementNode ?? firstBlockElement
      try insertionParent.splice(index, 0, nodes)
      try last.selectEnd()
      return
    }
    guard let foundBlock = firstBlock else {
      let blocksParent = try wrapInlineNodes(nodes)
      let nodeToSelect = try blocksParent.getLastDescendant()
      var caret = try caretFromPoint(anchor, .next)
      for block in try blocksParent.getChildren() {
        caret = try insertNodeToNearestRootAtCaret(block, caret)
      }
      try nodeToSelect?.selectEnd()
      return
    }
    if let blockElement = foundBlock as? ElementNode,
      try getSlotHostKey(blockElement) != nil
        || (!blockElement.isParentRequired()
          && !isBlockContainer(blockElement.getParentOrThrow()))
    {
      let (_, index) = try removeTextAndSplitBlock(self)
      let inlineNodes = try extractInlineFromBlocks(nodes)
      try blockElement.splice(index, 0, inlineNodes)
      if let lastInserted = inlineNodes.last {
        try lastInserted.selectEnd()
      } else {
        try blockElement.select(index, index)
      }
      return
    }
    let blocksParent = try wrapInlineNodes(nodes)
    let nodeToSelect = try blocksParent.getLastDescendant()!
    let blocks = try blocksParent.getChildren()
    let isAfterEmptyLine = try isPointAfterEmptyLine(firstPoint)
    func isMergeable(_ node: LexicalNode?) throws -> Bool {
      guard !isAfterEmptyLine, let element = node as? ElementNode, try INTERNAL_isBlock(element),
        try !element.isEmpty(), let firstBlockElement = firstBlock as? ElementNode
      else {
        return false
      }
      return try !firstBlockElement.isEmpty() || firstBlockElement.canMergeWhenEmpty()
    }
    let shouldInsert = try !((firstBlock as? ElementNode)?.isEmpty() ?? false)
    let insertedParagraph = try shouldInsert ? insertParagraph() : nil
    if insertedParagraph != nil, try !foundBlock.isAttached() {
      firstNode = try anchor.getNode()
      firstBlock = try findMatchingParent(firstNode, INTERNAL_isBlock)
    }
    let lastToInsert = blocks.last
    var firstToInsert = blocks.first
    if try isMergeable(firstToInsert) {
      guard let firstBlockElement = firstBlock as? ElementNode else {
        throw LexicalError(
          "Expected node \(String(describing: type(of: firstNode))) of type \(firstNode.getType()) to have a block ElementNode ancestor"
        )
      }
      try firstBlockElement.append(contentsOf: (firstToInsert as! ElementNode).getChildren())
      firstToInsert = blocks.count > 1 ? blocks[1] : nil
    }
    if let firstToInsert {
      guard let firstBlock else {
        throw LexicalError(
          "Expected node \(String(describing: type(of: firstNode))) of type \(firstNode.getType()) to have a block ancestor"
        )
      }
      try insertRangeAfter(firstBlock, firstToInsert)
    }
    let lastInsertedBlock = try findMatchingParent(nodeToSelect, INTERNAL_isBlock)
    let insertSelection = try nodeToSelect.selectEnd()
    if let insertedParagraph {
      if let lastInsertedElement = lastInsertedBlock as? ElementNode,
        try insertedParagraph.canMergeWhenEmpty() || lastToInsert.map(INTERNAL_isBlock) == true
      {
        try lastInsertedElement.append(contentsOf: insertedParagraph.getChildren())
        try insertedParagraph.remove()
      } else if try insertedParagraph.isEmpty() {
        try insertedParagraph.remove()
      }
    }
    if let firstBlockElement = firstBlock as? ElementNode, try firstBlockElement.isEmpty() {
      try firstBlockElement.remove()
    }
    if let lastChild = try (firstBlock as? ElementNode)?.getLastChild() as? LineBreakNode,
      lastInsertedBlock !== firstBlock
    {
      try lastChild.remove()
    }
    let normalizedCaret = try normalizeCaret(caretFromPoint(insertSelection.anchor, .next))
    try setPointFromCaret(insertSelection.anchor, normalizedCaret)
    try setPointFromCaret(insertSelection.focus, normalizedCaret)
  }

  /// Splits the block at the selection and returns the new block after it,
  /// with the selection at its start.
  @discardableResult
  public func insertParagraph() throws -> ElementNode? {
    if !isCollapsed() {
      try removeText()
    }
    let anchorNode = try anchor.getNode()
    if anchor.type == .element, try isBlockContainer(anchorNode) {
      let paragraph = try createParagraphNode()
      try (anchorNode as! ElementNode).splice(anchor.offset, 0, [paragraph])
      try paragraph.select()
      return paragraph
    }
    let (_, index) = try removeTextAndSplitBlock(self)
    let block = try findMatchingParent(anchor.getNode(), INTERNAL_isBlock)
    if let block, try getSlotHostKey(block) != nil {
      return nil
    }
    guard let block = block as? ElementNode else {
      throw LexicalError("Expected ancestor to be a block ElementNode")
    }
    let firstToAppend = try block.getChildAtIndex(index)
    let nodesToInsert = try firstToAppend.map { try [$0] + $0.getNextSiblings() } ?? []
    if let newBlock = try block.insertNewAfter(self, restoreSelection: false) as? ElementNode {
      try newBlock.append(contentsOf: nodesToInsert)
      try newBlock.selectStart()
      return newBlock
    }
    return nil
  }

  /// Inserts a line break, leaving the selection after it (or before it
  /// with `selectStart`).
  public func insertLineBreak(_ selectStart: Bool = false) throws {
    let lineBreak = try createLineBreakNode()
    try insertNodes([lineBreak])
    if selectStart {
      let parent = try lineBreak.getParentOrThrow()
      let index = try lineBreak.getIndexWithinParent()
      try parent.select(index, index)
    }
  }

  /// The selected nodes, with partly selected text split off into nodes of
  /// its own.
  public func extract() throws -> [LexicalNode] {
    let nodes = try getNodes()
    if isCollapsed() {
      return nodes
    }
    let backward = try isBackward()
    let (a, b) = try caretRangeFromSelection(self).getTextSlices()
    let slices = [a, b]
    var extracted: [LexicalNode] = []
    for node in nodes {
      let replacement: LexicalNode? =
        try getTextPointCaretSliceForNode(slices, node).map {
          try splitTextPointCaretSlice($0, self)
        }
        ?? node
      if let replacement {
        extracted.append(replacement)
      }
    }
    if nodes.count == 1, extracted.count == 1, let node = extracted[0] as? TextNode {
      let (start, end) = backward ? (focus, anchor) : (anchor, focus)
      try start.set(node.getKey(), 0, .text)
      try end.set(node.getKey(), node.getTextContentSize(), .text)
    }
    return extracted
  }
}

extension NodeSelection {
  public func insertRawText(_ text: String) throws {}

  public func insertText(_ text: String) throws {}

  /// Inserts `nodes` after the last selected node, replacing the selection.
  public func insertNodes(_ nodes: [LexicalNode]) throws {
    let selectedNodes = try getNodes().filter { try getSlotHostKey($0) == nil }
    guard let lastSelectedNode = selectedNodes.last else {
      return
    }
    let selectionAtEnd: RangeSelection
    if let text = lastSelectedNode as? TextNode {
      selectionAtEnd = try text.select()
    } else {
      let index = try lastSelectedNode.getIndexWithinParent() + 1
      selectionAtEnd = try lastSelectedNode.getParentOrThrow().select(index, index)
    }
    try selectionAtEnd.insertNodes(nodes)
    for node in selectedNodes {
      try node.remove()
    }
  }
}

/// `$insertNodes`: inserts `nodes` at the selection, or at the end of the
/// document when there is none.
public func insertNodes(_ nodes: [LexicalNode]) throws {
  var selection = try getSelection() ?? getPreviousSelection()
  if selection == nil {
    selection = try getRoot().selectEnd()
  }
  try selection!.insertNodes(nodes)
}

/// `RawTextVisitor`.
public struct RawTextVisitor {
  public var linebreak: () throws -> Void
  public var tab: () throws -> Void
  public var text: (String) throws -> Void

  public init(
    linebreak: @escaping () throws -> Void, tab: @escaping () throws -> Void,
    text: @escaping (String) throws -> Void
  ) {
    self.linebreak = linebreak
    self.tab = tab
    self.text = text
  }
}

/// `tokenizeRawText`: splits `text` at `\n`, `\r\n` and `\t`.
public func tokenizeRawText(_ text: String, _ visitor: RawTextVisitor) throws {
  // Over UTF-16 so that "\r\n" is two code units, as in the JS regex.
  let units = Array(text.utf16)
  var start = 0
  var i = 0
  func flush(_ end: Int) throws {
    if end > start {
      try visitor.text(String(decoding: units[start..<end], as: UTF16.self))
    }
  }
  while i < units.count {
    let unit = units[i]
    if unit == 0x0A {
      try flush(i)
      try visitor.linebreak()
      start = i + 1
    } else if unit == 0x0D, i + 1 < units.count, units[i + 1] == 0x0A {
      try flush(i)
      try visitor.linebreak()
      i += 1
      start = i + 1
    } else if unit == 0x09 {
      try flush(i)
      try visitor.tab()
      start = i + 1
    }
    i += 1
  }
  try flush(units.count)
}

/// `$generateNodesFromRawText`.
public func generateNodesFromRawText(_ text: String) throws -> [LexicalNode] {
  var nodes: [LexicalNode] = []
  try tokenizeRawText(
    text,
    RawTextVisitor(
      linebreak: { nodes.append(try createLineBreakNode()) },
      tab: { nodes.append(try createTabNode()) },
      text: { nodes.append(try createTextNode($0)) }))
  return nodes
}

private func extractInlineFromBlocks(_ nodes: [LexicalNode]) throws -> [LexicalNode] {
  var inlineNodes: [LexicalNode] = []
  for node in nodes {
    if node is LineBreakNode {
      continue
    }
    if try (node is ElementNode || node is DecoratorNode) && !node.isInline() {
      if let element = node as? ElementNode {
        inlineNodes += try extractInlineFromBlocks(element.getChildren())
      }
      continue
    }
    inlineNodes.append(node)
  }
  return inlineNodes
}

private func removeTextAndSplitBlock(
  _ selection: RangeSelection, stopAtUnsplittableNode: Bool = false
) throws -> (LexicalNode, Int) {
  var selection_ = selection
  if !selection.isCollapsed() {
    try selection_.removeText()
  }
  if let newSelection = try getSelection() as? RangeSelection {
    selection_ = newSelection
  }
  let anchor = selection_.anchor
  var node = try anchor.getNode()
  var offset = anchor.offset
  while try !INTERNAL_isBlock(node) && getSlotHostKey(node) == nil {
    let prevNode = node
    (node, offset) = try splitNodeAtPoint(node, offset, stopAtUnsplittableNode)
    if prevNode.is(node) {
      break
    }
  }
  return (node, offset)
}

private func splitNodeAtPoint(
  _ node: LexicalNode, _ offset: Int, _ stopAtUnsplittableNode: Bool
) throws -> (ElementNode, Int) {
  guard let parent = try node.getParent() else {
    let paragraph = try createParagraphNode()
    try getRoot().append(paragraph)
    try paragraph.select()
    return (try getRoot(), 0)
  }
  if let text = node as? TextNode {
    let split = try text.splitText(offset)
    if split.isEmpty {
      return (parent, try node.getIndexWithinParent())
    }
    let x = offset == 0 ? 0 : 1
    return (parent, try split[0].getIndexWithinParent() + x)
  }
  guard let element = node as? ElementNode, offset != 0 else {
    return (parent, try node.getIndexWithinParent())
  }
  if let firstToAppend = try element.getChildAtIndex(offset) {
    let insertPoint = RangeSelection(
      createPoint(element.__key, offset, .element), createPoint(element.__key, offset, .element),
      0, "")
    if let newElement = try element.insertNewAfter(insertPoint) as? ElementNode {
      try newElement.append(contentsOf: [firstToAppend] + firstToAppend.getNextSiblings())
    } else if stopAtUnsplittableNode {
      return (element, offset)
    }
  }
  return (parent, try element.getIndexWithinParent() + 1)
}

private func isInlineRunNode(_ node: LexicalNode) throws -> Bool {
  try node is LineBreakNode || isInlineElementOrDecoratorNode(node) || node is TextNode
    || node.isParentRequired()
}

private func wrapInlineNodes(_ nodes: [LexicalNode]) throws -> ParagraphNode {
  let virtualRoot = try createParagraphNode()
  var currentBlock: ElementNode?
  for (i, node) in nodes.enumerated() {
    if try isInlineRunNode(node) {
      if currentBlock == nil {
        let block = try node.createParentElementNode()
        currentBlock = block
        try virtualRoot.append(block)
        let nextNode: LexicalNode? = i + 1 < nodes.count ? nodes[i + 1] : nil
        if try node is LineBreakNode && (nextNode == nil || !isInlineRunNode(nextNode!)) {
          continue
        }
      }
      try currentBlock!.append(node)
    } else {
      try virtualRoot.append(node)
      currentBlock = nil
    }
  }
  return virtualRoot
}

// MARK: Formatting

private func updateTextFormat(
  _ selection: any BaseSelection, _ applyFormat: (Int) throws -> Int
) throws {
  if let nodeSelection = selection as? NodeSelection {
    for node in try nodeSelection.getNodes() {
      if let node = node as? InlineFormattableNode {
        try node.setFormat(applyFormat(node.getFormat()))
      }
    }
    return
  }
  guard let selection = selection as? RangeSelection else { return }
  let nodes = try selection.isCollapsed() ? [] : selection.getNodes()
  var slices: [TextPointCaretSlice?] = []
  if !nodes.isEmpty {
    let (a, b) = try caretRangeFromSelection(selection).getTextSlices()
    slices = [a, b]
  }
  var hasText = false
  var skippedStart: Point?
  var firstText: TextNode?
  var firstFormat: Int?
  var lastFormat = 0
  for node in nodes {
    if let text = node as? TextNode {
      if let start = skippedStart, let firstText {
        try start.set(firstText.__key, 0, .text)
        skippedStart = nil
      }
      let slice = getTextPointCaretSliceForNode(slices, text)
      if !hasText, let slice, slice.distance == 0 {
        let start = try selection.isBackward() ? selection.focus : selection.anchor
        if start.type == .text && start.key == text.__key {
          skippedStart = start
        }
      }
      hasText = true
      if let slice, slice.distance == 0 {
        continue
      }
      let nextFormat = try applyFormat(text.getFormat())
      let originalSize = try skippedStart != nil ? text.getTextContentSize() : 0
      let replacement: TextNode? =
        try slice != nil && !isTokenOrSegmented(text)
        ? splitTextPointCaretSlice(slice!, selection) : text
      if let replacement {
        try replacement.setFormat(nextFormat)
        if firstFormat == nil {
          firstFormat = nextFormat
          firstText = replacement
          if let start = skippedStart, try replacement.getTextContentSize() != originalSize {
            try start.set(replacement.__key, 0, .text)
            skippedStart = nil
          }
        }
        lastFormat = nextFormat
      }
    } else if let element = node as? ElementNode {
      try element.setTextFormat(applyFormat(element.getTextFormat()))
    } else if let node = node as? InlineFormattableNode {
      try node.setFormat(applyFormat(node.getFormat()))
    }
  }
  if !hasText {
    try selection.setFormat(applyFormat(selection.format))
    try setCompositionKey(nil)
  } else if let firstFormat {
    selection.format = firstFormat | lastFormat
  }
}

/// `$setTextFormat`: sets (`true`) or clears (`false`) each listed format.
public func setTextFormat(
  _ selection: any BaseSelection, _ formats: [(TextFormatType, Bool)]
) throws {
  if formats.isEmpty {
    return
  }
  try updateTextFormat(selection) { initial in
    var format = initial
    for (type, value) in formats {
      format = toggleTextFormatType(format, type, value ? TEXT_TYPE_TO_FORMAT[type] ?? 0 : 0)
    }
    return format
  }
}

/// `$formatText`.
public func formatText(
  _ selection: any BaseSelection, _ formatType: TextFormatType, _ alignWithFormat: Int? = nil
) throws {
  let effectiveAlign =
    alignWithFormat == nil
    ? (selection as? RangeSelection).map { toggleTextFormatType($0.format, formatType, nil) }
    : alignWithFormat
  try updateTextFormat(selection) { toggleTextFormatType($0, formatType, effectiveAlign) }
}

private func getCharacterOffset(_ point: Point) throws -> Int {
  let offset = point.offset
  if point.type == .text {
    return offset
  }
  let parent = try point.getNode() as! ElementNode
  return try offset == parent.getChildrenSize() ? parent.getTextContent().utf16.count : 0
}

/// `$getCharacterOffsets`: the text offsets of the selection's points.
public func getCharacterOffsets(_ selection: any BaseSelection) throws -> (Int, Int) {
  guard let (anchor, focus) = selection.getStartEndPoints() else {
    return (0, 0)
  }
  if anchor.type == .element && focus.type == .element && anchor.key == focus.key
    && anchor.offset == focus.offset
  {
    return (0, 0)
  }
  return (try getCharacterOffset(anchor), try getCharacterOffset(focus))
}

/// `$collapseAtStart`: asks the elements the selection starts in to handle
/// a backspace at their start.
package func collapseAtStart(_ selection: RangeSelection, _ startNode: LexicalNode) throws -> Bool {
  var node: LexicalNode? = startNode
  while let current = node {
    if let element = current as? ElementNode {
      if try element.collapseAtStart(selection) {
        return true
      }
      if try isRootOrShadowRoot(element) {
        break
      }
    }
    if try current.getPreviousSibling() != nil {
      break
    }
    node = try current.getParent()
  }
  return false
}

/// `INTERNAL_$expandSelectionToWholeDocument`: a selection that covers the
/// whole document becomes the root's element range.
package func INTERNAL_expandSelectionToWholeDocument(_ selection: RangeSelection) throws {
  let root = try getRoot()
  if try root.isEmpty() || !isBlockFullySelected(root, selection) {
    return
  }
  try selection.anchor.set(root.getKey(), 0, .element)
  try selection.focus.set(root.getKey(), root.getChildrenSize(), .element)
}
