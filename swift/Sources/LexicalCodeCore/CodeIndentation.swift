import Lexical
import LexicalExtension
import LexicalUtils

// Port of packages/lexical-code-core/src/CodeIndentation.ts.

/// `$isSelectionInCode(selection)`.
private func isSelectionInCode(_ selection: BaseSelection?) throws -> Bool {
  guard let selection = selection as? RangeSelection else {
    return false
  }
  let anchorNode = try selection.anchor.getNode()
  let maybeAnchorCodeNode: LexicalNode? =
    isCodeNode(anchorNode) ? anchorNode : try anchorNode.getParent()
  let focusNode = try selection.focus.getNode()
  let maybeFocusCodeNode: LexicalNode? =
    isCodeNode(focusNode) ? focusNode : try focusNode.getParent()

  guard let maybeAnchorCodeNode = maybeAnchorCodeNode as? CodeNode else {
    return false
  }
  return maybeAnchorCodeNode.is(maybeFocusCodeNode)
}

/// `$getCodeLines(selection)`: the CodeHighlightNodes and TabNodes of the
/// selection, split by LineBreakNode. If the selection ends at the start of
/// the last line, that line is considered empty. Empty lines are discarded.
private func getCodeLines(_ selection: RangeSelection) throws -> [[TextNode]] {
  let nodes = try selection.getNodes()
  var lines: [[TextNode]] = []
  if nodes.count == 1 && isCodeNode(nodes[0]) {
    return lines
  }
  var lastLine: [TextNode] = []
  for node in nodes {
    try invariant(
      isCodeHighlightNode(node) || isTabNode(node) || isLineBreakNode(node),
      "Expected selection to be inside CodeBlock and consisting of CodeHighlightNode, TabNode and LineBreakNode"
    )
    if isLineBreakNode(node) {
      if !lastLine.isEmpty {
        lines.append(lastLine)
        lastLine = []
      }
    } else if let node = node as? TextNode {
      lastLine.append(node)
    }
  }
  if !lastLine.isEmpty {
    let selectionEnd = try selection.isBackward() ? selection.anchor : selection.focus

    // Discard the last line if the selection ends exactly at the
    // start of the line (no real selection)
    let lastPoint = createPoint(lastLine[0].getKey(), 0, .text)
    if !selectionEnd.is(lastPoint) {
      lines.append(lastLine)
    }
  }

  return lines
}

/// `$handleTab(shiftKey)`: the command Tab (or Shift+Tab) stands for in a
/// code block, or `nil` outside one.
private func handleTab(_ shiftKey: Bool) throws -> LexicalCommand<Void>? {
  guard let selection = try getSelection() as? RangeSelection, try isSelectionInCode(selection)
  else {
    return nil
  }
  let indentOrOutdent = !shiftKey ? INDENT_CONTENT_COMMAND : OUTDENT_CONTENT_COMMAND
  let tabOrOutdent = !shiftKey ? INSERT_TAB_COMMAND : OUTDENT_CONTENT_COMMAND

  let anchor = selection.anchor
  let focus = selection.focus

  // 1. early decision when there is no real selection
  if anchor.is(focus) {
    return tabOrOutdent
  }

  // 2. If only empty lines or multiple non-empty lines are selected: indent/outdent
  let codeLines = try getCodeLines(selection)
  if codeLines.count != 1 {
    return indentOrOutdent
  }

  let codeLine = codeLines[0]
  let codeLineLength = codeLine.count

  try invariant(codeLineLength != 0, "$getCodeLines only extracts non-empty lines")

  // Take into account the direction of the selection
  let selectionFirst: Point
  let selectionLast: Point
  if try selection.isBackward() {
    selectionFirst = focus
    selectionLast = anchor
  } else {
    selectionFirst = anchor
    selectionLast = focus
  }

  // find boundary elements of the line
  // since codeLine only contains TabNode | CodeHighlightNode
  // the result of these functions should is of Type TabNode | CodeHighlightNode
  let firstOfLine = try getFirstCodeNodeOfLine(codeLine[0])
  let lastOfLine = try getLastCodeNodeOfLine(codeLine[0])

  let anchorOfLine = createPoint(firstOfLine.getKey(), 0, .text)
  let focusOfLine = createPoint(lastOfLine.getKey(), try lastOfLine.getTextContentSize(), .text)

  // 3. multiline because selection started strictly before the line
  if try selectionFirst.isBefore(anchorOfLine) {
    return indentOrOutdent
  }

  // 4. multiline because the selection stops strictly after the line
  if try focusOfLine.isBefore(selectionLast) {
    return indentOrOutdent
  }

  // The selection if within the line.
  // 4. If it does not touch both borders, it needs a tab
  if try anchorOfLine.isBefore(selectionFirst) || selectionLast.isBefore(focusOfLine) {
    return tabOrOutdent
  }

  // 5. Selection is matching a full line on non-empty code
  return indentOrOutdent
}

