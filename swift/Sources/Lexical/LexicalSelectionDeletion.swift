// Ported from packages/lexical/src/LexicalSelection.ts: deletion and
// keyboard movement of a RangeSelection.
//
// JS moves the selection by a character, word or line through the DOM's
// `Selection.modify`, after `$modifySelectionAroundDecoratorsAndBlocks` has
// handled the moves that cross decorators and blocks. Without a view (a
// headless editor, and JS under jsdom, which has no `modify`) only that
// first step runs. The port has no text layout to move through yet, so it
// always takes that path; `SelectionMovement` is the seam the TextKit views
// will fill.

/// `'move'` or `'extend'`.
public enum SelectionAlter: String, Sendable {
  case move, extend
}

/// `'character'`, `'word'` or `'lineboundary'`.
public enum SelectionGranularity: String, Sendable {
  case character, word, lineboundary
}

/// `$needsBlockCursorBeside`: whether a caret beside `node` needs a block
/// cursor, as a block decorator, a top-level shadow root and an element that
/// can't be empty do.
public func needsBlockCursorBeside(_ node: LexicalNode?) throws -> Bool {
  guard let node, try !node.isInline() else {
    return false
  }
  if node is DecoratorNode {
    return true
  }
  if let element = node as? ElementNode {
    if element.isShadowRoot() {
      let parent = try element.getParent()
      return !(parent?.isShadowRoot() ?? false)
    }
    return try !element.canBeEmpty()
  }
  return false
}

private func modifySelectionAroundDecoratorsAndBlocks(
  _ selection: RangeSelection, _ alter: SelectionAlter, _ isBackward: Bool,
  _ granularity: SelectionGranularity, decoratorsOnly: Bool = false
) throws -> Bool {
  if alter == .move && granularity == .character && !selection.isCollapsed() {
    let (src, dst) =
      try isBackward == selection.isBackward()
      ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
    try dst.set(src.key, src.offset, src.type)
    return true
  }
  let initialFocus = try caretFromPoint(selection.focus, isBackward ? .previous : .next)
  let isLineBoundary = granularity == .lineboundary
  let collapse = alter == .move
  var focus = initialFocus
  // JS compares `focus === initialFocus`: whether any step moved it.
  var moved = false
  var checkForBlock = !decoratorsOnly
  var isolated = false
  if try !isExtendableTextPointCaret(focus) {
    for siblingCaret in try focus.siblings() {
      checkForBlock = false
      if let origin = siblingCaret.origin as? DecoratorNode {
        if origin.isIsolated() {
          isolated = true
          break
        }
        focus = siblingCaret
        moved = true
        if try isLineBoundary && origin.isInline() {
          continue
        }
      }
      break
    }
    if isolated {
      return true
    }
    if checkForBlock {
      for nextCaret in try extendCaretToRange(initialFocus).iterNodeCarets(
        alter == .extend ? .shadowRoot : .root)
      {
        if nextCaret.isChild {
          if try !nextCaret.origin.isInline() {
            focus = nextCaret
            moved = true
          }
        } else if nextCaret.origin is ElementNode {
          continue
        } else if nextCaret.origin is DecoratorNode, try !nextCaret.origin.isInline() {
          focus = nextCaret
          moved = true
        }
        break
      }
    }
  }
  if !moved {
    return false
  }
  if collapse && !isLineBoundary, let decorator = focus.origin as? DecoratorNode,
    decorator.isKeyboardSelectable()
  {
    let nodeSelection = createNodeSelection()
    nodeSelection.add(decorator.getKey())
    try setSelection(nodeSelection)
    return true
  }
  focus = try normalizeCaret(focus)
  if collapse {
    try setPointFromCaret(selection.anchor, focus)
  }
  try setPointFromCaret(selection.focus, focus)
  return checkForBlock || !isLineBoundary
}

/// `$extendSelectionForDeletion`. Returns whether it stopped at a slot.
private func extendSelectionForDeletion(
  _ selection: RangeSelection, _ isBackward: Bool, _ granularity: SelectionGranularity
) throws -> Bool {
  if try modifySelectionAroundDecoratorsAndBlocks(selection, .extend, isBackward, granularity) {
    return false
  }
  // JS continues through `Selection.modify`; without text layout the port
  // stops here, as JS does when the DOM selection has no `modify`.
  return false
}

