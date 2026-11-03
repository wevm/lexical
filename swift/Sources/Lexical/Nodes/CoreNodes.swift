// Ports of packages/lexical/src/nodes/LexicalRootNode.ts,
// LexicalParagraphNode.ts, LexicalLineBreakNode.ts, LexicalTabNode.ts and
// LexicalDecoratorNode.ts (the model; rendering lives in the UI layers).

// MARK: - RootNode

/// The root of every editor state, with the key `root`.
public final class RootNode: ElementNode {
  package var __cachedText: String?

  public override class func config() -> NodeConfig? {
    NodeConfig(RootNode.self, type: "root", extends: ElementNode.self)
  }

  public required init(key: NodeKey? = nil) throws {
    try super.init(key: "root")
  }

  public override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    false
  }

  // `__cachedText` is not carried across a clone: as in JS, a clone starts
  // without it until the reconciler fills it in.

  public override func getTopLevelElementOrThrow() throws -> LexicalNode {
    throw LexicalError("getTopLevelElementOrThrow: root nodes are not top level elements")
  }

  public override func getTextContent() throws -> String {
    if let cachedText = try getCachedText(self) {
      return cachedText
    }
    return try super.getTextContent()
  }

  public override func getTextContentSize() throws -> Int {
    if let cachedText = try getCachedText(self) {
      return cachedText.utf16.count
    }
    return try super.getTextContentSize()
  }

  public override func remove(preserveEmptyParent: Bool = false) throws {
    throw LexicalError("remove: cannot be called on root nodes")
  }

  public override func replace<N: LexicalNode>(
    _ replaceWith: N, includeChildren: Bool = false
  )
    throws -> N
  {
    throw LexicalError("replace: cannot be called on root nodes")
  }

  public override func insertBefore(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  )
    throws -> LexicalNode
  {
    throw LexicalError("insertBefore: cannot be called on root nodes")
  }

  public override func insertAfter(
    _ nodeToInsert: LexicalNode, restoreSelection: Bool = true
  )
    throws -> LexicalNode
  {
    throw LexicalError("insertAfter: cannot be called on root nodes")
  }

  @discardableResult
  public override func splice(
    _ start: Int, _ deleteCount: Int, _ nodesToInsert: [LexicalNode]
  )
    throws -> Self
  {
    for node in nodesToInsert {
      try invariant(
        node is ElementNode || node is DecoratorNode,
        "rootNode.splice: Only element or decorator nodes can be inserted to the root node")
    }
    return try super.splice(start, deleteCount, nodesToInsert)
  }

  public override class func importJSON(_ serializedNode: JSONObject) throws -> LexicalNode {
    // We don't create a root, and instead use the existing root.
    try getRoot().updateFromJSON(serializedNode)
  }

  public override func collapseAtStart(_ selection: RangeSelection) throws -> Bool { true }
}

func getCachedText(_ node: RootNode) throws -> String? {
  let cachedText = try node.getLatest().__cachedText
  if let cachedText, try isCurrentlyReadOnlyMode() || getActiveEditor()._dirtyType == NO_DIRTY_NODES
  {
    return cachedText
  }
  return nil
}

/// `$createRootNode()`.
public func createRootNode() throws -> RootNode {
  try RootNode()
}

/// `$isRootNode(node)`.
public func isRootNode(_ node: LexicalNode?) -> Bool { node is RootNode }

// MARK: - ParagraphNode

/// A paragraph of inline content.
open class ParagraphNode: ElementNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(ParagraphNode.self, type: "paragraph", extends: ElementNode.self)
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let dom = getDocument().createElement("p")
    if let classNames = getCachedClassNameArray(config.theme, "paragraph") {
      dom.classList.add(contentsOf: classNames)
    }
    return dom
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  {
    false
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let output = try super.exportDOM(editor)
    if let element = output.element as? DOMElement {
      if try isEmpty() {
        element.append(getDocument().createElement("br"))
      }
      let formatType = try getFormatType()
      if !formatType.isEmpty {
        element.style.setProperty("text-align", formatType)
      }
    }
    return DOMExportOutput(element: output.element)
  }

  open override func exportJSON(_ compact: Bool = false) throws -> JSONObject {
    var json = try super.exportJSON(compact)
    // If the paragraph's text styles are not serialized, take them from its
    // first text node, which is where they come from.
    if json["textFormat"] == nil || json["textStyle"] == nil {
      let firstTextNode = try getChildren().first { $0 is TextNode } as? TextNode
      let textFormat = try firstTextNode?.getFormat() ?? getTextFormat()
      let textStyle = try firstTextNode?.getStyle() ?? getTextStyle()
      if !compact || textFormat != 0 {
        json["textFormat"] = .int(textFormat)
      }
      if !compact || textStyle != "" {
        json["textStyle"] = .string(textStyle)
      }
    }
    return json
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection else {
      return false
    }
    if try getFormatType() == "" && getIndent() == 0 && getStyle() == "" {
      return false
    }
    if try isBlockFullySelected(self, selection) {
      let textContent = try getTextContent()
      return try textContent != "" && selection.getTextContent() == textContent
    }
    return false
  }

  open override func insertNewAfter(
    _ rangeSelection: RangeSelection, restoreSelection: Bool = true
  )
    throws -> LexicalNode?
  {
    let newElement = try createParagraphNode()
    try newElement.setTextFormat(rangeSelection.format)
    try newElement.setTextStyle(rangeSelection.style)
    let direction = try getDirection()
    try newElement.setDirection(direction)
    try newElement.setFormat(getFormatType())
    try newElement.setStyle(getStyle())
    try insertAfter(newElement, restoreSelection: restoreSelection)
    return newElement
  }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool {
    let children = try getChildren()
    // If we have an empty (trimmed) first paragraph and try and remove it,
    // delete the paragraph as long as we have another sibling to go to
    let onlyWhitespace = try children.allSatisfy { node in
      try node is TextNode && !node.getTextContent().contains(where: { !$0.isJSWhitespace })
    }
    if children.isEmpty || onlyWhitespace {
      if try getNextSibling() != nil {
        try selectNext()
        try remove()
        return true
      }
      if try getPreviousSibling() != nil {
        try selectPrevious()
        try remove()
        return true
      }
    }
    return false
  }
}

