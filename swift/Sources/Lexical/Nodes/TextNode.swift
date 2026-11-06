// Port of packages/lexical/src/nodes/LexicalTextNode.ts (the model; the DOM
// rendering lives in the UI layers).

/// `TextFormatType`: `bold`, `underline`, `strikethrough`, `italic`,
/// `highlight`, `code`, `subscript`, `superscript`, `lowercase`, `uppercase`,
/// or `capitalize`.
public typealias TextFormatType = String

/// `TextModeType`: `normal`, `token`, or `segmented`.
public typealias TextModeType = String

/// `TextDetailType`: `directionless` or `unmergable`.
public typealias TextDetailType = String

/// A run of text with one format and style.
open class TextNode: LexicalNode {
  @exclusivity(unchecked) package var __text = ""
  @exclusivity(unchecked) package var __format = 0
  @exclusivity(unchecked) package var __style = ""
  @exclusivity(unchecked) package var __mode = 0
  @exclusivity(unchecked) package var __detail = 0

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TextNode.self, type: "text", extends: LexicalNode.self,
      json: [
        SchemaField(
          "detail", aliasedValue(numberValue(), DETAIL_TYPE_TO_DETAIL.mapValues(JSONValue.int)),
          of: TextNode.self,
          get: { try .int($0.getDetail()) },
          set: { try $0.setDetail(jsToInt32($1?.numberValue ?? 0)) }),
        SchemaField(
          "format", aliasedValue(numberValue(), TEXT_TYPE_TO_FORMAT.mapValues(JSONValue.int)),
          of: TextNode.self,
          get: { try .int($0.getFormat()) },
          set: { try $0.setFormat(jsToInt32($1?.numberValue ?? 0)) }),
        SchemaField(
          "mode", enumValue(["normal", "token", "segmented"]), of: TextNode.self,
          get: { try .string($0.getMode()) },
          set: { $0.__mode = $1?.stringValue.flatMap { TEXT_MODE_TO_TYPE[$0] } ?? IS_NORMAL }),
        SchemaField(
          "style", stringValue(), of: TextNode.self,
          get: { try .string($0.getStyle()) },
          set: { try $0.setStyle($1?.stringValue ?? "") }),
        SchemaField(
          "text", stringValue(), of: TextNode.self,
          get: { try .string($0.getTextContent()) },
          set: { try $0.setTextContent($1?.stringValue ?? "") }),
      ])
  }

  public required init(key: NodeKey? = nil) throws {
    try super.init(key: key)
  }

  public convenience init(_ text: String, key: NodeKey? = nil) throws {
    try self.init(key: key)
    __text = text
  }

  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prevNode = prevNode as! TextNode
    // afterCloneTextNode
    __detail = prevNode.__detail
    __format = prevNode.__format
    __mode = prevNode.__mode
    __style = prevNode.__style
    __text = prevNode.__text
  }

  open func getFormat() throws -> Int {
    try getLatest().__format
  }

  open func getDetail() throws -> Int {
    try getLatest().__detail
  }

  open func getMode() throws -> TextModeType {
    TEXT_TYPE_TO_MODE[try getLatest().__mode] ?? "normal"
  }

  open func getStyle() throws -> String {
    try getLatest().__style
  }

  public func isToken() throws -> Bool {
    try getLatest().__mode == IS_TOKEN
  }

  public func isComposing() throws -> Bool {
    try __key == getCompositionKey()
  }

  public func isSegmented() throws -> Bool {
    try getLatest().__mode == IS_SEGMENTED
  }

  public func isDirectionless() throws -> Bool {
    try (getLatest().__detail & IS_DIRECTIONLESS) != 0
  }

  public func isUnmergeable() throws -> Bool {
    try (getLatest().__detail & IS_UNMERGEABLE) != 0
  }

  public func hasFormat(_ type: TextFormatType) throws -> Bool {
    let formatFlag = TEXT_TYPE_TO_FORMAT[type] ?? 0
    return try (getFormat() & formatFlag) != 0
  }

  /// Whether the node is a plain `TextNode` in `normal` mode, which
  /// normalization may merge with its neighbors.
  public func isSimpleText() throws -> Bool {
    let latest = try getLatest()
    return latest.__type == "text" && latest.__mode == 0
  }

  open override func getTextContent() throws -> String {
    try getLatest().__text
  }

  public func getFormatFlags(_ type: TextFormatType, _ alignWithFormat: Int?) throws -> Int {
    toggleTextFormatType(try getLatest().__format, type, alignWithFormat)
  }

  open func canHaveFormat() -> Bool { true }

  // MARK: View

  open override class func importDOM() -> DOMConversionMap? { coreImporters }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let format = __format
    let outerTag = textElementOuterTag(format)
    let innerTag = textElementInnerTag(format)
    let dom = getDocument().createElement(outerTag ?? innerTag)
    var innerDOM = dom
    if try hasFormat("code") {
      dom.setAttribute("spellcheck", "false")
    }
    if outerTag != nil {
      innerDOM = getDocument().createElement(innerTag)
      dom.appendChild(innerDOM)
    }
    try createTextInnerDOM(innerDOM, self, innerTag, format, __text, config)
    if !__style.isEmpty {
      setDOMStyleFromCSS(dom.style, __style)
    }
    return dom
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  {
    let prevNode = prevNode as! TextNode
    let nextText = __text
    let prevFormat = prevNode.__format
    let nextFormat = __format
    let prevOuterTag = textElementOuterTag(prevFormat)
    let nextOuterTag = textElementOuterTag(nextFormat)
    let prevInnerTag = textElementInnerTag(prevFormat)
    let nextInnerTag = textElementInnerTag(nextFormat)
    if (prevOuterTag ?? prevInnerTag) != (nextOuterTag ?? nextInnerTag) {
      return true
    }
    if prevOuterTag == nextOuterTag && prevInnerTag != nextInnerTag {
      guard let prevInnerDOM = dom.firstChild else {
        throw LexicalError("updateDOM: prevInnerDOM is null or undefined")
      }
      let nextInnerDOM = getDocument().createElement(nextInnerTag)
      try createTextInnerDOM(nextInnerDOM, self, nextInnerTag, nextFormat, nextText, config)
      dom.replaceChild(nextInnerDOM, prevInnerDOM)
      return false
    }
    var innerDOM = dom
    if nextOuterTag != nil, prevOuterTag != nil {
      guard let first = dom.firstChild as? DOMElement else {
        throw LexicalError("updateDOM: innerDOM is null or undefined")
      }
      innerDOM = first
    }
    try setTextNodeDOMText(nextText, innerDOM, self)
    if let textClassNames = config.theme["text"]?.objectValue, prevFormat != nextFormat {
      setTextThemeClassNames(nextInnerTag, prevFormat, nextFormat, innerDOM, textClassNames)
    }
    let prevStyle = prevNode.__style
    let nextStyle = __style
    if prevStyle != nextStyle {
      setDOMStyleFromCSS(dom.style, nextStyle, prevStyle)
      removeEmptyDOMAttribute(dom, "style")
    }
    return false
  }

  /// Plain HTML for copy and paste and headless export, which can't rely
  /// on CSS classes: `white-space: pre-wrap`, text-transform for the case
  /// formats, and `<b>`, `<i>`, `<s>`, `<u>` wrappers.
  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let output = try super.exportDOM(editor)
    guard var element = output.element else {
      throw LexicalError("Expected TextNode createDOM to always return a HTMLElement")
    }
    guard let styled = element as? DOMElement else {
      throw LexicalError("Expected TextNode createDOM to always return a HTMLElement")
    }
    styled.style.setProperty("white-space", "pre-wrap")
    if try hasFormat("lowercase") {
      styled.style.setProperty("text-transform", "lowercase")
    } else if try hasFormat("uppercase") {
      styled.style.setProperty("text-transform", "uppercase")
    } else if try hasFormat("capitalize") {
      styled.style.setProperty("text-transform", "capitalize")
    }
    for (format, tag) in [
      ("bold", "b"), ("italic", "i"), ("strikethrough", "s"), ("underline", "u"),
    ]
    where try hasFormat(format) {
      let wrapper = getDocument().createElement(tag)
      wrapper.appendChild(element)
      element = wrapper
    }
    return DOMExportOutput(element: element)
  }

  open override func isInline() throws -> Bool { true }

  /// Called when the selection lands in this node; a subclass may move it.
  open func selectionTransform(
    _ prevSelection: BaseSelection?, _ nextSelection: RangeSelection
  )
    throws
  {}

  @discardableResult
  open func setFormat(_ format: Int) throws -> Self {
    let latest = try getWritable()
    latest.__format = format
    return latest
  }

  @discardableResult
  public func setFormat(_ format: TextFormatType) throws -> Self {
    try setFormat(TEXT_TYPE_TO_FORMAT[format] ?? 0)
  }

  @discardableResult
  open func setDetail(_ detail: Int) throws -> Self {
    let latest = try getWritable()
    latest.__detail = detail
    return latest
  }

  @discardableResult
  public func setDetail(_ detail: TextDetailType) throws -> Self {
    try setDetail(DETAIL_TYPE_TO_DETAIL[detail] ?? 0)
  }

  @discardableResult
  open func setStyle(_ style: String) throws -> Self {
    let latest = try getWritable()
    latest.__style = style
    return latest
  }

  @discardableResult
  public func toggleFormat(_ type: TextFormatType) throws -> Self {
    let format = try getFormat()
    let newFormat = toggleTextFormatType(format, type, nil)
    return try setFormat(newFormat)
  }

  @discardableResult
  public func toggleDirectionless() throws -> Self {
    let latest = try getWritable()
    latest.__detail ^= IS_DIRECTIONLESS
    return latest
  }

  @discardableResult
  public func toggleUnmergeable() throws -> Self {
    let latest = try getWritable()
    latest.__detail ^= IS_UNMERGEABLE
    return latest
  }

  @discardableResult
  open func setMode(_ type: TextModeType) throws -> Self {
    let mode = TEXT_MODE_TO_TYPE[type] ?? IS_NORMAL
    if try getLatest().__mode == mode {
      return self
    }
    let latest = try getWritable()
    latest.__mode = mode
    return latest
  }

  @discardableResult
  open func setTextContent(_ text: String) throws -> Self {
    if try getLatest().__text == text {
      return self
    }
    let writable = try getWritable()
    writable.__text = text
    return writable
  }

  @discardableResult
  open func select(
    _ _anchorOffset: Int? = nil, _ _focusOffset: Int? = nil
  ) throws
    -> RangeSelection
  {
    try errorOnReadOnly()
    var anchorOffset = _anchorOffset
    var focusOffset = _focusOffset
    let selection = try getSelection()
    let text = try getTextContent()
    let key = __key
    let lastOffset = text.utf16.count
    if anchorOffset == nil {
      anchorOffset = lastOffset
    }
    if focusOffset == nil {
      focusOffset = lastOffset
    }
    guard let selection = selection as? RangeSelection else {
      return try internalMakeRangeSelection(key, anchorOffset!, key, focusOffset!, .text, .text)
    }
    let compositionKey = try getCompositionKey()
    if compositionKey == selection.anchor.key || compositionKey == selection.focus.key {
      try setCompositionKey(key)
    }
    try selection.setTextNodeRange(self, anchorOffset!, self, focusOffset!)
    return selection
  }

  @discardableResult
  open override func selectStart() throws -> RangeSelection {
    try select(0, 0)
  }

  @discardableResult
  open override func selectEnd() throws -> RangeSelection {
    let size = try getTextContentSize()
    return try select(size, size)
  }

  /// Replaces `delCount` UTF-16 code units at `offset` with `newText`.
  @discardableResult
  open func spliceText(
    _ offset: Int, _ delCount: Int, _ newText: String, moveSelection: Bool = false
  ) throws -> TextNode {
    let writableSelf = try getWritable()
    let text = writableSelf.__text
    let handledTextLength = newText.utf16.count
    var index = offset
    if index < 0 {
      index = handledTextLength + index
      if index < 0 {
        index = 0
      }
    }
    let selection = try getSelection()
    if moveSelection, let selection = selection as? RangeSelection {
      let newOffset = offset + handledTextLength
      try selection.setTextNodeRange(writableSelf, newOffset, writableSelf, newOffset)
    }
    let updatedText = text.jsSlice(0, index) + newText + text.jsSlice(index + delCount)
    writableSelf.__text = updatedText
    return writableSelf
  }

  open func canInsertTextBefore() throws -> Bool { true }

  open func canInsertTextAfter() throws -> Bool { true }

  /// Splits the node at the given UTF-16 offsets, keeping this node as the
  /// first part (unless it is segmented), and returns the parts.
  @discardableResult
  open func splitText(_ splitOffsets: Int...) throws -> [TextNode] {
    try splitText(offsets: splitOffsets)
  }

  @discardableResult
  open func splitText(offsets: [Int]) throws -> [TextNode] {
    try errorOnReadOnly()
    let latest = try getLatest()
    let textContent = try latest.getTextContent()
    if textContent.isEmpty {
      return []
    }
    let key = latest.__key
    let compositionKey = try getCompositionKey()
    let textLength = textContent.utf16.count
    var splitOffsets = offsets.sorted()
    splitOffsets.append(textLength)
    var parts: [String] = []
    var start = 0
    var offsetIndex = 0
    while start < textLength && offsetIndex <= splitOffsets.count {
      // JS reads `undefined` past the end, and `undefined > start` is false.
      if offsetIndex < splitOffsets.count {
        let end = splitOffsets[offsetIndex]
        if end > start {
          parts.append(textContent.jsSlice(start, end))
          start = end
        }
      }
      offsetIndex += 1
    }
    if parts.count == 1 {
      return [latest]
    }
    let firstPart = parts[0]
    let parent = try latest.getParent()
    let writableNode: TextNode
    let format = try latest.getFormat()
    let style = try latest.getStyle()
    let detail = latest.__detail
    var hasReplacedSelf = false

    // Prepare to handle selection
    var startTextPoint: Point? = nil
    var endTextPoint: Point? = nil
    let selection = try getSelection()
    if let selection = selection as? RangeSelection {
      let (startPoint, endPoint) =
        try selection.isBackward()
        ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      if startPoint.type == .text && startPoint.key == key {
        startTextPoint = startPoint
      }
      if endPoint.type == .text && endPoint.key == key {
        endTextPoint = endPoint
      }
    }

    if try latest.isSegmented() {
      // Create a new TextNode
      writableNode = try createTextNode(firstPart)
      writableNode.__format = format
      writableNode.__style = style
      writableNode.__detail = detail
      writableNode.__state = cloneNodeState(latest, writableNode)
      hasReplacedSelf = true
    } else {
      // For the first part, update the existing node
      writableNode = try latest.setTextContent(firstPart)
    }

    // Then handle all other parts
    var splitNodes: [TextNode] = [writableNode]
    for part in parts.dropFirst() {
      let sibling = try createTextNode(part)
      sibling.__format = format
      sibling.__style = style
      sibling.__detail = detail
      sibling.__state = cloneNodeState(latest, sibling)
      let siblingKey = sibling.__key
      if compositionKey == key {
        try setCompositionKey(siblingKey)
      }
      splitNodes.append(sibling)
    }

    // Move the selection to the best location in the split string. The end
    // point is always left-biased, and the start point is generally
    // left-biased unless the end point would land on a later node in the
    // split, in which case it is right-biased.
    let originalStartOffset = startTextPoint?.offset
    let originalEndOffset = endTextPoint?.offset
    var startOffset = 0
    for node in splitNodes {
      if startTextPoint == nil && endTextPoint == nil {
        break
      }
      let endOffset = try startOffset + node.getTextContentSize()
      if let point = startTextPoint, let original = originalStartOffset,
        original <= endOffset && original >= startOffset
      {
        // Set the start point to the first valid node
        try point.set(node.getKey(), original - startOffset, .text)
        if original < endOffset {
          // The start isn't on a border so it can't be moved later.
          startTextPoint = nil
        }
      }
      if let point = endTextPoint, let original = originalEndOffset,
        original <= endOffset && original >= startOffset
      {
        try point.set(node.getKey(), original - startOffset, .text)
        break
      }
      startOffset = endOffset
    }

    // Insert the nodes into the parent's children
    if let parent {
      try internalMarkSiblingsAsDirty(self)
      let writableParent = try parent.getWritable()
      let insertionIndex = try getIndexWithinParent()
      if hasReplacedSelf {
        try writableParent.splice(insertionIndex, 0, splitNodes)
        try remove()
      } else {
        try writableParent.splice(insertionIndex, 1, splitNodes)
      }
      if let selection = selection as? RangeSelection {
        try updateElementSelectionOnCreateDeleteNode(
          selection, parent, insertionIndex, parts.count - 1)
      }
    }

    return splitNodes
  }

  /// Merges `target`, the previous or next sibling, into this node.
  @discardableResult
  public func mergeWithSibling(_ target: TextNode) throws -> TextNode {
    let isBefore = try target === getPreviousSibling()
    if try !isBefore && target !== getNextSibling() {
      throw LexicalError("mergeWithSibling: sibling must be a previous or next sibling")
    }
    let key = __key
    let targetKey = target.__key
    let text = __text
    let textLength = text.utf16.count
    let compositionKey = try getCompositionKey()

    if compositionKey == targetKey {
      try setCompositionKey(key)
    }
    if let selection = try getSelection() as? RangeSelection {
      let anchor = selection.anchor
      let focus = selection.focus
      if anchor.key == targetKey {
        try adjustPointOffsetForMergedSibling(anchor, isBefore, key, target, textLength)
      }
      if focus.key == targetKey {
        try adjustPointOffsetForMergedSibling(focus, isBefore, key, target, textLength)
      }
    }
    let targetText = target.__text
    let newText = isBefore ? targetText + text : text + targetText
    try setTextContent(newText)
    let writableSelf = try getWritable()
    try target.remove()
    return writableSelf
  }

  /// Whether the node is an entity (a hashtag, a mention) whose text is
  /// checked by `registerLexicalTextEntity`.
  open func isTextEntity() -> Bool { false }
}

