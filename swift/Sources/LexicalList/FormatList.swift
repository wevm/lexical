import Lexical
import LexicalUtils

// Port of packages/lexical-list/src/formatList.ts.

private func isSelectingEmptyListItem(
  _ anchorNode: LexicalNode, _ nodes: [LexicalNode]
) throws
  -> Bool
{
  guard let item = anchorNode as? ListItemNode else { return false }
  return try nodes.isEmpty
    || (nodes.count == 1 && item.is(nodes[0]) && item.getChildrenSize() == 0)
}

/// `$newListFrom(list, listType)`: the list that replaces `list` when its
/// type changes, a copy of it.
private func newListFrom(_ list: ElementNode, _ listType: ListType) throws -> ListNode {
  if let list = list as? ListNode {
    return try copyNode(list).setListType(listType)
  }
  return try createListNode(listType)
}

/// `$insertList(listType)`: turns the selected blocks into a list of
/// `listType`, or changes the type of the selected lists.
public func insertList(_ listType: ListType) throws {
  guard let selection = try getSelection() else { return }
  var nodes = try selection.getNodes()
  if let selection = selection as? RangeSelection {
    let anchor = selection.getStartEndPoints()!.0
    let anchorNode = try anchor.getNode()
    let anchorNodeParent = try anchorNode.getParent()
    if try isBlockContainer(anchorNode) {
      let container = anchorNode as! ElementNode
      if let firstChild = try container.getFirstChild() {
        nodes = try firstChild.selectStart().getNodes()
      } else {
        let paragraph = try createParagraphNode()
        try container.append(paragraph)
        nodes = try paragraph.select().getNodes()
      }
    } else if try isSelectingEmptyListItem(anchorNode, nodes) {
      if try isBlockContainer(anchorNodeParent) {
        let list = try createListNode(listType)
        try anchorNode.replace(list)
        let listItem = try createListItemNode()
        if let element = anchorNode as? ElementNode {
          try listItem.setFormat(element.getFormatType())
          try listItem.setIndent(element.getIndent())
        }
        try list.append(listItem)
      } else if let item = anchorNode as? ListItemNode {
        let parent = try item.getParentOrThrow()
        // A named-slot value's assignment is managed by its slot's owner.
        if try getSlotHost(parent) == nil {
          let list = try newListFrom(parent, listType)
          try append(list, parent.getChildren())
          try parent.replace(list)
        }
      }
      return
    }
  }

  var handled = Set<NodeKey>()
  var kept = try keptTops(nodes)
  for node in nodes {
    if let element = node as? ElementNode, try element.isEmpty(), !isListItemNode(element),
      // An empty container the selection passes over stays a container.
      try !isBlockContainer(element),
      // A named-slot value's slot assignment is managed by its owner.
      try getSlotHost(element) == nil, !handled.contains(element.getKey())
    {
      try createListOrMerge(element, listType)
      continue
    }
    var parent: ElementNode? =
      isLeafNode(node)
      ? try node.getParent()
      : (try (node as? ListItemNode)?.isEmpty() == true ? node as? ElementNode : nil)
    while let current = parent {
      if try isBlockContainer(current) {
        // A block in a container is not converted, nor is its container.
        break
      }
      let parentKey = current.getKey()
      if let list = current as? ListNode {
        // A list occupying a named slot is left as it is.
        if !handled.contains(parentKey), try getSlotHost(list) == nil {
          let newListNode = try newListFrom(list, listType)
          try append(newListNode, list.getChildren())
          try list.replace(newListNode)
          handled.insert(parentKey)
        }
        break
      }
      let nextParent = try current.getParent()
      if try isBlockContainer(nextParent) {
        if let target = try listTarget(current, node, &kept),
          !handled.contains(target.getKey())
        {
          handled.insert(target.getKey())
          try createListOrMerge(target, listType)
        }
        break
      }
      parent = nextParent
    }
  }
}

