// The DOM side of nodes: `createDOM`, `updateDOM` and `exportDOM` build
// `DOMElement`s (see DOM/), which HTML export serializes. The native views
// don't render this DOM; they reconcile through TextKit.

/// `DOMExportOutput`: the DOM a node exports (an element, a fragment, a
/// text node or nothing) and an optional hook to run on it once its
/// children are appended.
public struct DOMExportOutput {
  /// Runs once the children are appended; a returned node replaces it.
  public var after: ((DOMNode?) throws -> DOMNode?)?
  public var element: DOMNode?
  /// Appends the exported children somewhere other than `element`.
  public var append: ((DOMDocumentFragment) throws -> Void)?
  /// The children to export in place of the node's own.
  public var getChildNodes: (() throws -> [LexicalNode])?

  public init(
    element: DOMNode?, after: ((DOMNode?) throws -> DOMNode?)? = nil,
    append: ((DOMDocumentFragment) throws -> Void)? = nil,
    getChildNodes: (() throws -> [LexicalNode])? = nil
  ) {
    self.element = element
    self.after = after
    self.append = append
    self.getChildNodes = getChildNodes
  }
}

private let blockTags: Set<String> = [
  "address", "article", "aside", "blockquote", "canvas", "dd", "div", "dl", "dt", "fieldset",
  "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr",
  "li", "main", "nav", "noscript", "ol", "p", "pre", "section", "table", "td", "tfoot", "ul",
  "video",
]

/// `isBlockDomNode(node)`: a block-level tag not displayed inline.
public func isBlockDomNode(_ node: DOMNode) -> Bool {
  guard let element = node as? DOMElement else { return false }
  if element.style["display"].hasPrefix("inline") { return false }
  return blockTags.contains(element.localName)
}

/// `EditorDOMRenderConfig` (`@experimental`): how an editor turns nodes
/// into DOM. Start from `.default` and replace the hooks to change it, as
/// `DOMRenderExtension` does.
public struct EditorDOMRenderConfig {
  public var createDOM: (LexicalNode, Editor) throws -> DOMElement
  public var decorateDOM: (LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void
  public var exportDOM: (LexicalNode, Editor) throws -> DOMExportOutput
  public var extractWithChild:
    (LexicalNode, LexicalNode, BaseSelection?, String, Editor) throws -> Bool
  public var getDOMSlot: (LexicalNode, DOMElement, Editor) throws -> DOMSlot
  public var shouldExclude: (LexicalNode, BaseSelection?, Editor) throws -> Bool
  public var shouldInclude: (LexicalNode, BaseSelection?, Editor) throws -> Bool
  public var updateDOM: (LexicalNode, LexicalNode, DOMElement, Editor) throws -> Bool

  /// `DEFAULT_EDITOR_DOM_CONFIG`.
  public static var `default`: EditorDOMRenderConfig {
    EditorDOMRenderConfig(
      createDOM: { node, editor in try node.createDOM(editor._config, editor) },
      decorateDOM: { _, _, _, _ in },
      exportDOM: { node, editor in
        // Use the HTML config's export override, if there is one.
        if let override = editor._nodes.get(node.getType())?.exportDOM {
          return try override(editor, node)
        }
        return try node.exportDOM(editor)
      },
      extractWithChild: { node, child, selection, destination, _ in
        guard let element = node as? ElementNode else { return false }
        return try element.extractWithChild(child, selection, destination)
      },
      getDOMSlot: { node, dom, _ in node.getDOMSlot(dom) },
      shouldExclude: { node, _, _ in
        guard let element = node as? ElementNode else { return false }
        return try element.excludeFromCopy("html")
      },
      shouldInclude: { node, selection, _ in
        guard let selection else { return true }
        return try node.isSelected(selection)
      },
      updateDOM: { next, prev, dom, editor in try next.updateDOM(prev, dom, editor._config) }
    )
  }
}

/// `$getEditorDOMRenderConfig(editor)`.
public func getEditorDOMRenderConfig(_ editor: Editor) -> EditorDOMRenderConfig {
  editor._config.dom ?? .default
}

/// `$getDocument()`: the document nodes create their DOM in, one per
/// thread (the global `document` of JS). It needs no active editor, so
/// `createDOM` and `exportDOM` work outside an update.
public func getDocument() -> DOMDocument {
  ThreadContext.current.defaultDocument
}

/// `DOMSlot`: where a node's DOM children go. `before` and `after` bound
/// the managed children within `element`.
public struct DOMSlot {
  public let element: DOMElement
  public let before: DOMNode?
  public let after: DOMNode?

  public init(_ element: DOMElement, before: DOMNode? = nil, after: DOMNode? = nil) {
    self.element = element
    self.before = before
    self.after = after
  }

  public func withBefore(_ before: DOMNode?) -> DOMSlot {
    DOMSlot(element, before: before, after: after)
  }
  public func withAfter(_ after: DOMNode?) -> DOMSlot {
    DOMSlot(element, before: before, after: after)
  }
  public func withElement(_ element: DOMElement) -> DOMSlot {
    DOMSlot(element, before: before, after: after)
  }