private func stopLineDeletionAtLineBreak(_ selection: RangeSelection) throws {
  for caret in try caretRangeFromSelection(selection).iterNodeCarets(.shadowRoot) {
    if caret.isSibling && caret.origin is LineBreakNode {
      try setPointFromCaret(selection.focus, rewindSiblingCaret(caret))
      return
    }
  }
}

private func deleteTextByGranularity(
  _ selection: RangeSelection, _ isBackward: Bool, _ granularity: SelectionGranularity
) throws {
  let wasCollapsed = selection.isCollapsed()
  let anchor = selection.anchor
  let focus = selection.focus
  var stoppedAtSlot = false
  if wasCollapsed {
    if try granularity == .word && selection.forwardDeletion(anchor, anchor.getNode(), isBackward) {
      return
    }
    stoppedAtSlot = try extendSelectionForDeletion(selection, isBackward, granularity)
    if granularity == .lineboundary {
      try stopLineDeletionAtLineBreak(selection)
    }
  }
  if try granularity == .lineboundary && !selection.isCollapsed()
    && findMatchingParent(anchor.getNode(), INTERNAL_isBlock)
      !== findMatchingParent(focus.getNode(), INTERNAL_isBlock)
  {
    try focus.set(anchor.key, anchor.offset, anchor.type)
  }
  if selection.isCollapsed() {
    if !stoppedAtSlot {
      try selection.deleteCharacter(isBackward)
    }
  } else {
    if !wasCollapsed {
      try INTERNAL_expandSelectionToWholeDocument(selection)
    }
    try selection.removeText()
  }
}

/// JS `\s`, and the characters `String.prototype.trim` removes.
private func isJSWhitespace(_ unit: UInt16) -> Bool {
  switch unit {
  case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F,
    0x3000, 0xFEFF:
    true
  default: false
  }
}

private func jsTrim(_ units: ArraySlice<UInt16>) -> ArraySlice<UInt16> {
  var slice = units
  while let first = slice.first, isJSWhitespace(first) { slice = slice.dropFirst() }
  while let last = slice.last, isJSWhitespace(last) { slice = slice.dropLast() }
  return slice
}

private func removeSegment(_ textNode: TextNode, _ isBackward: Bool, _ offset: Int) throws {
  let units = Array(try textNode.getTextContent().utf16)
  // `split(/(?=\s)/g)`: a new part starts at each whitespace unit but the
  // first.
  var split: [ArraySlice<UInt16>] = []
  var start = 0
  for i in units.indices where i > 0 && isJSWhitespace(units[i]) {
    split.append(units[start..<i])
    start = i
  }
  split.append(units[start..<units.count])
  if units.isEmpty {
    split = []
  }
  let splitLength = split.count
  var segmentOffset = 0
  var restoreOffset: Int? = 0
  for i in 0..<splitLength {
    let text = split[i]
    let isLast = i == splitLength - 1
    restoreOffset = segmentOffset
    segmentOffset += text.count
    if (isBackward && segmentOffset == offset) || segmentOffset > offset || isLast {
      split.remove(at: i)
      if isLast {
        restoreOffset = nil
      }
      break
    }
  }
  let joined = split.flatMap { $0 }
  let nextTextContent = jsTrim(joined[...])
  if nextTextContent.isEmpty {
    try textNode.remove()
  } else {
    try textNode.setTextContent(String(decoding: nextTextContent, as: UTF16.self))
    try textNode.select(restoreOffset, restoreOffset)
  }
}

private func isSurrogate(_ unit: UInt16) -> Bool { (0xD800...0xDFFF).contains(unit) }

/// `shouldDeleteExactlyOneCodeUnit`: false when the text holds a surrogate
/// pair or an emoji (Unicode `Emoji`, which includes digits, `#` and `*`).
private func shouldDeleteExactlyOneCodeUnit(_ text: String) throws -> Bool {
  try invariant(
    text.utf16.count > 1,
    "shouldDeleteExactlyOneCodeUnit: expecting to be called only with sequences of two or more code units"
  )
  let containsSurrogatePair = text.utf16.contains(where: isSurrogate)
  let containsEmoji = text.unicodeScalars.contains { $0.properties.isEmoji }
  return !(containsSurrogatePair || containsEmoji)
}

