import Lexical
import LexicalExtension
import LexicalSelection
import LexicalUtils

// Port of registerRichText and its helpers from
// packages/lexical-rich-text/src/index.ts.
//
// Not ported yet: the copy, cut, paste, drop and drag handlers, which need
// @lexical/clipboard; they arrive with LexicalClipboard. Two branches are
// left out by design (DESIGN.md, "Rich text"): the `IS_IOS`/`IS_SAFARI`
// `beforeinput` fall-throughs of Backspace and Enter, because native views
// have no `beforeinput` to fall through to, and inline-grid line
// navigation, which needs CSS grid layout.

/// `DRAG_DROP_PASTE`: files dropped or pasted into the editor.
public let DRAG_DROP_PASTE = LexicalCommand<[DataTransferFile]>("DRAG_DROP_PASTE_FILE")

/// `EscapeFormatTrigger`: the interactions that can escape a format.
public enum EscapeFormatTrigger: String, Sendable {
  case enter, click, arrow, space, tab
}

/// `TriggerConfig`: the triggers that escape one format. A trigger set to
/// `true` escapes the format; `nil` is an absent key. With
/// `onlyAtBoundary`, the format is escaped only at the start or end of a
/// formatted text node with no sibling in that direction.
public struct TriggerConfig: Equatable, Sendable {
  public var enter: Bool?
  public var click: Bool?
  public var arrow: Bool?
  public var space: Bool?
  public var tab: Bool?
  public var onlyAtBoundary: Bool?

  public init(
    enter: Bool? = nil, click: Bool? = nil, arrow: Bool? = nil, space: Bool? = nil,
    tab: Bool? = nil, onlyAtBoundary: Bool? = nil
  ) {
    self.enter = enter
    self.click = click
    self.arrow = arrow
    self.space = space
    self.tab = tab
    self.onlyAtBoundary = onlyAtBoundary
  }

  public subscript(trigger: EscapeFormatTrigger) -> Bool? {
    switch trigger {
    case .enter: enter
    case .click: click
    case .arrow: arrow
    case .space: space
    case .tab: tab
    }
  }

  /// `{...self, ...override}`.
  func merged(with override: TriggerConfig) -> TriggerConfig {
    TriggerConfig(
      enter: override.enter ?? enter, click: override.click ?? click,
      arrow: override.arrow ?? arrow, space: override.space ?? space, tab: override.tab ?? tab,
      onlyAtBoundary: override.onlyAtBoundary ?? onlyAtBoundary)
  }
}

/// `EscapeFormatTriggerConfig`: the triggers of each format. A format
/// mapped to `nil` (JS `null`, as in `["uppercase": nil]`) has escape
/// explicitly disabled, which matters when overriding the defaults.
public typealias EscapeFormatTriggerConfig = [TextFormatType: TriggerConfig?]

/// `DEFAULT_ESCAPE_FORMAT_TRIGGERS`.
public let DEFAULT_ESCAPE_FORMAT_TRIGGERS: EscapeFormatTriggerConfig = [
  "capitalize": TriggerConfig(enter: true, space: true, tab: true),
  "lowercase": TriggerConfig(enter: true, space: true, tab: true),
  "uppercase": TriggerConfig(enter: true, space: true, tab: true),
]

/// `ShouldHandlePasteAsFiles`: whether a paste carrying files is handled by
/// dispatching `DRAG_DROP_PASTE` with them, rather than as HTML.
public typealias ShouldHandlePasteAsFiles = (_ files: [DataTransferFile], _ hasTextContent: Bool)
  -> Bool

/// `defaultShouldHandlePasteAsFiles`: files are handled only when the
/// clipboard carries no text content.
public func defaultShouldHandlePasteAsFiles(
  _ files: [DataTransferFile], _ hasTextContent: Bool
)
  -> Bool
{
  !files.isEmpty && !hasTextContent
}

private enum EscapeDirection { case start, end, both }

