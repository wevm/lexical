import Lexical
import LexicalExtension

// Port of packages/lexical-code-core/src/CodeNode.ts. The generated JSON
// implementations (LexicalCodeCoreGeneratedJSON.ts) are a JS speed-up of
// the schema walk and are not ported; the schema below drives both.

public let DEFAULT_CODE_LANGUAGE = "javascript"

/// `getDefaultCodeLanguage()`. Configurable through the extensions.
public func getDefaultCodeLanguage() -> String { DEFAULT_CODE_LANGUAGE }

private func hasChildDOMNodeTag(_ node: DOMNode, _ tagName: String) -> Bool {
  for child in node.childNodes {
    if let child = child as? DOMElement, child.tagName == tagName { return true }
    if hasChildDOMNodeTag(child, tagName) { return true }
  }
  return false
}

private let LANGUAGE_DATA_ATTRIBUTE = "data-language"
private let HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE = "data-highlight-language"
private let THEME_DATA_ATTRIBUTE = "data-theme"

/// `CodeNode`: a code block, whose children are `CodeHighlightNode`s,
/// `TabNode`s and `LineBreakNode`s.
open class CodeNode: ElementNode {
  public var __language: String?
  public var __theme: String?
  /// Not serialized: the highlighter extensions set it as they run.
  public var __isSyntaxHighlightSupported = false

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CodeNode.self, type: "code", extends: ElementNode.self,
      json: [
        // Read straight off the fields; applied through the setters, which
        // normalize a falsy value to undefined.
        SchemaField(
          "language", optional(nullable(stringValue())), of: CodeNode.self,
          get: { $0.__language.map(JSONValue.string) },
          set: { try $0.setLanguage($1?.stringValue) }),
        SchemaField(
          "theme", optional(stringValue()), of: CodeNode.self,
          get: { $0.__theme.map(JSONValue.string) },
          set: { try $0.setTheme($1?.stringValue) }),
      ],
      children: .inline)
  }

  open override class func importDOM() -> DOMConversionMap? {
    [
      // Typically <pre> is used for code blocks, and <code> for inline code
      // styles, but a multi-line <code> makes a block. Otherwise it passes
      // through to the inline format TextNode handles.
      "code": { node in
        // `/\r?\n/` matches any "\n"; over UTF-16, because Swift reads
        // "\r\n" as one Character.
        let isMultiLine =
          node.textContent.utf16.contains(0x0A) || hasChildDOMNodeTag(node, "BR")
        return isMultiLine ? DOMConversion(priority: 1, convertPreElement) : nil
      },
      "div": { _ in DOMConversion(priority: 1, convertDivElement) },
      "pre": { _ in DOMConversion(priority: 0, convertPreElement) },
      "table": { node in
        guard let table = node as? DOMElement, isGitHubCodeTable(table) else { return nil }
        return DOMConversion(
          priority: 3, { _ in DOMConversionOutput(node: [try createCodeNode()]) })
      },
      "td": { node in
        guard let td = node as? DOMElement else { return nil }
        // A no-op for a table cell in a code table that is not a code line;
        // otherwise it falls back to the table cell conversion.
        if isGitHubCodeCell(td) || closestTable(td).map(isGitHubCodeTable) == true {
          return DOMConversion(priority: 3, convertCodeNoop)
        }
        return nil
      },
      "tr": { node in
        guard let tr = node as? DOMElement, closestTable(tr).map(isGitHubCodeTable) == true
        else { return nil }
        return DOMConversion(priority: 3, convertCodeNoop)
      },
    ]
  }

  /// `new CodeNode(language, key)`.
  public convenience init(_ language: String?, key: NodeKey? = nil) throws {
    try self.init(key: key)
    __language = language?.isEmpty == false ? language : nil
  }

  // Written rather than synthesized from the schema, because
  // `__isSyntaxHighlightSupported` is not a serialized property.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prevNode = prevNode as! CodeNode
    __language = prevNode.__language
    __theme = prevNode.__theme
    __isSyntaxHighlightSupported = prevNode.__isSyntaxHighlightSupported
  }

  // View

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("code")
    try writeCodeAttributes(element, config.theme)
    return element
  }

  /// The class, `spellcheck`, language, theme and style `createDOM` and
  /// `exportDOM` both write.
  private func writeCodeAttributes(_ element: DOMElement, _ theme: EditorThemeClasses) throws {
    addClassNamesToElement(element, theme["code"]?.stringValue)
    element.setAttribute("spellcheck", "false")
    if let language = try getLanguage() {
      element.setAttribute(LANGUAGE_DATA_ATTRIBUTE, language)
      if try getIsSyntaxHighlightSupported() {
        element.setAttribute(HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE, language)
      }
    }
    if let theme = try getTheme() {
      element.setAttribute(THEME_DATA_ATTRIBUTE, theme)
    }
    let style = try getStyle()
    if !style.isEmpty {
      setDOMStyleFromCSS(element.style, style)
    }
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevNode = prevNode as! CodeNode
    let language = __language
    let prevLanguage = prevNode.__language

    if let language {
      if language != prevLanguage {
        dom.setAttribute(LANGUAGE_DATA_ATTRIBUTE, language)
      }
    } else if prevLanguage != nil {
      dom.removeAttribute(LANGUAGE_DATA_ATTRIBUTE)
    }

    let isSyntaxHighlightSupported = __isSyntaxHighlightSupported
    if prevNode.__isSyntaxHighlightSupported, let prevLanguage {
      if isSyntaxHighlightSupported, let language {
        if language != prevLanguage {
          dom.setAttribute(HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE, language)
        }
      } else {
        dom.removeAttribute(HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE)
      }
    } else if isSyntaxHighlightSupported, let language {
      dom.setAttribute(HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE, language)
    }

    if let theme = __theme {
      if theme != prevNode.__theme {
        dom.setAttribute(THEME_DATA_ATTRIBUTE, theme)
      }
    } else if prevNode.__theme != nil {
      dom.removeAttribute(THEME_DATA_ATTRIBUTE)
    }

    if __style != prevNode.__style {
      setDOMStyleFromCSS(dom.style, __style, prevNode.__style)
    }
    return false
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = getDocument().createElement("pre")
    try writeCodeAttributes(element, editor._config.theme)
    return DOMExportOutput(element: element)
  }

  // Mutation

  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    // Using CodeNode without CodeExtension is deprecated.
    if try getPeerDependencyFromEditor(getEditor(), "@lexical/code") == nil {
      if let el = try exitCodeNodeOnEnter(selection) { return el }
    }
    // If the selection is within the code block, find all leading tabs and
    // spaces of the current line, and start the new line with them, so
    // that leading indentation is preserved.
    let anchor = selection.anchor
    let focus = selection.focus
    let firstPoint = try anchor.isBefore(focus) ? anchor : focus
    let firstSelectionNode = try firstPoint.getNode()
    if let firstSelectionNode = firstSelectionNode as? TextNode {
      var node: LexicalNode? = try getFirstCodeNodeOfLine(firstSelectionNode)
      var insertNodes: [TextNode] = []
      while true {
        if let tab = node as? TabNode {
          insertNodes.append(try createTabNode())
          node = try tab.getNextSibling()
        } else if let highlight = node as? CodeHighlightNode {
          let text = Array(try highlight.getTextContent().utf16)
          var spaces = 0
          while spaces < text.count && text[spaces] == 0x20 { spaces += 1 }
          if spaces != 0 {
            insertNodes.append(try createCodeHighlightNode(String(repeating: " ", count: spaces)))
          }
          if spaces != text.count { break }
          node = try highlight.getNextSibling()
        } else {
          break
        }
      }
      // An empty text node splits into nothing; JS then throws a TypeError
      // reading `getIndexWithinParent` of `undefined`, which aborts the update.
      guard let split = try firstSelectionNode.splitText(anchor.offset).first else {
        throw LexicalError("insertNewAfter: the selected text node is empty")
      }
      let x = anchor.offset == 0 ? 0 : 1
      let index = try split.getIndexWithinParent() + x
      let codeNode = try firstSelectionNode.getParentOrThrow()
      try codeNode.splice(index, 0, [try createLineBreakNode()] + insertNodes)
      if let last = insertNodes.last {
        try last.select()
      } else if anchor.offset == 0 {
        try split.selectPrevious()
      } else {
        try split.getNextSibling()!.selectNext(0, 0)
      }
    }
    if let codeNode = firstSelectionNode as? CodeNode {
      let offset = selection.anchor.offset
      try codeNode.splice(offset, 0, [try createLineBreakNode()])
      try codeNode.select(offset + 1, offset + 1)
    }
    return nil
  }

  open override func canIndent() -> Bool { false }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool {
    let paragraph = try createParagraphNode()
    for child in try getChildren() { try paragraph.append(child) }
    try replace(paragraph)
    return true
  }

  @discardableResult
  open func setLanguage(_ language: String?) throws -> Self {
    let writable = try getWritable()
    writable.__language = language?.isEmpty == false ? language : nil
    return writable
  }

  open func getLanguage() throws -> String? { try getLatest().__language }

  @discardableResult
  open func setIsSyntaxHighlightSupported(_ isSupported: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__isSyntaxHighlightSupported = isSupported
    return writable
  }

  open func getIsSyntaxHighlightSupported() throws -> Bool {
    try getLatest().__isSyntaxHighlightSupported
  }

  @discardableResult
  open func setTheme(_ theme: String?) throws -> Self {
    let writable = try getWritable()
    writable.__theme = theme?.isEmpty == false ? theme : nil
    return writable
  }

  open func getTheme() throws -> String? { try getLatest().__theme }
}