/// `$isListItemContent(node)`: whether a list item can hold `node` where
/// NodeStructureExtension enforces declared structure.
private func isListItemContent(_ node: LexicalNode) -> Bool {
  INTERNAL_isInlineContent(node) || isListNode(node)
    || ((node as? ParagraphNode).map { !$0.isShadowRoot() } ?? false)
}

/// `$keptTops(nodes)`: where structure is enforced, for the node in the
/// nearest block container that holds each selected node, whether it stays
/// and only the blocks inside it become lists.
private func keptTops(_ nodes: [LexicalNode]) throws -> [NodeKey: Bool] {
  var kept: [NodeKey: Bool] = [:]
  guard try INTERNAL_isNodeStructureEnforced(getEditor()) else { return kept }
  for node in nodes {
    var top: LexicalNode? = node
    while let current = top, try !isBlockContainer(current.getParent()) {
      top = try current.getParent()
    }
    if let element = top as? ElementNode, try !isBlockContainer(element),
      kept[element.getKey()] == nil
    {
      kept[element.getKey()] = try !element.getChildren().allSatisfy(isListItemContent)
    }
  }
  return kept
}

/// `$listTarget(top, leaf, kept)`: the node `insertList` turns into a list
/// item for the selected `leaf`, or `nil` for none.
private func listTarget(
  _ top: ElementNode, _ leaf: LexicalNode, _ kept: inout [NodeKey: Bool]
) throws -> ElementNode? {
  guard try INTERNAL_isNodeStructureEnforced(getEditor()) else { return top }
  let keep: Bool
  if let value = kept[top.getKey()] {
    keep = value
  } else {
    keep = try !top.getChildren().allSatisfy(isListItemContent)
    kept[top.getKey()] = keep
  }
  if !keep {
    return top
  }
  // The block that holds the selected text, never inline content.
  return try findMatchingParent(leaf) { try INTERNAL_isBlock($0) && !INTERNAL_isInlineContent($0) }
    as? ElementNode
}

private func append(_ node: ElementNode, _ nodesToAppend: [LexicalNode]) throws {
  try node.splice(node.getChildrenSize(), 0, nodesToAppend)
}

@discardableResult
private func createListOrMerge(_ node: ElementNode, _ listType: ListType) throws -> ListNode {
  if let list = node as? ListNode {
    return list
  }
  let previousSibling = try node.getPreviousSibling()
  let nextSibling = try node.getNextSibling()
  let listItem = try createListItemNode()
  try append(listItem, node.getChildren())
  let targetList: ListNode
  if let previous = previousSibling as? ListNode, try listType == previous.getListType() {
    try previous.append(listItem)
    // With the same type of list on both sides, merge them.
    if let next = nextSibling as? ListNode, try listType == next.getListType() {
      try append(previous, next.getChildren())
      try next.remove()
    }
    targetList = previous
  } else if let next = nextSibling as? ListNode, try listType == next.getListType() {
    try next.getFirstChildOrThrow().insertBefore(listItem)
    targetList = next
  } else {
    let list = try createListNode(listType)
    try list.append(listItem)
    try node.replace(list)
    targetList = list
  }
  // The list item must be attached before its indent is set.
  try listItem.setFormat(node.getFormatType())
  try listItem.setIndent(node.getIndent())
  // Element points on the list move to the list item.
  if let selection = try getSelection() as? RangeSelection {
    if targetList.getKey() == selection.anchor.key {
      try selection.anchor.set(listItem.getKey(), selection.anchor.offset, .element)
    }
    if targetList.getKey() == selection.focus.key {
      try selection.focus.set(listItem.getKey(), selection.focus.offset, .element)
    }
  }
  try node.remove()
  return targetList
}

/// `mergeLists(list1, list2)`: appends `list2`'s children to `list1`,
/// joining nested lists at the seam, and removes `list2`.
public func mergeLists(_ list1: ListNode, _ list2: ListNode) throws {
  try joinLists(list1, list2, onlyChild: false)
}