private func isSelectionAtEndOfRoot(_ selection: RangeSelection) throws -> Bool {
  let focus = selection.focus
  return try focus.key == "root" && focus.offset == getRoot().getChildrenSize()
}

private func isSelectionAtStartOfRoot(_ selection: RangeSelection) -> Bool {
  selection.focus.key == "root" && selection.focus.offset == 0
}

/// `$isBlockCursorAtRootEdge`: a collapsed element point at the start or
/// end of the root beside a child that renders a block cursor, where
/// horizontal arrows have nowhere to go.
private func isBlockCursorAtRootEdge(
  _ selection: RangeSelection, _ direction: CaretDirection
)
  throws -> Bool
{
  guard selection.isCollapsed() else { return false }
  let isNext = direction == .next
  guard try isNext ? isSelectionAtEndOfRoot(selection) : isSelectionAtStartOfRoot(selection)
  else { return false }
  let offset = selection.focus.offset
  return try needsBlockCursorBeside(getRoot().getChildAtIndex(isNext ? offset - 1 : offset))
}

private func isSelectionCollapsedAtFrontOfIndentedBlock(
  _ selection: RangeSelection
) throws
  -> Bool
{
  guard selection.isCollapsed(), selection.anchor.offset == 0 else { return false }
  let anchorNode = try selection.anchor.getNode()
  if anchorNode is RootNode { return false }
  let element = try getNearestBlockElementAncestorOrThrow(anchorNode)
  return try element.getIndent() > 0
    && (element.is(anchorNode) || anchorNode.is(element.getFirstDescendant()))
}

private func escapeFormatsForTrigger(
  _ selection: RangeSelection, _ trigger: EscapeFormatTrigger, _ direction: EscapeDirection,
  _ config: EscapeFormatTriggerConfig
) throws {
  var isBoundary = false
  var anchorNode: TextNode? = nil
  if selection.isCollapsed(), selection.anchor.type == .text,
    let node = try selection.anchor.getNode() as? TextNode
  {
    anchorNode = node
    let offset = selection.anchor.offset
    let atEnd = try offset == node.getTextContentSize() && node.getNextSibling() == nil
    let atStart = try offset == 0 && node.getPreviousSibling() == nil
    switch direction {
    case .end: isBoundary = atEnd
    case .start: isBoundary = atStart
    case .both: isBoundary = atEnd || atStart
    }
  }
  var didEscapeBoundary = false
  for (format, triggers) in config {
    guard let triggers = triggers ?? nil, triggers[trigger] == true else { continue }
    if triggers.onlyAtBoundary == true {
      guard isBoundary, let anchorNode, try anchorNode.hasFormat(format) else { continue }
      didEscapeBoundary = true
    }
    if selection.hasFormat(format) {
      selection.toggleFormat(format)
    }
  }
  if didEscapeBoundary {
    selection.setStyle("")
  }
}

private func tryEnterFromBlockCursor(
  _ selection: RangeSelection, _ direction: CaretDirection
)
  throws -> Bool
{
  guard selection.isCollapsed(), selection.anchor.type == .element else { return false }
  let caret = try caretFromPoint(selection.anchor, direction)
  if let child = try caret.getNodeAtCaret() as? ElementNode, isShadowRootNode(child),
    try !child.isInline()
  {
    try setSelectionFromCaretRange(
      getCollapsedCaretRange(normalizeCaret(getChildCaret(child, direction))))
    return true
  }
  return false
}

private func tryBlockCursorShadowRootNavigation(
  _ selection: RangeSelection, _ direction: CaretDirection
) throws -> Bool {
  try tryExitShadowRootToBlockCursor(selection, direction)
    || tryEnterFromBlockCursor(selection, direction)
}

