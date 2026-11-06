import Lexical

// Port of packages/lexical-mark/src/MarkNode.ts. The generated JSON
// implementations (LexicalMarkGeneratedJSON.ts) are a JS speed-up of the
// schema walk and are not ported; the schema below drives both.

private let NO_IDS: [String] = []

/// `MarkNode`: an inline `<mark>` over the content that one or more ids
/// (comments, for example) annotate. It takes the theme's `mark` class, and
/// `markOverlap` while it has more than one id.
open class MarkNode: ElementNode {
  public var __ids: [String] = NO_IDS

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      MarkNode.self, type: "mark", extends: ElementNode.self,
      // The accessors are the methods, so a subclass that overrides
      // `setIDs` decides what an import stores; `afterCloneFrom` carries
      // the field.
      json: [
        SchemaField(
          "ids", arrayValue(stringValue()), of: MarkNode.self,
          get: { node in .array(try node.getIDs().map { .string($0) }) },
          set: { node, value in
            try node.setIDs(value?.arrayValue?.compactMap(\.stringValue) ?? [])
          })
      ],
      inline: true, children: .inline)
  }

  /// `new MarkNode(ids, key)`.
  public convenience init(_ ids: [String] = [], key: NodeKey? = nil) throws {
    try self.init(key: key)
    __ids = ids
  }

  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __ids = (prevNode as! MarkNode).__ids
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("mark")
    addClassNamesToElement(element, config.theme["mark"]?.stringValue)
    if __ids.count > 1 {
      addClassNamesToElement(element, config.theme["markOverlap"]?.stringValue)
    }
    return element
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevIDs = (prevNode as? MarkNode)?.__ids ?? NO_IDS
    let overlapTheme = config.theme["markOverlap"]?.stringValue
    // Mirror createDOM's rule (`ids.count > 1` carries the overlap class)
    // rather than enumerating transitions.
    let hadOverlap = prevIDs.count > 1
    let hasOverlap = __ids.count > 1
    if hadOverlap != hasOverlap {
      if hasOverlap {
        addClassNamesToElement(dom, overlapTheme)
      } else {
        removeClassNamesFromElement(dom, overlapTheme)
      }
    }
    return false
  }

  open func hasID(_ id: String) throws -> Bool {
    try getIDs().contains(id)
  }

  open func getIDs() throws -> [String] {
    try getLatest().__ids
  }

  @discardableResult
  open func setIDs(_ ids: [String]) throws -> Self {
    let selfNode = try getWritable()
    selfNode.__ids = ids
    return selfNode
  }

  @discardableResult
  open func addID(_ id: String) throws -> Self {
    let selfNode = try getWritable()
    return selfNode.__ids.contains(id) ? selfNode : try selfNode.setIDs(selfNode.__ids + [id])
  }

  @discardableResult
  open func deleteID(_ id: String) throws -> Self {
    let selfNode = try getWritable()
    guard let idx = selfNode.__ids.firstIndex(of: id) else { return selfNode }
    var ids = selfNode.__ids
    ids.remove(at: idx)
    return try selfNode.setIDs(ids)
  }

  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    let markNode = try createMarkNode(__ids)
    try insertAfter(markNode, restoreSelection: restoreSelection)
    return markNode
  }

  open override func canInsertTextBefore() throws -> Bool { false }

  open override func canInsertTextAfter() throws -> Bool { false }

  open override func canBeEmpty() throws -> Bool { false }

  open override func isInline() throws -> Bool { true }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection, destination != "html" else {
      return false
    }
    let anchor = selection.anchor
    let focus = selection.focus
    let anchorNode = try anchor.getNode()
    let focusNode = try focus.getNode()
    let isBackward = try selection.isBackward()
    let selectionLength = isBackward ? anchor.offset - focus.offset : focus.offset - anchor.offset
    return try isParentOf(anchorNode) && isParentOf(focusNode)
      && getTextContent().utf16.count == selectionLength
  }

  open override func excludeFromCopy(_ destination: String? = nil) throws -> Bool {
    destination != "clone"
  }
}

/// `$createMarkNode(ids)`.
public func createMarkNode(_ ids: [String] = []) throws -> MarkNode {
  try applyNodeReplacement(MarkNode(ids))
}

/// `$isMarkNode(node)`.
public func isMarkNode(_ node: LexicalNode?) -> Bool {
  node is MarkNode
}