/// `$createCodeNode(language, theme)`.
public func createCodeNode(_ language: String? = nil, _ theme: String? = nil) throws -> CodeNode {
  try create(CodeNode.self).setLanguage(language).setTheme(theme)
}

/// `$isCodeNode(node)`.
public func isCodeNode(_ node: LexicalNode?) -> Bool { node is CodeNode }

private func convertPreElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  let element = domNode as? DOMElement
  // exportDOM writes data-theme next to data-language, so read it back
  // too; otherwise the theme is dropped on every HTML round trip.
  return DOMConversionOutput(node: [
    try createCodeNode(
      element?.getAttribute(LANGUAGE_DATA_ATTRIBUTE), element?.getAttribute(THEME_DATA_ATTRIBUTE))
  ])
}

private func convertDivElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let div = domNode as? DOMElement, isCodeElement(div) else {
    return DOMConversionOutput()
  }
  return DOMConversionOutput(node: [try createCodeNode()])
}

private func convertCodeNoop(_: DOMNode) throws -> DOMConversionOutput? { DOMConversionOutput() }

private func isCodeElement(_ div: DOMElement) -> Bool {
  div.style["font-family"].contains("monospace")
}

private func isGitHubCodeCell(_ cell: DOMElement) -> Bool {
  cell.classList.contains("js-file-line")
}