/// `$createParagraphNode()`.
public func createParagraphNode() throws -> ParagraphNode {
  try applyNodeReplacement(ParagraphNode())
}

/// `$isParagraphNode(node)`.
public func isParagraphNode(_ node: LexicalNode?) -> Bool { node is ParagraphNode }

extension Character {
  /// Whether JS's `\s` matches every scalar of this character.
  package var isJSWhitespace: Bool {
    unicodeScalars.allSatisfy { scalar in
      switch scalar.value {
      case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F,
        0x3000, 0xFEFF:
        true
      default: false
      }
    }
  }
}

// MARK: - LineBreakNode

/// A line break within a block.
open class LineBreakNode: LexicalNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(LineBreakNode.self, type: "linebreak", extends: LexicalNode.self)
  }

  open override func getTextContent() throws -> String { "\n" }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    getDocument().createElement("br")
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  )
    throws -> Bool
  {
    false
  }

  open override func isInline() throws -> Bool { true }
}

/// `$createLineBreakNode()`.
public func createLineBreakNode() throws -> LineBreakNode {
  try applyNodeReplacement(LineBreakNode())
}

/// `$isLineBreakNode(node)`.
public func isLineBreakNode(_ node: LexicalNode?) -> Bool { node is LineBreakNode }

// MARK: - TabNode

/// A horizontal tab. Its text, detail, and mode are fixed.
open class TabNode: TextNode {
  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TabNode.self, type: "tab", extends: TextNode.self,
      json: [
        // Export-only: the values are fixed for a tab, so they are derived on
        // import rather than applied.
        SchemaField(
          "detail", numberValue(Double(IS_UNMERGEABLE)), of: TabNode.self,
          get: { .int($0.__detail) }, set: nil),
        SchemaField(
          "mode", enumValue(["normal"]), of: TabNode.self,
          get: { $0.__mode == 0 ? "normal" : nil }, set: nil),
        SchemaField(
          "text", stringValue("\t"), of: TabNode.self,
          get: { try .string($0.getTextContent()) }, set: nil),
      ])
  }

  public required init(key: NodeKey? = nil) throws {
    try super.init(key: key)
    __text = "\t"
    __detail = IS_UNMERGEABLE
  }

  /// Always stores `'\t'`, whatever it is given.
  @discardableResult
  open override func setTextContent(_ text: String) throws -> Self {
    try super.setTextContent("\t")
  }

  @discardableResult
  open override func spliceText(
    _ offset: Int, _ delCount: Int, _ newText: String, moveSelection: Bool = false
  ) throws -> TextNode {
    try invariant(
      (newText == "" && delCount == 0) || (newText == "\t" && delCount == 1),
      "TabNode does not support spliceText")
    return self
  }

  @discardableResult
  open override func setDetail(_ detail: Int) throws -> Self {
    try invariant(detail == IS_UNMERGEABLE, "TabNode does not support setDetail")
    return self
  }

  @discardableResult
  open override func setMode(_ type: TextModeType) throws -> Self {
    try invariant(type == "normal", "TabNode does not support setMode")
    return self
  }

  open override func canInsertTextBefore() throws -> Bool { false }

  open override func canInsertTextAfter() throws -> Bool { false }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let dom = try super.createDOM(config, editor)
    if let classNames = getCachedClassNameArray(config.theme, "tab") {
      dom.classList.add(contentsOf: classNames)
    }
    return dom
  }
}

/// `$createTabNode()`.
public func createTabNode() throws -> TabNode {
  try applyNodeReplacement(TabNode())
}

/// `$isTabNode(node)`.
public func isTabNode(_ node: LexicalNode?) -> Bool { node is TabNode }

// MARK: - DecoratorNode

/// A node rendered by an arbitrary view rather than as text: an image, an
/// embed. Subclasses return what to render from `decorate`.
open class DecoratorNode: LexicalNode, SlotNode {
  package var __slotHost: NodeKey?
  package var __slots: SlotMap?

  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prevNode = prevNode as! DecoratorNode
    if __key == prevNode.__key {
      __slotHost = prevNode.__slotHost
      try invariant(
        __slotHost == nil || __parent == nil,
        "DecoratorNode: node \(__key) is both slotted into host \(__slotHost ?? "null") and a child of parent \(__parent ?? "null"); __slotHost and __parent are mutually exclusive"
      )
      __slots = prevNode.__slots
    }
  }

  /// What the UI layer renders for this node: a SwiftUI view, for the
  /// native views. `nil` renders nothing.
  open func decorate(_ editor: Editor) throws -> Any? { nil }

  open func isIsolated() -> Bool { false }

  open override func isInline() throws -> Bool { true }

  open func isKeyboardSelectable() -> Bool { true }
}

/// `$isDecoratorNode(node)`.
public func isDecoratorNode(_ node: LexicalNode?) -> Bool { node is DecoratorNode }
