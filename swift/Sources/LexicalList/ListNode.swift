import Lexical

// Port of packages/lexical-list/src/LexicalListNode.ts.

/// `ListType`: `"number"`, `"bullet"` or `"check"`.
public typealias ListType = String

/// `ListNodeTagType`: `"ul"` or `"ol"`.
public typealias ListNodeTagType = String

/// The legacy tag-form `listType` spellings some older documents carry.
private let TAG_TO_LIST_TYPE: [String: ListType] = ["ol": "number", "ul": "bullet"]

/// `$structureMerges(list)`: whether NodeStructureExtension merges this
/// list with the next one itself.
private func structureMerges(_ list: ListNode) throws -> Bool {
  let klass = type(of: list)
  guard let next = try list.getNextSibling(), type(of: next) == klass else { return false }
  return try INTERNAL_isNodeStructureEnforced(getEditor())
    && getDeclaredNodeStructure(klass).declaredBy.mergeAdjacent == klass
}

/// `ListNode`: an `<ol>` or `<ul>`, holding list items.
open class ListNode: ElementNode, NodeStructureMergeHook, NodeStructureSplitHook {
  public var __tag: ListNodeTagType = "ol"
  public var __start: Int = 1
  public var __listType: ListType = "number"

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ListNode.self, type: "list", extends: ElementNode.self,
      json: [
        // 'ul'/'ol' are the legacy tag-form listType. Applied through
        // setListType, which also maintains the derived __tag.
        SchemaField(
          "listType",
          aliasedValue(
            enumValue([.string("number"), .string("bullet"), .string("check")]),
            TAG_TO_LIST_TYPE.mapValues { .string($0) }),
          of: ListNode.self, get: { .string($0.__listType) },
          set: { try $0.setListType($1?.stringValue ?? "number") }),
        SchemaField("start", numberValue(1), field: \ListNode.__start),
        // Derived from listType: written on export, not applied on import.
        SchemaField(
          "tag", enumValue([.string("ul"), .string("ol")]), of: ListNode.self,
          get: { .string($0.__tag) }, set: nil),
      ],
      transform: { node in
        let node = node as! ListNode
        // Where NodeStructureExtension enforces declared structure, it
        // merges adjacent lists itself.
        if try !structureMerges(node) {
          try mergeNextSiblingListIfSameType(node)
        }
        try updateChildrenListItemValue(node)
      },
      children: .list([.klass(ListItemNode.self)]),
      // What mergeNextSiblingListIfSameType compares (start is not).
      mergeAdjacent: .by([.field("listType")]))
  }

  open override class func importDOM() -> DOMConversionMap? {
    [
      "ol": { _ in DOMConversion(priority: 0, convertListNode) },
      "ul": { _ in DOMConversion(priority: 0, convertListNode) },
    ]
  }

  /// `new ListNode(listType, start, key)`.
  public convenience init(
    _ listType: ListType = "number", start: Int = 1, key: NodeKey? = nil
  )
    throws
  {
    try self.init(key: key)
    let listType = TAG_TO_LIST_TYPE[listType] ?? listType
    __listType = listType
    __tag = listType == "number" ? "ol" : "ul"
    __start = start
  }

  /// `listType` and `tag` are accessor properties, which the schema does
  /// not carry across a clone.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prev = prevNode as! ListNode
    __tag = prev.__tag
    __start = prev.__start
    __listType = prev.__listType
  }

  open func getTag() throws -> ListNodeTagType {
    try getLatest().__tag
  }

  @discardableResult
  open func setListType(_ type: ListType) throws -> Self {
    let writable = try getWritable()
    writable.__listType = type
    writable.__tag = type == "number" ? "ol" : "ul"
    return writable
  }

  open func getListType() throws -> ListType {
    try getLatest().__listType
  }

  open func getStart() throws -> Int {
    try getLatest().__start
  }

  @discardableResult
  open func setStart(_ start: Int) throws -> Self {
    let writable = try getWritable()
    writable.__start = start
    return writable
  }

  // View

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let dom = getDocument().createElement(__tag)
    if __start != 1 {
      dom.setAttribute("start", String(__start))
    }
    try setListThemeClassNames(dom, config.theme, self)
    return dom
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevNode = prevNode as! ListNode
    if prevNode.__tag != __tag || prevNode.__listType != __listType {
      return true
    }
    try setListThemeClassNames(dom, config.theme, self)
    if prevNode.__start != __start {
      dom.setAttribute("start", String(__start))
    }
    return false
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = try createDOM(editor._config, editor)
    if __start != 1 {
      element.setAttribute("start", String(__start))
    }
    if __listType == "check" {
      element.setAttribute("__lexicalListType", "check")
    }
    // convertListNode reads `dir` back off the <ol>/<ul>, and this override
    // does not call super.exportDOM, which would write it.
    if let direction = try getDirection(), !direction.isEmpty {
      element.dir = direction
    }
    return DOMExportOutput(element: element)
  }

  open override func canBeEmpty() throws -> Bool { false }

  /// `INTERNAL_$afterStructureMerge(last, first)` (internal): joins nested
  /// lists at the seam of a merge the node structure normalization made.
  open func INTERNAL_afterStructureMerge(_ last: LexicalNode, _ first: LexicalNode) throws {
    try mergeNestedListsAtSeam(last, first, onlyChild: true)
  }

  /// `INTERNAL_$afterStructureSplit(original, preceding)` (internal): an
  /// ordered list part continues the numbering of the items before it.
  open func INTERNAL_afterStructureSplit(_ original: ElementNode, _ before: [LexicalNode]) throws {
    guard try getListType() == "number", let original = original as? ListNode else { return }
    var start = try original.getStart()
    for node in before {
      // As updateChildrenListItemValue counts: an item that only wraps a
      // nested list has no number of its own.
      if let item = node as? ListItemNode, try !isListNode(item.getFirstChild()) {
        start += 1
      }
    }
    try setStart(start)
  }

  open override func canIndent() -> Bool { false }

  @discardableResult
  open override func splice(
    _ start: Int, _ deleteCount: Int, _ nodesToInsert: [LexicalNode]
  ) throws -> Self {
    var listItemNodesToInsert = nodesToInsert
    for (i, node) in nodesToInsert.enumerated() where !isListItemNode(node) {
      let listItem = try createListItemNode()
      if let element = node as? ElementNode, try !(isListNode(element) || element.isInline()) {
        // A block can't stay a block inside a list item, so it is unwrapped
        // into its own children.
        try listItem.append(contentsOf: element.getChildren())
      } else {
        try listItem.append(node)
      }
      listItemNodesToInsert[i] = listItem
    }
    return try super.splice(start, deleteCount, listItemNodesToInsert)
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    isListItemNode(child)
  }

  /// `createListItemNode()`: the item to be a child of this list,
  /// `createListItemNode()` (the free function) by default.
  open func createListItemNode() throws -> ListItemNode {
    try LexicalList.createListItemNode()
  }
}

