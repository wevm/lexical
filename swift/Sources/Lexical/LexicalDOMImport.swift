// The legacy HTML import contract of nodes: `importDOM` maps a DOM node
// name to the conversions that may turn it into Lexical nodes, and the
// editor caches them by name (`_htmlConversions`). `@lexical/html`'s
// `generateNodesFromDOM` walks the DOM with them.

/// `DOMConversionOutput`: what a conversion makes of a DOM node.
public struct DOMConversionOutput {
  /// Rewrites the Lexical nodes made from the DOM node's children.
  public var after: (([LexicalNode]) throws -> [LexicalNode])?
  /// Applied to each Lexical node made from a descendant.
  public var forChild: DOMChildConversion?
  /// The nodes made; the last one receives the children.
  public var node: [LexicalNode]

  public init(
    node: [LexicalNode] = [], forChild: DOMChildConversion? = nil,
    after: (([LexicalNode]) throws -> [LexicalNode])? = nil
  ) {
    self.node = node
    self.forChild = forChild
    self.after = after
  }

  public init(
    node: LexicalNode?, forChild: DOMChildConversion? = nil,
    after: (([LexicalNode]) throws -> [LexicalNode])? = nil
  ) {
    self.init(node: node.map { [$0] } ?? [], forChild: forChild, after: after)
  }
}

/// `DOMChildConversion`: maps a descendant's Lexical node (given its
/// parent's) to the node to keep, or `nil` to drop it.
public typealias DOMChildConversion = (LexicalNode, LexicalNode?) throws -> LexicalNode?

/// `DOMConversionFn`.
public typealias DOMConversionFn = (DOMNode) throws -> DOMConversionOutput?

/// `DOMConversion`: a conversion and its priority (0–4); the highest
/// priority wins, and the last registered among equals.
public struct DOMConversion {
  public var conversion: DOMConversionFn
  public var priority: Int

  public init(priority: Int = 0, _ conversion: @escaping DOMConversionFn) {
    self.conversion = conversion
    self.priority = priority
  }
}

/// `DOMConversionMap`: lower-case node names (`p`, `#text`) to a function
/// that offers a conversion for a DOM node, or `nil` to decline it.
public typealias DOMConversionMap = [String: (DOMNode) throws -> DOMConversion?]

extension LexicalNode {
  /// The importers of the class and of the `html.import` config, by node
  /// name, in registration order (`initializeConversionCache`).
  static func conversionCache(
    _ nodes: JSMap<String, RegisteredNode>, _ additional: DOMConversionMap?
  ) -> [String: [(DOMNode) throws -> DOMConversion?]] {
    var cache: [String: [(DOMNode) throws -> DOMConversion?]] = [:]
    func add(_ map: DOMConversionMap) {
      for key in map.keys.sorted() { cache[key, default: []].append(map[key]!) }
    }
    // JS skips a class whose static importDOM is its superclass's; Swift
    // can't tell, and repeating a conversion changes nothing (equal
    // priorities keep the last, which is the same one).
    for (_, node) in nodes {
      if let map = node.klass.importDOM() { add(map) }
    }
    if let additional { add(additional) }
    return cache
  }

}

/// `isInlineDomNode(node)`: a text node, or an element with an inline tag
/// or an inline `display`.
public func isInlineDomNode(_ node: DOMNode) -> Bool {
  if let element = node as? DOMElement, element.style["display"].hasPrefix("inline") {
    return true
  }
  return inlineTags.contains(node.nodeName.lowercased())
}

private let inlineTags: Set<String> = [
  "a", "abbr", "acronym", "b", "cite", "code", "del", "em", "i", "ins", "kbd", "label", "mark",
  "output", "q", "ruby", "s", "samp", "span", "strong", "sub", "sup", "time", "u", "tt", "var",
  "#text",
]

/// `isDOMTextNode(node)`.
public func isDOMTextNode(_ node: DOMNode?) -> Bool { node is DOMText }

