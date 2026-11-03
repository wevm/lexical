import Lexical
import LexicalUtils

// Port of the nodes of packages/lexical-rich-text/src/index.ts: HeadingNode,
// QuoteNode and their HTML importers.

/// `HeadingTagType`: `h1` to `h6`.
public typealias HeadingTagType = String

let headingTags: [HeadingTagType] = ["h1", "h2", "h3", "h4", "h5", "h6"]

/// `quoteShadowRootState`: whether a quote opts in to shadow root behavior,
/// holding blocks instead of inline content. Serialized (flat) only when
/// `true`.
nonisolated(unsafe) public let quoteShadowRootState = createState(
  "shadowRoot", parse: { $0?.boolValue ?? false })

/// `QuoteNode`: a `<blockquote>`.
open class QuoteNode: ElementNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(
      QuoteNode.self, type: "quote", extends: ElementNode.self,
      stateConfigs: [RequiredNodeStateConfig(quoteShadowRootState, flat: true)],
      children: .inline)
  }

  open override class func importDOM() -> DOMConversionMap? {
    ["blockquote": { _ in DOMConversion(convertBlockquoteElement) }]
  }

  /// `true` when this quote has opted in to shadow root behavior with
  /// `setIsShadowRoot` or `createQuoteNode(shadowRoot: true)`, in which case
  /// it contains block-level children rather than inline content.
  ///
  /// `isShadowRoot()` doesn't throw in Swift; outside a read or an update,
  /// where JS throws, it reads this version of the node.
  open override func isShadowRoot() -> Bool {
    (try? getState(self, quoteShadowRootState))
      ?? ((try? getState(self, quoteShadowRootState, .direct)) ?? false)
  }

  /// Opt this quote in to (or out of) shadow root behavior. This does not
  /// restructure existing children.
  @discardableResult
  open func setIsShadowRoot(_ isShadowRoot: Bool) throws -> Self {
    try setState(self, quoteShadowRootState, isShadowRoot)
  }

  // View

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("blockquote")
    addClassNamesToElement(element, config.theme["quote"]?.stringValue)
    return element
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    false
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    try exportBlockDOM(self, super.exportDOM(editor))
  }

  // Mutation

  open override func insertNewAfter(
    _ rangeSelection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    let newBlock = try createParagraphNode()
    try newBlock.setTextFormat(rangeSelection.format)
    try newBlock.setTextStyle(rangeSelection.style)
    try newBlock.setDirection(getDirection())
    try newBlock.setFormat(getFormatType())
    try newBlock.setStyle(getStyle())
    try insertAfter(newBlock, restoreSelection: restoreSelection)
    return newBlock
  }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool {
    if isShadowRoot() {
      // A shadow root quote holds block-level children, so collapsing
      // dissolves the quote and lifts the blocks out as siblings rather
      // than merging them into a single paragraph.
      for child in try getChildren() {
        try insertBefore(child)
      }
      try remove()
      return true
    }
    let paragraph = try createParagraphNode()
    for child in try getChildren() {
      try paragraph.append(child)
    }
    try replace(paragraph)
    return true
  }

  open override func canMergeWhenEmpty() -> Bool { true }
}

/// `$createQuoteNode({shadowRoot})`.
public func createQuoteNode(shadowRoot: Bool = false) throws -> QuoteNode {
  let node = try applyNodeReplacement(QuoteNode())
  return shadowRoot ? try node.setIsShadowRoot(true) : node
}

/// `$isQuoteNode(node)`.
public func isQuoteNode(_ node: LexicalNode?) -> Bool { node is QuoteNode }

