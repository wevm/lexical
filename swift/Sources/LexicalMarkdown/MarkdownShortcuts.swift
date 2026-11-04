import Lexical
import LexicalCodeCore

// Port of packages/lexical-markdown/src/MarkdownShortcuts.ts.

private let SPACE = UInt16(UInt8(ascii: " "))

private func runElementTransformers(
  _ parentNode: ElementNode, _ anchorNode: TextNode, _ anchorOffset: Int,
  _ elementTransformers: [ElementTransformer], _ triggerOnEnter: Bool = false
) throws -> Bool {
  guard try isBlockContainer(parentNode.getParent()),
    try parentNode.getFirstChild() === anchorNode
  else { return false }
  let textContent = try anchorNode.getTextContent()
  // A caret far from the line start cannot be part of a block-level
  // markdown trigger.
  if !triggerOnEnter && textContent.utf16Unit(at: anchorOffset - 1) != SPACE { return false }

  for transformer in elementTransformers {
    let match = textContent.jsMatch(transformer.regExp)
    let expectedMatchLength =
      triggerOnEnter || (match?.text.jsEndsWith(" ") ?? false) ? anchorOffset : anchorOffset - 1
    if let match, match.length == expectedMatchLength {
      let nextSiblings = try anchorNode.getNextSiblings()
      let parts = try anchorNode.splitText(anchorOffset)
      let siblings = parts.count > 1 ? [parts[1]] + nextSiblings : nextSiblings
      if try transformer.replace(parentNode, siblings, match, false) != false {
        try parts[0].remove()
        return true
      }
    }
  }
  return false
}

private func runMultilineElementTransformers(
  _ parentNode: ElementNode, _ anchorNode: TextNode, _ anchorOffset: Int,
  _ elementTransformers: [MultilineElementTransformer], _ triggerOnEnter: Bool = false
) throws -> Bool {
  guard try isBlockContainer(parentNode.getParent()),
    try parentNode.getFirstChild() === anchorNode
  else { return false }
  let textContent = try anchorNode.getTextContent()
  if !triggerOnEnter && textContent.utf16Unit(at: anchorOffset - 1) != SPACE { return false }

  for transformer in elementTransformers {
    // Only a transformer whose end is optional applies to typing.
    if let regExpEnd = transformer.regExpEnd, !regExpEnd.optional { continue }
    guard let match = textContent.jsMatch(transformer.regExpStart) else { continue }
    let matchLength =
      triggerOnEnter || match.text.jsEndsWith(" ") ? anchorOffset : anchorOffset - 1
    if match.length != matchLength { continue }
    let nextSiblings = try anchorNode.getNextSiblings()
    let parts = try anchorNode.splitText(anchorOffset)
    let siblings = parts.count > 1 ? [parts[1]] + nextSiblings : nextSiblings
    if try transformer.replace(parentNode, siblings, match, nil, nil, false) != false {
      try parts[0].remove()
      return true
    }
  }
  return false
}

private func runTextMatchTransformers(
  _ anchorNode: TextNode, _ anchorOffset: Int,
  _ transformersByTrigger: [String: [TextMatchTransformer]]
) throws -> Bool {
  var textContent = try anchorNode.getTextContent()
  guard let lastChar = textContent.jsCharAt(anchorOffset - 1),
    let transformers = transformersByTrigger[lastChar]
  else { return false }
  // Typing in the middle of the content: match up to the caret.
  if anchorOffset < textContent.jsLength {
    textContent = textContent.jsSlice(0, anchorOffset)
  }
  for transformer in transformers {
    guard let replace = transformer.replace,
      let match = textContent.jsMatch(transformer.regExp)
    else { continue }
    let startIndex = match.index
    let endIndex = startIndex + match.length
    let replaceNode =
      startIndex == 0
      ? try anchorNode.splitText(endIndex)[0] : try anchorNode.splitText(startIndex, endIndex)[1]
    try replaceNode.selectNext(0, 0)
    _ = try replace(replaceNode, match)
    return true
  }
  return false
}