private func isWhitespaceDomTextNode(_ node: DOMNode?) -> Bool {
  guard let text = node as? DOMText, !text.data.isEmpty else { return false }
  return text.data.unicodeScalars.allSatisfy { $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" }
}

/// `isOnlyChildInBlockNode(node)` (`@experimental`): `node` is the only
/// child, ignoring whitespace text, of a block element.
public func isOnlyChildInBlockNode(_ node: DOMNode) -> Bool {
  guard let parent = node.parentElement, isBlockDomNode(parent),
    let first = parent.firstChild, let last = parent.lastChild
  else { return false }
  let startsWith = first === node || (first.nextSibling === node && isWhitespaceDomTextNode(first))
  let endsWith = last === node || (last.previousSibling === node && isWhitespaceDomTextNode(last))
  return startsWith && endsWith
}

/// `isLastChildInBlockNode(node)` (`@experimental`): `node` is the last
/// child, ignoring whitespace text, of a block element, and not its only
/// one.
public func isLastChildInBlockNode(_ node: DOMNode) -> Bool {
  guard let parent = node.parentElement, isBlockDomNode(parent),
    let first = parent.firstChild, let last = parent.lastChild
  else { return false }
  if first === node || (first.nextSibling === node && isWhitespaceDomTextNode(first)) {
    return false
  }
  return last === node || (last.previousSibling === node && isWhitespaceDomTextNode(last))
}

/// JS `parseInt(string, 10)`: the leading integer after whitespace, or
/// `nil` (`NaN`).
package func jsParseInt(_ string: String) -> Int? {
  var scalars = Substring(jsTrim(string)).unicodeScalars[...]
  var sign = 1
  if let first = scalars.first, first == "-" || first == "+" {
    sign = first == "-" ? -1 : 1
    scalars = scalars.dropFirst()
  }
  var value = 0
  var digits = 0
  for scalar in scalars {
    guard ("0"..."9").contains(scalar) else { break }
    value = value &* 10 &+ Int(scalar.value - 48)
    digits += 1
  }
  return digits > 0 ? sign * value : nil
}

/// `setNodeIndentFromDOM(element, node)`: the indent from
/// `data-lexical-indent`, else from `padding-inline-start` in 40px steps.
public func setNodeIndentFromDOM(_ elementDom: DOMElement, _ elementNode: ElementNode) throws {
  if let attribute = elementDom.getAttribute("data-lexical-indent"),
    let parsed = jsParseInt(attribute), parsed >= 0
  {
    try elementNode.setIndent(parsed)
    return
  }
  let indentSize = jsParseInt(elementDom.style["padding-inline-start"]) ?? 0
  // Math.round rounds halves up.
  try elementNode.setIndent(Int((Double(indentSize) / 40 + 0.5).rounded(.down)))
}

/// `$setDirectionFromDOM(node, element)`: a `dir` of `ltr` or `rtl`.
@discardableResult
public func setDirectionFromDOM<T: ElementNode>(_ node: T, _ domNode: DOMElement) throws -> T {
  let dir = domNode.getAttribute("dir")
  return dir == "ltr" || dir == "rtl" ? try node.setDirection(dir) : node
}

/// `$setFormatFromDOM(node, element)`: a valid `text-align`.
@discardableResult
public func setFormatFromDOM<T: ElementNode>(_ node: T, _ domNode: DOMElement) throws -> T {
  let alignment = domNode.style["text-align"]
  return !alignment.isEmpty && ELEMENT_TYPE_TO_FORMAT[alignment] != nil
    ? try node.setFormat(alignment) : node
}

// MARK: - Core node importers

extension ParagraphNode {
  /// `$convertParagraphElement`.
  static func convertParagraphElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
    guard let element = domNode as? DOMElement else { return nil }
    let node = try createParagraphNode()
    try setFormatFromDOM(node, element)
    try setNodeIndentFromDOM(element, node)
    // The legacy `align` attribute, when CSS sets no alignment.
    if try node.getFormatType() == "", let align = element.getAttribute("align"),
      ELEMENT_TYPE_TO_FORMAT[align] != nil
    {
      try node.setFormat(align)
    }
    try setDirectionFromDOM(node, element)
    return DOMConversionOutput(node: node)
  }
}

private let nodeNameToTextFormat: [String: TextFormatType] = [
  "code": "code", "em": "italic", "i": "italic", "mark": "highlight", "s": "strikethrough",
  "strong": "bold", "sub": "subscript", "sup": "superscript", "u": "underline",
]

/// `applyTextFormatFromStyle(style, shouldApply)`: a child conversion that
/// adds the formats the style implies (and `shouldApply`).
private func applyTextFormatFromStyle(
  _ style: CSSStyleDeclaration, _ shouldApply: TextFormatType? = nil
) -> DOMChildConversion {
  let fontWeight = style["font-weight"]
  let textDecoration = style["text-decoration"].split(separator: " ").map(String.init)
  var formats: [TextFormatType] = []
  if fontWeight == "700" || fontWeight == "bold" { formats.append("bold") }
  if textDecoration.contains("line-through") { formats.append("strikethrough") }
  if style["font-style"] == "italic" { formats.append("italic") }
  if textDecoration.contains("underline") { formats.append("underline") }
  let verticalAlign = style["vertical-align"]
  if verticalAlign == "sub" { formats.append("subscript") }
  if verticalAlign == "super" { formats.append("superscript") }
  let textTransform = style["text-transform"]
  if ["lowercase", "uppercase", "capitalize"].contains(textTransform) {
    formats.append(textTransform)
  }
  if let shouldApply { formats.append(shouldApply) }
  return { lexicalNode, _ in
    guard let node = lexicalNode as? InlineFormattableNode else { return lexicalNode }
    for format in formats {
      let flag = TEXT_TYPE_TO_FORMAT[format] ?? 0
      let current = try node.getFormat()
      if current & flag == 0 { try node.setFormat(current ^ flag) }
    }
    return lexicalNode
  }
}