/// `$outdentLineAtCaret(selection, tabSize)`: outdents the single line the
/// collapsed caret sits on.
///
/// `$getCodeLines` drops a trailing line when the selection ends exactly at
/// its start — for a collapsed caret that is always true, so the caret's own
/// line never reaches the outdent loop and the line has to be resolved from
/// the anchor here. Applies the same rule as that loop: strip a leading
/// TabNode, or `tabSize` leading spaces when the extension is configured for
/// them.
private func outdentLineAtCaret(_ selection: RangeSelection, _ tabSize: Int?) throws {
  let anchorNode = try selection.anchor.getNode()
  // An element point (e.g. the caret on a blank line) has no line to outdent.
  if !isCodeHighlightNode(anchorNode) && !isTabNode(anchorNode) {
    return
  }
  let firstOfLine = try getFirstCodeNodeOfLine(anchorNode)
  if isTabNode(firstOfLine) {
    try firstOfLine.remove()
  } else if let tabSize, let firstOfLine = firstOfLine as? CodeHighlightNode {
    _ = try outdentLeadingSpaces(firstOfLine, tabSize, selection)
  }
}

/// `$handleMultilineIndent(type, tabSize)`.
private func handleMultilineIndent(
  _ type: LexicalCommand<Void>, _ tabSize: Int? = nil
) throws
  -> Bool
{
  guard let selection = try getSelection() as? RangeSelection, try isSelectionInCode(selection)
  else {
    return false
  }

  let codeLines = try getCodeLines(selection)
  let codeLinesLength = codeLines.count

  // Special Indent case
  // Selection is collapsed at the beginning of a line
  if codeLinesLength == 0 && selection.isCollapsed() {
    if type === INDENT_CONTENT_COMMAND {
      try selection.insertNodes([createTabNode()])
    } else {
      try outdentLineAtCaret(selection, tabSize)
    }
    return true
  }

  // Special Indent case
  // Selection is matching only one LineBreak
  if try codeLinesLength == 0 && type === INDENT_CONTENT_COMMAND
    && selection.getTextContent() == "\n"
  {
    let tabNode = try createTabNode()
    let lineBreakNode = try createLineBreakNode()
    let direction: CaretDirection = try selection.isBackward() ? .previous : .next
    try selection.insertNodes([tabNode, lineBreakNode])
    _ = try setSelectionFromCaretRange(
      getCaretRangeInDirection(
        getCaretRange(
          getTextPointCaret(tabNode, .next, 0),
          normalizeCaret(getSiblingCaret(lineBreakNode, .next))),
        direction))

    return true
  }

  // Indent Non Empty Lines
  for i in 0..<codeLinesLength {
    let line = codeLines[i]
    // a line here is never empty
    if !line.isEmpty {
      var firstOfLine: LexicalNode = line[0]

      // make sure to consider the first node on the first line
      // because the line might not be fully selected
      if i == 0 {
        firstOfLine = try getFirstCodeNodeOfLine(firstOfLine)
      }

      if type === INDENT_CONTENT_COMMAND {
        let tabNode = try createTabNode()
        try firstOfLine.insertBefore(tabNode)
        // First real code line may need selection adjustment
        // when firstOfLine is at the selection boundary
        if i == 0 {
          let anchorPoint = try selection.isBackward() ? selection.focus : selection.anchor
          let anchorLine = createPoint(firstOfLine.getKey(), 0, .text)

          if anchorPoint.is(anchorLine) {
            try anchorPoint.set(tabNode.getKey(), 0, .text)
          }
        }
      } else if isTabNode(firstOfLine) {
        try firstOfLine.remove()
      } else if let tabSize, let firstOfLine = firstOfLine as? CodeHighlightNode {
        // Outdent space-indented lines (e.g. code formatted with prettier).
        _ = try outdentLeadingSpaces(firstOfLine, tabSize, selection)
      }
    }
  }
  return true
}