private func runTextFormatTransformers(
  _ anchorNode: TextNode, _ anchorOffset: Int,
  _ textFormatTransformers: [String: [TextFormatTransformer]]
) throws -> Bool {
  let textContent = try anchorNode.getTextContent()
  let closeTagEndIndex = anchorOffset - 1
  // Quick check whether this may be the end of an inline markdown style.
  guard let closeChar = textContent.jsCharAt(closeTagEndIndex),
    let matchers = textFormatTransformers[closeChar]
  else { return false }

  for matcher in matchers {
    let tag = matcher.tag
    let tagLength = tag.jsLength
    let closeTagStartIndex = closeTagEndIndex - tagLength + 1

    // A tag of more than one character must match the text before.
    if tagLength > 1
      && !isEqualSubString(textContent, closeTagStartIndex, tag, 0, tagLength)
    {
      continue
    }
    // A space before the closing tag cancels inline markdown.
    if textContent.utf16Unit(at: closeTagStartIndex - 1) == SPACE { continue }
    // Some tags cannot be used within words, so they need a newline, space
    // or punctuation after them.
    let afterCloseTagChar = characterAt(textContent, closeTagEndIndex + 1)
    if matcher.intraword == false, let afterCloseTagChar, !afterCloseTagChar.isEmpty,
      !isPunctuationOrSpace(afterCloseTagChar)
    {
      continue
    }

    let closeNode = anchorNode
    var openNode = closeNode
    var openTagStartIndex = getOpenTagStartIndex(textContent, closeTagStartIndex, tag)

    // Search the preceding text siblings for the opening tag when it is not
    // in the node of the closing tag.
    var sibling: LexicalNode? = openNode
    while openTagStartIndex < 0, let previous = try sibling?.getPreviousSibling() {
      sibling = previous
      if previous is LineBreakNode { break }
      if let text = previous as? TextNode {
        if try text.hasFormat("code") { continue }
        let siblingTextContent = try text.getTextContent()
        openNode = text
        openTagStartIndex = getOpenTagStartIndex(
          siblingTextContent, siblingTextContent.jsLength, tag)
      }
    }
    // No opening tag.
    if openTagStartIndex < 0 { continue }
    // No content between the opening and closing tags.
    if openNode === closeNode && openTagStartIndex + tagLength == closeTagStartIndex { continue }

    // A longer tag of repeating characters (`***` against `**`).
    let prevOpenNodeText = try openNode.getTextContent()
    if openTagStartIndex > 0 && prevOpenNodeText.jsCharAt(openTagStartIndex - 1) == closeChar {
      continue
    }
    // Some tags cannot be used within words, so they need a newline, space
    // or punctuation before them.
    let beforeOpenTagChar = characterBefore(prevOpenNodeText, openTagStartIndex)
    if matcher.intraword == false, let beforeOpenTagChar, !beforeOpenTagChar.isEmpty,
      !isPunctuationOrSpace(beforeOpenTagChar)
    {
      continue
    }
    // Per CommonMark, code spans take precedence over other inline
    // formatting.
    if !matcher.format.contains("code"),
      try isInsideUnclosedCodeSpan(openNode, openTagStartIndex)
    {
      continue
    }

    // Remove the tags, the closing one first so that the offsets of the
    // opening one do not shift.
    let prevCloseNodeText = try closeNode.getTextContent()
    let closeNodeText =
      prevCloseNodeText.jsSlice(0, closeTagStartIndex)
      + prevCloseNodeText.jsSlice(closeTagEndIndex + 1)
    try closeNode.setTextContent(closeNodeText)
    let openNodeText = openNode === closeNode ? closeNodeText : prevOpenNodeText
    try openNode.setTextContent(
      openNodeText.jsSlice(0, openTagStartIndex)
        + openNodeText.jsSlice(openTagStartIndex + tagLength))
    let selection = try getSelection()
    let nextSelection = createRangeSelection()
    try setSelection(nextSelection)
    // Adjust the offset for the removed characters.
    let newOffset = closeTagEndIndex - tagLength * (openNode === closeNode ? 2 : 1) + 1
    try nextSelection.anchor.set(openNode.__key, openTagStartIndex, .text)
    try nextSelection.focus.set(closeNode.__key, newOffset, .text)

    // Apply the formats to the selected text.
    for format in matcher.format {
      try nextSelection.formatText(format, TEXT_TYPE_TO_FORMAT[format])
    }
    // Collapse the selection to the focus.
    try nextSelection.anchor.set(
      nextSelection.focus.key, nextSelection.focus.offset, nextSelection.focus.type)
    // Clear the formats from the collapsed selection.
    for format in matcher.format where nextSelection.hasFormat(format) {
      nextSelection.toggleFormat(format)
    }
    if let selection = selection as? RangeSelection {
      nextSelection.format = selection.format
    }
    return true
  }
  return false
}

