import Lexical

// Port of packages/lexical-extension/src/SelectBlockExtension.ts.

private func hasCommonTopParent(
  _ nodes: [LexicalNode], _ commonTopParent: LexicalNode
) throws
  -> Bool
{
  try nodes.allSatisfy { try commonTopParent.is($0.getTopLevelElement()) }
}

/// `SelectBlockConfig`.
public struct SelectBlockConfig {
  /// `true` to disable this extension.
  public var disabled: Bool
  /// `true` to select all of this editor when all of a nested editor's
  /// content is already selected.
  public var cascadeSelection: Bool

  public init(disabled: Bool = false, cascadeSelection: Bool = false) {
    self.disabled = disabled
    self.cascadeSelection = cascadeSelection
  }
}

private func handleSelectAll(
  _ editor: Editor, _ triggerEditor: Editor, _ cascadeSelection: Bool
) throws -> Bool {
  if triggerEditor !== editor {
    // The command bubbled up from a nested editor. Without cascadeSelection
    // it is ignored so that the nested editor's own handlers take care of
    // it. With cascadeSelection it is only handled when the nested editor's
    // content is already fully selected.
    if !cascadeSelection {
      return false
    }
    // read(.pending) reflects an update in progress or queued in the nested
    // editor without flushing it.
    let isAllSelected = try triggerEditor.read(.pending) {
      guard let nestedSelection = try getSelection() as? RangeSelection else { return false }
      return try isBlockFullySelected(getRoot(), nestedSelection)
    }
    if !isAllSelected {
      return false
    }
    _ = try selectAll()
    return true
  }

  let selection = try getSelection()
  if let selection = selection as? NodeSelection {
    let selectedNodes = try selection.getNodes()
    guard let firstNode = selectedNodes.first else {
      // An empty NodeSelection, defer to the default handlers.
      return false
    }
    let topParent = try firstNode.getTopLevelElement()
    guard let topParent, !(topParent is RootNode), !topParent.is(firstNode),
      // Multiple nodes that do not share a common ancestor.
      try selectedNodes.count <= 1 || hasCommonTopParent(selectedNodes, topParent)
    else {
      _ = try selectAll()
      return true
    }
    // If firstNode is a decorator, then it is equal to topParent.
    if let topParent = topParent as? ElementNode {
      _ = try topParent.select(0, topParent.getChildrenSize())
    }
    return true
  }

  guard let selection = selection as? RangeSelection else {
    return false
  }

  let anchorNode = try selection.anchor.getNode()
  if let blockNode = try anchorNode.getTopLevelElement() as? ElementNode,
    // A selection that crosses block boundaries expands to the next
    // enclosing scope instead of shrinking to the anchor's block.
    try blockNode.is(selection.focus.getNode().getTopLevelElement()),
    // An empty block is fully selected by its caret.
    try !isBlockFullySelected(blockNode, selection)
  {
    _ = try blockNode.select(0, blockNode.getChildrenSize())
    return true
  }
  // Named slots are isolated sub-scopes between the block and the document:
  // expand to the innermost enclosing slot frame that is not yet fully
  // selected before escalating to the whole document.
  var frame = try getSlotFrame(anchorNode)
  while let current = frame {
    if let element = current as? ElementNode, try !isBlockFullySelected(element, selection) {
      _ = try element.select(0, element.getChildrenSize())
      return true
    }
    let host = try getSlotHost(current)
    frame = try host.flatMap { try getSlotFrame($0) }
  }
  if try !isBlockFullySelected(getRoot(), selection) {
    // Don't trigger selectAll if the document is already fully selected.
    _ = try selectAll()
  }
  return true
}

/// `SelectBlockExtension`: select all selects the block around the
/// selection first, then the enclosing slot frames, then the document.
public let SelectBlockExtension = defineExtension(
  name: "@lexical/extension/SelectBlock", config: SelectBlockConfig(),
  dependencies: [PreventSelectAllExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    let preventSelectAllStores = try state.getDependency(PreventSelectAllExtension).output
    return mergeRegister([
      // PreventSelectAllExtension is only a dependency in support of this
      // extension, so its disabled state is kept in sync.
      try effect {
        try preventSelectAllStores.disabled.set(stores.disabled.value)
      },
      try effect { () -> (() -> Void)? in
        if stores.disabled.value {
          return nil
        }
        // This must be in a higher priority bucket than
        // COMMAND_PRIORITY_EDITOR for cascadeSelection to work: listeners run
        // for all editors in priority bucket order (nested editor first
        // within each bucket).
        return editor.registerCommand(
          SELECT_ALL_COMMAND,
          { _, triggerEditor in
            try handleSelectAll(editor, triggerEditor, stores.cascadeSelection.peek())
          }, COMMAND_PRIORITY_LOW)
      },
    ])
  })