/// `$handleShiftLines(type, event)`: alt+arrow moves the selected lines up
/// or down.
private func handleShiftLines(
  _ type: LexicalCommand<KeyboardEvent>, _ event: KeyboardEvent
)
  throws -> Bool
{
  // We only care about the alt+arrow keys
  guard let selection = try getSelection() as? RangeSelection else {
    return false
  }

  // I'm not quite sure why, but it seems like calling anchor.getNode() collapses the selection here
  // So first, get the anchor and the focus, then get their nodes
  let anchor = selection.anchor
  let focus = selection.focus
  let anchorOffset = anchor.offset
  let focusOffset = focus.offset
  let anchorNode = try anchor.getNode()
  let focusNode = try focus.getNode()
  let arrowIsUp = type === KEY_ARROW_UP_COMMAND

  // Ensure the selection is within the codeblock
  guard try isSelectionInCode(selection),
    let anchorNode = anchorNode as? TextNode,
    isCodeHighlightNode(anchorNode) || isTabNode(anchorNode),
    let focusNode = focusNode as? TextNode,
    isCodeHighlightNode(focusNode) || isTabNode(focusNode)
  else {
    return false
  }
  if !event.altKey {
    // Handle moving selection out of the code block, given there are no
    // siblings that can natively take the selection.
    if selection.isCollapsed() {
      let codeNode = try anchorNode.getParentOrThrow()
      if try arrowIsUp && anchorOffset == 0 && anchorNode.getPreviousSibling() == nil {
        let codeNodeSibling = try codeNode.getPreviousSibling()
        if codeNodeSibling == nil {
          try codeNode.selectPrevious()
          event.preventDefault()
          return false
        }
      } else if try !arrowIsUp && anchorOffset == anchorNode.getTextContentSize()
        && anchorNode.getNextSibling() == nil
      {
        let codeNodeSibling = try codeNode.getNextSibling()
        if codeNodeSibling == nil {
          try codeNode.selectNext()
          event.preventDefault()
          return false
        }
      }
    }
    return false
  }

  let start: LexicalNode
  let end: LexicalNode
  if try anchorNode.isBefore(focusNode) {
    start = try getFirstCodeNodeOfLine(anchorNode)
    end = try getLastCodeNodeOfLine(focusNode)
  } else {
    start = try getFirstCodeNodeOfLine(focusNode)
    end = try getLastCodeNodeOfLine(anchorNode)
  }

  let range = try start.getNodesBetween(end)
  for node in range {
    if !isCodeHighlightNode(node) && !isTabNode(node) && !isLineBreakNode(node) {
      return false
    }
  }

  // After this point, we know the selection is within the codeblock. We may not be able to
  // actually move the lines around, but we want to return true either way to prevent
  // the event's default behavior
  event.preventDefault()
  event.stopPropagation()  // required to stop cursor movement under Firefox

  let maybeLinebreak = try arrowIsUp ? start.getPreviousSibling() : end.getNextSibling()
  guard let linebreak = maybeLinebreak as? LineBreakNode else {
    return true
  }
  guard
    let sibling = try arrowIsUp ? linebreak.getPreviousSibling() : linebreak.getNextSibling()
  else {
    return true
  }

  // A LineBreakNode sibling means the adjacent line is blank, so it has no
  // node of its own to anchor the move to — $getFirstCodeNodeOfLine /
  // $getLastCodeNodeOfLine hand that linebreak straight back, and it belongs
  // to a *different* line. Anchoring on it splices the moving line into the
  // line on the far side of the blank one, merging the two.
  let adjacentLineIsBlank = isLineBreakNode(sibling)
  let maybeInsertionPoint: LexicalNode? =
    !adjacentLineIsBlank && (isCodeHighlightNode(sibling) || isTabNode(sibling))
    ? (arrowIsUp ? try getFirstCodeNodeOfLine(sibling) : try getLastCodeNodeOfLine(sibling))
    : nil
  var insertionPoint: LexicalNode = maybeInsertionPoint ?? sibling
  try linebreak.remove()
  for node in range { try node.remove() }
  if adjacentLineIsBlank {
    // The blank line's position is immediately after the sibling linebreak,
    // in both directions.
    for node in range {
      try insertionPoint.insertAfter(node)
      insertionPoint = node
    }
    try insertionPoint.insertAfter(linebreak)
  } else if type === KEY_ARROW_UP_COMMAND {
    for node in range { try insertionPoint.insertBefore(node) }
    try insertionPoint.insertBefore(linebreak)
  } else {
    try insertionPoint.insertAfter(linebreak)
    insertionPoint = linebreak
    for node in range {
      try insertionPoint.insertAfter(node)
      insertionPoint = node
    }
  }

  _ = try selection.setTextNodeRange(anchorNode, anchorOffset, focusNode, focusOffset)

  return true
}

