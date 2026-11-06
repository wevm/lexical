import Lexical

// Port of packages/lexical-list/src/LexicalListItemNode.ts.

/// `$pointsAround(parent, items)`: for the selection's element points in
/// `parent`, which holds the consecutive `items`, what puts each of them
/// back once the items are moved into a list inserted in their place.
private func pointsAround(
  _ parent: ElementNode, _ items: [LexicalNode]
) throws
  -> ((ListNode) throws -> Void)?
{
  guard let selection = try getSelection() as? RangeSelection else { return nil }
  let parentKey = parent.getKey()
  let first = try items[0].getIndexWithinParent()
  let saved = [selection.anchor, selection.focus]
    .filter { $0.type == .element && $0.key == parentKey }
    .map { (offset: $0.offset, point: $0) }
  if saved.isEmpty {
    return nil
  }
  return { _ in
    for (offset, point) in saved {
      if offset <= first {
        try point.set(parentKey, offset, .element)
      } else if offset < first + items.count {
        let item = items[offset - first - 1] as! ElementNode
        try point.set(item.getKey(), item.getChildrenSize(), .element)
      } else {
        try point.set(parentKey, offset - items.count + 1, .element)
      }
    }
  }
}

/// The deepest list nesting `setIndent` will walk to, which bounds the work
/// an untrusted `indent` could otherwise make.
private let MAX_LIST_ITEM_INDENT: Double = 128