private func tryExitShadowRootToBlockCursor(
  _ selection: RangeSelection, _ direction: CaretDirection
) throws -> Bool {
  guard selection.isCollapsed() else { return false }
  let focusCaret = try caretFromPoint(selection.focus, direction)
  // Walk up from focus to find the nearest shadow root ancestor, starting
  // from the focus node itself since it may be a shadow root.
  guard let shadowRoot = try findMatchingParent(focusCaret.origin, { isShadowRootNode($0) })
  else { return false }
  // An element point's caret origin is a child of the focus node, which may
  // be (or sit in) a shadow root the selection isn't inside.
  let focusNode = try selection.focus.getNode()
  if try !shadowRoot.is(focusNode) && !hasAncestor(focusNode, shadowRoot) {
    return false
  }
  // The focus has to be at the edge of the shadow root in this direction.
  let textRange = try getCaretRange(focusCaret, getSiblingCaret(shadowRoot, direction))
  let slices = try textRange.getTextSlices()
  if [slices.0, slices.1].contains(where: { ($0?.getTextContentSize() ?? 0) > 0 }) {
    return false
  }
  // Normalize the anchor in case it started with a ChildCaret.
  let range = try getCaretRange(textRange.anchor.getSiblingCaret(), textRange.focus)
  // From the edge to the shadow root, only parent nodes are expected, as
  // sibling carets.
  var prevOrigin = range.anchor.origin
  for caret in try range.iterNodeCarets() {
    guard try caret.isSibling && caret.origin.is(prevOrigin.getParent()) else { return false }
    prevOrigin = caret.origin
  }
  // Walk outward through nested shadow roots until a sibling needs a block
  // cursor beside it.
  var prevShadow = shadowRoot
  for caret in try extendCaretToRange(getSiblingCaret(shadowRoot, direction)).iterNodeCarets() {
    // A caret whose origin is the parent of the current level is the
    // "leave" step walking outward; keep going only through shadow roots.
    if try caret.origin.is(prevShadow.getParent()) {
      if !isShadowRootNode(caret.origin) { break }
      prevShadow = caret.origin
      continue
    }
    if try needsBlockCursorBeside(caret.origin) {
      let sr = getSiblingCaret(prevShadow, direction)
      try setSelectionFromCaretRange(getCaretRange(sr, sr))
      return true
    }
    break
  }
  return false
}

private func isSelectableBlockDecorator(_ node: LexicalNode?) throws -> Bool {
  guard let decorator = node as? DecoratorNode else { return false }
  return try !decorator.isInline() && !decorator.isIsolated() && decorator.isKeyboardSelectable()
}

private func selectNode(_ key: NodeKey) throws {
  let nodeSelection = createNodeSelection()
  nodeSelection.add(key)
  try setSelection(nodeSelection)
}

/// `$tryDecoratorLineNavigation`: an up or down arrow that leaves the top
/// block beside a selectable block decorator selects the decorator. JS
/// probes `Selection.modify('move', …, 'line')`; Swift asks
/// `Editor.selectionMovement`, and, like JS without a DOM selection, does
/// nothing without one.
private func tryDecoratorLineNavigation(
  _ selection: RangeSelection, _ isBackward: Bool
) throws
  -> Bool
{
  guard selection.isCollapsed() else { return false }
  let focus = selection.focus
  let focusNode = try focus.getNode()
  let direction: CaretDirection = isBackward ? .previous : .next
  let focusCaret = try caretFromPoint(focus, direction)
  if focus.type == .element, focusNode is ElementNode, try isRootOrShadowRoot(focusNode) {
    if let adjacentChild = try focusCaret.getNodeAtCaret(),
      try isSelectableBlockDecorator(adjacentChild)
    {
      try selectNode(adjacentChild.getKey())
      return true
    }
    return false
  }
  let start = try focusNode as? ElementNode ?? focusNode.getParentOrThrow()
  guard
    let topBlock = try findMatchingParent(
      start,
      { node in
        guard let element = node as? ElementNode else { return false }
        return try !element.isInline() && isRootOrShadowRoot(element.getParent())
      })
  else { return false }
  guard let adjacentSibling = try getSiblingCaret(topBlock, direction).getNodeAtCaret(),
    try isSelectableBlockDecorator(adjacentSibling)
  else { return false }
  // Empty blocks always escape on line-move, so skip the layout probe.
  if try topBlock.getTextContentSize() == 0 {
    try selectNode(adjacentSibling.getKey())
    return true
  }
  guard let movement = try getActiveEditor().selectionMovement,
    let (key, offset, _) = try movement.landedPoint(
      from: focus, isBackward: isBackward, granularity: .line)
  else { return false }
  guard let movedNode = try getNodeByKey(key) else { return false }
  // No movement means the line move hit the block edge.
  if key == focus.key && offset == focus.offset {
    try selectNode(adjacentSibling.getKey())
    return true
  }
  if try movedNode.is(topBlock) || hasAncestor(movedNode, topBlock) {
    return false
  }
  try selectNode(adjacentSibling.getKey())
  return true
}

