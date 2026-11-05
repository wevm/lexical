import Lexical

// Port of HorizontalRuleExtension from
// packages/lexical-extension/src/HorizontalRuleExtension.ts (the node is in
// HorizontalRuleNode.swift).

/// `INSERT_HORIZONTAL_RULE_COMMAND`.
public let INSERT_HORIZONTAL_RULE_COMMAND = LexicalCommand<Void>(
  "INSERT_HORIZONTAL_RULE_COMMAND")

// `$insertNodeToNearestRoot` comes from `@lexical/utils`, which this module
// does not depend on yet; this is a copy of `LexicalUtils`' port.

private func hasContentAfter(_ caret: PointCaret) throws -> Bool {
  if try isExtendableTextPointCaret(caret) { return true }
  var c: NodeCaret? = caret.isText ? caret.getSiblingCaret() : caret
  while let current = c {
    if try current.getNodeAtCaret() != nil { return true }
    let parent = try current.isChild ? current.origin : current.origin.getParent()
    if try isBlockContainer(parent) { return false }
    c = try current.getParentCaret(.shadowRoot)
  }
  return false
}

private func insertNodeToNearestRoot<T: LexicalNode>(_ node: T) throws {
  let selection = try getSelection() ?? getPreviousSelection()
  var initialCaret: PointCaret?
  if let range = selection as? RangeSelection {
    initialCaret = try caretFromPoint(range.focus, .next)
  } else {
    if let selection, let lastNode = try selection.getNodes().last {
      initialCaret = getSiblingCaret(lastNode, .next) as SiblingCaret
    }
    if initialCaret == nil {
      initialCaret = try getChildCaret(getRoot(), .previous).getFlipped().insert(
        createParagraphNode())
    }
  }
  let caret = initialCaret!
  let insertCaret = try insertNodeToNearestRootAtCaret(
    node, caret,
    hasContentAfter(caret)
      ? SplitAtPointCaretNextOptions(shouldSplit: { _, edge in edge != .last }) : nil)
  let adjacent = try getAdjacentChildCaret(insertCaret)
  let selectionCaret = try adjacent.map { $0.isChild ? try normalizeCaret($0) : insertCaret }
  _ = try setSelectionFromCaretRange(getCollapsedCaretRange(selectionCaret ?? insertCaret))
}

/// `$toggleNodeSelection`.
private func toggleNodeSelection(_ node: LexicalNode, _ shiftKey: Bool = false) throws {
  let selection = try getSelection()
  let wasSelected = try node.isSelected()
  let key = node.getKey()
  let nodeSelection: NodeSelection
  if shiftKey, let selection = selection as? NodeSelection {
    nodeSelection = selection
  } else {
    nodeSelection = createNodeSelection()
    try setSelection(nodeSelection)
  }
  if wasSelected {
    nodeSelection.delete(key)
  } else {
    nodeSelection.add(key)
  }
}

/// `HorizontalRuleExtension`: registers `HorizontalRuleNode`, inserts one
/// with `INSERT_HORIZONTAL_RULE_COMMAND`, and selects one that is clicked
/// (Shift adds it to a node selection).
///
/// JS also adds the theme's `hrSelected` class (default `selected`) to the
/// `<hr>` of a selected rule, through a mutation listener and the
/// `NodeSelectionExtension` signals. The views draw the rule and its
/// selected state from their theme's `horizontalRule`, so that part is not
/// ported; the click arrives as `CLICK_COMMAND` with the rule's key as its
/// `targetKey`.
public let HorizontalRuleExtension = defineExtension(
  name: "@lexical/extension/HorizontalRule",
  dependencies: [EditorStateExtension, NodeSelectionExtension],
  nodes: { [.klass(HorizontalRuleNode.self)] },
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        INSERT_HORIZONTAL_RULE_COMMAND,
        { _, _ in
          guard let selection = try getSelection() as? RangeSelection else {
            return false
          }
          _ = try selection.focus.getNode()
          try insertNodeToNearestRoot(createHorizontalRuleNode())
          return true
        }, COMMAND_PRIORITY_EDITOR),
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          if let targetKey = event.targetKey,
            let node = try getNodeByKey(targetKey) as? HorizontalRuleNode
          {
            try toggleNodeSelection(node, event.shiftKey)
            return true
          }
          return false
        }, COMMAND_PRIORITY_LOW),
    ])
  })
