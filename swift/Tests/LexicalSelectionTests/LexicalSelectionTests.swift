import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx.
//
// Upstream renders a React RichTextPlugin editor into jsdom and drives it
// with the DOM: it sets the native selection, dispatches key events, and
// compares the container's innerHTML and the window selection. The port
// builds the same editor with RichTextExtension and keeps the DOM steps in
// model terms (the DOM helpers below): the DOM of each node is the one the
// reconciler mounts (a keyed element per node, a `#text` inside a text
// node's element, a managed `<br>` in an empty block), a native selection
// change resolves its points as `$internalResolveSelectionPoints` does, and
// `Selection.modify` is the word-break polyfill of the upstream selection
// test utils. The HTML is compared through `renderedHTML`, and the expected
// window selection through the DOM path of the committed selection.
//
// Not ported: the cases that build lists, links, tables or markdown.

private let theme: EditorThemeClasses = [
  "code": "editor-code",
  "heading": [
    "h1": "editor-heading-h1", "h2": "editor-heading-h2", "h3": "editor-heading-h3",
    "h4": "editor-heading-h4", "h5": "editor-heading-h5", "h6": "editor-heading-h6",
  ],
  "image": "editor-image",
  "list": ["ol": "editor-list-ol", "ul": "editor-list-ul"],
  "listitem": "editor-listitem",
  "paragraph": "editor-paragraph",
  "quote": "editor-quote",
  "text": [
    "bold": "editor-text-bold", "code": "editor-text-code", "hashtag": "editor-text-hashtag",
    "italic": "editor-text-italic", "link": "editor-text-link",
    "strikethrough": "editor-text-strikethrough", "underline": "editor-text-underline",
    "underlineStrikethrough": "editor-text-underlineStrikethrough",
  ],
]

private let rootOpen =
  "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\">"

// MARK: - The DOM in model terms

/// A DOM node of the mounted editor, as the reconciler creates it.
private enum DOMTarget {
  /// The `#text` inside a text node's element.
  case text(TextNode)
  /// The keyed element of a node.
  case node(LexicalNode)
  /// The managed `<br>` of an element.
  case managedBreak(ElementNode)
}

/// Whether the reconciler gives `element` a managed `<br>`: a block that is
/// empty or ends in a line break or an inline decorator.
private func hasManagedLineBreak(_ element: ElementNode) throws -> Bool {
  if try element.isInline() { return false }
  guard let last = try element.getLastChild() else { return true }
  return try last is LineBreakNode || (last is DecoratorNode && last.isInline())
}

private func domChildren(_ target: DOMTarget) throws -> [DOMTarget] {
  guard case .node(let node) = target else { return [] }
  if let text = node as? TextNode { return [.text(text)] }
  guard let element = node as? ElementNode else { return [] }
  var children = try element.getChildren().map { DOMTarget.node($0) }
  if try hasManagedLineBreak(element) {
    children.append(.managedBreak(element))
  }
  return children
}

/// `$getNodeFromDOM`: the node of a DOM node or of its nearest keyed
/// ancestor.
private func nodeFromDOM(_ target: DOMTarget) -> LexicalNode {
  switch target {
  case .text(let text): text
  case .node(let node): node
  case .managedBreak(let element): element
  }
}

/// `getNodeFromPath`: the DOM node at `path` under the root element.
private func domTarget(_ path: [Int]) throws -> DOMTarget {
  var target = DOMTarget.node(try getRoot())
  for index in path {
    target = try domChildren(target)[index]
  }
  return target
}

/// The DOM path of a node's element.
private func domPath(_ node: LexicalNode) throws -> [Int] {
  var path: [Int] = []
  var current = node
  while let parent = try current.getParent() {
    path.insert(try current.getIndexWithinParent(), at: 0)
    current = parent
  }
  return path
}

/// The DOM point `$updateDOMSelection` writes for a model point.
private func domPoint(_ point: Point) throws -> (DOMTarget, Int) {
  let node = try point.getNode()
  if point.type == .text, let text = node as? TextNode {
    return (.text(text), point.offset)
  }
  return (.node(node), point.offset)
}

private func pathOf(_ target: DOMTarget) throws -> [Int] {
  switch target {
  case .text(let text): try domPath(text) + [0]
  case .node(let node): try domPath(node)
  case .managedBreak(let element):
    try domPath(element) + [try element.getChildrenSize()]
  }
}

/// `shouldResolveAncestor`.
private func shouldResolveAncestor(
  _ resolvedElement: ElementNode, _ lastPoint: Point?
) throws
  -> Bool
{
  guard let lastPoint, let parent = try resolvedElement.getParent() else { return true }
  return try !parent.canBeEmpty() || !parent.is(lastPoint.getNode())
}

/// `$internalResolveSelectionPoint` for the DOM above: a `#text`, a block's
/// element, or a managed `<br>`.
private func resolveDOMPoint(_ dom: DOMTarget, _ offset: Int, _ lastPoint: Point?) throws -> Point {
  if case .text(let text) = dom {
    return createPoint(text.getKey(), offset, .text)
  }
  let childNodes = try domChildren(dom)
  var resolvedOffset = offset
  var moveSelectionToEnd = false
  if resolvedOffset == childNodes.count && !childNodes.isEmpty {
    moveSelectionToEnd = true
    resolvedOffset = childNodes.count - 1
  }
  let direction: CaretDirection = moveSelectionToEnd ? .next : .previous
  if resolvedOffset < childNodes.count,
    let text = nodeFromDOM(childNodes[resolvedOffset]) as? TextNode
  {
    return createPoint(text.getKey(), try getTextNodeOffset(text, .direction(direction)), .text)
  }
  guard var resolvedElement = nodeFromDOM(dom) as? ElementNode else {
    throw LexicalError("resolveDOMPoint: a leaf's DOM is not modeled")
  }
  if case .node = dom {
    // `ElementDOMSlot.resolveChildIndex` on the element's own DOM.
    resolvedOffset = min(min(offset, childNodes.count), try resolvedElement.getChildrenSize())
  } else {
    // A point in the managed <br> resolves to the element's place in its
    // parent.
    resolvedOffset = try resolvedElement.getIndexWithinParent()
    resolvedElement = try resolvedElement.getParentOrThrow()
  }
  if try moveSelectionToEnd && resolvedOffset >= resolvedElement.getChildrenSize() {
    resolvedOffset = max(0, try resolvedElement.getChildrenSize() - 1)
  }
  var child = try resolvedElement.getChildAtIndex(resolvedOffset)
  if let childElement = child as? ElementNode,
    try shouldResolveAncestor(childElement, lastPoint)
  {
    let descendant =
      try moveSelectionToEnd
      ? childElement.getLastDescendant() : childElement.getFirstDescendant()
    if let descendant {
      child = descendant
      resolvedElement = try (descendant as? ElementNode) ?? descendant.getParentOrThrow()
    } else {
      resolvedElement = childElement
    }
    resolvedOffset = 0
  }
  if let text = child as? TextNode {
    return createPoint(text.getKey(), try getTextNodeOffset(text, .direction(direction)), .text)
  }
  if child !== resolvedElement && moveSelectionToEnd {
    resolvedOffset = min(try resolvedElement.getChildrenSize(), resolvedOffset + 1)
  }
  return createPoint(resolvedElement.getKey(), resolvedOffset, .element)
}

/// `$resolveSelectionPointOnBoundary`.
private func resolveSelectionPointOnBoundary(
  _ point: Point, _ isBackward: Bool, _ isCollapsed: Bool
) throws {
  let node = try point.getNode()
  let before = point.offset == 0
  if try !before && point.offset != node.getTextContent().utf16.count {
    return
  }
  let direction: CaretDirection = before ? .previous : .next
  let sibling = try getSiblingCaret(node, direction).getNodeAtCaret()
  if before != isBackward, let element = sibling as? ElementNode, try element.isInline(),
    !before || !isCollapsed
  {
    try point.set(element.__key, before ? element.getChildrenSize() : 0, .element)
  } else if before && !isBackward {
    if let text = sibling as? TextNode, let current = node as? TextNode,
      try !current.isUnmergeable()
    {
      try point.set(text.__key, text.getTextContent().utf16.count, .text)
    }
  } else if sibling == nil && (isCollapsed || (!before && isBackward)) {
    if let parent = try node.getParent(), try parent.isInline(),
      try before || (!parent.canInsertTextAfter() && parent.getTextContentSize() > 1),
      let parentSibling = try getSiblingCaret(parent, direction).getNodeAtCaret() as? TextNode
    {
      try point.set(
        parentSibling.__key, before ? parentSibling.getTextContent().utf16.count : 0, .text)
    }
  }
}

/// `$normalizeSelectionPointsForBoundaries`.
private func normalizeSelectionPointsForBoundaries(_ anchor: Point, _ focus: Point) throws {
  guard anchor.type == .text && focus.type == .text else { return }
  let isBackward = try anchor.isBefore(focus)
  let isCollapsed = anchor.is(focus)
  try resolveSelectionPointOnBoundary(anchor, isBackward, isCollapsed)
  try resolveSelectionPointOnBoundary(focus, !isBackward, isCollapsed)
  if isCollapsed {
    try focus.set(anchor.key, anchor.offset, anchor.type)
  }
}