private func exitNodeSelectionToward(_ node: LexicalNode, _ direction: CaretDirection) throws {
  let caret = getSiblingCaret(node, direction)
  if let sibling = try caret.getAdjacentCaret(), let element = sibling.origin as? ElementNode,
    try !element.isInline(), element.isShadowRoot()
  {
    try setSelectionFromCaretRange(getCollapsedCaretRange(caret))
  } else if direction == .next {
    try node.selectNext(0, 0)
  } else {
    try node.selectPrevious()
  }
}

/// `$convertContiguousNodeSelection`: a NodeSelection of contiguous
/// siblings becomes the RangeSelection that covers them.
private func convertContiguousNodeSelection(
  _ selection: NodeSelection, _ direction: CaretDirection
) throws -> Bool {
  let carets = try sortedSiblingCarets(selection.getNodes())
  guard let firstCaret = carets.first, let lastCaret = carets.last else { return false }
  for i in 0..<(carets.count - 1) {
    guard try carets[i + 1].origin.is(carets[i].getNodeAtCaret()) else { return false }
  }
  try setSelectionFromCaretRange(
    getCaretRangeInDirection(getCaretRange(rewindSiblingCaret(firstCaret), lastCaret), direction))
  // The vertical arrow handlers leave the extension to the native default
  // action, which reads the committed selection.
  try flushSyncAfterUpdate()
  return true
}

private func sortedSiblingCarets(_ nodes: [LexicalNode]) throws -> [SiblingCaret] {
  let carets = nodes.map { getSiblingCaret($0, .next) }
  // `Array.prototype.sort` with a comparator that throws: compare first,
  // then sort by the results.
  var error: Error?
  let sorted = carets.sorted { a, b in
    do { return try comparePointCaretNext(a, b) < 0 } catch let caught {
      if error == nil { error = caught }
      return false
    }
  }
  if let error { throw error }
  return sorted
}

/// `$promoteNodeSelectionToBlockEdge`: MOVE_TO_START or MOVE_TO_END with a
/// NodeSelection collapses to the edge of the surrounding block.
private func promoteNodeSelectionToBlockEdge(
  _ selection: NodeSelection, _ isBackward: Bool, _ event: KeyboardEvent
) throws -> Bool {
  event.preventDefault()
  event.stopPropagation()
  let nodes = try selection.getNodes()
  if nodes.isEmpty { return true }
  let sorted = try sortedSiblingCarets(nodes)
  let targetNode = (isBackward ? sorted.first! : sorted.last!).origin
  let block =
    try findMatchingParent(
      targetNode,
      { node in
        guard let element = node as? ElementNode, node !== targetNode else { return false }
        return try !element.isInline()
      }) as? ElementNode ?? getRoot()
  let offset = try isBackward ? 0 : block.getChildrenSize()
  try block.select(offset, offset)
  return true
}

private func nearestBlock(_ node: LexicalNode) throws -> ElementNode? {
  try findMatchingParent(node) { node in
    guard let element = node as? ElementNode else { return false }
    return try !element.isInline()
  } as? ElementNode
}

