import Lexical

// Port of packages/lexical-code-core/src/CodeHighlightNode.ts.

/// `CodeHighlightNode`: a run of code text, with the token type a
/// highlighter gave it. It cannot be formatted.
open class CodeHighlightNode: TextNode {
  public var __highlightType: String?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CodeHighlightNode.self, type: "code-highlight", extends: TextNode.self,
      json: [
        // Read straight off the field; applied through setHighlightType,
        // which normalizes a falsy value to undefined.
        SchemaField(
          "highlightType", optional(nullable(stringValue())), of: CodeHighlightNode.self,
          get: { $0.__highlightType.map(JSONValue.string) },
          set: { try $0.setHighlightType($1?.stringValue) })
      ])
  }

  /// `new CodeHighlightNode(text, highlightType, key)`. `highlightType` has
  /// no default so that `CodeHighlightNode(text)` is the inherited init.
  public convenience init(
    _ text: String, _ highlightType: String?, key: NodeKey? = nil
  )
    throws
  {
    try self.init(text, key: key)
    __highlightType = highlightType
  }

  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __highlightType = (prevNode as! CodeHighlightNode).__highlightType
  }

  open func getHighlightType() throws -> String? { try getLatest().__highlightType }

  @discardableResult
  open func setHighlightType(_ highlightType: String?) throws -> Self {
    let writable = try getWritable()
    writable.__highlightType = highlightType?.isEmpty == false ? highlightType : nil
    return writable
  }

  open override func canHaveFormat() -> Bool { false }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = try super.createDOM(config, editor)
    addClassNamesToElement(element, getHighlightThemeClass(config.theme, __highlightType))
    return element
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let update = try super.updateDOM(prevNode, dom, config)
    let prevClassName = getHighlightThemeClass(
      config.theme, (prevNode as! CodeHighlightNode).__highlightType)
    let nextClassName = getHighlightThemeClass(config.theme, __highlightType)
    if prevClassName != nextClassName {
      if let prevClassName { removeClassNamesFromElement(dom, prevClassName) }
      if let nextClassName { addClassNamesToElement(dom, nextClassName) }
    }
    return update
  }

  /// Formatting (bold, underline, …) is ignored.
  @discardableResult
  open override func setFormat(_ format: Int) throws -> Self { self }

  open override func isParentRequired() -> Bool { true }

  open override func createParentElementNode() throws -> ElementNode { try createCodeNode() }
}

private func getHighlightThemeClass(
  _ theme: EditorThemeClasses, _ highlightType: String?
)
  -> String?
{
  guard let highlightType, !highlightType.isEmpty else { return nil }
  return theme["codeHighlight"]?.objectValue?[highlightType]?.stringValue
}

/// `$createCodeHighlightNode(text, highlightType)`.
public func createCodeHighlightNode(
  _ text: String = "", _ highlightType: String? = nil
) throws
  -> CodeHighlightNode
{
  try applyNodeReplacement(CodeHighlightNode(text, highlightType))
}

/// `$isCodeHighlightNode(node)`.
public func isCodeHighlightNode(_ node: LexicalNode?) -> Bool { node is CodeHighlightNode }