private func updateSelectionFormatStyle(_ selection: RangeSelection, _ format: Int, _ style: String)
{
  if selection.format != format || selection.style != style {
    selection.format = format
    selection.style = style
    selection.dirty = true
  }
}

/// The update a `selectionchange` runs (`onSelectionChange`): the selection
/// read from the DOM points, then its format. `collapsedSelectionFormat`,
/// which only applies when the caret lands where the reconciler just put it,
/// is not modeled; no case here lands there.
private func applyNativeSelection(
  _ editor: Editor, _ anchorDOM: DOMTarget, _ anchorOffset: Int, _ focusDOM: DOMTarget,
  _ focusOffset: Int
) throws {
  let lastSelection = try getPreviousSelection() as? RangeSelection
  let anchor = try resolveDOMPoint(anchorDOM, anchorOffset, lastSelection?.anchor)
  let focus = try resolveDOMPoint(focusDOM, focusOffset, lastSelection?.focus)
  try normalizeSelectionPointsForBoundaries(anchor, focus)
  let selection = RangeSelection(
    anchor, focus, lastSelection?.format ?? 0, lastSelection?.style ?? "")
  if let lastSelection {
    try internalRefreshSelectionFormatAndStyle(selection, lastSelection.anchor.key)
  }
  try setSelection(selection)
  if selection.isCollapsed() {
    // `$updateCollapsedSelectionFormatStyle`.
    let anchorNode = try anchor.getNode()
    if let text = anchorNode as? TextNode {
      updateSelectionFormatStyle(selection, try text.getFormat(), try text.getStyle())
    } else if try editor.isComposing() || getRoot().getTextContent() != "" {
      if let element = anchorNode as? ElementNode, try element.isEmpty() {
        try internalRefreshSelectionFormatAndStyle(selection, "")
      } else {
        updateSelectionFormatStyle(selection, selection.format, "")
      }
    }
  } else {
    let nodes = try selection.getNodes()
    let isBackward = try selection.isBackward()
    let startOffset = isBackward ? focusOffset : anchorOffset
    let endOffset = isBackward ? anchorOffset : focusOffset
    let startKey = isBackward ? focus.key : anchor.key
    let endKey = isBackward ? anchor.key : focus.key
    var combinedFormat = IS_ALL_FORMATTING
    var hasTextNodes = false
    for (i, node) in nodes.enumerated() {
      let textContentSize = try node.getTextContentSize()
      if let text = node as? TextNode, textContentSize != 0,
        !((i == 0 && text.__key == startKey && startOffset == textContentSize)
          || (i == nodes.count - 1 && text.__key == endKey && endOffset == 0))
      {
        hasTextNodes = true
        combinedFormat &= try text.getFormat()
        if combinedFormat == 0 {
          break
        }
      }
    }
    selection.format = hasTextNodes ? combinedFormat : 0
  }
}

/// `getLastTextNode` of the upstream utils, for a `#text`: the text node
/// before it in document order.
private func previousTextNode(_ text: TextNode) throws -> TextNode? {
  let all = try getRoot().getAllTextNodes()
  guard let index = all.firstIndex(where: { $0 === text || $0.is(text) }), index > 0 else {
    return nil
  }
  return all[index - 1]
}

private let wordBreakScalars: Set<UInt16> = Set(
  Array(".,\\/#!$%^&*;:{}=-`~()".utf16)
    + [0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20, 0xA0, 0x1680, 0x2028, 0x2029, 0x202F, 0x205F, 0xFEFF]
    + Array(0x2000...0x200A))

/// `wordBreakPolyfillRegex`: `\s`, the listed punctuation, surrogates, and
/// CJK symbols and punctuation.
private func isWordBreak(_ unit: UInt16) -> Bool {
  wordBreakScalars.contains(unit) || (0xD800...0xDFFF).contains(unit)
    || (0x3000...0x303F).contains(unit)
}

private struct Segment {
  let index: Int
  let length: Int
  let isWordLike: Bool
}

/// `getWordsFromString` of the polyfill, in UTF-16 units.
private func getWordsFromString(_ units: [UInt16]) -> [Segment] {
  var segments: [Segment] = []
  var start = 0
  var i = 0
  while i < units.count {
    let isWordLike = !isWordBreak(units[i])
    var j = i
    while j < units.count && !isWordBreak(units[j]) == isWordLike { j += 1 }
    segments.append(Segment(index: i, length: j - i, isWordLike: isWordLike))
    start = j
    i = j
  }
  _ = start
  return segments
}

/// The `Selection.prototype.modify` polyfill of the upstream selection test
/// utils, where `Selection.modify` lands a caret, for a point in a text node.
private final class PolyfillMovement: SelectionMovement {
  func landedPoint(
    from point: Point, isBackward: Bool, granularity: SelectionGranularity
  ) throws -> (NodeKey, Int, PointType)? {
    guard point.type == .text, let text = try point.getNode() as? TextNode else { return nil }
    let offset = point.offset
    let units = Array(try text.getTextContent().utf16)
    switch granularity {
    case .character:
      if isBackward {
        if offset > 0 { return (text.getKey(), offset - 1, .text) }
        var sibling = try text.getPreviousSibling()
        if sibling == nil {
          sibling = try text.getParentOrThrow().getPreviousSibling().flatMap {
            try ($0 as? ElementNode)?.getLastChild()
          }
        }
        guard let previous = sibling as? TextNode else { return nil }
        return (previous.getKey(), try previous.getTextContentSize() - 1, .text)
      }
      if offset < units.count { return (text.getKey(), offset + 1, .text) }
      guard let next = try text.getNextSibling() as? TextNode else { return nil }
      return (next.getKey(), 0, .text)
    case .word:
      let target = isBackward ? Array(units[0..<offset]) : Array(units[offset...])
      let segments = getWordsFromString(target)
      var index = offset
      var foundWordNode = false
      if isBackward {
        for segment in segments.reversed() {
          if segment.isWordLike {
            index = segment.index
            foundWordNode = true
          } else if foundWordNode {
            break
          } else {
            index = segment.index
          }
        }
      } else {
        for segment in segments {
          let nextIndex = segment.index + segment.length
          if segment.isWordLike {
            index = nextIndex
            foundWordNode = true
          } else if foundWordNode {
            break
          } else {
            index = nextIndex
          }
        }
        index += offset
      }
      return (text.getKey(), index, .text)
    default:
      return nil
    }
  }
}

// MARK: - The fixture

/// The editor upstream's `TestBase` renders: rich text, the test theme and
/// nodes, mounted, with the native selection at the start of the first
/// paragraph.
private final class Fixture {
  let editor: Editor
  private let movement = PolyfillMovement()

  init() throws {
    editor = try buildEditorFromExtensions(
      defineExtension(
        name: "@test",
        // `HistoryPlugin`, whose default delay is 1000 ms.
        dependencies: [RichTextExtension, configExtension(HistoryExtension) { $0.delay = 1000 }],
        nodes: { defaultTestNodes },
        editorConfig: InitialEditorConfig(theme: theme)))
    try editor.setRootElement(TestRootElement())
    editor.selectionMovement = movement
    drainMicrotasks()
    // Focus first element
    try setNativeSelection([0, 0], 0, [0, 0], 0)
  }

  func dispose() { editor.dispose() }

  func update(_ fn: @escaping () throws -> Void) throws {
    try LexicalTesting.update(editor, fn)
  }

  /// `setNativeSelectionWithPaths`, then the `selectionchange` it fires.
  func setNativeSelection(
    _ anchorPath: [Int], _ anchorOffset: Int, _ focusPath: [Int], _ focusOffset: Int
  ) throws {
    let editor = self.editor
    try update {
      try applyNativeSelection(
        editor, try domTarget(anchorPath), anchorOffset, try domTarget(focusPath), focusOffset)
    }
  }

  /// `container.innerHTML`.
  func html() throws -> String { rootOpen + (try renderedHTML(editor)) + "</div>" }

  /// The window selection the reconciler leaves, as DOM paths.
  func domSelection() throws -> String {
    try editor.read(.latest) {
      guard let selection = try getSelection() as? RangeSelection else { return "none" }
      let (anchor, anchorOffset) = try domPoint(selection.anchor)
      let (focus, focusOffset) = try domPoint(selection.focus)
      return "\(try pathOf(anchor)):\(anchorOffset) \(try pathOf(focus)):\(focusOffset)"
    }
  }

  /// `applySelectionInputs`.
  func apply(_ inputs: [SelectionInput]) throws {
    for input in inputs {
      for _ in 0..<input.times {
        try apply(input)
      }
    }
  }

