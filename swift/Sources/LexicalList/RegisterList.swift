import Lexical
import LexicalUtils

// Port of packages/lexical-list/src/registerList.ts.

/// `UPDATE_LIST_START_COMMAND`'s payload.
public struct UpdateListStartPayload {
  public var listNodeKey: NodeKey
  public var newStart: Int
  public init(listNodeKey: NodeKey, newStart: Int) {
    self.listNodeKey = listNodeKey
    self.newStart = newStart
  }
}

public let UPDATE_LIST_START_COMMAND = createCommand(
  "UPDATE_LIST_START_COMMAND", payload: UpdateListStartPayload.self)
public let INSERT_UNORDERED_LIST_COMMAND = createCommand(
  "INSERT_UNORDERED_LIST_COMMAND", payload: Void.self)
public let INSERT_ORDERED_LIST_COMMAND = createCommand(
  "INSERT_ORDERED_LIST_COMMAND", payload: Void.self)
public let REMOVE_LIST_COMMAND = createCommand("REMOVE_LIST_COMMAND", payload: Void.self)

/// `RegisterListOptions`.
public struct RegisterListOptions {
  public var restoreNumbering: Bool
  public init(restoreNumbering: Bool = false) { self.restoreNumbering = restoreNumbering }
}

/// `registerList(editor, options)`: the list commands, Enter and Backspace
/// in list items, and the transforms that keep an item's text format and
/// style in step with its first text.
public func registerList(
  _ editor: Editor, _ options: RegisterListOptions? = nil
) throws
  -> () -> Void
{
  try mergeRegister(
    editor.registerCommand(
      INSERT_ORDERED_LIST_COMMAND,
      { _, _ in
        try insertList("number")
        return true
      }, COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      UPDATE_LIST_START_COMMAND,
      { payload, _ in
        guard let listNode = try getNodeByKey(payload.listNodeKey) as? ListNode else {
          return false
        }
        if try listNode.getListType() == "number" {
          try listNode.setStart(payload.newStart)
          try updateChildrenListItemValue(listNode)
        }
        return true
      }, COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      INSERT_UNORDERED_LIST_COMMAND,
      { _, _ in
        try insertList("bullet")
        return true
      }, COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      REMOVE_LIST_COMMAND,
      { _, _ in
        try removeList()
        return true
      }, COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      INSERT_PARAGRAPH_COMMAND,
      { _, _ in try handleListInsertParagraph(options?.restoreNumbering ?? false) },
      COMMAND_PRIORITY_LOW),
    editor.registerCommand(
      KEY_BACKSPACE_COMMAND,
      { event, _ in
        guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
          selection.anchor.offset == 0
        else { return false }
        var current: LexicalNode = try selection.anchor.getNode()
        while !isListItemNode(current) {
          if try current.getPreviousSibling() != nil {
            return false
          }
          guard let parent = try current.getParent() else { return false }
          current = parent
        }
        if let item = current as? ListItemNode, try item.collapseAtStart(selection) {
          event.preventDefault()
          return true
        }
        return false
      }, COMMAND_PRIORITY_BEFORE_EDITOR),
    try editor.registerNodeTransform(ListItemNode.self) { node in
      if let firstChild = try node.getFirstChild() {
        if let text = firstChild as? TextNode {
          let style = try text.getStyle()
          let format = try text.getFormat()
          if try node.getTextStyle() != style {
            try node.setTextStyle(style)
          }
          if try node.getTextFormat() != format {
            try node.setTextFormat(format)
          }
        }
      } else if let selection = try getSelection() as? RangeSelection,
        try selection.style != node.getTextStyle() || selection.format != node.getTextFormat(),
        selection.isCollapsed(), try node.is(selection.anchor.getNode())
      {
        // An empty item takes the selection's format and style.
        try node.setTextStyle(selection.style).setTextFormat(selection.format)
      }
    },
    try editor.registerNodeTransform(TextNode.self) { node in
      if let listItemParentNode = try node.getParent() as? ListItemNode,
        try node.is(listItemParentNode.getFirstChild())
      {
        let style = try node.getStyle()
        let format = try node.getFormat()
        if try style != listItemParentNode.getTextStyle()
          || format != listItemParentNode.getTextFormat()
        {
          try listItemParentNode.setTextStyle(style).setTextFormat(format)
        }
      }
    })
}

/// `registerListStrictIndentTransform(editor)`: an item can be indented at
/// most one level deeper than the item before it.
public func registerListStrictIndentTransform(_ editor: Editor) throws -> () -> Void {
  func formatListIndentStrict(_ listItemNode: ListItemNode) throws {
    guard let listNode = try listItemNode.getParent() as? ListNode,
      try !isListNode(listItemNode.getFirstChild())
    else { return }
    let startingListItemNode = try findMatchingParent(listItemNode) { node in
      try isListItemNode(node) && isListNode(node.getParent())
        && isListItemNode(node.getPreviousSibling())
    }
    if startingListItemNode == nil, try listItemNode.getIndent() > 0 {
      try listItemNode.setIndent(0)
    } else if let starting = startingListItemNode as? ListItemNode,
      let prevListItemNode = try starting.getPreviousSibling() as? ListItemNode
    {
      let endListItemNode = try findChildrenEndListItemNode(prevListItemNode)
      if let endListNode = try endListItemNode.getParent() as? ListNode {
        let prevDepth = try getListDepth(endListNode)
        let depth = try getListDepth(listNode)
        if prevDepth + 1 < depth {
          try listItemNode.setIndent(prevDepth)
        }
      }
    }
  }
  return try editor.registerNodeTransform(ListNode.self) { listNode in
    var queue: [ListNode] = [listNode]
    while !queue.isEmpty {
      let node = queue.removeFirst()
      for child in try node.getChildren() {
        guard let item = child as? ListItemNode else { continue }
        try formatListIndentStrict(item)
        if let firstChild = try item.getFirstChild() as? ListNode {
          queue.append(firstChild)
        }
      }
    }
  }
}

private func findChildrenEndListItemNode(_ listItemNode: ListItemNode) throws -> ListItemNode {
  var current = listItemNode
  var firstChild = try current.getFirstChild()
  while let list = firstChild as? ListNode {
    guard let lastChild = try list.getLastChild() as? ListItemNode else { break }
    current = lastChild
    firstChild = try current.getFirstChild()
  }
  return current
}
