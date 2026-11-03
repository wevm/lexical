import Lexical

// Port of packages/lexical-extension/src/DecoratorTextExtension.ts.

nonisolated(unsafe) private let formatState = createState(
  "format",
  parse: { value -> Int in
    if case .number(let n)? = value, n.isFinite { return Int(n) }
    return 0
  })

/// `DecoratorTextNode`: an inline decorator that carries a text format, so
/// that `FORMAT_TEXT_COMMAND` formats it along with the text around it.
open class DecoratorTextNode: DecoratorNode, InlineFormattableNode {
  open override class func config() -> NodeConfig? {
    NodeConfig(
      DecoratorTextNode.self, type: "decorator-text", extends: DecoratorNode.self,
      stateConfigs: [RequiredNodeStateConfig(formatState, flat: true)])
  }

  public func getFormat() throws -> Int { try getState(self, formatState) }

  /// `getFormat(version)`.
  public func getFormat(_ version: NodeStateVersion) throws -> Int {
    try getState(self, formatState, version)
  }

  public func getFormatFlags(_ type: TextFormatType, _ alignWithFormat: Int?) throws -> Int {
    toggleTextFormatType(try getFormat(), type, alignWithFormat)
  }

  public func hasFormat(_ type: TextFormatType) throws -> Bool {
    let formatFlag = TEXT_TYPE_TO_FORMAT[type] ?? 0
    return (try getFormat() & formatFlag) != 0
  }

  @discardableResult
  public func setFormat(_ format: Int) throws -> Self {
    try setState(self, formatState, format)
  }

  /// `setFormat(updater)`.
  @discardableResult
  public func setFormat(_ updater: (Int) throws -> Int) throws -> Self {
    try setState(self, formatState, updater)
  }

  @discardableResult
  public func toggleFormat(_ type: TextFormatType) throws -> Self {
    let format = try getFormat()
    return try setFormat(toggleTextFormatType(format, type, nil))
  }

  open override func isInline() throws -> Bool { true }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    getDocument().createElement("span")
  }
}

/// `$isDecoratorTextNode(node)`.
public func isDecoratorTextNode(_ node: LexicalNode?) -> Bool {
  node is DecoratorTextNode
}

/// `applyFormatFromStyle(lexicalNode, style, shouldApply)`: turns on the
/// formats that inline CSS expresses (as Google Docs writes them).
@discardableResult
public func applyFormatFromStyle(
  _ lexicalNode: DecoratorTextNode, _ style: CSSStyleDeclaration,
  _ shouldApply: TextFormatType? = nil
) throws -> DecoratorTextNode {
  let fontWeight = style.getPropertyValue("font-weight")
  let textDecoration = style.getPropertyValue("text-decoration").split(
    separator: " ", omittingEmptySubsequences: false
  ).map(String.init)
  // Google Docs uses span tags + font-weight for bold text
  let hasBoldFontWeight = fontWeight == "700" || fontWeight == "bold"
  // Google Docs uses span tags + text-decoration: line-through for
  // strikethrough text
  let hasLinethroughTextDecoration = textDecoration.contains("line-through")
  // Google Docs uses span tags + font-style for italic text
  let hasItalicFontStyle = style.getPropertyValue("font-style") == "italic"
  // Google Docs uses span tags + text-decoration: underline for underline
  // text
  let hasUnderlineTextDecoration = textDecoration.contains("underline")
  // Google Docs uses span tags + vertical-align to specify subscript and
  // superscript
  let verticalAlign = style.getPropertyValue("vertical-align")

  if hasBoldFontWeight, try !lexicalNode.hasFormat("bold") {
    try lexicalNode.toggleFormat("bold")
  }
  if hasLinethroughTextDecoration, try !lexicalNode.hasFormat("strikethrough") {
    try lexicalNode.toggleFormat("strikethrough")
  }
  if hasItalicFontStyle, try !lexicalNode.hasFormat("italic") {
    try lexicalNode.toggleFormat("italic")
  }
  if hasUnderlineTextDecoration, try !lexicalNode.hasFormat("underline") {
    try lexicalNode.toggleFormat("underline")
  }
  if verticalAlign == "sub", try !lexicalNode.hasFormat("subscript") {
    try lexicalNode.toggleFormat("subscript")
  }
  if verticalAlign == "super", try !lexicalNode.hasFormat("superscript") {
    try lexicalNode.toggleFormat("superscript")
  }
  if let shouldApply, !shouldApply.isEmpty, try !lexicalNode.hasFormat(shouldApply) {
    try lexicalNode.toggleFormat(shouldApply)
  }
  return lexicalNode
}

/// `DEFAULT_TAG_NAME_TO_FORMAT`, in the key order of the JS object.
public let DEFAULT_TAG_NAME_TO_FORMAT: KeyValuePairs<String, TextFormatType> = [
  "b": "bold", "code": "code", "em": "italic", "i": "italic", "mark": "highlight",
  "s": "strikethrough", "strong": "bold", "sub": "subscript", "sup": "superscript",
  "u": "underline",
]

private func wrapElementWith(_ element: DOMNode, _ tag: String) -> DOMElement {
  let el = (element.ownerDocument ?? getDocument()).createElement(tag)
  _ = el.appendChild(element)
  return el
}

/// `applyFormatToDom(lexicalNode, domNode, tagNameToFormat)`: wraps
/// `domNode` in an element for each format the node has. JS also exports
/// it as `$applyFormatToDom`; without the `$` prefix the two are one name.
public func applyFormatToDom(
  _ lexicalNode: DecoratorTextNode, _ domNode: DOMNode,
  _ tagNameToFormat: KeyValuePairs<String, TextFormatType> = DEFAULT_TAG_NAME_TO_FORMAT
) throws -> DOMNode {
  var rval = domNode
  for (tag, format) in tagNameToFormat where try lexicalNode.hasFormat(format) {
    rval = wrapElementWith(rval, tag)
  }
  return rval
}

/// `DecoratorTextExtension`: registers `DecoratorTextNode`.
public let DecoratorTextExtension = defineExtension(
  name: "@lexical/extension/DecoratorText", nodes: { [.klass(DecoratorTextNode.self)] })