/// `ListItemNode`: an `<li>`.
open class ListItemNode: ElementNode, NodeStructureLiftHook {
  public var __value: Int = 1
  public var __checked: Bool?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      ListItemNode.self, type: "listitem", extends: ElementNode.self,
      json: [
        // getChecked computes from the parent list's type; setChecked is a
        // bare field write.
        SchemaField(
          "checked", optional(booleanValue()), of: ListItemNode.self,
          get: { try $0.getChecked().map(JSONValue.bool) },
          set: { $0.__checked = $1?.boolValue }),
        // Bounds the inherited field: applying it nests or unwraps one whole
        // list per level. Derived: getIndent() computes it from the nesting.
        SchemaField(
          "indent",
          numberValue(0, integer: true, min: 0, max: MAX_LIST_ITEM_INDENT, clamp: true),
          of: ListItemNode.self, get: { try .int($0.getIndent()) },
          set: { try $0.setIndent(clampedInt($1?.numberValue ?? 0)) }, derived: true),
        // Derived: the list renumbers its items.
        SchemaField("value", numberValue(1), field: \ListItemNode.__value, derived: true),
      ],
      transform: { node in try listItemTransform(node as! ListItemNode) },
      children: .list([.inline, .klass(ListNode.self)]))
  }

  open override class func importDOM() -> DOMConversionMap? {
    ["li": { _ in DOMConversion(priority: 0, convertListItemElement) }]
  }

  /// `new ListItemNode(value, checked, key)`.
  public convenience init(_ value: Int = 1, checked: Bool? = nil, key: NodeKey? = nil) throws {
    try self.init(key: key)
    __value = value
    __checked = checked
  }

  /// `checked` is an accessor property, which the schema does not carry
  /// across a clone.
  open override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    let prev = prevNode as! ListItemNode
    __value = prev.__value
    __checked = prev.__checked
  }

  // View

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("li")
    try updateListItemDOM(nil, element, config)
    return element
  }

  open func updateListItemDOM(
    _ prevNode: ListItemNode?, _ dom: DOMElement, _ config: EditorConfig
  ) throws {
    try updateListItemChecked(dom, self, prevNode)
    dom.setAttribute("value", String(__value))
    try setListItemThemeClassNames(dom, config.theme, self)
    let prevStyle = prevNode?.__style ?? ""
    let nextStyle = __style
    if prevStyle != nextStyle {
      setDOMStyleFromCSS(dom.style, nextStyle, prevStyle)
    }
    applyMarkerStyles(dom, self, prevNode)
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    try updateListItemDOM(prevNode as? ListItemNode, dom, config)
    return false
  }

  open override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = try createDOM(editor._config)
    let formatType = try getFormatType()
    if !formatType.isEmpty {
      element.style.setProperty("text-align", formatType)
    }
    if let direction = try getDirection(), !direction.isEmpty {
      element.dir = direction
    }
    if try isNestedListNode(self) {
      return DOMExportOutput(
        element: element,
        after: { containerElement in
          if let container = containerElement as? DOMElement,
            let prevSibling = container.previousElementSibling, prevSibling.nodeName == "LI"
          {
            while let child = container.firstChild {
              prevSibling.append(child)
            }
            container.remove()
          }
          return containerElement
        })
    }
    return DOMExportOutput(element: element)
  }

  // Mutation

  @discardableResult
  open override func append(contentsOf nodes: [LexicalNode]) throws -> Self {
    for node in nodes {
      if let element = node as? ElementNode, try canMergeWith(element) {
        try append(contentsOf: element.getChildren())
        try element.remove()
      } else {
        try super.append(contentsOf: [node])
      }
    }
    return self
  }

  @discardableResult
  open override func replace<N: LexicalNode>(
    _ replaceWithNode: N, includeChildren: Bool = false
  )
    throws -> N
  {
    if isListItemNode(replaceWithNode) {
      return try super.replace(replaceWithNode)
    }
    try setIndent(0)
    guard let list = try getParentOrThrow() as? ListNode else {
      return replaceWithNode
    }
    if list.__first == getKey() {
      try list.insertBefore(replaceWithNode)
    } else if list.__last == getKey() {
      try list.insertAfter(replaceWithNode)
    } else {
      // Split the list.
      let newList = try copyNode(list)
      var nextSibling = try getNextSibling()
      while let nodeToAppend = nextSibling {
        nextSibling = try nodeToAppend.getNextSibling()
        try newList.append(nodeToAppend)
      }
      try list.insertAfter(replaceWithNode)
      try replaceWithNode.insertAfter(newList)
    }
    let toReplaceKey = __key
    var prevSizeBeforeChildrenTransfer = 0
    if includeChildren {
      guard let element = replaceWithNode as? ElementNode else {
        throw LexicalError("includeChildren should only be true for ElementNodes")
      }
      prevSizeBeforeChildrenTransfer = try element.getChildrenSize()
      try element.splice(prevSizeBeforeChildrenTransfer, 0, getChildren())
      // LexicalNode.replace remaps element points on the replaced node; this
      // override skips it, and remove() below would move them to a sibling.
      if let selection = try getSelection() as? RangeSelection {
        for point in [selection.anchor, selection.focus]
        where point.key == toReplaceKey && point.type == .element {
          try point.set(
            element.getKey(), prevSizeBeforeChildrenTransfer + point.offset, .element)
        }
      }
    }
    try remove()
    if try list.getChildrenSize() == 0 {
      try list.remove()
    }
    return replaceWithNode
  }

  @discardableResult
  open override func insertAfter(
    _ node: LexicalNode, restoreSelection: Bool = true
  ) throws
    -> LexicalNode
  {
    guard let listNode = try getParentOrThrow() as? ListNode else {
      throw LexicalError("insertAfter: list node is not parent of list item node")
    }
    if isListItemNode(node) {
      return try super.insertAfter(node, restoreSelection: restoreSelection)
    }
    let siblings = try getNextSiblings()
    // Split the lists and insert the node between them.
    try listNode.insertAfter(node, restoreSelection: restoreSelection)
    if !siblings.isEmpty {
      let newListNode = try copyNode(listNode)
      // The items moving into the new list keep the numbers they were
      // rendered with, so it starts from the first of them (#7032).
      if try newListNode.getListType() == "number", let firstSibling = siblings[0] as? ListItemNode
      {
        try newListNode.setStart(getNewListStart(listNode, firstSibling))
      }
      for sibling in siblings {
        try newListNode.append(sibling)
      }
      try node.insertAfter(newListNode, restoreSelection: restoreSelection)
    }
    return node
  }

  open override func remove(preserveEmptyParent: Bool = false) throws {
    let prevSibling = try getPreviousSibling()
    let nextSibling = try getNextSibling()
    try super.remove(preserveEmptyParent: preserveEmptyParent)
    // Only join the surrounding sublists when they are the same kind of
    // list, otherwise the second one loses its listType.
    if let prevList = try nestedList(prevSibling), let nextList = try nestedList(nextSibling),
      try prevList.getListType() == nextList.getListType()
    {
      try mergeLists(prevList, nextList)
      try nextSibling!.remove()
    }
  }

  open override func resetOnCopyNodeFrom(_ original: LexicalNode) throws {
    try super.resetOnCopyNodeFrom(original)
    if try (original as? ListItemNode)?.getChecked() == true {
      try setChecked(false)
    }
  }

  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  )
    throws -> LexicalNode?
  {
    let newElement = try copyNode(self)
    try insertAfter(newElement, restoreSelection: restoreSelection)
    return newElement
  }

  open override func collapseAtStart(_ selection: RangeSelection) throws -> Bool {
    if try isNestedListNode(self) {
      return false
    }
    let listNode = try getParentOrThrow()
    let listNodeParent = try listNode.getParentOrThrow()
    if isListItemNode(listNodeParent) {
      try handleOutdent(self)
      return true
    }
    let paragraph = try createParagraphNode().append(contentsOf: getChildren())
    let nextSiblings = try getNextSiblings()
    if !nextSiblings.isEmpty {
      let newList = try copyNode(listNode)
      try newList.append(contentsOf: nextSiblings)
      try listNode.insertAfter(newList)
    }
    try listNode.insertAfter(paragraph)
    try remove()
    if try listNode.getChildrenSize() == 0 {
      try listNode.remove()
    }
    try paragraph.selectStart()
    return true
  }

  open func getValue() throws -> Int {
    try getLatest().__value
  }

  @discardableResult
  open func setValue(_ value: Int) throws -> Self {
    let writable = try getWritable()
    writable.__value = value
    return writable
  }

  /// `getChecked()`: whether the item is checked, or `nil` outside a
  /// checklist.
  open func getChecked() throws -> Bool? {
    let latest = try getLatest()
    let listType = try (getParent() as? ListNode)?.getListType()
    return listType == "check" ? latest.__checked ?? false : nil
  }

  @discardableResult
  open func setChecked(_ checked: Bool?) throws -> Self {
    let writable = try getWritable()
    writable.__checked = checked
    return writable
  }

  @discardableResult
  open func toggleChecked() throws -> Self {
    let writable = try getWritable()
    return try writable.setChecked(!(writable.__checked ?? false))
  }

  open override func getIndent() throws -> Int {
    // Without a parent, the node is likely being serialized.
    guard let parent = try getParent(), try isAttached() else {
      return try getLatest().__indent
    }
    var listNodeParent: ElementNode = try parent.getParentOrThrow()
    var indentLevel = 0
    while isListItemNode(listNodeParent) {
      listNodeParent = try listNodeParent.getParentOrThrow().getParentOrThrow()
      indentLevel += 1
    }
    return indentLevel
  }

  @discardableResult
  open override func setIndent(_ indent: Int) throws -> Self {
    try invariant(indent >= 0, "Indent value must be non-negative.")
    // Not clamped here: the bound belongs on the parse path.
    var currentIndent = try getIndent()
    while currentIndent != indent {
      if currentIndent < indent {
        try handleIndent(self)
        currentIndent += 1
      } else {
        try handleOutdent(self)
        currentIndent -= 1
      }
    }
    return self
  }

  open override func canInsertAfter(_ node: LexicalNode) throws -> Bool {
    isListItemNode(node)
  }

  open override func canReplaceWith(_ replacement: LexicalNode) throws -> Bool {
    isListItemNode(replacement)
  }

  open override func canMergeWith(_ node: ElementNode) throws -> Bool {
    isListItemNode(node) || isParagraphNode(node)
  }

  /// `INTERNAL_$liftsFromStructure(child, explicit)` (internal): a nested
  /// list may only be all a list item holds; next to other content it is
  /// lifted out, but only from an item the update created or changed.
  open func INTERNAL_liftsFromStructure(_ child: LexicalNode, _ explicit: Bool) throws -> Bool {
    if try !isListNode(child) || getChildrenSize() <= 1 {
      return false
    }
    if explicit {
      return true
    }
    let editor = try getEditor()
    let key = getKey()
    return !editor.getEditorState()._nodeMap.has(key) || editor._cloneNotNeeded.has(key)
  }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection else { return false }
    let anchorNode = try selection.anchor.getNode()
    let focusNode = try selection.focus.getNode()
    return try isParentOf(anchorNode) && isParentOf(focusNode)
      && getTextContent().utf16.count == selection.getTextContent().utf16.count
  }

  open override func isParentRequired() -> Bool { true }

  open override func createParentElementNode() throws -> ElementNode {
    try createListNode("bullet")
  }

  open override func canMergeWhenEmpty() -> Bool { true }
}