/// `$handleMoveTo(type, event)`: Home/End move to the start of the code (or
/// of the line) and the end of the line.
private func handleMoveTo(
  _ type: LexicalCommand<KeyboardEvent>, _ event: KeyboardEvent
) throws
  -> Bool
{
  guard let selection = try getSelection() as? RangeSelection else {
    return false
  }

  let anchor = selection.anchor
  let focus = selection.focus
  let anchorNode = try anchor.getNode()
  let focusNode = try focus.getNode()
  let isMoveToStart = type === MOVE_TO_START

  // Ensure the selection is within the codeblock
  guard try isSelectionInCode(selection),
    isCodeHighlightNode(anchorNode) || isTabNode(anchorNode),
    let focusLineNode = focusNode as? TextNode,
    isCodeHighlightNode(focusLineNode) || isTabNode(focusLineNode)
  else {
    return false
  }

  let direction = try getCodeLineDirection(focusLineNode)
  let moveToStart = direction == "rtl" ? !isMoveToStart : isMoveToStart

  // Shift variant: let the non-shift branches resolve the target via
  // framework helpers (`selectNext` / `selectStart` / `setTextNodeRange` /
  // `node.select`), then restore the original anchor so we end up with an
  // extended selection rather than a collapsed caret. This keeps point
  // shapes (text vs. element) consistent between shift and non-shift.
  let originalAnchorKey = anchor.key
  let originalAnchorOffset = anchor.offset
  let originalAnchorType = anchor.type
  if moveToStart {
    if let start = try getStartOfCodeInLine(focusLineNode, focus.offset) {
      let node = start.node
      let offset = start.offset
      if isLineBreakNode(node) {
        try node.selectNext(0, 0)
      } else if let node = node as? TextNode {
        _ = try selection.setTextNodeRange(node, offset, node, offset)
      }
    } else {
      try focusLineNode.getParentOrThrow().selectStart()
    }
  } else {
    // The end of a line of CodeHighlightNodes and TabNodes is one of them.
    if let node = try getEndOfCodeInLine(focusLineNode) as? TextNode {
      try node.select()
    }
  }
  if event.shiftKey {
    try selection.anchor.set(originalAnchorKey, originalAnchorOffset, originalAnchorType)
  }

  event.preventDefault()
  event.stopPropagation()

  return true
}