  private func apply(_ input: SelectionInput) throws {
    let editor = self.editor
    var nativeMove: (DOMTarget, Int)?
    try update {
      let selection = try getSelection()
      let range = selection as? RangeSelection
      switch input {
      case .insertText(let text):
        try selection?.insertText(text)
      case .insertParagraph:
        try range?.insertParagraph()
      case .moveBackward:
        let event = KeyboardEvent("keydown", key: "ArrowLeft")
        _ = try editor.dispatchCommand(KEY_DOWN_COMMAND, event)
        // `moveNativeSelectionBackward` for a collapsed caret.
        if !event.defaultPrevented, let range = try getSelection() as? RangeSelection {
          let (dom, offset) = try domPoint(range.anchor)
          if case .text(let text) = dom {
            if offset == 0 {
              let last = try #require(try previousTextNode(text))
              nativeMove = (.text(last), try last.getTextContentSize())
            } else {
              nativeMove = (dom, offset - 1)
            }
          } else {
            nativeMove = (dom, offset - 1)
          }
        }
      case .moveEnd:
        if let text = try range?.anchor.getNode() as? TextNode {
          try text.select()
        }
      case .deleteBackward:
        try range?.deleteCharacter(true)
      case .deleteWordBackward:
        try range?.deleteWord(true)
      case .deleteWordForward:
        try range?.deleteWord(false)
      case .formatText(let format):
        try range?.formatText(format)
      case .moveNativeSelection:
        // The selectionchange runs after this update.
        break
      case .insertTokenNode(let text):
        let node = try createTextNode(text).setMode("token")
        try range?.insertNodes([node])
      case .insertSegmentedNode(let text):
        let node = try createTextNode(text).setMode("segmented")
        try range?.insertNodes([node])
        try node.selectNext()
      case .convertToTokenNode:
        let node = try createTextNode(try selection?.getTextContent() ?? "").setMode("token")
        try range?.insertNodes([node])
        try node.selectNext()
      case .convertToSegmentedNode:
        let node = try createTextNode(try selection?.getTextContent() ?? "").setMode("segmented")
        try range?.insertNodes([node])
        try node.selectNext()
      case .undo:
        _ = try withIsApple(false) {
          try editor.dispatchCommand(
            KEY_DOWN_COMMAND, KeyboardEvent("keydown", key: "z", ctrlKey: true))
        }
      case .redo:
        _ = try withIsApple(false) {
          try editor.dispatchCommand(
            KEY_DOWN_COMMAND, KeyboardEvent("keydown", key: "z", ctrlKey: true, shiftKey: true))
        }
      case .pastePlain(let text):
        let clipboardData = DataTransfer()
        clipboardData.setData("text/plain", text)
        _ = try editor.dispatchCommand(
          PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: clipboardData)))
      }
    }
    if case .moveNativeSelection(let anchorPath, let anchorOffset, let focusPath, let focusOffset) =
      input
    {
      try setNativeSelection(anchorPath, anchorOffset, focusPath, focusOffset)
    } else if let (dom, offset) = nativeMove {
      try update { try applyNativeSelection(editor, dom, offset, dom, offset) }
    }
  }
}

/// One input of `applySelectionInputs`.
enum SelectionInput: Sendable {
  case insertText(String)
  case insertParagraph
  case moveBackward(Int)
  case moveEnd
  case deleteBackward(Int)
  case deleteWordBackward(Int)
  case deleteWordForward(Int)
  case formatText(String)
  case moveNativeSelection([Int], Int, [Int], Int)
  case insertTokenNode(String)
  case insertSegmentedNode(String)
  case convertToTokenNode
  case convertToSegmentedNode
  case pastePlain(String)
  case undo(Int)
  case redo(Int)

  var times: Int {
    switch self {
    case .moveBackward(let n), .deleteBackward(let n), .deleteWordBackward(let n),
      .deleteWordForward(let n), .undo(let n), .redo(let n):
      n
    default: 1
    }
  }
}

private let formatBold = SelectionInput.formatText("bold")
private let formatItalic = SelectionInput.formatText("italic")
private let formatUnderline = SelectionInput.formatText("underline")
private let formatStrikeThrough = SelectionInput.formatText("strikethrough")

private func typing(_ word: String) -> [SelectionInput] {
  word.map { .insertText(String($0)) }
}

private func paragraphs(_ inner: String...) -> String {
  rootOpen + inner.map { "<p class=\"editor-paragraph\" dir=\"auto\">\($0)</p>" }.joined()
    + "</div>"
}

private let emptyParagraph = "<br data-lexical-managed-linebreak=\"true\">"
private func span(_ text: String) -> String { "<span data-lexical-text=\"true\">\(text)</span>" }
private func bold(_ text: String) -> String {
  "<strong class=\"editor-text-bold\" data-lexical-text=\"true\">\(text)</strong>"
}

/// One case of upstream's `suite`.
struct SuiteCase: Sendable, CustomTestStringConvertible {
  let number: Int
  let name: String
  let inputs: [SelectionInput]
  let expectedHTML: String
  /// `anchorPath:anchorOffset focusPath:focusOffset`.
  let expectedSelection: String
  var testDescription: String { "\(name) (#\(number))" }

  static func selection(
    _ anchorPath: [Int], _ anchorOffset: Int, _ focusPath: [Int], _ focusOffset: Int
  ) -> String {
    "\(anchorPath):\(anchorOffset) \(focusPath):\(focusOffset)"
  }