private func joinLists(_ list1: ListNode, _ list2: ListNode, onlyChild: Bool) throws {
  try mergeNestedListsAtSeam(list1.getLastChild(), list2.getFirstChild(), onlyChild: onlyChild)
  let toMerge = try list2.getChildren()
  if !toMerge.isEmpty {
    try list1.append(contentsOf: toMerge)
  }
  try list2.remove()
}

/// `mergeNestedListsAtSeam(listItem1, listItem2, onlyChild)` (internal):
/// where two lists meet, joins two items that only wrap nested lists of the
/// same type.
func mergeNestedListsAtSeam(
  _ listItem1: LexicalNode?, _ listItem2: LexicalNode?, onlyChild: Bool = false
) throws {
  guard let item1 = listItem1 as? ListItemNode, let item2 = listItem2 as? ListItemNode,
    let list1 = try nestedList(item1), let list2 = try nestedList(item2)
  else { return }
  if onlyChild, try item1.getChildrenSize() != 1 || item2.getChildrenSize() != 1 {
    return
  }
  // A nested <ul> must not swallow a nested <ol>.
  guard try list1.getListType() == list2.getListType() else { return }
  try joinLists(list1, list2, onlyChild: onlyChild)
  if !onlyChild {
    try item2.remove()
    return
  }
  // ElementNode's own remove: ListItemNode.remove would also join the item
  // after this one.
  try removeNode(item2, restoreSelection: true, preserveEmptyParent: false)
}

/// `$removeList()`: turns the lists the selection touches back into
/// paragraphs.
public func removeList() throws {
  guard let selection = try getSelection() as? RangeSelection else { return }
  var listNodes: [ListNode] = []
  func add(_ list: ListNode) {
    if !listNodes.contains(where: { $0 === list }) { listNodes.append(list) }
  }
  let nodes = try selection.getNodes()
  let anchorNode = try selection.anchor.getNode()
  if try isSelectingEmptyListItem(anchorNode, nodes) {
    add(try getTopListNode(anchorNode))
  } else {
    for node in nodes where isLeafNode(node) {
      if let listItemNode = try getNearestNodeOfType(node, ListItemNode.self) {
        add(try getTopListNode(listItemNode))
      }
    }
  }
  for listNode in listNodes {
    // A list occupying a named slot is left as it is.
    if try getSlotHost(listNode) != nil {
      continue
    }
    var insertionPoint: ElementNode = listNode
    for listItemNode in try getAllListItems(listNode) {
      let paragraph = try createParagraphNode().setTextStyle(selection.style)
        .setTextFormat(selection.format)
      // The paragraph stands in for the item, so it keeps the item's own
      // element state.
      try paragraph.setFormat(listItemNode.getFormatType())
        .setIndent(listItemNode.getIndent())
        .setDirection(listItemNode.getDirection())
        .setStyle(listItemNode.getStyle())
      try append(paragraph, listItemNode.getChildren())
      try insertionPoint.insertAfter(paragraph)
      insertionPoint = paragraph
      // A point on the item itself moves to the same offset in the
      // paragraph, which holds the same children.
      if listItemNode.__key == selection.anchor.key {
        try setPointFromCaret(
          selection.anchor,
          normalizeCaret(getChildCaretAtIndex(paragraph, selection.anchor.offset, .next)))
      }
      if listItemNode.__key == selection.focus.key {
        try setPointFromCaret(
          selection.focus,
          normalizeCaret(getChildCaretAtIndex(paragraph, selection.focus.offset, .next)))
      }
      try listItemNode.remove()
    }
    try listNode.remove()
  }
}

/// `updateChildrenListItemValue(list)`: numbers the list's items from its
/// start, and clears `checked` outside a checklist.
public func updateChildrenListItemValue(_ list: ListNode) throws {
  let isNotChecklist = try list.getListType() != "check"
  var value = try list.getStart()
  for child in try list.getChildren() {
    guard let item = child as? ListItemNode else { continue }
    if try item.getValue() != value {
      try item.setValue(value)
    }
    if isNotChecklist, try item.getLatest().__checked != nil {
      try item.setChecked(nil)
    }
    if try !isListNode(item.getFirstChild()) {
      value += 1
    }
  }
}