private func updateCaretSelectionForUnicodeCharacter(
  _ selection: RangeSelection, _ isBackward: Bool
) throws {
  let anchor = selection.anchor
  let focus = selection.focus
  let anchorNode = try anchor.getNode()
  let focusNode = try focus.getNode()
  if anchorNode === focusNode && anchor.type == .text && focus.type == .text {
    let anchorOffset = anchor.offset
    let focusOffset = focus.offset
    let isBefore = anchorOffset < focusOffset
    let startOffset = isBefore ? anchorOffset : focusOffset
    let endOffset = isBefore ? focusOffset : anchorOffset
    let characterOffset = endOffset - 1
    if startOffset != characterOffset {
      let text = try anchorNode.getTextContent().jsSlice(startOffset, endOffset)
      if try shouldDeleteExactlyOneCodeUnit(text) {
        if isBackward {
          try focus.set(focus.key, characterOffset, focus.type)
        } else {
          try anchor.set(anchor.key, characterOffset, anchor.type)
        }
      }
    }
  }
}

/// The JS state of `deleteCharacter`'s scan for a block to merge.
private enum MergeState {
  case initial
  case mergeNextBlock(ElementNode)
  case mergeBlock(caret: ChildCaret, block: ElementNode)
}

extension RangeSelection {
  /// Moves or extends the selection by `granularity`. Only the moves across
  /// decorators and blocks are ported; the rest needs text layout.
  public func modify(
    _ alter: SelectionAlter, _ isBackward: Bool, _ granularity: SelectionGranularity
  ) throws {
    if try modifySelectionAroundDecoratorsAndBlocks(self, alter, isBackward, granularity) {
      return
    }
    // JS continues through the DOM selection, which the port does not have.
  }

  /// Whether a forward delete at `anchor` stops because a shadow root
  /// follows.
  public func forwardDeletion(
    _ anchor: Point, _ anchorNode: LexicalNode, _ isBackward: Bool
  ) throws -> Bool {
    if try !isBackward
      && ((anchor.type == .element && anchorNode is ElementNode
        && anchor.offset == (anchorNode as! ElementNode).getChildrenSize())
        || (anchor.type == .text && anchor.offset == anchorNode.getTextContentSize()))
    {
      let parent = try anchorNode.getParent()
      let nextSibling = try anchorNode.getNextSibling() ?? parent?.getNextSibling()
      if let next = nextSibling as? ElementNode, next.isShadowRoot() {
        return true
      }
    }
    return false
  }