  static let named: [SuiteCase] = [
    SuiteCase(
      number: 1, name: "Simple typing", inputs: typing("Hello"),
      expectedHTML: paragraphs(span("Hello")),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)
    ),
    SuiteCase(
      number: 2, name: "Simple typing in bold", inputs: [formatBold] + typing("Hello"),
      expectedHTML: paragraphs(bold("Hello")),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)
    ),
    SuiteCase(
      number: 3, name: "Simple typing in italic", inputs: [formatItalic] + typing("Hello"),
      expectedHTML: paragraphs(
        "<em class=\"editor-text-italic\" data-lexical-text=\"true\">Hello</em>"),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
    SuiteCase(
      number: 4, name: "Simple typing in italic + bold",
      inputs: [formatItalic, formatBold] + typing("Hello"),
      expectedHTML: paragraphs(
        "<strong class=\"editor-text-bold editor-text-italic\" data-lexical-text=\"true\">Hello</strong>"
      ), expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
    SuiteCase(
      number: 5, name: "Simple typing in underline", inputs: [formatUnderline] + typing("Hello"),
      expectedHTML: paragraphs(
        "<span class=\"editor-text-underline\" data-lexical-text=\"true\">Hello</span>"),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
    SuiteCase(
      number: 6, name: "Simple typing in strikethrough",
      inputs: [formatStrikeThrough] + typing("Hello"),
      expectedHTML: paragraphs(
        "<span class=\"editor-text-strikethrough\" data-lexical-text=\"true\">Hello</span>"),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
    SuiteCase(
      number: 7, name: "Simple typing in underline + strikethrough",
      inputs: [formatUnderline, formatStrikeThrough] + typing("Hello"),
      expectedHTML: paragraphs(
        "<span class=\"editor-text-underlineStrikethrough\" data-lexical-text=\"true\">Hello</span>"
      ),
      expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
    SuiteCase(
      number: 8, name: "Deletion",
      inputs: typing("123") + [.deleteBackward(1)] + typing("45") + [.deleteBackward(1)]
        + typing("6"),
      expectedHTML: paragraphs(span("1246")),
      expectedSelection: selection([0, 0, 0], 4, [0, 0, 0], 4)),
    SuiteCase(
      number: 9, name: "Creation of an token node", inputs: [.insertTokenNode("Dominic Gannaway")],
      expectedHTML: paragraphs(span("Dominic Gannaway")),
      expectedSelection: selection([0, 0, 0], 16, [0, 0, 0], 16)),
    SuiteCase(
      number: 10, name: "Convert text to an token node",
      inputs: [
        .insertText("Dominic Gannaway"), .moveNativeSelection([0, 0, 0], 0, [0, 0, 0], 16),
        .convertToTokenNode,
      ], expectedHTML: paragraphs(span("Dominic Gannaway")),
      expectedSelection: selection([0], 1, [0], 1)),
    SuiteCase(
      number: 11, name: "Creation of a segmented node",
      inputs: [.insertSegmentedNode("Dominic Gannaway")],
      expectedHTML: paragraphs(span("Dominic Gannaway")),
      expectedSelection: selection([0], 1, [0], 1)),
    SuiteCase(
      number: 12, name: "Convert text to a segmented node",
      inputs: [
        .insertText("Dominic Gannaway"), .moveNativeSelection([0, 0, 0], 0, [0, 0, 0], 16),
        .convertToSegmentedNode,
      ], expectedHTML: paragraphs(span("Dominic Gannaway")),
      expectedSelection: selection([0], 1, [0], 1)),
    SuiteCase(
      number: 13, name: "Format selection that starts and ends on element and retain selection",
      inputs: [
        .insertParagraph, .insertText("Hello world"), .insertParagraph,
        .moveNativeSelection([0], 0, [2], 0), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, bold("Hello world"), emptyParagraph),
      expectedSelection: selection([0], 0, [2], 0)),
    SuiteCase(
      number: 14,
      name: "Format multiline text selection that starts and ends on element and retain selection",
      inputs: [
        .insertParagraph, .insertText("Hello"), .insertParagraph, .insertText("world"),
        .insertParagraph, .moveNativeSelection([0], 0, [3], 0), formatBold,
      ],
      expectedHTML: paragraphs(emptyParagraph, bold("Hello"), bold("world"), emptyParagraph),
      expectedSelection: selection([0], 0, [3], 0)),
    SuiteCase(
      number: 15, name: "Format multiline text selection that starts and ends within text",
      inputs: [
        .insertText("Hello"), .insertParagraph, .insertText("world"),
        .moveNativeSelection([0, 0, 0], 2, [1, 0, 0], 2), formatBold,
      ], expectedHTML: paragraphs(span("He") + bold("llo"), bold("wo") + span("rld")),
      expectedSelection: selection([0, 1, 0], 0, [1, 0, 0], 2)),
    SuiteCase(
      number: 16,
      name: "Format selection that starts on text and ends on element and retain selection",
      inputs: [
        .insertParagraph, .insertText("Hello world"), .insertParagraph,
        .moveNativeSelection([1, 0, 0], 6, [2], 0), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, span("Hello ") + bold("world"), emptyParagraph),
      expectedSelection: selection([1, 1, 0], 0, [2], 0)),
    SuiteCase(
      number: 17,
      name: "Format selection that starts on element and ends on text and retain selection",
      inputs: [
        .insertParagraph, .insertText("Hello world"), .insertParagraph,
        .moveNativeSelection([0], 0, [1, 0, 0], 5), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, bold("Hello") + span(" world"), emptyParagraph),
      expectedSelection: selection([0], 0, [1, 0, 0], 5)),
    SuiteCase(
      number: 18,
      name: "Format selection that starts on middle of token node should format complete node",
      inputs: [
        .insertParagraph, .insertTokenNode("Hello"), .insertText(" world"), .insertParagraph,
        .moveNativeSelection([1, 0, 0], 2, [2], 0), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, bold("Hello") + bold(" world"), emptyParagraph),
      expectedSelection: selection([1, 0, 0], 2, [2], 0)),
    SuiteCase(
      number: 19,
      name: "Format selection that ends on middle of token node should format complete node",
      inputs: [
        .insertParagraph, .insertText("Hello "), .insertTokenNode("world"), .insertParagraph,
        .moveNativeSelection([0], 0, [1, 1, 0], 2), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, bold("Hello ") + bold("world"), emptyParagraph),
      expectedSelection: selection([0], 0, [1, 1, 0], 2)),
    SuiteCase(
      number: 20, name: "Format token node if it is the single one selected",
      inputs: [
        .insertParagraph, .insertTokenNode("Hello"), .insertText(" world"), .insertParagraph,
        .moveNativeSelection([1, 0, 0], 2, [1, 0, 0], 3), formatBold,
      ], expectedHTML: paragraphs(emptyParagraph, bold("Hello") + span(" world"), emptyParagraph),
      expectedSelection: selection([1, 0, 0], 2, [1, 0, 0], 3)),
    SuiteCase(
      number: 21,
      name:
        "Format selection that contains a token node in the middle should format the token node",
      inputs: [
        .insertParagraph, .insertText("Hello "), .insertTokenNode("beautiful"),
        .insertText(" world"), .insertParagraph, .moveNativeSelection([0], 0, [2], 0), formatBold,
      ],
      expectedHTML: paragraphs(
        emptyParagraph, bold("Hello ") + bold("beautiful") + bold(" world"), emptyParagraph),
      expectedSelection: selection([0], 0, [2], 0)),
  ]

  static let whitespaces: [(String, String)] = [
    (" ", "space"), ("\u{00a0}", "non-breaking space"), ("\u{2000}", "en quad"),
    ("\u{2001}", "em quad"), ("\u{2002}", "en space"), ("\u{2003}", "em space"),
    ("\u{2004}", "three-per-em space"), ("\u{2005}", "four-per-em space"),
    ("\u{2006}", "six-per-em space"), ("\u{2007}", "figure space"),
    ("\u{2008}", "punctuation space"), ("\u{2009}", "thin space"), ("\u{200A}", "hair space"),
  ]

  /// The whitespace cases.
  static let whitespace: [SuiteCase] = whitespaces.enumerated().flatMap { w, entry in
    let (character, name) = entry
    let printed = character == "\u{00a0}" ? "&nbsp;" : character
    let base = 22 + 9 * w
    return [
      SuiteCase(
        number: base, name: "Type two words separated by a \(name), delete word backward from end",
        inputs: [.insertText("Hello\(character)world"), .deleteWordBackward(1)],
        expectedHTML: paragraphs(span("Hello\(printed)")),
        expectedSelection: selection([0, 0, 0], 6, [0, 0, 0], 6)),
      SuiteCase(
        number: base + 1,
        name: "Type two words separated by a \(name), delete word forward from beginning",
        inputs: [
          .insertText("Hello\(character)world"), .moveNativeSelection([0, 0, 0], 0, [0, 0, 0], 0),
          .deleteWordForward(1),
        ], expectedHTML: paragraphs(span("\(printed)world")),
        expectedSelection: selection([0, 0, 0], 0, [0, 0, 0], 0)),
      SuiteCase(
        number: base + 2,
        name:
          "Type two words separated by a \(name), delete word forward from beginning of preceding whitespace",
        inputs: [
          .insertText("Hello\(character)world"), .moveNativeSelection([0, 0, 0], 5, [0, 0, 0], 5),
          .deleteWordForward(1),
        ], expectedHTML: paragraphs(span("Hello")),
        expectedSelection: selection([0, 0, 0], 5, [0, 0, 0], 5)),
      SuiteCase(
        number: base + 3,
        name:
          "Type two words separated by a \(name), delete word backward from end of trailing whitespace",
        inputs: [
          .insertText("Hello\(character)world"), .moveNativeSelection([0, 0, 0], 6, [0, 0, 0], 6),
          .deleteWordBackward(1),
        ], expectedHTML: paragraphs(span("world")),
        expectedSelection: selection([0, 0, 0], 0, [0, 0, 0], 0)),
      SuiteCase(
        number: base + 4, name: "Type a word, delete it and undo the deletion",
        inputs: [.insertText("Hello world"), .deleteWordBackward(1), .undo(1)],
        expectedHTML: paragraphs(span("Hello world")),
        expectedSelection: selection([0, 0, 0], 11, [0, 0, 0], 11)),
      SuiteCase(
        number: base + 5, name: "Type a word, delete it and undo the deletion",
        inputs: [.insertText("Hello world"), .deleteWordBackward(1), .undo(1), .redo(1)],
        expectedHTML: paragraphs(span("Hello ")),
        expectedSelection: selection([0, 0, 0], 6, [0, 0, 0], 6)),
      SuiteCase(
        number: base + 6,
        name: "Type a sentence, move the caret to the middle and move with the arrows to the start",
        inputs: [
          .insertText("this is weird test"), .moveNativeSelection([0, 0, 0], 14, [0, 0, 0], 14),
          .moveBackward(14),
        ], expectedHTML: paragraphs(span("this is weird test")),
        expectedSelection: selection([0, 0, 0], 0, [0, 0, 0], 0)),
      SuiteCase(
        number: base + 7,
        name: "Type a text and token text, move the caret to the end of the first text",
        inputs: [
          .insertText("Hello "), .insertTokenNode("Bob"), .moveBackward(1), .moveBackward(1),
          .moveEnd,
        ], expectedHTML: paragraphs(span("Hello ") + span("Bob")),
        expectedSelection: selection([0, 1, 0], 3, [0, 1, 0], 3)),
      SuiteCase(
        number: base + 8, name: "Paste text, move selection and delete word forward",
        inputs: [
          .pastePlain("ABD\tEFG"), .moveBackward(5), .insertText("C"), .moveBackward(1),
          .deleteWordForward(1),
        ], expectedHTML: paragraphs(span("ABD") + span("\t") + span("EFG")),
        expectedSelection: selection([0, 0, 0], 3, [0, 0, 0], 3)),
    ]
  }

  static let all = named + whitespace
}

/// One of the `Block selection moves when new nodes are inserted` cases.
struct BlockMoveCase: Sendable, CustomTestStringConvertible {
  typealias Expected = (LexicalNode, Int, LexicalNode, Int)
  let name: String
  let anchorOffset: Int
  let focusOffset: Int
  let fnBefore: (@Sendable (ElementNode, TextNode) throws -> Void)?
  let fn: @Sendable (ElementNode, TextNode) throws -> Expected
  var invertSelection = false
  var testDescription: String { name + (invertSelection ? " (inverse selection)" : "") }

  init(
    _ name: String, _ anchorOffset: Int, _ focusOffset: Int,
    before fnBefore: (@Sendable (ElementNode, TextNode) throws -> Void)? = nil,
    _ fn: @escaping @Sendable (ElementNode, TextNode) throws -> Expected
  ) {
    self.name = name
    self.anchorOffset = anchorOffset
    self.focusOffset = focusOffset
    self.fnBefore = fnBefore
    self.fn = fn
  }

  private static let insertBar: @Sendable (ElementNode, TextNode) throws -> Void = { _, text in
    try text.insertBefore(try createTextNode("bar"))
  }
  private static let appendBar: @Sendable (ElementNode, TextNode) throws -> Void = {
    paragraph, _ in
    try paragraph.append(try createTextNode("bar"))
  }

  static let base: [BlockMoveCase] = [
    // Collapsed selection on end; add/remove/replace beginning
    BlockMoveCase("insertBefore - Collapsed selection on end; add beginning", 2, 2) { p, text in
      try text.insertBefore(try createTextNode("2"))
      return (p, 3, p, 3)
    },
    BlockMoveCase("insertAfter - Collapsed selection on end; add beginning", 2, 2) { p, text in
      try text.insertAfter(try createTextNode("2"))
      return (p, 3, p, 3)
    },
    BlockMoveCase("splitText - Collapsed selection on end; add beginning", 2, 2) { p, text in
      _ = try text.splitText(1)
      return (p, 3, p, 3)
    },
    BlockMoveCase("remove - Collapsed selection on end; add beginning", 1, 1) { p, text in
      try text.remove()
      return (p, 0, p, 0)
    },
    BlockMoveCase("replace - Collapsed selection on end; replace beginning", 1, 1) { p, text in
      try text.replace(try createTextNode("replacement"))
      return (p, 1, p, 1)
    },
    // All selected; add/remove/replace on beginning
    BlockMoveCase("insertBefore - All selected; add on beginning", 0, 2) { p, text in
      try text.insertBefore(try createTextNode("2"))
      return (text, 0, p, 3)
    },
    BlockMoveCase("splitNodes - All selected; add on beginning", 0, 2) { p, original in
      let text = try original.splitText(1)[1]
      return (text, 0, p, 3)
    },
    BlockMoveCase("remove - All selected; remove on beginning", 0, 1) { p, text in
      try text.remove()
      return (p, 0, p, 0)
    },
    BlockMoveCase("replace - All selected; replace on beginning", 0, 1) { p, text in
      try text.replace(try createTextNode("replacement"))
      return (p, 0, p, 1)
    },
    // Selection beginning; add/remove/replace on end
    BlockMoveCase("insertBefore - Selection beginning; add on end", 0, 1, before: insertBar) {
      p, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      try #require(try p.getLastChild()).insertBefore(try createTextNode("2"))
      return (text2, 0, text1, 0)
    },
    BlockMoveCase("insertAfter - Selection beginning; add on end", 0, 1) { p, text in
      try #require(try p.getLastChild()).insertAfter(try createTextNode("2"))
      return (text, 0, p, 1)
    },
    BlockMoveCase("splitText - Selection beginning; add on end", 0, 1, before: insertBar) {
      _, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      let text = try text1.splitText(1)[1]
      return (text2, 0, text, 0)
    },
    BlockMoveCase("remove - Selection beginning; remove on end", 0, 1) { p, text in
      try #require(try p.getLastChild()).remove()
      return (text, 0, text, 3)
    },
    BlockMoveCase("replace - Selection beginning; replace on end", 0, 1) { p, _ in
      try #require(try p.getLastChild()).replace(try createTextNode("replacement"))
      return (p, 0, p, 1)
    },
    // All selected; add/remove/replace in end offset [1, 2] -> [1, N, 2]
    BlockMoveCase("insertBefore - All selected; add in end offset", 0, 1) { p, text in
      try #require(try p.getLastChild()).insertBefore(try createTextNode("2"))
      return (text, 0, p, 2)
    },
    BlockMoveCase("insertAfter - All selected; add in end offset", 0, 1) { p, text in
      try text.insertAfter(try createTextNode("2"))
      return (text, 0, p, 2)
    },
    BlockMoveCase("splitText - All selected; add in end offset", 0, 1, before: insertBar) {
      _, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      let text = try text1.splitText(1)[1]
      return (text2, 0, text, 0)
    },
    BlockMoveCase("remove - All selected; remove in end offset", 1, 2, before: insertBar) {
      p, text1 in
      try #require(try p.getLastChild()).remove()
      return (text1, 0, text1, 3)
    },
    BlockMoveCase("replace - All selected; replace in end offset", 1, 2, before: insertBar) {
      p, _ in
      try #require(try p.getLastChild()).replace(try createTextNode("replacement"))
      return (p, 1, p, 2)
    },
    // All selected; add/remove/replace in middle [1, 2, 3] -> [1, 2, N, 3]
    BlockMoveCase("insertBefore - All selected; add in middle", 0, 2, before: insertBar) {
      p, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      try #require(try p.getLastChild()).insertBefore(try createTextNode("2"))
      return (text2, 0, p, 3)
    },
    BlockMoveCase("insertAfter - All selected; add in middle", 0, 2, before: insertBar) {
      p, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      try text1.insertAfter(try createTextNode("2"))
      return (text2, 0, p, 3)
    },
    BlockMoveCase("splitText - All selected; add in middle", 0, 2, before: insertBar) {
      p, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      _ = try text1.splitText(1)
      return (text2, 0, p, 3)
    },
    BlockMoveCase("remove - All selected; remove in middle", 0, 2, before: insertBar) {
      p, text1 in
      let text2 = try #require(try text1.getPreviousSibling())
      try text1.remove()
      return (text2, 0, p, 1)
    },
    BlockMoveCase("replace - All selected; replace in middle", 0, 2, before: insertBar) {
      p, text1 in
      try text1.replace(try createTextNode("replacement"))
      return (p, 0, p, 2)
    },
    // Edge cases
    BlockMoveCase(
      "Selection resolves to the end of text node when it's at the end (1)", 3, 3,
      before: appendBar
    ) { p, text1 in
      let text2 = try #require(try p.getLastChild())
      try text1.insertBefore(try createTextNode("new"))
      return (text2, 3, text2, 3)
    },
    BlockMoveCase(
      "Selection resolves to the end of text node when it's at the end (2)", 0, 3,
      before: appendBar
    ) { p, text1 in
      let text2 = try #require(try p.getLastChild())
      try text1.insertBefore(try createTextNode("new"))
      return (text1, 0, text2, 3)
    },
    BlockMoveCase("remove - Remove with collapsed selection at offset #4221", 1, 1) { _, text1 in
      try #require(try text1.getNextSibling()).remove()
      return (text1, 3, text1, 3)
    },
    BlockMoveCase("remove - Remove with non-collapsed selection at offset", 0, 1) { _, text1 in
      try #require(try text1.getNextSibling()).remove()
      return (text1, 0, text1, 3)
    },
  ]

  static let all: [BlockMoveCase] = base.flatMap { testCase in
    var inverse = testCase
    inverse.invertSelection = true
    return [testCase, inverse]
  }
}

/// One of the `Decorator text content for selection` cases.
struct DecoratorTextCase: Sendable, CustomTestStringConvertible {
  struct Nodes {
    let textNode1: TextNode
    let textNode2: TextNode
    let decorator: DecoratorNode
    let paragraph: ParagraphNode
    let anchor: Point
    let focus: Point
  }
  let name: String
  let fn: @Sendable (Nodes) throws -> String
  var invertSelection = false
  var testDescription: String { name + (invertSelection ? " (inverse selection)" : "") }

  init(_ name: String, _ fn: @escaping @Sendable (Nodes) throws -> String) {
    self.name = name
    self.fn = fn
  }

  static let base: [DecoratorTextCase] = [
    DecoratorTextCase("Not included if cursor right before it") { n in
      try n.anchor.set(n.textNode1.getKey(), 1, .text)
      try n.focus.set(n.textNode1.getKey(), 1, .text)
      return ""
    },
    DecoratorTextCase("Not included if cursor right after it") { n in
      try n.anchor.set(n.textNode2.getKey(), 0, .text)
      try n.focus.set(n.textNode2.getKey(), 0, .text)
      return ""
    },
    DecoratorTextCase("Included if decorator is selected within text") { n in
      try n.anchor.set(n.textNode1.getKey(), 1, .text)
      try n.focus.set(n.textNode2.getKey(), 0, .text)
      return try n.decorator.getTextContent()
    },
    DecoratorTextCase("Included if decorator is selected with another node before it") { n in
      try n.anchor.set(n.textNode1.getKey(), 0, .text)
      try n.focus.set(n.textNode2.getKey(), 0, .text)
      return try n.textNode1.getTextContent() + n.decorator.getTextContent()
    },
    DecoratorTextCase("Included if decorator is selected with another node after it") { n in
      try n.anchor.set(n.textNode1.getKey(), 1, .text)
      try n.focus.set(n.textNode2.getKey(), 1, .text)
      return try n.decorator.getTextContent() + n.textNode2.getTextContent()
    },
    DecoratorTextCase("Included if decorator is selected as the only node") { n in
      try n.textNode1.remove()
      try n.textNode2.remove()
      try n.anchor.set(n.paragraph.getKey(), 0, .element)
      try n.focus.set(n.paragraph.getKey(), 1, .element)
      return try n.decorator.getTextContent()
    },
  ]

  static let all: [DecoratorTextCase] = base.flatMap { testCase in
    var inverse = testCase
    inverse.invertSelection = true
    return [testCase, inverse]
  }
}

/// `TestSlotHost` of the `Bare slot value is skipped (#8894)` case.
private final class TestSlotHost: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TestSlotHost.self, type: "test-slot-host", extends: ElementNode.self, slots: ["content"])
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  {
    false
  }
}

/// A `buildEditorFromExtensions` editor with RichTextExtension, `nodes`, and
/// `initialState` as its `$initialEditorState`.
private func buildRichTextEditor(
  nodes: [LexicalNodeConfig] = [], _ initialState: @escaping () throws -> Void
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "@test", dependencies: [RichTextExtension], nodes: { nodes },
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() })))
}