/// The `$transform` of ListItemNode: clears `checked` outside a checklist,
/// and wraps an item that is not in a list in one.
private func listItemTransform(_ node: ListItemNode) throws {
  let parent = try node.getParent()
  if let parent = parent as? ListNode {
    if try parent.getListType() != "check", try node.getChecked() != nil {
      try node.setChecked(nil)
    }
    return
  }
  guard let parent else { return }
  guard let newParent = try node.createParentElementNode() as? ListNode else {
    throw LexicalError("ListItemNode.createParentElementNode() must return a ListNode")
  }
  // The orphan and its neighboring orphans, in document order.
  var children: [LexicalNode] = [node]
  for previous in [true, false] {
    children.reverse()
    var sibling = try previous ? node.getPreviousSibling() : node.getNextSibling()
    while let current = sibling, isListItemNode(current) {
      children.append(current)
      sibling = try previous ? current.getPreviousSibling() : current.getNextSibling()
    }
  }
  let enforced = try INTERNAL_isNodeStructureEnforced(getEditor())
  let points = enforced ? try pointsAround(parent, children) : nil
  try node.insertBefore(newParent)
  try newParent.splice(0, 0, children)
  try points?(newParent)
  if enforced {
    // The ancestors' own transforms run on the result.
    var n: LexicalNode? = parent
    while let current = n, try !isRootOrShadowRoot(current) {
      try current.markDirty()
      n = try current.getParent()
    }
  }
  if try !isRootOrShadowRoot(parent), !enforced {
    try insertNodeToNearestRootAtCaret(
      newParent, rewindSiblingCaret(getSiblingCaret(newParent, .next)),
      SplitAtPointCaretNextOptions(shouldSplit: { _, _ in false }, removeEmptyDestination: true))
    if try parent.isEmpty(), try parent.isAttached() {
      try parent.remove()
    }
  }
}