/// `mergeNextSiblingListIfSameType(list)`: merges the next sibling list of
/// the same type into `list` (a `<ul>` with a `<ul>`, not with an `<ol>`).
public func mergeNextSiblingListIfSameType(_ list: ListNode) throws {
  if let nextSibling = try list.getNextSibling() as? ListNode,
    try list.getListType() == nextSibling.getListType()
  {
    try mergeLists(list, nextSibling)
  }
}

/// `$handleIndent(listItemNode)`: nests the item one level deeper.
public func handleIndent(_ listItemNode: ListItemNode) throws {
  if try isNestedListNode(listItemNode) {
    return
  }
  let parent = try listItemNode.getParent()
  let nextSibling = try listItemNode.getNextSibling()
  let previousSibling = try listItemNode.getPreviousSibling()
  // With nested lists on either side, merge them all together.
  if let nextInner = try nestedList(nextSibling), let innerList = try nestedList(previousSibling) {
    try innerList.append(listItemNode)
    // Only when the following sublist is the same kind of list.
    if try innerList.getListType() == nextInner.getListType() {
      try append(innerList, nextInner.getChildren())
      try nextSibling!.remove()
    }
  } else if let innerList = try nestedList(nextSibling) {
    if let firstChild = try innerList.getFirstChild() {
      try firstChild.insertBefore(listItemNode)
    }
  } else if let innerList = try nestedList(previousSibling) {
    try innerList.append(listItemNode)
  } else if let parent = parent as? ListNode {
    // Otherwise, a new nested list.
    let newListItem = try copyNode(listItemNode)
    let newList = try copyNode(parent)
    try newListItem.append(newList)
    try newList.append(listItemNode)
    if let previousSibling {
      try previousSibling.insertAfter(newListItem)
    } else if let nextSibling {
      try nextSibling.insertBefore(newListItem)
    } else {
      try parent.append(newListItem)
    }
  }
}

/// `$handleOutdent(listItemNode)`: moves the item one level up.
public func handleOutdent(_ listItemNode: ListItemNode) throws {
  if try isNestedListNode(listItemNode) {
    return
  }
  let parentList = try listItemNode.getParent()
  let grandparentListItem = try parentList?.getParent()
  let greatGrandparentList = try grandparentListItem?.getParent()
  // Without these ancestors it is not indented.
  guard isListNode(greatGrandparentList),
    let grandparentListItem = grandparentListItem as? ListItemNode,
    let parentList = parentList as? ListNode
  else { return }
  // The outdent below assumes the wrapper item holds only the nested list.
  if try grandparentListItem.getChildrenSize() > 1,
    try parentList.getChildrenSize() == 1
      || (!listItemNode.is(parentList.getFirstChild())
        && !listItemNode.is(parentList.getLastChild()))
  {
    try outdentFromItemWithContent(listItemNode, parentList, grandparentListItem)
    return
  }
  if try listItemNode.is(parentList.getFirstChild()) {
    // The first item goes before the grandparent.
    try grandparentListItem.insertBefore(listItemNode)
    if try parentList.isEmpty() {
      try grandparentListItem.remove()
    }
  } else if try listItemNode.is(parentList.getLastChild()) {
    // The last item goes after it.
    try grandparentListItem.insertAfter(listItemNode)
    if try parentList.isEmpty() {
      try grandparentListItem.remove()
    }
  } else {
    // Otherwise the siblings split into two nested lists around it.
    let previousSiblingsListItem = try copyNode(listItemNode)
    let previousSiblingsList = try copyNode(parentList)
    try previousSiblingsListItem.append(previousSiblingsList)
    for sibling in try listItemNode.getPreviousSiblings() {
      try previousSiblingsList.append(sibling)
    }
    let nextSiblingsListItem = try copyNode(listItemNode)
    let nextSiblingsList = try copyNode(parentList)
    try nextSiblingsListItem.append(nextSiblingsList)
    try append(nextSiblingsList, listItemNode.getNextSiblings())
    try grandparentListItem.insertBefore(previousSiblingsListItem)
    try grandparentListItem.insertAfter(nextSiblingsListItem)
    try grandparentListItem.replace(listItemNode)
  }
}

