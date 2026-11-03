import Lexical

// Port of packages/lexical-extension/src/TabIndentationExtension.ts.

// `$getNearestBlockElementAncestorOrThrow` and `$handleIndentAndOutdent`
// come from `@lexical/utils`, which this module does not depend on yet;
// these are copies of `LexicalUtils`' ports.

private func getNearestBlockElementAncestorOrThrow(_ startNode: LexicalNode) throws -> ElementNode {
  var node: LexicalNode? = startNode
  while let current = node {
    if let element = current as? ElementNode, try !element.isInline() {
      return element
    }
    node = try current.getParent()
  }
  throw LexicalError("Expected node \(startNode.__key) to have closest block element node.")
}

private func handleIndentAndOutdent(
  _ indentOrOutdent: (ElementNode) throws -> Void
) throws
  -> Bool
{
  guard let selection = try getSelection() as? RangeSelection else { return false }
  var alreadyHandled = Set<NodeKey>()
  for node in try selection.getNodes() {
    if alreadyHandled.contains(node.getKey()) { continue }
    var parentBlock: LexicalNode? = node
    while let current = parentBlock {
      if let element = current as? ElementNode, try !element.isInline() { break }
      parentBlock = try current.getParent()
    }
    guard let block = parentBlock as? ElementNode else { continue }
    if block.canIndent(), alreadyHandled.insert(block.getKey()).inserted {
      try indentOrOutdent(block)
    }
  }
  return !alreadyHandled.isEmpty
}

private func indentOverTab(_ selection: RangeSelection) throws -> Bool {
  let nodes = try selection.getNodes()
  let canIndentBlockNodes = try nodes.filter { node in
    guard try isBlockElementNode(node), let element = node as? ElementNode else { return false }
    return element.canIndent()
  }
  // 1. If selection spans across canIndent block nodes: indent
  if !canIndentBlockNodes.isEmpty {
    return true
  }
  // 2. If first (anchor/focus) is at block start: indent
  let anchor = selection.anchor
  let focus = selection.focus
  let first = try focus.isBefore(anchor) ? focus : anchor
  let firstNode = try first.getNode()
  let firstBlock = try getNearestBlockElementAncestorOrThrow(firstNode)
  if firstBlock.canIndent() {
    let firstBlockKey = firstBlock.getKey()
    var selectionAtStart = try createRangeSelection()
    try selectionAtStart.anchor.set(firstBlockKey, 0, .element)
    try selectionAtStart.focus.set(firstBlockKey, 0, .element)
    selectionAtStart = try normalizeSelection(selectionAtStart)
    if selectionAtStart.anchor.is(first) {
      return true
    }
  }
  // 3. Else: tab
  return false
}

/// `CanIndentPredicate`.
public typealias CanIndentPredicate = (ElementNode) throws -> Bool

/// `$defaultCanIndent`.
public func defaultCanIndent(_ node: ElementNode) throws -> Bool { node.canIndent() }

private func registerTabIndentation(
  _ editor: Editor, _ getMaxIndent: @escaping () -> Int?,
  _ getCanIndent: @escaping () -> CanIndentPredicate
) -> () -> Void {
  mergeRegister([
    editor.registerCommand(
      KEY_TAB_COMMAND,
      { event, _ in
        guard let selection = try getSelection() as? RangeSelection else {
          return false
        }
        event.preventDefault()
        let command =
          try indentOverTab(selection)
          ? (event.shiftKey ? OUTDENT_CONTENT_COMMAND : INDENT_CONTENT_COMMAND)
          : INSERT_TAB_COMMAND
        return try editor.dispatchCommand(command)
      }, COMMAND_PRIORITY_EDITOR),
    editor.registerCommand(
      INDENT_CONTENT_COMMAND,
      { _, _ in
        let currentMaxIndent = getMaxIndent()
        guard try getSelection() is RangeSelection else {
          return false
        }
        let currentCanIndent = getCanIndent()
        return try handleIndentAndOutdent { block in
          if try currentCanIndent(block) {
            let newIndent = try block.getIndent() + 1
            // JS tests `!currentMaxIndent`, so a maximum of 0 is no maximum.
            if currentMaxIndent == nil || currentMaxIndent == 0 || newIndent < currentMaxIndent! {
              try block.setIndent(newIndent)
            }
          }
        }
      }, COMMAND_PRIORITY_CRITICAL),
  ])
}

/// `registerTabIndentation(editor, maxIndent, $canIndent)`: Tab indents
/// (Shift+Tab outdents) when the selection is at the start of an
/// indentable block or spans several, and inserts a tab otherwise;
/// indentation stops below `maxIndent`.
public func registerTabIndentation(
  _ editor: Editor, _ maxIndent: Int? = nil,
  _ canIndent: @escaping CanIndentPredicate = defaultCanIndent
) -> () -> Void {
  registerTabIndentation(editor, { maxIndent }, { canIndent })
}

/// `registerTabIndentation` with signals for `maxIndent` and `$canIndent`,
/// read when a command is handled.
public func registerTabIndentation(
  _ editor: Editor, _ maxIndent: ReadonlySignal<Int?>,
  _ canIndent: ReadonlySignal<CanIndentPredicate>
) -> () -> Void {
  registerTabIndentation(editor, { maxIndent.peek() }, { canIndent.peek() })
}

/// `TabIndentationConfig`.
public struct TabIndentationConfig {
  public var disabled: Bool
  public var maxIndent: Int?
  /// `$canIndent`: by default every element whose `canIndent()` is true
  /// can be indented; this sets it per node without overriding the method.
  public var canIndent: CanIndentPredicate

  public init(
    disabled: Bool = false, maxIndent: Int? = nil,
    canIndent: @escaping CanIndentPredicate = defaultCanIndent
  ) {
    self.disabled = disabled
    self.maxIndent = maxIndent
    self.canIndent = canIndent
  }
}

/// `TabIndentationExtension`: `registerTabIndentation` with its config as
/// signals.
public let TabIndentationExtension = defineExtension(
  name: "@lexical/extension/TabIndentation", config: TabIndentationConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    let disabled = output.disabled
    let maxIndent = output.maxIndent
    let canIndent = output.canIndent
    return try effect { () -> (() -> Void)? in
      if disabled.value {
        return nil
      }
      return registerTabIndentation(editor, maxIndent, canIndent)
    }
  })