/// `registerCodeIndentation(editor, tabSize, escapeWithArrows)`: registers
/// the keyboard and command handlers that drive code-block indentation:
/// Tab / Shift+Tab, INDENT/OUTDENT_CONTENT_COMMAND, INSERT_TAB_COMMAND,
/// alt+arrow line shifting, and Home/End movement.
///
/// When `tabSize` is set, OUTDENT_CONTENT_COMMAND (Shift+Tab) also strips up
/// to that many leading spaces from a code line (see `outdentLeadingSpaces`).
public func registerCodeIndentation(
  _ editor: Editor, _ tabSize: Int? = nil, _ escapeWithArrows: Bool = false
) -> () -> Void {
  // When node is the last child pressing down/right or up/let arrow will insert paragraph
  // below it to allow adding more content.
  // These handlers must be executed before $handleShiftLines
  let escapeHandlers: [() -> Void] =
    escapeWithArrows
    ? [
      editor.registerCommand(
        KEY_ARROW_DOWN_COMMAND,
        { event, _ in event.altKey ? false : try onEscapeDown(isCodeNode, event) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ARROW_RIGHT_COMMAND, { event, _ in try onEscapeDown(isCodeNode, event) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ARROW_UP_COMMAND,
        { event, _ in event.altKey ? false : try onEscapeUp(isCodeNode, event) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ARROW_LEFT_COMMAND, { event, _ in try onEscapeUp(isCodeNode, event) },
        COMMAND_PRIORITY_LOW),
    ] : []
  return mergeRegister(
    escapeHandlers + [
      editor.registerCommand(
        KEY_TAB_COMMAND,
        { event, _ in
          guard let command = try handleTab(event.shiftKey) else {
            return false
          }
          event.preventDefault()
          _ = try editor.dispatchCommand(command)
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        INSERT_TAB_COMMAND,
        { _, _ in
          let selection = try getSelection()
          if !(try isSelectionInCode(selection)) {
            return false
          }
          try insertNodes([createTabNode()])
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        INDENT_CONTENT_COMMAND,
        { _, _ in try handleMultilineIndent(INDENT_CONTENT_COMMAND) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        OUTDENT_CONTENT_COMMAND,
        { _, _ in try handleMultilineIndent(OUTDENT_CONTENT_COMMAND, tabSize) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ARROW_UP_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? RangeSelection else {
            return false
          }
          let anchor = selection.anchor
          let anchorNode = try anchor.getNode()
          if !(try isSelectionInCode(selection)) {
            return false
          }
          // If at the start of a code block, prevent selection from moving out
          let parent = try anchorNode.getParent()
          if try selection.isCollapsed() && anchor.offset == 0
            && anchorNode.getPreviousSibling() == nil && isCodeNode(parent)
            && parent?.getPreviousSibling() == nil
          {
            event.preventDefault()
            return true
          }
          return try handleShiftLines(KEY_ARROW_UP_COMMAND, event)
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ARROW_DOWN_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? RangeSelection else {
            return false
          }
          let anchor = selection.anchor
          let anchorNode = try anchor.getNode()
          if !(try isSelectionInCode(selection)) {
            return false
          }
          // If at the end of a code block, prevent selection from moving out
          if try selection.isCollapsed() && anchor.offset == anchorNode.getTextContentSize()
            && anchorNode.getNextSibling() == nil
            && isCodeNode(anchorNode.getParentOrThrow())
            && anchorNode.getParentOrThrow().getNextSibling() == nil
          {
            event.preventDefault()
            return true
          }
          return try handleShiftLines(KEY_ARROW_DOWN_COMMAND, event)
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        MOVE_TO_START, { event, _ in try handleMoveTo(MOVE_TO_START, event) },
        COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        MOVE_TO_END, { event, _ in try handleMoveTo(MOVE_TO_END, event) }, COMMAND_PRIORITY_LOW),
    ])
}

/// `CodeIndentConfig`.
public struct CodeIndentConfig {
  /// When true, the indent commands are not registered on the editor.
  /// This signal can be flipped at runtime to enable or disable indent
  /// handling without rebuilding the editor.
  public var disabled: Bool
  /// When set, treats that many leading spaces on a code line as one indent
  /// level for the OUTDENT_CONTENT_COMMAND (Shift+Tab). See
  /// `registerCodeIndentation`. When `nil` (the default), only TabNode
  /// removal is supported on outdent.
  ///
  /// Tab and INSERT_TAB_COMMAND continue to insert a TabNode regardless of
  /// this option.
  public var tabSize: Int?
  /// When `true`, this enables the ability to exit a code block that has no
  /// adjacent elements using the ArrowLeft/ArrowUp keys if the cursor is at
  /// the beginning, or the ArrowRight/ArrowDown keys if the cursor is at the
  /// end. When `false` (default), pressing the arrow keys will not move the
  /// cursor if there are no adjacent elements around the code block.
  public var escapeWithArrows: Bool

  public init(disabled: Bool = false, escapeWithArrows: Bool = false, tabSize: Int? = nil) {
    self.disabled = disabled
    self.escapeWithArrows = escapeWithArrows
    self.tabSize = tabSize
  }
}

/// `CodeIndentExtension`: keyboard-driven indentation for code blocks
/// (Tab / Shift+Tab, alt+arrow line shifts, Home/End within a line). The
/// highlighter extensions declare it as a dependency; code blocks without
/// syntax highlighting can use it on its own.
public let CodeIndentExtension = defineExtension(
  name: "@lexical/code-indent", config: safeCast(CodeIndentConfig()),
  dependencies: [CodeExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if stores.disabled.value {
        return nil
      }
      return registerCodeIndentation(
        editor, stores.tabSize.value, stores.escapeWithArrows.value)
    }
  })