/// The root's children as `heading` or `paragraph`.
private func rootTypes(_ editor: Editor) throws -> [String] {
  try editor.read { try getRoot().getChildren().map { $0.getType() } }
}

/// A mounted `createTestEditor` with HeadingNode registered, as upstream's
/// test editor has it.
private func mountedHeadingEditor() throws -> Editor {
  try createMountedTestEditor(nodes: [.klass(HeadingNode.self)])
}

/// The style-value cases: a paragraph's text with `style`, the
/// `$createRangeSelection` + `insertParagraph` setup, then `body`.
private func withStyledText(
  _ style: String, _ body: @escaping (RangeSelection) throws -> Void
)
  throws
{
  let editor = try createMountedTestEditor()
  defer { editor.dispose() }
  try update(editor) {
    let paragraph = try createParagraphNode()
    let textNode = try createTextNode("Hello, World!")
    try textNode.setStyle(style)
    try paragraph.append(textNode)
    try getRoot().append(paragraph)
    let selection = createRangeSelection()
    try setSelection(selection)
    try selection.insertParagraph()
    try setAnchorPoint(textNode.getKey(), 0, .text)
    try setFocusPoint(textNode.getKey(), 10, .text)
    try body(selection)
  }
}

@Suite struct LexicalSelectionSuite {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Expect initial output to be a block with no text."
    ))
  func initialOutput() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    #expect(
      try fixture.html()
        == rootOpen
        + "<p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Bold format preserved when typing between consecutive line breaks"
    ))
  func boldPreservedBetweenLineBreaks() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.apply([formatBold, .insertText("hello")])
    // Move cursor between "he" and "llo"
    try fixture.update {
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
      let textNode = try #require(try paragraph.getFirstChildOrThrow() as? TextNode)
      try textNode.select(2, 2)
    }
    // Shift+Enter twice (logical line breaks)
    try fixture.update {
      if let selection = try getSelection() as? RangeSelection {
        try selection.insertLineBreak()
        try selection.insertLineBreak()
      }
    }
    // Move cursor between the two line breaks in the paragraph
    try fixture.update {
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
      let children = try paragraph.getChildren()
      var betweenOffset: Int?
      for i in 0..<(children.count - 1)
      where children[i] is LineBreakNode && children[i + 1] is LineBreakNode {
        betweenOffset = i + 1
        break
      }
      let offset = try #require(betweenOffset, "Expected to find consecutive line breaks")
      try paragraph.select(offset, offset)
    }
    try fixture.apply([.insertText("x")])
    try fixture.editor.read(.latest) { () throws in
      let xNode = try getRoot().getAllTextNodes().first { try $0.getTextContent() == "x" }
      #expect(xNode != nil)
      #expect(try xNode?.hasFormat("bold") == true)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing (#1)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in bold (#2)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in italic (#3)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in italic + bold (#4)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in underline (#5)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in strikethrough (#6)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Simple typing in underline + strikethrough (#7)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Deletion (#8)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Creation of an token node (#9)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Convert text to an token node (#10)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Creation of a segmented node (#11)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Convert text to a segmented node (#12)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that starts and ends on element and retain selection (#13)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format multiline text selection that starts and ends on element and retain selection (#14)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format multiline text selection that starts and ends within text (#15)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that starts on text and ends on element and retain selection (#16)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that starts on element and ends on text and retain selection (#17)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that starts on middle of token node should format complete node (#18)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that ends on middle of token node should format complete node (#19)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format token node if it is the single one selected (#20)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Format selection that contains a token node in the middle should format the token node (#21)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a space, delete word backward from end (#22)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a space, delete word forward from beginning (#23)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a space, delete word forward from beginning of preceding whitespace (#24)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a space, delete word backward from end of trailing whitespace (#25)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#28)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#29)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a non-breaking space, delete word backward from end (#31)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a non-breaking space, delete word forward from beginning (#32)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a non-breaking space, delete word forward from beginning of preceding whitespace (#33)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a non-breaking space, delete word backward from end of trailing whitespace (#34)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#37)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#38)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en quad, delete word backward from end (#40)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en quad, delete word forward from beginning (#41)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en quad, delete word forward from beginning of preceding whitespace (#42)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en quad, delete word backward from end of trailing whitespace (#43)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#46)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#47)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em quad, delete word backward from end (#49)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em quad, delete word forward from beginning (#50)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em quad, delete word forward from beginning of preceding whitespace (#51)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em quad, delete word backward from end of trailing whitespace (#52)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#55)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#56)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en space, delete word backward from end (#58)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en space, delete word forward from beginning (#59)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en space, delete word forward from beginning of preceding whitespace (#60)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a en space, delete word backward from end of trailing whitespace (#61)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#64)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#65)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em space, delete word backward from end (#67)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em space, delete word forward from beginning (#68)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em space, delete word forward from beginning of preceding whitespace (#69)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a em space, delete word backward from end of trailing whitespace (#70)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#73)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#74)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a three-per-em space, delete word backward from end (#76)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a three-per-em space, delete word forward from beginning (#77)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a three-per-em space, delete word forward from beginning of preceding whitespace (#78)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a three-per-em space, delete word backward from end of trailing whitespace (#79)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#82)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#83)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a four-per-em space, delete word backward from end (#85)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a four-per-em space, delete word forward from beginning (#86)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a four-per-em space, delete word forward from beginning of preceding whitespace (#87)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a four-per-em space, delete word backward from end of trailing whitespace (#88)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#91)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#92)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a six-per-em space, delete word backward from end (#94)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a six-per-em space, delete word forward from beginning (#95)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a six-per-em space, delete word forward from beginning of preceding whitespace (#96)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a six-per-em space, delete word backward from end of trailing whitespace (#97)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#100)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#101)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a figure space, delete word backward from end (#103)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a figure space, delete word forward from beginning (#104)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a figure space, delete word forward from beginning of preceding whitespace (#105)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a figure space, delete word backward from end of trailing whitespace (#106)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#109)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#110)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a punctuation space, delete word backward from end (#112)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a punctuation space, delete word forward from beginning (#113)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a punctuation space, delete word forward from beginning of preceding whitespace (#114)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a punctuation space, delete word backward from end of trailing whitespace (#115)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#118)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#119)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a thin space, delete word backward from end (#121)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a thin space, delete word forward from beginning (#122)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a thin space, delete word forward from beginning of preceding whitespace (#123)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a thin space, delete word backward from end of trailing whitespace (#124)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#127)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#128)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a hair space, delete word backward from end (#130)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a hair space, delete word forward from beginning (#131)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a hair space, delete word forward from beginning of preceding whitespace (#132)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type two words separated by a hair space, delete word backward from end of trailing whitespace (#133)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a sentence, move the caret to the middle and move with the arrows to the start (#136)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a text and token text, move the caret to the end of the first text (#137)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#30)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#39)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#48)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#57)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#66)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#75)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#84)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#93)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#102)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#111)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#120)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#129)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Paste text, move selection and delete word forward (#138)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#107)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#108)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#116)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#117)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#125)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#126)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#134)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#135)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#26)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#27)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#35)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#36)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#44)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#45)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#53)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#54)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#62)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#63)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#71)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#72)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#80)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#81)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#89)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#90)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#98)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Type a word, delete it and undo the deletion (#99)"
    ),
    arguments: SuiteCase.all)
  func suiteCase(_ testUnit: SuiteCase) throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.apply(testUnit.inputs)
    // Validate HTML matches
    #expect(try fixture.html() == testUnit.expectedHTML)
    // Validate selection matches
    #expect(try fixture.domSelection() == testUnit.expectedSelection)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insert text one selected node element selection"
    ))
  func insertTextOneSelectedNodeElementSelection() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let root = try getRoot()
      let paragraph = try #require(try root.getFirstChild() as? ParagraphNode)
      let elementNode = try createTestElementNode()
      let text = try createTextNode("foo")
      try paragraph.append(elementNode)
      try elementNode.append(text)
      let selection = createRangeSelection()
      try selection.anchor.set(text.__key, 0, .text)
      try selection.focus.set(paragraph.__key, 1, .element)
      try selection.insertText("")
      #expect(try root.getTextContent() == "")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > getNodes resolves nested block nodes"
    ))
  func getNodesResolvesNestedBlockNodes() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let elementNode = try createTestElementNode()
      let text = try createTextNode()
      try paragraph.append(elementNode)
      try elementNode.append(text)
      let selectedNodes = try #require(try getSelection()).getNodes()
      #expect(selectedNodes.count == 1)
      #expect(selectedNodes.first?.getKey() == text.getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Collapsed selection on end; add beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Collapsed selection on end; add beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Collapsed selection on end; add beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Collapsed selection on end; add beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Collapsed selection on end; add beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Collapsed selection on end; add beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Collapsed selection on end; add beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Collapsed selection on end; add beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Collapsed selection on end; replace beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Collapsed selection on end; replace beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add on beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add on beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitNodes - All selected; add on beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitNodes - All selected; add on beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove on beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove on beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace on beginning"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace on beginning (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Selection beginning; add on end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Selection beginning; add on end (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Selection beginning; add on end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Selection beginning; add on end (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Selection beginning; add on end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Selection beginning; add on end (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Selection beginning; remove on end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Selection beginning; remove on end (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Selection beginning; replace on end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Selection beginning; replace on end (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in end offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in end offset (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in end offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in end offset (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in end offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in end offset (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in end offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in end offset (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in end offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in end offset (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in middle (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in middle (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in middle (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in middle (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in middle (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (1)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (1) (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (2)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (2) (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with collapsed selection at offset #4221"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with collapsed selection at offset #4221 (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with non-collapsed selection at offset"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with non-collapsed selection at offset (inverse selection)"
    ),
    arguments: BlockMoveCase.all)
  func blockSelectionMoves(_ testCase: BlockMoveCase) throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    let (anchorOffset, focusOffset) =
      testCase.invertSelection
      ? (testCase.focusOffset, testCase.anchorOffset)
      : (testCase.anchorOffset, testCase.focusOffset)
    try fixture.update { () throws in
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let textNode = try createTextNode("foo")
      // Note: line break can't be selected by the DOM
      let linebreak = try createLineBreakNode()
      guard let selection = try getSelection() as? RangeSelection else { return }
      try paragraph.append(textNode, linebreak)
      try testCase.fnBefore?(paragraph, textNode)
      try selection.anchor.set(paragraph.getKey(), anchorOffset, .element)
      try selection.focus.set(paragraph.getKey(), focusOffset, .element)
      let (expectedAnchor, expectedAnchorOffset, expectedFocus, expectedFocusOffset) =
        try testCase.fn(paragraph, textNode)
      if !testCase.invertSelection {
        #expect(selection.anchor.key == expectedAnchor.__key)
        #expect(selection.anchor.offset == expectedAnchorOffset)
        #expect(selection.focus.key == expectedFocus.__key)
        #expect(selection.focus.offset == expectedFocusOffset)
      } else {
        #expect(selection.anchor.key == expectedFocus.__key)
        #expect(selection.anchor.offset == expectedFocusOffset)
        #expect(selection.focus.key == expectedAnchor.__key)
        #expect(selection.focus.offset == expectedAnchorOffset)
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > isBackward"
    ))
  func isBackward() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let paragraphKey = paragraph.getKey()
      let textNode = try createTextNode("foo")
      let textNodeKey = textNode.getKey()
      // Note: line break can't be selected by the DOM
      let linebreak = try createLineBreakNode()
      guard let selection = try getSelection() as? RangeSelection else { return }
      let anchor = selection.anchor
      let focus = selection.focus
      try paragraph.append(textNode, linebreak)
      try anchor.set(textNodeKey, 0, .text)
      try focus.set(textNodeKey, 0, .text)
      #expect(try !selection.isBackward())
      try anchor.set(paragraphKey, 1, .element)
      try focus.set(paragraphKey, 1, .element)
      #expect(try !selection.isBackward())
      try anchor.set(paragraphKey, 0, .element)
      try focus.set(paragraphKey, 1, .element)
      #expect(try !selection.isBackward())
      try anchor.set(paragraphKey, 1, .element)
      try focus.set(paragraphKey, 0, .element)
      #expect(try selection.isBackward())
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right before it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right before it (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right after it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right after it (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected within text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected within text (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node before it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node before it (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node after it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node after it (inverse selection)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected as the only node"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected as the only node (inverse selection)"
    ),
    arguments: DecoratorTextCase.all)
  func decoratorTextContent(_ testCase: DecoratorTextCase) throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let textNode1 = try createTextNode("1")
      let textNode2 = try createTextNode("2")
      let decorator = try createTestDecoratorNode()
      try paragraph.append(textNode1, decorator, textNode2)
      guard let selection = try getSelection() as? RangeSelection else { return }
      let expectedTextContent = try testCase.fn(
        DecoratorTextCase.Nodes(
          textNode1: textNode1, textNode2: textNode2, decorator: decorator, paragraph: paragraph,
          anchor: testCase.invertSelection ? selection.focus : selection.anchor,
          focus: testCase.invertSelection ? selection.anchor : selection.focus))
      #expect(try selection.getTextContent() == expectedTextContent)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insertParagraph > three text nodes at offset 0 on third node"
    ))
  func insertParagraphThreeTextNodes() throws {
    let testEditor = try createMountedTestEditor()
    defer { testEditor.dispose() }
    try update(testEditor) {
      let paragraph = try createParagraphNode()
      let text = try createTextNode("Hello ")
      let text2 = try createTextNode("awesome")
      try text2.toggleFormat("bold")
      let text3 = try createTextNode(" world")
      try paragraph.append(text, text2, text3)
      try getRoot().append(paragraph)
      try setAnchorPoint(text3.getKey(), 0, .text)
      try setFocusPoint(text3.getKey(), 0, .text)
      guard let selection = try getSelection() as? RangeSelection else { return }
      try selection.insertParagraph()
    }
    #expect(
      try renderedHTML(testEditor)
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">Hello </span><strong data-lexical-text=\"true\">awesome</strong></p><p dir=\"auto\"><span data-lexical-text=\"true\"> world</span></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insertParagraph > four text nodes at offset 0 on third node"
    ))
  func insertParagraphFourTextNodes() throws {
    let testEditor = try createMountedTestEditor()
    defer { testEditor.dispose() }
    try update(testEditor) {
      let paragraph = try createParagraphNode()
      let text = try createTextNode("Hello ")
      let text2 = try createTextNode("awesome ")
      try text2.toggleFormat("bold")
      let text3 = try createTextNode("beautiful")
      let text4 = try createTextNode(" world")
      try text4.toggleFormat("bold")
      try paragraph.append(text, text2, text3, text4)
      try getRoot().append(paragraph)
      try setAnchorPoint(text3.getKey(), 0, .text)
      try setFocusPoint(text3.getKey(), 0, .text)
      guard let selection = try getSelection() as? RangeSelection else { return }
      try selection.insertParagraph()
    }
    #expect(
      try renderedHTML(testEditor)
        == "<p dir=\"auto\"><span data-lexical-text=\"true\">Hello </span><strong data-lexical-text=\"true\">awesome </strong></p><p dir=\"auto\"><span data-lexical-text=\"true\">beautiful</span><strong data-lexical-text=\"true\"> world</strong></p>"
    )
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $patchStyle > should patch the style with the new style object"
    ))
  func patchStyleWithObject() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("Hello, World!")
      try textNode.setStyle("font-family: serif; color: red;")
      try paragraph.append(textNode)
      try getRoot().append(paragraph)
      let selection = createRangeSelection()
      try setSelection(selection)
      try selection.insertParagraph()
      try setAnchorPoint(textNode.getKey(), 0, .text)
      try setFocusPoint(textNode.getKey(), 10, .text)
      try patchStyleText(selection, ["color": "blue", "font-family": "Arial"])
      #expect(try getSelectionStyleValueForProperty(selection, "font-family", "") == "Arial")
      #expect(try getSelectionStyleValueForProperty(selection, "color", "") == "blue")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $patchStyle > should patch the style with property function"
    ))
  func patchStyleWithFunction() throws {
    let fixture = try Fixture()
    defer { fixture.dispose() }
    try fixture.update { () throws in
      let currentColor = "red"
      let nextColor = "blue"
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("Hello, World!")
      try textNode.setStyle("color: \(currentColor);")
      try paragraph.append(textNode)
      try getRoot().append(paragraph)
      let selection = createRangeSelection()
      try setSelection(selection)
      try selection.insertParagraph()
      try setAnchorPoint(textNode.getKey(), 0, .text)
      try setFocusPoint(textNode.getKey(), 10, .text)
      var calls: [(String?, StylePatchTarget)] = []
      try patchStyleText(
        selection,
        [
          "color": .compute { current, target in
            calls.append((current, target))
            return nextColor
          }
        ])
      #expect(try getSelectionStyleValueForProperty(selection, "color", "") == nextColor)
      #expect(calls.count == 1)
      let lastCall = try #require(calls.last)
      #expect(lastCall.0 == currentColor)
      guard case .text = lastCall.1 else {
        Issue.record("expected the patch to be called with a TextNode")
        return
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Node.replace > moves selection to next text node if replacing with decorator"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Node.replace > moves selection to parent if next sibling is not a text node"
    ),
    arguments: [false, true])
  func nodeReplace(_ nextIsDecorator: Bool) throws {
    let testEditor = try createMountedTestEditor()
    defer { testEditor.dispose() }
    var text2: TextNode!
    var text3: TextNode!
    var paragraph: ParagraphNode!
    try update(testEditor) {
      paragraph = try createParagraphNode()
      let text1 = try createTextNode("Hello ")
      text2 = try createTextNode("awesome")
      try text2.toggleFormat("bold")
      text3 = try createTextNode(" world")
      try paragraph.append(text1, text2, text3)
      try getRoot().append(paragraph)
    }
    try update(testEditor) { () throws in
      let (key, offset): (NodeKey, Int)
      if !nextIsDecorator {
        try text2.select(1, 1)
        try text2.replace(try createTestDecoratorNode())
        (key, offset) = (text3.__key, 0)
      } else {
        try text3.replace(try createTestDecoratorNode())
        try text2.select(1, 1)
        try text2.replace(try createTestDecoratorNode())
        (key, offset) = (paragraph.__key, 2)
      }
      guard let selection = try getSelection() as? RangeSelection else { return }
      #expect(selection.anchor.key == key)
      #expect(selection.anchor.offset == offset)
      #expect(selection.focus.key == key)
      #expect(selection.focus.offset == offset)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that $getStyleObjectFromRawCSS handles unformatted css text  > "
    ))
  func styleObjectFromUnformattedCSS() throws {
    try withStyledText("   font-family  : Arial  ;  color    :   red   ;top     : 50px") {
      selection throws in
      #expect(try getSelectionStyleValueForProperty(selection, "font-family", "") == "Arial")
      #expect(try getSelectionStyleValueForProperty(selection, "color", "") == "red")
      #expect(try getSelectionStyleValueForProperty(selection, "top", "") == "50px")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that getStyleObjectFromRawCSS handles values with colons > "
    ))
  func styleObjectWithColons() throws {
    try withStyledText("font-family: double:prefix:Arial; color: color:white; font-size: 30px") {
      selection throws in
      #expect(
        try getSelectionStyleValueForProperty(selection, "font-family", "")
          == "double:prefix:Arial")
      #expect(try getSelectionStyleValueForProperty(selection, "color", "") == "color:white")
      #expect(try getSelectionStyleValueForProperty(selection, "font-size", "") == "30px")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that getStyleObjectFromRawCSS handles comments and semicolons inside values > "
    ))
  func styleObjectWithCommentsAndSemicolons() throws {
    try withStyledText(
      "background-image: url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\"); /* ignored */ content: \"semi;colon:value\"; color: red;"
    ) { selection throws in
      #expect(
        try getSelectionStyleValueForProperty(selection, "background-image", "")
          == "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\")")
      #expect(
        try getSelectionStyleValueForProperty(selection, "content", "") == "\"semi;colon:value\"")
      #expect(try getSelectionStyleValueForProperty(selection, "color", "") == "red")
    }
  }

  // MARK: $setBlocksType

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed selection in text"
    ))
  func setBlocksTypeCollapsedInText() throws {
    let testEditor = try mountedHeadingEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      let paragraph1 = try createParagraphNode()
      let text1 = try createTextNode("text 1")
      let paragraph2 = try createParagraphNode()
      let text2 = try createTextNode("text 2")
      try root.append(paragraph1, paragraph2)
      try paragraph1.append(text1)
      try paragraph2.append(text2)
      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint(text1.__key, text1.__text.utf16.count, .text)
      try setFocusPoint(text1.__key, text1.__text.utf16.count, .text)
      try setBlocksType(selection) { try createHeadingNode("h1") }
      #expect(try root.getChildren().map { $0.__type } == ["heading", "paragraph"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed selection in element"
    ))
  func setBlocksTypeCollapsedInElement() throws {
    let testEditor = try mountedHeadingEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      try root.append(try createParagraphNode(), try createParagraphNode())
      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint("root", 0, .element)
      try setFocusPoint("root", 0, .element)
      try setBlocksType(selection) { try createHeadingNode("h1") }
      #expect(try root.getChildren().map { $0.__type } == ["heading", "paragraph"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two elements, same top-element"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two elements, same top-element #2"
    ),
    arguments: [1, 2])
  func setBlocksTypeTwoElements(_ copy: Int) throws {
    // Upstream has this test twice, with the same body.
    let testEditor = try mountedHeadingEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      let paragraph1 = try createParagraphNode()
      let text1 = try createTextNode("text 1")
      let paragraph2 = try createParagraphNode()
      let text2 = try createTextNode("text 2")
      try root.append(paragraph1, paragraph2)
      try paragraph1.append(text1)
      try paragraph2.append(text2)
      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint(text1.__key, 0, .text)
      try setFocusPoint(text2.__key, text1.__text.utf16.count, .text)
      try setBlocksType(selection) { try createHeadingNode("h1") }
      #expect(try root.getChildren().map { $0.__type } == ["heading", "heading"])
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two empty elements, same top-element"
    ))
  func setBlocksTypeTwoEmptyElements() throws {
    let testEditor = try mountedHeadingEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      let paragraph1 = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      try root.append(paragraph1, paragraph2)
      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint(paragraph1.__key, 0, .element)
      try setFocusPoint(paragraph2.__key, 0, .element)
      try setBlocksType(selection) { try createHeadingNode("h1") }
      let rootChildren = try root.getChildren()
      #expect(rootChildren.map { $0.__type } == ["heading", "heading"])
      let sel = try #require(try getSelection() as? RangeSelection)
      #expect(pointString(sel.anchor) == "\(rootChildren[0].__key):0:element")
      #expect(pointString(sel.focus) == "\(rootChildren[1].__key):0:element")
      #expect(try sel.getNodes().map { $0.getKey() } == rootChildren.map { $0.getKey() })
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at element offset 0 of non-empty next block is skipped"
    ))
  func tripleClickElementOffsetNonEmpty() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let paragraph2 = try createParagraphNode()
      try getRoot().append(
        try createParagraphNode().append(text1), try paragraph2.append(try createLineBreakNode()))
      // Browser triple-click: focus lands on the text node of the following
      // block at offset 0, even though visually only paragraph1's content is
      // selected.
      let selection = try text1.selectStart()
      try selection.focus.set(paragraph2.getKey(), 0, .element)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "paragraph"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at element offset 0 of empty next block is converted"
    ))
  func tripleClickElementOffsetEmpty() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let paragraph2 = try createParagraphNode()
      try getRoot().append(try createParagraphNode().append(text1), paragraph2)
      let selection = try text1.selectStart()
      try selection.focus.set(paragraph2.getKey(), 0, .element)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at offset 0 of next block is skipped"
    ))
  func tripleClickTextOffsetZero() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2))
      let selection = try text1.select().setTextNodeRange(text1, 0, text2, 0)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "paragraph"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Non-zero focus offset in next block still converts both blocks"
    ))
  func nonZeroFocusOffsetConvertsBoth() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2))
      let selection = try text1.select().setTextNodeRange(text1, 0, text2, 1)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection spanning multiple blocks skips only the focus block"
    ))
  func tripleClickSpanningMultipleBlocks() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      let text3 = try createTextNode("text 3")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2),
        try createParagraphNode().append(text3))
      let selection = try text1.select().setTextNodeRange(text1, 0, text3, 0)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "heading", "paragraph"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Focus at offset 0 in next block whose first descendant has a prior sibling still converts focus block"
    ))
  func focusAfterPriorSiblingConverts() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2a = try createTextNode("foo")
      let text2b = try createTextNode("bar").setFormat("bold")
      try getRoot().append(
        try createParagraphNode().append(text1),
        // text2b is the second child; offset 0 of text2b is NOT at the start
        // of paragraph2 (text2a precedes it).
        try createParagraphNode().append(text2a, text2b))
      let selection = try text1.select().setTextNodeRange(text1, 0, text2b, 0)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: focus at block end is skipped"
    ))
  func backwardFocusAtBlockEndSkipped() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2))
      let selection = try text1.select().setTextNodeRange(
        text2, 0, text1, text1.getTextContentSize())
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["paragraph", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: non-zero focus offset converts both blocks"
    ))
  func backwardNonZeroFocusConvertsBoth() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2))
      let selection = try text1.select().setTextNodeRange(
        text2, 1, text1, text1.getTextContentSize() - 1)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["heading", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection spanning multiple blocks skips only focus block"
    ))
  func backwardSpanningMultipleBlocks() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      let text3 = try createTextNode("text 3")
      try getRoot().append(
        try createParagraphNode().append(text1), try createParagraphNode().append(text2),
        try createParagraphNode().append(text3))
      let selection = try text1.select().setTextNodeRange(
        text3, text3.getTextContentSize(), text1, text1.getTextContentSize())
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["paragraph", "heading", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: focus at block end with element point is skipped"
    ))
  func backwardFocusElementPointSkipped() throws {
    let testEditor = try buildRichTextEditor {
      let text1 = try createTextNode("text 1")
      let text2 = try createTextNode("text 2")
      let paragraph2 = try createParagraphNode().append(text2)
      try getRoot().append(try createParagraphNode().append(text1), paragraph2)
      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint(text2.getKey(), 0, .text)
      try setFocusPoint(paragraph2.getKey(), 0, .element)
      try setBlocksType(selection) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    #expect(try rootTypes(testEditor) == ["paragraph", "heading"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Bare slot value is skipped (#8894)"
    ))
  func bareSlotValueSkipped() throws {
    let testEditor = try buildRichTextEditor(nodes: [.klass(TestSlotHost.self)]) {
      let host = try create(TestSlotHost.self)
      let paragraph = try createParagraphNode()
      let text = try createTextNode("slot text")
      try paragraph.append(text)
      try setSlot(host, "content", paragraph)
      try getRoot().append(host)
      try setBlocksType(text.select(0)) { try createHeadingNode("h1") }
    }
    defer { testEditor.dispose() }
    try testEditor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      let paragraph = try #require(try getSlot(host, "content") as? ParagraphNode)
      #expect(try paragraph.getTextContent() == "slot text")
    }
  }
}
