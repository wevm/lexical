import Lexical

// Port of packages/lexical-list/src/utils.ts.

/// `$getListDepth(listNode)`: the depth of `listNode` from the root, 1 for
/// a list that is not nested.
public func getListDepth(_ listNode: ListNode) throws -> Int {
  var depth = 1
  var parent = try listNode.getParent()
  while let current = parent {
    if isListItemNode(current) {
      let parentList = try current.getParent()
      if isListNode(parentList) {
        depth += 1
        parent = try parentList!.getParent()
        continue
      }
      try invariant(false, "A ListItemNode must have a ListNode for a parent.")
    }
    return depth
  }
  return depth
}

/// `$getTopListNode(listItem)`: the outermost list holding `listItem`.
public func getTopListNode(_ listItem: LexicalNode) throws -> ListNode {
  guard let parentList = try listItem.getParent() as? ListNode else {
    throw LexicalError("A ListItemNode must have a ListNode for a parent.")
  }
  var list = parentList
  var parent: ElementNode? = parentList
  while let current = parent {
    parent = try current.getParent()
    if let parentList = parent as? ListNode {
      list = parentList
    }
  }
  return list
}

/// `$isLastItemInList(listItem)`: whether `listItem` holds no nested list
/// and no list item above it has later siblings.
public func isLastItemInList(_ listItem: ListItemNode) throws -> Bool {
  var isLast = true
  if try isListNode(listItem.getFirstChild()) {
    return false
  }
  var parent: ElementNode? = listItem
  while let current = parent {
    if isListItemNode(current), try !current.getNextSiblings().isEmpty {
      isLast = false
    }
    parent = try current.getParent()
  }
  return isLast
}

/// `$getAllListItems(node)`: the leaf list items of `node`, depth first.
public func getAllListItems(_ node: ListNode) throws -> [ListItemNode] {
  var listItemNodes: [ListItemNode] = []
  for listItemNode in try node.getChildren().compactMap({ $0 as? ListItemNode }) {
    if let firstChild = try listItemNode.getFirstChild() as? ListNode {
      listItemNodes += try getAllListItems(firstChild)
    } else {
      listItemNodes.append(listItemNode)
    }
  }
  return listItemNodes
}

/// `$isNestedListNode(node)`: whether `node` is a list item whose first
/// child is a list.
public func isNestedListNode(_ node: LexicalNode?) throws -> Bool {
  guard let item = node as? ListItemNode else { return false }
  return try isListNode(item.getFirstChild())
}

/// The nested list of a list item for which `isNestedListNode` holds.
func nestedList(_ node: LexicalNode?) throws -> ListNode? {
  guard let item = node as? ListItemNode else { return nil }
  return try item.getFirstChild() as? ListNode
}

/// `$findNearestListItemNode(node)`: the nearest list item at or above
/// `node`.
public func findNearestListItemNode(_ node: LexicalNode) throws -> ListItemNode? {
  try findMatchingParent(node) { isListItemNode($0) } as? ListItemNode
}

/// `$removeHighestEmptyListParent(sublist)`: removes the highest list or
/// list item above `sublist` that holds only it.
public func removeHighestEmptyListParent(_ sublist: ElementNode) throws {
  var emptyListPtr: ElementNode = sublist
  while try emptyListPtr.getNextSibling() == nil, try emptyListPtr.getPreviousSibling() == nil {
    guard let parent = try emptyListPtr.getParent(),
      isListItemNode(parent) || isListNode(parent)
    else { break }
    emptyListPtr = parent
  }
  try emptyListPtr.remove()
}

/// `$getNewListStart(list, listItem)`: where a list split off at
/// `listItem` starts, the number `listItem` was rendered with.
public func getNewListStart(_ list: ListNode, _ listItem: ListItemNode) throws -> Int {
  try listItem.getValue()
}