/// `$createTextNode(text)`.
public func createTextNode(_ text: String = "") throws -> TextNode {
  try applyNodeReplacement(TextNode(text))
}

/// `$isTextNode(node)`.
public func isTextNode(_ node: LexicalNode?) -> Bool { node is TextNode }

extension String {
  /// `String.prototype.slice(start, end)` over UTF-16 code units, with JS's
  /// handling of negative and out-of-range indices.
  package func jsSlice(_ start: Int, _ end: Int? = nil) -> String {
    let utf16 = self.utf16
    let length = utf16.count
    func clamp(_ i: Int) -> Int { i < 0 ? Swift.max(length + i, 0) : Swift.min(i, length) }
    let from = clamp(start)
    let to = end.map(clamp) ?? length
    if from >= to {
      return ""
    }
    let lower = utf16.index(utf16.startIndex, offsetBy: from)
    let upper = utf16.index(lower, offsetBy: to - from)
    return String(utf16[lower..<upper])
      ?? String(decoding: Array(utf16[lower..<upper]), as: UTF16.self)
  }

  /// The UTF-16 code unit at `index`, as JS `text[index]` reads it.
  package func utf16Unit(at index: Int) -> UInt16? {
    let utf16 = self.utf16
    guard index >= 0, index < utf16.count else { return nil }
    return utf16[utf16.index(utf16.startIndex, offsetBy: index)]
  }
}