private func applyMarkerStyles(_ dom: DOMElement, _ node: ListItemNode, _ prevNode: ListItemNode?) {
  let nextTextStyle = node.__textStyle
  let prevTextStyle = prevNode?.__textStyle ?? ""
  if prevNode != nil, prevTextStyle == nextTextStyle {
    return
  }
  let styles = getStyleObjectFromCSS(nextTextStyle)
  for (k, v) in styles {
    dom.style.setProperty("--listitem-marker-\(k)", v)
  }
  if !prevTextStyle.isEmpty {
    for (k, _) in getStyleObjectFromCSS(prevTextStyle) where styles[k] == nil {
      _ = dom.style.removeProperty("--listitem-marker-\(k)")
    }
  }
}

/// `$setListItemThemeClassNames(dom, editorThemeClasses, node)`.
private func setListItemThemeClassNames(
  _ dom: DOMElement, _ editorThemeClasses: EditorThemeClasses, _ node: ListItemNode
) throws {
  guard let listTheme = editorThemeClasses["list"]?.objectValue else { return }
  let listItemClassName = listTheme["listitem"]?.stringValue
  let nestedListItemClassName = listTheme["nested"]?.objectValue?["listitem"]?.stringValue
  let parentNode = try node.getParent()
  let isCheckList = try (parentNode as? ListNode)?.getListType() == "check"
  let checked = try node.getChecked()
  let isNested = try node.getChildren().contains { isListNode($0) }
  // The variable classes are removed first, so the class names stay in one
  // order however the element got here.
  var classesToRemove: [String] = []
  if let c = listTheme["listitemChecked"]?.stringValue { classesToRemove.append(c) }
  if let c = listTheme["listitemUnchecked"]?.stringValue { classesToRemove.append(c) }
  if let nestedListItemClassName {
    classesToRemove += normalizeClassNames(nestedListItemClassName)
  }
  if !classesToRemove.isEmpty {
    dom.classList.remove(contentsOf: classesToRemove)
  }
  var classesToAdd: [String] = []
  if let listItemClassName {
    classesToAdd += normalizeClassNames(listItemClassName)
  }
  if isCheckList {
    let key = checked == true ? "listitemChecked" : "listitemUnchecked"
    if let c = listTheme[key]?.stringValue { classesToAdd.append(c) }
  }
  if let nestedListItemClassName, isNested {
    classesToAdd += normalizeClassNames(nestedListItemClassName)
  }
  if !classesToAdd.isEmpty {
    dom.classList.add(contentsOf: classesToAdd)
  }
}