/// `$outdentFromItemWithContent(listItemNode, parentList, wrapper)`: the
/// outdent when the wrapper item also holds other content, which stays.
private func outdentFromItemWithContent(
  _ listItemNode: ListItemNode, _ parentList: ListNode, _ wrapper: ListItemNode
) throws {
  let nextSiblings = try listItemNode.getNextSiblings()
  let after = try parentList.getNextSiblings()
  try wrapper.insertAfter(listItemNode)
  var last: ListItemNode = listItemNode
  if !nextSiblings.isEmpty {
    let nextSiblingsListItem = try copyNode(listItemNode)
    let nextSiblingsList = try copyNode(parentList)
    try nextSiblingsListItem.append(nextSiblingsList)
    try append(nextSiblingsList, nextSiblings)
    try last.insertAfter(nextSiblingsListItem)
    last = nextSiblingsListItem
  }
  if !after.isEmpty {
    let afterListItem = try copyNode(wrapper)
    try last.insertAfter(afterListItem)
    try append(afterListItem, after)
  }
  if try parentList.isEmpty() {
    try parentList.remove()
  }
  if try wrapper.isEmpty() {
    try wrapper.remove()
  }
}

/// `$handleListInsertParagraph(restoreNumbering)`: Enter in an empty (or
/// whitespace-only) list item ends the list there, or outdents a nested
/// one. Returns whether it handled the Enter.
@discardableResult
public func handleListInsertParagraph(_ restoreNumbering: Bool = false) throws -> Bool {
  guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed() else {
    return false
  }
  let anchor = try selection.anchor.getNode()
  var listItem: ListItemNode?
  if let item = anchor as? ListItemNode, try item.getChildrenSize() == 0 {
    listItem = item
  } else if anchor is TextNode, let parentListItem = try anchor.getParent() as? ListItemNode,
    try parentListItem.getChildren().allSatisfy({
      try ($0 as? TextNode).map { try jsTrim($0.getTextContent()).isEmpty } ?? false
    })
  {
    listItem = parentListItem
  }
  guard let listItem else { return false }
  let topListNode = try getTopListNode(listItem)
  guard let parent = try listItem.getParent() as? ListNode else {
    throw LexicalError("A ListItemNode must have a ListNode for a parent.")
  }
  let grandparent = try parent.getParent()
  let replacementNode: ElementNode
  if try isBlockContainer(grandparent) {
    replacementNode = try createParagraphNode()
    try topListNode.insertAfter(replacementNode)
  } else if let grandparent = grandparent as? ListItemNode {
    replacementNode = try copyNode(grandparent)
    try grandparent.insertAfter(replacementNode)
  } else {
    return false
  }
  try replacementNode.setTextStyle(selection.style).setTextFormat(selection.format).select()
  let nextSiblings = try listItem.getNextSiblings()
  if !nextSiblings.isEmpty {
    let newStart = restoreNumbering ? try getNewListStart(parent, listItem) : 1
    let newList = try copyNode(parent).setStart(newStart)
    if let item = replacementNode as? ListItemNode {
      let newListItem = try copyNode(item)
      try newListItem.append(newList)
      try item.insertAfter(newListItem)
    } else {
      try replacementNode.insertAfter(newList)
    }
    try newList.append(contentsOf: nextSiblings)
  }
  // Don't leave nested empty lists behind.
  try removeHighestEmptyListParent(listItem)
  return true
}