/// `$setListThemeClassNames(dom, editorThemeClasses, node)`.
private func setListThemeClassNames(
  _ dom: DOMElement, _ editorThemeClasses: EditorThemeClasses, _ node: ListNode
) throws {
  var classesToAdd: [String] = []
  var classesToRemove: [String] = []
  if let listTheme = editorThemeClasses["list"]?.objectValue {
    let listLevelsClassNames = listTheme["\(node.__tag)Depth"]?.arrayValue ?? []
    let listDepth = try getListDepth(node) - 1
    let normalizedListDepth =
      listLevelsClassNames.isEmpty ? 0 : listDepth % listLevelsClassNames.count
    let listLevelClassName =
      normalizedListDepth < listLevelsClassNames.count
      ? listLevelsClassNames[normalizedListDepth].stringValue : nil
    let listClassName = listTheme[node.__tag]?.stringValue
    var nestedListClassName: String?
    let checklistClassName = listTheme["checklist"]?.stringValue
    if let nested = listTheme["nested"]?.objectValue, let list = nested["list"]?.stringValue,
      !list.isEmpty
    {
      nestedListClassName = list
    }
    if let listClassName {
      classesToAdd.append(listClassName)
    }
    if let checklistClassName, node.__listType == "check" {
      classesToAdd.append(checklistClassName)
    }
    if let listLevelClassName {
      classesToAdd += normalizeClassNames(listLevelClassName)
      for (i, name) in listLevelsClassNames.enumerated() where i != normalizedListDepth {
        classesToRemove += normalizeClassNames(name.stringValue)
      }
    }
    if let nestedListClassName {
      let nestedListItemClasses = normalizeClassNames(nestedListClassName)
      if listDepth > 1 {
        classesToAdd += nestedListItemClasses
      } else {
        classesToRemove += nestedListItemClasses
      }
    }
  }
  if !classesToRemove.isEmpty {
    dom.classList.remove(contentsOf: classesToRemove)
  }
  if !classesToAdd.isEmpty {
    dom.classList.add(contentsOf: classesToAdd)
  }
}