private let BACKTICK = UInt16(UInt8(ascii: "`"))

/// Per CommonMark, code spans take precedence over other inline
/// formatting: whether an unclosed backtick precedes `offset`, which puts
/// it inside a code span that is not closed yet.
private func isInsideUnclosedCodeSpan(_ node: TextNode, _ offset: Int) throws -> Bool {
  let text = Array(try node.getTextContent().utf16)
  let backtickCount = text.prefix(max(0, offset)).filter { $0 == BACKTICK }.count
  return backtickCount % 2 != 0
}

private func getOpenTagStartIndex(_ string: String, _ maxIndex: Int, _ tag: String) -> Int {
  let tagLength = tag.jsLength
  var i = maxIndex
  while i >= tagLength {
    let startIndex = i - tagLength
    // A space after the opening tag cancels the transformation.
    if isEqualSubString(string, startIndex, tag, 0, tagLength)
      && string.utf16Unit(at: startIndex + tagLength) != SPACE
    {
      return startIndex
    }
    i -= 1
  }
  return -1
}

/// The offset of `point` in characters from the start of its parent
/// element rather than of its own node. Node transforms (hashtags,
/// autolinks…) can split or merge the leaves around the caret in the same
/// update, so the "one character typed" heuristic compares these (#5366).
private func getOffsetInParent(_ point: Point) throws -> Int {
  let node = try point.getNode()
  var offset = 0
  if node is TextNode {
    offset = point.offset
  } else if let element = node as? ElementNode {
    for child in try element.getChildren().prefix(point.offset) {
      offset += try child.getTextContentSize()
    }
  }
  var sibling = try node.getPreviousSibling()
  while let current = sibling {
    offset += try current.getTextContentSize()
    sibling = try current.getPreviousSibling()
  }
  return offset
}

private func isEqualSubString(
  _ stringA: String, _ aStart: Int, _ stringB: String, _ bStart: Int, _ length: Int
) -> Bool {
  for i in 0..<max(0, length)
  where stringA.utf16Unit(at: aStart + i) != stringB.utf16Unit(at: bStart + i) {
    return false
  }
  return true
}

/// `indexBy(list, callback)`: the items by the key `callback` gives, in
/// order, leaving out those without one.
func indexBy<T>(_ list: [T], _ callback: (T) -> String?) -> [String: [T]] {
  var index: [String: [T]] = [:]
  for item in list {
    if let key = callback(item), !key.isEmpty { index[key, default: []].append(item) }
  }
  return index
}

