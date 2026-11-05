import Lexical
import LexicalText
import LexicalUtils

// Port of packages/lexical-react/src/shared/useCharacterLimit.ts: the logic
// of `CharacterLimitPlugin`, which a view shows the count of.

/// `useCharacterLimit(editor, maxCharacters, {strlen, remainingCharacters})`:
/// wraps the text past `maxCharacters` in `OverflowNode`s and reports the
/// characters left (negative past the limit) to `remainingCharacters`, at
/// once and after each change of content. `strlen` measures text, in UTF-16
/// code units by default; the limit falls between grapheme clusters. Returns
/// the clean-up.
public func registerCharacterLimit(
  _ editor: Editor, maxCharacters: Int,
  strlen: @escaping (String) -> Int = { $0.utf16.count },
  remainingCharacters: @escaping (Int) -> Void = { _ in }
) throws -> () -> Void {
  guard editor.hasNodes([OverflowNode.self]) else {
    throw LexicalError("useCharacterLimit: OverflowNode not registered on editor")
  }
  var text = try editor.read(.latest) { try rootTextContent() }
  var lastComputedTextLength: Int?

  func updateCharacterLimit() throws {
    let textLength = strlen(text)
    let textLengthAboveThreshold =
      textLength > maxCharacters
      || (lastComputedTextLength.map { $0 > maxCharacters } ?? false)
    remainingCharacters(maxCharacters - textLength)
    if lastComputedTextLength == nil || textLengthAboveThreshold {
      let offset = findOffset(text, maxCharacters, strlen)
      try editor.update(tag: HISTORY_MERGE_TAG) { try wrapOverflowedNodes(offset) }
    }
    lastComputedTextLength = textLength
  }

  // Count the content already there: an update listener does not fire on
  // registration.
  try updateCharacterLimit()

  return mergeRegister(
    editor.registerTextContentListener { text = $0 },
    editor.registerUpdateListener { payload in
      if editor.isComposing() || (payload.dirtyLeaves.size == 0 && payload.dirtyElements.size == 0)
      {
        return
      }
      try updateCharacterLimit()
    },
    editor.registerCommand(
      DELETE_CHARACTER_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        let anchorNode = try selection.anchor.getNode()
        let overflow = try anchorNode.getParent()
        let overflowParent = try overflow?.getParent()
        let parentNext = try overflowParent?.getNextSibling()
        try selection.deleteCharacter(isBackward)
        if let overflowParent, try overflowParent.isEmpty() {
          try overflowParent.remove()
        } else if let parentNext = parentNext as? ElementNode, try parentNext.isEmpty() {
          try parentNext.remove()
        }
        return true
      }, COMMAND_PRIORITY_LOW))
}

/// The UTF-16 offset in `text` after the grapheme clusters that fit in
/// `maxCharacters` as `strlen` measures them.
func findOffset(_ text: String, _ maxCharacters: Int, _ strlen: (String) -> Int) -> Int {
  var offsetUtf16 = 0
  var offset = 0
  for grapheme in text {
    let nextOffset = offset + strlen(String(grapheme))
    if nextOffset > maxCharacters {
      break
    }
    offset = nextOffset
    offsetUtf16 += grapheme.utf16.count
  }
  return offsetUtf16
}