  /// Deletes a character backward or forward (or the selected content).
  public func deleteCharacter(_ isBackward: Bool) throws {
    let wasCollapsed = isCollapsed()
    if isCollapsed() {
      let anchor = self.anchor
      var anchorNode: LexicalNode? = try anchor.getNode()
      if try forwardDeletion(anchor, anchorNode!, isBackward) {
        let element = anchorNode as? ElementNode
        let nextSibling = try element?.getNextSibling()
        if try
          !(element != nil && element!.isEmpty()
          && (nextSibling as? ElementNode)?.isShadowRoot() == true)
        {
          return
        }
      }
      let direction: CaretDirection = isBackward ? .previous : .next
      let initialCaret = try caretFromPoint(anchor, direction)
      let initialRange = try extendCaretToRange(initialCaret)
      let (sliceA, sliceB) = try initialRange.getTextSlices()
      if [sliceA, sliceB].allSatisfy({ $0 == nil || $0!.distance == 0 }) {
        if anchor.type == .element {
          let adjacent = try initialCaret.getNodeAtCaret()
          if let adjacent = adjacent as? ElementNode, try needsBlockCursorBeside(adjacent) {
            let container = try adjacent.getParent()
            try adjacent.remove()
            if let restored = try restoreEmptyContainerParagraph(container, adjacent) {
              try restored.selectStart()
            }
            return
          }
        }
        var state = MergeState.initial
        loop: for caret in try initialRange.iterNodeCarets(.shadowRoot) {
          if caret.isChild {
            let origin = caret.origin as! ElementNode
            if try origin.isInline() {
              // Step into inline elements.
            } else if origin.isShadowRoot() {
              if case .mergeBlock = state {
                break loop
              }
              if let anchorOrigin = initialRange.anchor.origin as? ElementNode,
                try anchorOrigin.isEmpty()
              {
                let normCaret = try normalizeCaret(caret)
                try updateRangeSelectionFromCaretRange(self, getCaretRange(normCaret, normCaret))
                try anchorOrigin.remove()
              }
              return
            } else {
              switch state {
              case .mergeNextBlock(let block), .mergeBlock(_, let block):
                state = .mergeBlock(caret: caret, block: block)
              case .initial: break
              }
            }
          } else if case .mergeBlock = state {
            break loop
          } else if caret.isSibling {
            if let origin = caret.origin as? ElementNode {
              if try !origin.isInline() {
                state = .mergeNextBlock(origin)
              } else if try !origin.isParentOf(initialRange.anchor.origin) {
                break loop
              }
              continue
            } else if let decorator = caret.origin as? DecoratorNode {
              if decorator.isIsolated() {
                // An isolated decorator is not deleted by a character.
              } else if case .mergeNextBlock = state,
                try decorator.isKeyboardSelectable() || !decorator.isInline(),
                let anchorOrigin = initialRange.anchor.origin as? ElementNode,
                try anchorOrigin.isEmpty()
              {
                try anchorOrigin.remove()
                let nodeSelection = createNodeSelection()
                nodeSelection.add(decorator.getKey())
                try setSelection(nodeSelection)
              } else {
                let container = try decorator.getParent()
                try decorator.remove()
                if let restored = try restoreEmptyContainerParagraph(container, decorator) {
                  try restored.selectStart()
                }
              }
              return
            } else if caret.origin is LineBreakNode {
              try caret.origin.remove()
              return
            }
            break loop
          }
        }
        if case .mergeBlock(let caret, let block) = state {
          if try !getSlotNames(block).isEmpty {
            return
          }
          let origin = caret.origin as! ElementNode
          if try origin.isEmpty() && !block.isEmpty() && origin.getParent() === block.getParent() {
            try origin.remove(preserveEmptyParent: true)
            return
          }
          try updateRangeSelectionFromCaretRange(
            self,
            getCaretRange(
              try !origin.isEmpty() && block.isEmpty()
                ? rewindSiblingCaret(getSiblingCaret(block, caret.direction))
                : initialRange.anchor,
              caret))
          return try removeText()
        }
        var node: LexicalNode? = try anchor.getNode()
        while let current = node {
          if try getSlotHostKey(current) != nil {
            return
          }
          if let element = current as? ElementNode, element.isShadowRoot() {
            break
          }
          node = try current.getParent()
        }
      }
      let focus = self.focus
      _ = try extendSelectionForDeletion(self, isBackward, .character)
      if !isCollapsed() {
        let focusNode = try focus.type == .text ? focus.getNode() as? TextNode : nil
        anchorNode = try anchor.type == .text ? anchor.getNode() : nil
        if let focusNode, try focusNode.isSegmented() {
          let offset = focus.offset
          let textContentSize = try focusNode.getTextContentSize()
          if focusNode.is(anchorNode) || (isBackward && offset != textContentSize)
            || (!isBackward && offset != 0)
          {
            try removeSegment(focusNode, isBackward, offset)
            return
          }
        } else if let anchorText = anchorNode as? TextNode, try anchorText.isSegmented() {
          let offset = anchor.offset
          let textContentSize = try anchorText.getTextContentSize()
          if anchorText.is(focusNode) || (isBackward && offset != 0)
            || (!isBackward && offset != textContentSize)
          {
            try removeSegment(anchorText, isBackward, offset)
            return
          }
        }
        try updateCaretSelectionForUnicodeCharacter(self, isBackward)
      } else if isBackward && anchor.offset == 0 {
        if try collapseAtStart(self, anchor.getNode()) {
          return
        }
      }
    }
    if !wasCollapsed {
      try INTERNAL_expandSelectionToWholeDocument(self)
    }
    try removeText()
    if isBackward && !wasCollapsed && isCollapsed() && anchor.type == .element
      && anchor.offset == 0
    {
      try ensureRootHasParagraph()
    }
  }

  /// Deletes to the line boundary backward or forward.
  public func deleteLine(_ isBackward: Bool) throws {
    try deleteTextByGranularity(self, isBackward, .lineboundary)
  }

  /// Deletes a word backward or forward.
  public func deleteWord(_ isBackward: Bool) throws {
    try deleteTextByGranularity(self, isBackward, .word)
  }
}