/// `registerMarkdownShortcuts(editor, transformers)`: runs the
/// transformers as the user types (a block's leading markdown and a space,
/// a closing tag, a text match's trigger) and, for the multiline element
/// transformers and the element transformers that set `triggerOnEnter`, on
/// Enter at the end of a line.
@discardableResult
public func registerMarkdownShortcuts(
  _ editor: Editor, _ transformers: [any Transformer] = TRANSFORMERS
) throws -> () -> Void {
  let byType = transformersByType(transformers)
  let elementTransformersForEnter = byType.element.filter(\.triggerOnEnter)
  let textFormatTransformersByTrigger = indexBy(byType.textFormat) { $0.tag.jsSlice(-1) }
  let textMatchTransformersByTrigger = indexBy(byType.textMatch) { $0.trigger }
  // Composition end fires per IME commit, so the transformers run only when
  // the committed character can close a trigger: a space for the element
  // triggers, and the text format and text match triggers.
  var compositionEndTriggerChars: Set<String> = [" "]
  for t in byType.textFormat { compositionEndTriggerChars.insert(t.tag.jsSlice(-1)) }
  for t in byType.textMatch {
    if let trigger = t.trigger { compositionEndTriggerChars.insert(trigger) }
  }

  for transformer in transformers {
    let dependencies: [LexicalNode.Type]
    switch transformer {
    case let t as ElementTransformer: dependencies = t.dependencies
    case let t as TextMatchTransformer: dependencies = t.dependencies
    case let t as MultilineElementTransformer: dependencies = t.dependencies
    default: dependencies = []
    }
    for node in dependencies where !editor.hasNode(node) {
      throw LexicalError(
        "MarkdownShortcuts: missing dependency \(node.getType()) for transformer. Ensure node dependency is included in editor initial config."
      )
    }
  }

  func transform(
    _ parentNode: ElementNode, _ anchorNode: TextNode, _ anchorOffset: Int
  ) throws
    -> Bool
  {
    try runElementTransformers(parentNode, anchorNode, anchorOffset, byType.element)
      || runMultilineElementTransformers(
        parentNode, anchorNode, anchorOffset, byType.multilineElement)
      || runTextMatchTransformers(anchorNode, anchorOffset, textMatchTransformersByTrigger)
      || runTextFormatTransformers(anchorNode, anchorOffset, textFormatTransformersByTrigger)
  }

  return mergeRegister(
    editor.registerUpdateListener { payload in
      // Collaboration and undo/redo changes are already transformed.
      if payload.tags.has(COLLABORATION_TAG) || payload.tags.has(HISTORIC_TAG) { return }
      // While composing (e.g. backticks), wait until the user commits.
      if editor.isComposing() { return }
      // A composition end commits the text without moving the selection,
      // and may move the anchor by more than one, so the typed-character
      // heuristics are skipped for it.
      let isCompositionEnd = payload.tags.has(COMPOSITION_END_TAG)
      let editorState = payload.editorState
      let prevEditorState = payload.prevEditorState
      let selection = try editorState.read { try getSelection() }
      let prevSelection = try prevEditorState.read { try getSelection() }
      // Only a collapsed range that moved: the user is typing.
      guard let prevSelection = prevSelection as? RangeSelection,
        let selection = selection as? RangeSelection, selection.isCollapsed(),
        !selection.is(prevSelection) || isCompositionEnd
      else { return }
      let anchorKey = selection.anchor.key
      let anchorOffset = selection.anchor.offset
      guard let anchorNode = editorState._nodeMap.get(anchorKey) as? TextNode,
        payload.dirtyLeaves.has(anchorKey)
      else { return }
      if !isCompositionEnd && anchorOffset != 1 {
        let offset = try editorState.read { try getOffsetInParent(selection.anchor) }
        let prevOffset = try prevEditorState.read { try getOffsetInParent(prevSelection.anchor) }
        if offset > prevOffset + 1 { return }
      }
      if isCompositionEnd {
        let closeChar = try editorState.read { try anchorNode.getTextContent() }.jsCharAt(
          anchorOffset - 1)
        guard let closeChar, compositionEndTriggerChars.contains(closeChar) else { return }
      }
      try editor.update {
        guard canContainTransformableMarkdown(anchorNode),
          let parentNode = try anchorNode.getParent(), !(parentNode is CodeNode)
        else { return }
        if try transform(parentNode, anchorNode, selection.anchor.offset) {
          try addUpdateTag(HISTORY_PUSH_TAG)
        }
      }
    },
    editor.registerCommand(
      KEY_ENTER_COMMAND,
      { event, _ in
        if event?.shiftKey == true { return false }
        guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed()
        else { return false }
        let anchorOffset = selection.anchor.offset
        guard let anchorNode = try selection.anchor.getNode() as? TextNode,
          canContainTransformableMarkdown(anchorNode),
          let parentNode = try anchorNode.getParent(), !(parentNode is CodeNode),
          try anchorOffset == anchorNode.getTextContent().jsLength
        else { return false }
        if try runMultilineElementTransformers(
          parentNode, anchorNode, anchorOffset, byType.multilineElement, true)
          || runElementTransformers(
            parentNode, anchorNode, anchorOffset, elementTransformersForEnter, true)
        {
          event?.preventDefault()
          return true
        }
        return false
      }, COMMAND_PRIORITY_LOW))
}