/// `$wrapOverflowedNodes(offset)`: wraps the leaves past `offset` (in the
/// root's text content, slots included) in `OverflowNode`s, and unwraps those
/// that are within it again.
package func wrapOverflowedNodes(_ offset: Int) throws {
  var accumulatedLength = 0
  for dfsNode in try dfsWithSlots() {
    let node = dfsNode.node
    // `ElementNode.getTextContent()` puts "\n\n" after each block child but
    // the last, which the offset counts.
    if let prevSibling = try node.getPreviousSibling() as? ElementNode,
      try !prevSibling.isInline()
    {
      accumulatedLength += 2
    }
    // A slot value root has no parent to replace it in, so it is not wrapped,
    // but its text counts.
    let isSlotValueLeaf = try isLeafNode(node) && getSlotHost(node) != nil
    let needsOverflowParent =
      try isLeafNode(node) && !isSlotValueLeaf
      && findMatchingParent(node, { isOverflowNode($0) }) == nil

    if let node = node as? OverflowNode {
      let previousLength = accumulatedLength
      let nextLength = accumulatedLength + (try node.getTextContentSize())
      if nextLength <= offset {
        let parent = try node.getParent()
        let previousSibling = try node.getPreviousSibling()
        let nextSibling = try node.getNextSibling()
        try unwrapNode(node)
        // Restore the selection when the overflow's children are removed.
        if let selection = try getSelection() as? RangeSelection,
          try !selection.anchor.getNode().isAttached() || !selection.focus.getNode().isAttached()
        {
          if let previousSibling = previousSibling as? TextNode {
            try previousSibling.select()
          } else if let nextSibling = nextSibling as? TextNode {
            try nextSibling.select()
          } else if let parent {
            try parent.select()
          }
        }
      } else if previousLength < offset {
        let descendant = try node.getFirstDescendant()
        let descendantLength = try descendant?.getTextContentSize() ?? 0
        // Simple text can be split to fit the overflow more closely.
        let firstDescendantIsSimpleText = try (descendant as? TextNode)?.isSimpleText() ?? false
        let firstDescendantDoesNotOverflow = previousLength + descendantLength <= offset
        if firstDescendantIsSimpleText || firstDescendantDoesNotOverflow {
          try unwrapNode(node)
        }
      }
    } else if isSlotValueLeaf {
      accumulatedLength += try node.getTextContentSize()
    } else if needsOverflowParent {
      let previousAccumulatedLength = accumulatedLength
      accumulatedLength += try node.getTextContentSize()
      if accumulatedLength > offset, !isOverflowNode(try node.getParent()) {
        let previousSelection = try getSelection()
        let overflowNode: OverflowNode
        // Simple text is split at the limit.
        if previousAccumulatedLength < offset, let text = node as? TextNode,
          try text.isSimpleText()
        {
          let overflowedText = try text.splitText(offset - previousAccumulatedLength)[1]
          overflowNode = try wrapNode(overflowedText)
        } else {
          overflowNode = try wrapNode(node)
        }
        if let previousSelection {
          try setSelection(previousSelection)
        }
        try mergePrevious(overflowNode)
        if let nextNode = try overflowNode.getNextSibling() as? OverflowNode {
          try mergePrevious(nextNode)
        }
      }
    }
  }
}

private func wrapNode(_ node: LexicalNode) throws -> OverflowNode {
  let overflowNode = try createOverflowNode()
  try node.replace(overflowNode)
  try overflowNode.append(node)
  return overflowNode
}

/// `$mergePrevious(overflowNode)`: moves the children of the `OverflowNode`
/// before `overflowNode` into it, with the selection in either.
package func mergePrevious(_ overflowNode: OverflowNode) throws {
  guard let previousNode = try overflowNode.getPreviousSibling() as? OverflowNode else {
    return
  }
  let firstChild = try overflowNode.getFirstChild()
  let previousNodeChildren = try previousNode.getChildren()
  let previousNodeChildrenLength = previousNodeChildren.count
  if let firstChild {
    for child in previousNodeChildren {
      try firstChild.insertBefore(child)
    }
  } else {
    try overflowNode.append(contentsOf: previousNodeChildren)
  }
  if let selection = try getSelection() as? RangeSelection {
    let anchor = selection.anchor
    let anchorNode = try anchor.getNode()
    let focus = selection.focus
    let focusNode = try focus.getNode()
    if anchorNode.is(previousNode) {
      try anchor.set(overflowNode.getKey(), anchor.offset, .element)
    } else if anchorNode.is(overflowNode) {
      try anchor.set(
        overflowNode.getKey(), previousNodeChildrenLength + anchor.offset, .element)
    }
    if focusNode.is(previousNode) {
      try focus.set(overflowNode.getKey(), focus.offset, .element)
    } else if focusNode.is(overflowNode) {
      try focus.set(
        overflowNode.getKey(), previousNodeChildrenLength + focus.offset, .element)
    }
  }
  try previousNode.remove()
}