// MARK: - DOM helpers

private func textElementOuterTag(_ format: Int) -> String? {
  if format & IS_CODE != 0 { return "code" }
  if format & IS_HIGHLIGHT != 0 { return "mark" }
  if format & IS_SUBSCRIPT != 0 { return "sub" }
  if format & IS_SUPERSCRIPT != 0 { return "sup" }
  return nil
}

private func textElementInnerTag(_ format: Int) -> String {
  if format & IS_BOLD != 0 { return "strong" }
  if format & IS_ITALIC != 0 { return "em" }
  return "span"
}

/// The text formats in the key order of JS `TEXT_TYPE_TO_FORMAT`.
private let textFormatKeysInOrder = TEXT_TYPE_TO_FORMAT.keys.sorted()

private func setTextThemeClassNames(
  _ tag: String, _ prevFormat: Int, _ nextFormat: Int, _ dom: DOMElement,
  _ textClassNames: JSONObject
) {
  let classList = dom.classList
  if let classNames = themeClassNames(textClassNames["base"]) {
    classList.add(contentsOf: classNames)
  }
  // Underline and strikethrough together have their own class, since both
  // set text-decoration.
  var hasUnderlineStrikethrough = false
  let prevUnderlineStrikethrough =
    prevFormat & IS_UNDERLINE != 0 && prevFormat & IS_STRIKETHROUGH != 0
  let nextUnderlineStrikethrough =
    nextFormat & IS_UNDERLINE != 0 && nextFormat & IS_STRIKETHROUGH != 0
  if let classNames = themeClassNames(textClassNames["underlineStrikethrough"]) {
    if nextUnderlineStrikethrough {
      hasUnderlineStrikethrough = true
      if !prevUnderlineStrikethrough { classList.add(contentsOf: classNames) }
    } else if prevUnderlineStrikethrough {
      classList.remove(contentsOf: classNames)
    }
  }
  for key in textFormatKeysInOrder {
    let flag = TEXT_TYPE_TO_FORMAT[key]!
    guard let classNames = themeClassNames(textClassNames[key]) else { continue }
    if nextFormat & flag != 0 {
      if hasUnderlineStrikethrough && (key == "underline" || key == "strikethrough") {
        if prevFormat & flag != 0 { classList.remove(contentsOf: classNames) }
        continue
      }
      if prevFormat & flag == 0 || (prevUnderlineStrikethrough && key == "underline")
        || key == "strikethrough"
      {
        classList.add(contentsOf: classNames)
      }
    } else if prevFormat & flag != 0 {
      classList.remove(contentsOf: classNames)
    }
  }
  removeEmptyDOMAttribute(dom, "class")
}

/// `$setTextContent`: writes the node's text (with the composition suffix
/// while composing) into the first child of its DOM slot.
private func setTextNodeDOMText(_ nextText: String, _ dom: DOMElement, _ node: TextNode) throws {
  let text = try nextText + (node.isComposing() ? COMPOSITION_SUFFIX : "")
  let editor = try getEditor()
  let slot = try getEditorDOMRenderConfig(editor).getDOMSlot(node, dom, editor)
  guard let textChild = slot.getFirstChild() as? DOMText else {
    try slot.insertChild(getDocument().createTextNode(text))
    return
  }
  if textChild.data != text { textChild.data = text }
}

private func createTextInnerDOM(
  _ innerDOM: DOMElement, _ node: TextNode, _ innerTag: String, _ format: Int, _ text: String,
  _ config: EditorConfig
) throws {
  try setTextNodeDOMText(text, innerDOM, node)
  if let textClassNames = config.theme["text"]?.objectValue {
    setTextThemeClassNames(innerTag, 0, format, innerDOM, textClassNames)
  }
}