private func isGitHubCodeTable(_ table: DOMElement) -> Bool {
  table.classList.contains("js-file-line-container")
}

/// `element.closest('table')`.
private func closestTable(_ element: DOMElement) -> DOMElement? {
  var current: DOMElement? = element
  while let node = current {
    if node.localName == "table" { return node }
    current = node.parentElement
  }
  return nil
}

/// `$exitCodeNodeOnEnter(selection)`: Enter on a code block that ends in
/// two line breaks, with the caret after them, removes them and puts a
/// new paragraph after the block.
public func exitCodeNodeOnEnter(_ selection: RangeSelection) throws -> ParagraphNode? {
  let anchor = selection.anchor
  guard selection.isCollapsed(), anchor.type == .element,
    let codeNode = try anchor.getNode() as? CodeNode
  else { return nil }
  let childrenSize = try codeNode.getChildrenSize()
  guard childrenSize >= 2, anchor.offset == childrenSize,
    let lastChild = try codeNode.getLastChild() as? LineBreakNode,
    try lastChild.getPreviousSibling() is LineBreakNode
  else { return nil }
  let newElement = try createParagraphNode()
  try codeNode.splice(childrenSize - 2, 2, []).insertAfter(newElement, restoreSelection: false)
  try newElement.select()
  return newElement
}