private func updateListItemChecked(
  _ dom: DOMElement, _ listItemNode: ListItemNode, _ prevListItemNode: ListItemNode?
) throws {
  // Only leaf items of a checklist are checkboxes.
  let isCheckbox =
    try (listItemNode.getParent() as? ListNode)?.getListType() == "check"
    && !isListNode(listItemNode.getFirstChild())
  if !isCheckbox {
    dom.removeAttribute("role")
    dom.removeAttribute("tabIndex")
    dom.removeAttribute("aria-checked")
  } else {
    dom.setAttribute("role", "checkbox")
    dom.setAttribute("tabIndex", "-1")
    dom.setAttribute("aria-checked", try listItemNode.getChecked() == true ? "true" : "false")
  }
}

private func convertListItemElement(_ domNode: DOMNode) throws -> DOMConversionOutput? {
  guard let domNode = domNode as? DOMElement else { return nil }
  if domNode.classList.contains("task-list-item") {
    for child in domNode.children where child.tagName == "INPUT" {
      return try convertCheckboxInput(child)
    }
  }
  if domNode.classList.contains("joplin-checkbox") {
    for child in domNode.children
    where child.classList.contains("checkbox-wrapper") && !child.children.isEmpty
      && child.children[0].tagName == "INPUT"
    {
      return try convertCheckboxInput(child.children[0])
    }
  }
  let ariaCheckedAttr = domNode.getAttribute("aria-checked")
  let checked: Bool? =
    ariaCheckedAttr == "true" ? true : ariaCheckedAttr == "false" ? false : nil
  let node = try createListItemNode(checked)
  try setFormatFromDOM(node, domNode)
  return DOMConversionOutput(
    node: try setDirectionFromDOM(node, domNode),
    after: { children in try setFormatFromChildren(node, children) })
}

private func convertCheckboxInput(_ domNode: DOMElement) throws -> DOMConversionOutput {
  guard domNode.getAttribute("type") == "checkbox" else {
    return DOMConversionOutput(node: nil)
  }
  let node = try createListItemNode(domNode.hasAttribute("checked"))
  return DOMConversionOutput(
    node: node, after: { children in try setFormatFromChildren(node, children) })
}

/// Google Docs puts the alignment on the `<p>` inside the `<li>`.
private func setFormatFromChildren(
  _ listItemNode: ListItemNode, _ children: [LexicalNode]
) throws
  -> [LexicalNode]
{
  if children.count == 1, let paragraph = children[0] as? ParagraphNode,
    try listItemNode.getFormatType().isEmpty, try !paragraph.getFormatType().isEmpty
  {
    try listItemNode.setFormat(paragraph.getFormatType())
    return try paragraph.getChildren()
  }
  return children
}

/// `$createListItemNode(checked)`: a list item; `true` or `false` makes it
/// a checkbox, checked or not.
public func createListItemNode(_ checked: Bool? = nil) throws -> ListItemNode {
  try applyNodeReplacement(ListItemNode(1, checked: checked))
}

/// `$isListItemNode(node)`.
public func isListItemNode(_ node: LexicalNode?) -> Bool { node is ListItemNode }