/// `HeadingNode`: an `<h1>` to `<h6>`.
open class HeadingNode: ElementNode {
  public var __tag: HeadingTagType = "h1"

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      HeadingNode.self, type: "heading", extends: ElementNode.self,
      json: [
        SchemaField("tag", enumValue(headingTags.map { .string($0) }), field: \HeadingNode.__tag)
      ], children: .inline)
  }

  open override class func importDOM() -> DOMConversionMap? {
    var map: DOMConversionMap = [:]
    for tag in headingTags {
      map[tag] = { _ in DOMConversion(convertHeadingElement) }
    }
    map["p"] = { node in
      // A Google Docs title is a paragraph whose first child is a 26pt span;
      // the span makes the heading, so the paragraph makes nothing.
      guard let firstChild = node.firstChild, isGoogleDocsTitle(firstChild) else { return nil }
      return DOMConversion(priority: 3) { _ in DOMConversionOutput() }
    }
    map["span"] = { node in
      guard isGoogleDocsTitle(node) else { return nil }
      return DOMConversion(priority: 3) { _ in
        DOMConversionOutput(node: [try createHeadingNode("h1")])
      }
    }
    return map
  }

  /// `new HeadingNode(tag, key)`.
  public convenience init(_ tag: HeadingTagType, key: NodeKey? = nil) throws {
    try self.init(key: key)
    __tag = tag
  }

  open func getTag() throws -> HeadingTagType {
    try getLatest().__tag
  }

  @discardableResult
  open func setTag(_ tag: HeadingTagType) throws -> Self {
    let writable = try getWritable()
    writable.__tag = tag
    return writable
  }

  // View

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let tag = __tag
    let element = getDocument().createElement(tag)
    if let classNames = config.theme["heading"]?.objectValue {
      addClassNamesToElement(element, classNames[tag]?.stringValue)
    }
    return element
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    (prevNode as! HeadingNode).__tag != __tag
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    try exportBlockDOM(self, super.exportDOM(editor))
  }

  // Mutation

  /// `insertNewAfter(selection?, restoreSelection)`: a paragraph after the
  /// heading, or the second half of a heading split in the middle.
  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    try insertNewAfter(selection as RangeSelection?, restoreSelection)
  }

  /// `insertNewAfter()` with no selection, which always adds a paragraph.
  @discardableResult
  public func insertNewAfter(restoreSelection: Bool = true) throws -> ElementNode {
    try insertNewAfter(nil, restoreSelection)
  }

  private func insertNewAfter(
    _ selection: RangeSelection?, _ restoreSelection: Bool
  ) throws
    -> ElementNode
  {
    let lastDesc = try getLastDescendant()
    let isAtEnd =
      try lastDesc == nil || (selection.map { try isAtEndOfNode($0.anchor, self) } ?? false)
    let isAtStart = try selection.map { try isAtStartOfNode($0.anchor, self) } ?? false
    let newElement: ElementNode
    if isAtEnd || selection == nil {
      newElement = try createParagraphNode()
    } else {
      // The heading is split in two, so the second half keeps the block
      // format and style of the first, like ParagraphNode does.
      newElement = try createHeadingNode(getTag()).setFormat(getFormatType()).setStyle(getStyle())
    }
    try newElement.setDirection(getDirection())
    try insertAfter(newElement, restoreSelection: restoreSelection)
    if try isAtStart && !isEmpty() {
      let paragraph = try createParagraphNode()
      try paragraph.select()
      try replace(paragraph, includeChildren: true)
    }
    return newElement
  }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool {
    try collapseAtStart()
  }

  /// `collapseAtStart()`: an empty heading becomes a paragraph.
  @discardableResult
  open func collapseAtStart() throws -> Bool {
    if try isEmpty() {
      let paragraph = try createParagraphNode()
      for child in try getChildren() {
        try paragraph.append(child)
      }
      try replace(paragraph)
    }
    return true
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    true
  }
}

/// `$createHeadingNode(headingTag)`.
public func createHeadingNode(_ headingTag: HeadingTagType = "h1") throws -> HeadingNode {
  try applyNodeReplacement(HeadingNode(headingTag))
}

/// `$isHeadingNode(node)`.
public func isHeadingNode(_ node: LexicalNode?) -> Bool { node is HeadingNode }

/// The `exportDOM` of both block nodes: an empty block gets a `<br>`, and
/// the alignment and direction go on the element.
private func exportBlockDOM(
  _ node: ElementNode, _ output: DOMExportOutput
) throws
  -> DOMExportOutput
{
  if let element = output.element as? DOMElement {
    if try node.isEmpty() {
      element.append(getDocument().createElement("br"))
    }
    let formatType = try node.getFormatType()
    if !formatType.isEmpty {
      element.style.setProperty("text-align", formatType)
    }
    if let direction = try node.getDirection(), !direction.isEmpty {
      element.dir = direction
    }
  }
  return DOMExportOutput(element: output.element)
}

private func isGoogleDocsTitle(_ domNode: DOMNode) -> Bool {
  guard let element = domNode as? DOMElement, element.localName == "span" else { return false }
  return element.style.getPropertyValue("font-size") == "26pt"
}

private func convertHeadingElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let element = domNode as? DOMElement else { return nil }
  let nodeName = element.localName
  guard headingTags.contains(nodeName) else { return DOMConversionOutput() }
  let node = try createHeadingNode(nodeName)
  try setNodeIndentFromDOM(element, node)
  try setFormatFromDOM(node, element)
  try setDirectionFromDOM(node, element)
  return DOMConversionOutput(node: [node])
}

private func convertBlockquoteElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let element = domNode as? DOMElement else { return nil }
  let node = try createQuoteNode()
  try setFormatFromDOM(node, element)
  try setNodeIndentFromDOM(element, node)
  try setDirectionFromDOM(node, element)
  return DOMConversionOutput(node: [node])
}