  @discardableResult
  public func insertChild(_ dom: DOMNode) throws -> DOMSlot {
    let anchor = getInsertionAnchor()
    try invariant(
      anchor == nil || anchor?.parentElement === element,
      "DOMSlot.insertChild: before is not in element")
    element.insertBefore(dom, anchor)
    return self
  }

  @discardableResult
  public func removeChild(_ dom: DOMNode) throws -> DOMSlot {
    try invariant(dom.parentElement === element, "DOMSlot.removeChild: dom is not in element")
    element.removeChild(dom)
    return self
  }

  @discardableResult
  public func replaceChild(_ dom: DOMNode, _ prevDom: DOMNode) throws -> DOMSlot {
    try invariant(
      prevDom.parentElement === element, "DOMSlot.replaceChild: prevDom is not in element")
    element.replaceChild(dom, prevDom)
    return self
  }

  public func getFirstChild() -> DOMNode? {
    let first = getFirstChildAnchor().map(\.nextSibling) ?? element.firstChild
    return first === getInsertionAnchor() ? nil : first
  }

  public func getFirstChildAnchor() -> DOMNode? { after }
  public func getInsertionAnchor() -> DOMNode? { before }
}

// MARK: Class names and styles

/// `normalizeClassNames(...classNames)`: the space-separated tokens of the
/// strings, in order.
public func normalizeClassNames(_ classNames: String?...) -> [String] {
  normalizeClassNames(contentsOf: classNames)
}

public func normalizeClassNames(contentsOf classNames: [String?]) -> [String] {
  classNames.flatMap { className in
    (className ?? "").split(whereSeparator: { $0.unicodeScalars.allSatisfy(isJSSpace) })
      .map(String.init)
  }
}

private func isJSSpace(_ scalar: Unicode.Scalar) -> Bool {
  scalar.value <= 0xFFFF && isJSWhitespace(UInt16(scalar.value))
}

/// `addClassNamesToElement(element, ...classNames)`.
public func addClassNamesToElement(_ element: DOMElement, _ classNames: String?...) {
  let tokens = normalizeClassNames(contentsOf: classNames)
  if !tokens.isEmpty { element.classList.add(contentsOf: tokens) }
}

/// `removeClassNamesFromElement(element, ...classNames)`.
public func removeClassNamesFromElement(_ element: DOMElement, _ classNames: String?...) {
  let tokens = normalizeClassNames(contentsOf: classNames)
  if !tokens.isEmpty { element.classList.remove(contentsOf: tokens) }
}

/// `getCachedClassNameArray(theme, key)`: the class names a theme entry
/// names, as a string of space-separated names or an array of them.
public func getCachedClassNameArray(_ theme: EditorThemeClasses, _ key: String) -> [String]? {
  themeClassNames(theme[key])
}

func themeClassNames(_ value: JSONValue?) -> [String]? {
  switch value {
  case .string(let names): normalizeClassNames(names)
  case .array(let names): names.compactMap(\.stringValue)
  default: nil
  }
}

/// `removeEmptyDOMAttribute(dom, name)`: drops a `class` or `style`
/// attribute that has become empty.
public func removeEmptyDOMAttribute(_ dom: DOMElement, _ attributeName: String) {
  if dom.getAttribute(attributeName) == "" { dom.removeAttribute(attributeName) }
}

private func setDOMStyleProperty(_ style: CSSStyleDeclaration, _ property: String, _ value: String)
{
  let flag = "!important"
  var trimmed = value
  while let last = trimmed.utf16.last, isJSWhitespace(last) { trimmed.removeLast() }
  if trimmed.utf16.count >= flag.utf16.count, trimmed.lowercased().hasSuffix(flag) {
    style.setProperty(property, jsTrim(String(trimmed.dropLast(flag.count))), "important")
  } else {
    style.setProperty(property, value, "")
  }
}

/// `setDOMStyleObject(style, object)`: sets each property, removing those
/// whose value is `nil`; a trailing `!important` sets the priority.
public func setDOMStyleObject(
  _ style: CSSStyleDeclaration, _ styleObject: KeyValuePairs<String, String?>
) {
  for (property, value) in styleObject {
    if let value {
      setDOMStyleProperty(style, property, value)
    } else {
      style.removeProperty(property)
    }
  }
}

/// `setDOMStyleFromCSS(style, cssText, prevCSSText)`: applies the
/// properties of `cssText` and removes those only in `prevCSSText`.
public func setDOMStyleFromCSS(
  _ style: CSSStyleDeclaration, _ cssText: String, _ prevCSSText: String = ""
) {
  if cssText == prevCSSText { return }
  var prev = getStyleObjectFromCSS(prevCSSText)
  for (property, value) in getStyleObjectFromCSS(cssText) {
    prev[property] = nil
    setDOMStyleProperty(style, property, value)
  }
  for (property, _) in prev { style.removeProperty(property) }
}