private func isNodePre(_ node: DOMNode) -> Bool {
  guard let element = node as? DOMElement else { return false }
  return element.localName == "pre" || element.style["white-space"].hasPrefix("pre")
}

/// `findParentPreDOMNode(node)`: the nearest ancestor that preserves
/// whitespace (`<pre>` or `white-space: pre*`).
public func findParentPreDOMNode(_ node: DOMNode) -> DOMNode? {
  var parent = node.parentNode
  while let current = parent, !isNodePre(current) { parent = current.parentNode }
  return parent
}

/// The next (or previous) text node on the same line: stops at a block
/// element or a `<br>`.
private func findTextInLine(_ text: DOMText, forward: Bool) -> DOMText? {
  var node: DOMNode = text
  while true {
    var sibling = forward ? node.nextSibling : node.previousSibling
    while sibling == nil {
      guard let parent = node.parentElement else { return nil }
      node = parent
      sibling = forward ? node.nextSibling : node.previousSibling
    }
    node = sibling!
    if let element = node as? DOMElement {
      let display = element.style["display"]
      if (display.isEmpty && !isInlineDomNode(element))
        || (!display.isEmpty && !display.hasPrefix("inline"))
      {
        return nil
      }
    }
    while let descendant = forward ? node.firstChild : node.lastChild { node = descendant }
    if let text = node as? DOMText { return text }
    if node.nodeName == "BR" { return nil }
  }
}

private func isCollapsibleSpace(_ scalar: Unicode.Scalar) -> Bool {
  scalar == " " || scalar == "\t" || scalar == "\n"
}

/// `$convertTextDOMNode`: text with whitespace collapsed as a browser
/// renders it, or raw text inside `pre`.
private func convertTextDOMNode(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let text = domNode as? DOMText else { return nil }
  try invariant(text.parentElement != nil, "Expected parentElement of Text not to be null")
  if findParentPreDOMNode(text) != nil {
    return DOMConversionOutput(node: try generateNodesFromRawText(text.data))
  }
  // Drop \r, then collapse runs of space, tab and newline to one space.
  var collapsed = String.UnicodeScalarView()
  for scalar in text.data.unicodeScalars where scalar != "\r" {
    if isCollapsibleSpace(scalar) {
      if collapsed.last != " " || collapsed.isEmpty { collapsed.append(" ") }
    } else {
      collapsed.append(scalar)
    }
  }
  if collapsed.isEmpty { return DOMConversionOutput(node: nil) }
  if collapsed.first == " " {
    // A leading space goes at the start of a line, or after text that
    // ends in whitespace.
    var previous: DOMText? = text
    var isStartOfLine = true
    while let current = previous, let found = findTextInLine(current, forward: false) {
      previous = found
      if !found.data.isEmpty {
        if let last = found.data.unicodeScalars.last, isCollapsibleSpace(last) {
          collapsed.removeFirst()
        }
        isStartOfLine = false
        break
      }
    }
    if isStartOfLine, !collapsed.isEmpty { collapsed.removeFirst() }
  }
  if collapsed.last == " " {
    // A trailing space stays only if inline text follows on the line.
    var next: DOMText? = text
    var isEndOfLine = true
    while let current = next, let found = findTextInLine(current, forward: true) {
      next = found
      if found.data.unicodeScalars.contains(where: { !isCollapsibleSpace($0) && $0 != "\r" }) {
        isEndOfLine = false
        break
      }
    }
    if isEndOfLine { collapsed.removeLast() }
  }
  if collapsed.isEmpty { return DOMConversionOutput(node: nil) }
  return DOMConversionOutput(node: try createTextNode(String(collapsed)))
}

extension TextNode {
  static var coreImporters: DOMConversionMap {
    var map: DOMConversionMap = [
      "#text": { _ in DOMConversion(convertTextDOMNode) },
      "b": { _ in
        DOMConversion { domNode in
          guard let b = domNode as? DOMElement else { return nil }
          // Google Docs wraps copied HTML in <b style="font-weight:normal">.
          let normal = b.style["font-weight"] == "normal"
          return DOMConversionOutput(
            node: nil, forChild: applyTextFormatFromStyle(b.style, normal ? nil : "bold"))
        }
      },
      "span": { _ in
        DOMConversion { domNode in
          guard let span = domNode as? DOMElement else { return nil }
          return DOMConversionOutput(node: nil, forChild: applyTextFormatFromStyle(span.style))
        }
      },
    ]
    for name in nodeNameToTextFormat.keys {
      map[name] = { _ in
        DOMConversion { domNode in
          guard let element = domNode as? DOMElement,
            let format = nodeNameToTextFormat[element.localName]
          else { return DOMConversionOutput(node: nil) }
          return DOMConversionOutput(
            node: nil, forChild: applyTextFormatFromStyle(element.style, format))
        }
      }
    }
    return map
  }
}