/// `registerRichText(editor, escapeFormatTriggers, shouldHandlePasteAsFiles)`.
public func registerRichText(
  _ editor: Editor,
  _ escapeFormatTriggers: ReadonlySignal<EscapeFormatTriggerConfig> = signal(
    DEFAULT_ESCAPE_FORMAT_TRIGGERS),
  _ shouldHandlePasteAsFiles: ReadonlySignal<ShouldHandlePasteAsFiles> = signal(
    defaultShouldHandlePasteAsFiles)
) -> () -> Void {
  func arrowNodeSelection(
    _ event: KeyboardEvent, _ direction: (LexicalNode) throws -> CaretDirection
  ) throws -> (handled: Bool, selection: BaseSelection?) {
    var selection = try getSelection()
    if let nodeSelection = selection as? NodeSelection {
      // If selection is on a node, try to move it back to a range.
      let nodes = try nodeSelection.getNodes()
      if let first = nodes.first {
        let direction = try direction(first)
        if try event.shiftKey && convertContiguousNodeSelection(nodeSelection, direction) {
          // Fall through.
          selection = try getSelection()
        } else {
          event.preventDefault()
          try exitNodeSelectionToward(first, direction)
          return (true, selection)
        }
      }
    }
    return (false, selection)
  }

  return mergeRegister([
    editor.registerCommand(
      CLICK_COMMAND,
      { event, _ in
        let selection = try getSelection()
        if let selection = selection as? NodeSelection {
          // A click on an already-selected node is an interaction with that
          // node, not a deselect gesture (meta/lexical#8907).
          if let targetKey = event.targetKey, let target = try getNodeByKey(targetKey) {
            for node in try selection.getNodes() {
              if try node.is(target) || hasAncestor(target, node) {
                return false
              }
            }
          }
          selection.clear()
          return true
        }
        if let selection = selection as? RangeSelection {
          try escapeFormatsForTrigger(selection, .click, .both, escapeFormatTriggers.peek())
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_CHARACTER_COMMAND,
      { isBackward, _ in
        let selection = try getSelection()
        if let selection = selection as? RangeSelection {
          try selection.deleteCharacter(isBackward)
          return true
        } else if let selection = selection as? NodeSelection {
          try selection.deleteNodes()
          return true
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_WORD_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.deleteWord(isBackward)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      DELETE_LINE_COMMAND,
      { isBackward, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.deleteLine(isBackward)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { text, _ in
        if let selection = try getSelection() {
          try selection.insertText(text)
        }
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      REMOVE_TEXT_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.removeText()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      FORMAT_TEXT_COMMAND,
      { format, _ in
        let selection = try getSelection()
        guard selection is RangeSelection || selection is NodeSelection else { return false }
        try formatText(selection!, format)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      SET_TEXT_FORMAT_COMMAND,
      { formats, _ in
        let selection = try getSelection()
        guard selection is RangeSelection || selection is NodeSelection else { return false }
        try setTextFormat(selection!, formats)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      FORMAT_ELEMENT_COMMAND,
      { format, _ in
        let selection = try getSelection()
        guard selection is RangeSelection || selection is NodeSelection else { return false }
        for node in try selection!.getNodes() {
          try nearestBlock(node)?.setFormat(format)
        }
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INSERT_LINE_BREAK_COMMAND,
      { selectStart, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.insertLineBreak(selectStart)
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INSERT_PARAGRAPH_COMMAND,
      { _, _ in
        guard let selection = try getSelection() as? RangeSelection else { return false }
        try selection.insertParagraph()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INSERT_TAB_COMMAND,
      { _, _ in
        let tabNode = try createTabNode()
        if let selection = try getSelection() as? RangeSelection {
          try tabNode.setFormat(selection.format)
          try tabNode.setStyle(selection.style)
        }
        try insertNodes([tabNode])
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INDENT_CONTENT_COMMAND,
      { _, _ in
        try handleIndentAndOutdent { block in try block.setIndent(block.getIndent() + 1) }
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      OUTDENT_CONTENT_COMMAND,
      { _, _ in
        try handleIndentAndOutdent { block in
          let indent = try block.getIndent()
          if indent > 0 {
            try block.setIndent(max(0, indent - 1))
          }
        }
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_UP_COMMAND,
      { event, _ in
        let (handled, selection) = try arrowNodeSelection(event) { _ in .previous }
        if handled { return true }
        if let selection = selection as? RangeSelection {
          if isSelectionAtStartOfRoot(selection) {
            event.preventDefault()
            return true
          }
          if try !event.shiftKey && tryBlockCursorShadowRootNavigation(selection, .previous) {
            event.preventDefault()
            return true
          }
          if try !event.shiftKey && tryDecoratorLineNavigation(selection, true) {
            event.preventDefault()
            return true
          }
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_DOWN_COMMAND,
      { event, _ in
        let (handled, selection) = try arrowNodeSelection(event) { _ in .next }
        if handled { return true }
        if let selection = selection as? RangeSelection {
          if try isSelectionAtEndOfRoot(selection) {
            event.preventDefault()
            return true
          }
          if try !event.shiftKey && tryBlockCursorShadowRootNavigation(selection, .next) {
            event.preventDefault()
            return true
          }
          if try !event.shiftKey && tryDecoratorLineNavigation(selection, false) {
            event.preventDefault()
            return true
          }
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_LEFT_COMMAND,
      { event, _ in
        let (handled, selection) = try arrowNodeSelection(event) { node in
          try isParentRTL(node) ? .next : .previous
        }
        if handled { return true }
        guard let selection = selection as? RangeSelection else { return false }
        let leftDirection: CaretDirection =
          try isParentRTL(selection.anchor.getNode()) ? .next : .previous
        if try isBlockCursorAtRootEdge(selection, leftDirection) {
          event.preventDefault()
          return true
        }
        if try !event.shiftKey && tryBlockCursorShadowRootNavigation(selection, leftDirection) {
          event.preventDefault()
          return true
        }
        if !event.shiftKey {
          try escapeFormatsForTrigger(selection, .arrow, .start, escapeFormatTriggers.peek())
        }
        if try shouldOverrideDefaultCharacterSelection(selection, true) {
          event.preventDefault()
          try moveCharacter(selection, event.shiftKey, true)
          return true
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ARROW_RIGHT_COMMAND,
      { event, _ in
        let (handled, selection) = try arrowNodeSelection(event) { node in
          try isParentRTL(node) ? .previous : .next
        }
        if handled { return true }
        guard let selection = selection as? RangeSelection else { return false }
        let rightDirection: CaretDirection =
          try isParentRTL(selection.anchor.getNode()) ? .previous : .next
        if try isBlockCursorAtRootEdge(selection, rightDirection) {
          event.preventDefault()
          return true
        }
        if try !event.shiftKey && tryBlockCursorShadowRootNavigation(selection, rightDirection) {
          event.preventDefault()
          return true
        }
        if !event.shiftKey {
          try escapeFormatsForTrigger(selection, .arrow, .end, escapeFormatTriggers.peek())
        }
        if try shouldOverrideDefaultCharacterSelection(selection, false) {
          event.preventDefault()
          try moveCharacter(selection, event.shiftKey, false)
          return true
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_BACKSPACE_COMMAND,
      { event, editor in
        // Native views send key events to the editor itself, never to a
        // control inside a decorator, so JS's decorator pass-through has no
        // equivalent here.
        let selection = try getSelection()
        if let selection = selection as? RangeSelection {
          if try isSelectionCollapsedAtFrontOfIndentedBlock(selection) {
            event.preventDefault()
            return try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())
          }
        } else if !(selection is NodeSelection) {
          return false
        }
        event.preventDefault()
        return try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_DELETE_COMMAND,
      { event, editor in
        let selection = try getSelection()
        guard selection is RangeSelection || selection is NodeSelection else { return false }
        event.preventDefault()
        return try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, false)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ENTER_COMMAND,
      { event, editor in
        var selection = try getSelection()
        // A block decorator selected as a NodeSelection leaves Enter nothing
        // to act on, so select past it and insert a paragraph there.
        if let nodeSelection = selection as? NodeSelection {
          let nodes = try nodeSelection.getNodes()
          if nodes.count == 1, let decorator = nodes[0] as? DecoratorNode,
            try !decorator.isInline()
          {
            selection = try decorator.selectNext()
          }
        }
        guard let selection = selection as? RangeSelection else { return false }
        try escapeFormatsForTrigger(selection, .enter, .both, escapeFormatTriggers.peek())
        if let event {
          event.preventDefault()
          if event.shiftKey {
            return try editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, false)
          }
        }
        return try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND, ())
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_ESCAPE_COMMAND,
      { _, editor in
        guard try getSelection() is RangeSelection else { return false }
        editor.blur()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      SELECT_ALL_COMMAND,
      { _, _ in
        // Scope select-all to the slot frame the caret is in: slots are
        // isolated, so a whole-document select-all would escape the slot.
        let selection = try getSelection() as? RangeSelection
        if let selection, try getSlotFrame(selection.anchor.getNode()) != nil {
          try selectAll(selection)
        } else {
          try selectAll(nil)
        }
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_SPACE_COMMAND,
      { _, _ in
        if let selection = try getSelection() as? RangeSelection {
          try escapeFormatsForTrigger(selection, .space, .both, escapeFormatTriggers.peek())
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      KEY_TAB_COMMAND,
      { _, _ in
        if let selection = try getSelection() as? RangeSelection {
          try escapeFormatsForTrigger(selection, .tab, .both, escapeFormatTriggers.peek())
        }
        return false
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      MOVE_TO_END,
      { event, _ in
        let selection = try getSelection()
        if let selection = selection as? NodeSelection {
          return try promoteNodeSelectionToBlockEdge(selection, false, event)
        }
        guard let selection = selection as? RangeSelection else { return false }
        let anchor = selection.anchor
        guard anchor.type == .element, anchor.offset == 0,
          let element = try anchor.getNode() as? ElementNode,
          let firstChild = try element.getFirstChild() as? DecoratorNode, try firstChild.isInline()
        else { return false }
        // The native caret stops at an inline decorator at element offset 0,
        // so move it here; selectEnd handles every last-descendant case.
        let elementKey = element.getKey()
        let ending = try element.selectEnd()
        if event.shiftKey {
          try ending.anchor.set(elementKey, 0, .element)
        }
        event.preventDefault()
        event.stopPropagation()
        return true
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      MOVE_TO_START,
      { event, _ in
        let selection = try getSelection()
        if let selection = selection as? NodeSelection {
          return try promoteNodeSelectionToBlockEdge(selection, true, event)
        }
        guard let selection = selection as? RangeSelection else { return false }
        let anchor = selection.anchor
        let focus = selection.focus
        guard let focusBlock = try nearestBlock(focus.getNode()),
          let firstChild = try focusBlock.getFirstChild() as? DecoratorNode,
          try firstChild.isInline()
        else { return false }
        // Cross-block selections fall through to native handling.
        guard let anchorBlock = try nearestBlock(anchor.getNode()), anchorBlock.is(focusBlock)
        else { return false }
        let blockKey = focusBlock.getKey()
        if focus.type == .element && focus.key == blockKey && focus.offset == 0 {
          return false
        }
        try selection.focus.set(blockKey, 0, .element)
        if !event.shiftKey {
          try selection.anchor.set(blockKey, 0, .element)
        }
        event.preventDefault()
        event.stopPropagation()
        return true
      }, COMMAND_PRIORITY_EDITOR),
  ])
}