/// `$normalizeChildren(nodes, listNode)`: the children of a list converted
/// from HTML, each a list item holding either one nested list or other
/// inline content.
private func normalizeChildren(
  _ nodes: [LexicalNode], _ listNode: ListNode
) throws
  -> [ListItemNode]
{
  var normalizedListItems: [ListItemNode] = []
  for node in nodes {
    if let item = node as? ListItemNode {
      normalizedListItems.append(item)
      let children = try item.getChildren()
      if children.count > 1 {
        for child in children where isListNode(child) {
          normalizedListItems.append(try listNode.createListItemNode().append(child))
        }
      }
    } else {
      normalizedListItems.append(try listNode.createListItemNode().append(node))
    }
  }
  return normalizedListItems
}

private func isDomChecklist(_ domNode: DOMElement) -> Bool {
  if domNode.getAttribute("__lexicallisttype") == "check"
    // a GitHub checklist
    || domNode.classList.contains("contains-task-list")
    // a Joplin checklist
    || domNode.getAttribute("data-is-checklist") == "1"
  {
    return true
  }
  // A list whose children are checklist items is a checklist (a Google
  // Docs checklist).
  return domNode.children.contains { $0.hasAttribute("aria-checked") }
}

/// `HTMLOListElement.start`: the `start` attribute as an HTML integer, or 1.
func olStart(_ element: DOMElement) -> Int {
  guard let value = element.getAttribute("start") else { return 1 }
  var units = Substring(value).drop { " \t\n\u{0C}\r".contains($0) }
  var sign = 1
  if units.first == "-" || units.first == "+" {
    sign = units.first == "-" ? -1 : 1
    units = units.dropFirst()
  }
  let digits = units.prefix { $0.isASCII && $0.isNumber }
  guard !digits.isEmpty, let n = Int32(digits) else { return 1 }
  return sign * Int(n)
}

private func convertListNode(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let domNode = domNode as? DOMElement else { return nil }
  let node: ListNode
  if domNode.localName == "ol" {
    node = try createListNode("number", start: olStart(domNode))
  } else if isDomChecklist(domNode) {
    node = try createListNode("check")
  } else {
    node = try createListNode("bullet")
  }
  try setDirectionFromDOM(node, domNode)
  return DOMConversionOutput(
    node: node, after: { children in try normalizeChildren(children, node) })
}

/// `$createListNode(listType, start)`.
public func createListNode(_ listType: ListType = "number", start: Int = 1) throws -> ListNode {
  try applyNodeReplacement(ListNode(listType, start: start))
}

/// `$isListNode(node)`.
public func isListNode(_ node: LexicalNode?) -> Bool { node is ListNode }
