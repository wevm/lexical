import Lexical

private func defaultOnClear() throws {
  let root = try getRoot()
  let selection = try getSelection()
  let paragraph = try createParagraphNode()
  try root.clear()
  try root.append(paragraph)
  if selection != nil {
    try paragraph.select()
  }
  if let selection = selection as? RangeSelection {
    selection.format = 0
  }
}

/// `ClearEditorConfig`.
public struct ClearEditorConfig {
  /// `$onClear`: runs in the update that handles `CLEAR_EDITOR_COMMAND`.
  public var onClear: () throws -> Void

  public init(onClear: @escaping () throws -> Void = defaultOnClearEditor) {
    self.onClear = onClear
  }
}

/// `$defaultOnClear`: empties the root to one paragraph, selecting it when
/// there was a selection.
public func defaultOnClearEditor() throws { try defaultOnClear() }

/// `registerClearEditor(editor, $onClear)`.
public func registerClearEditor(
  _ editor: Editor, _ onClear: @escaping () throws -> Void = defaultOnClearEditor
) -> () -> Void {
  editor.registerCommand(
    CLEAR_EDITOR_COMMAND,
    { _, _ in
      try onClear()
      return true
    }, COMMAND_PRIORITY_EDITOR)
}

/// `ClearEditorExtension`: handles `CLEAR_EDITOR_COMMAND`.
public let ClearEditorExtension = defineExtension(
  name: "@lexical/extension/ClearEditor", config: ClearEditorConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let onClear = state.getOutput().onClear
    return try effect { () -> (() -> Void)? in registerClearEditor(editor, onClear.value) }
  })
