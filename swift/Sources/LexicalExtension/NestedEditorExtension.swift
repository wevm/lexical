import Lexical

/// `NestedEditorConfig`.
public struct NestedEditorConfig {
  /// `$getParentEditor`: by default the active editor, which must have been
  /// built from extensions.
  public var getParentEditor: () throws -> Editor
  public var inheritEditableFromParent: Bool

  public init(
    getParentEditor: @escaping () throws -> Editor = {
      let editor = try getEditor()
      _ = try LexicalBuilder.fromEditor(editor)
      return editor
    }, inheritEditableFromParent: Bool = false
  ) {
    self.getParentEditor = getParentEditor
    self.inheritEditableFromParent = inheritEditableFromParent
  }
}

/// The output of `NestedEditorExtension`.
public struct NestedEditorOutput {
  public let inheritEditableFromParent: Signal<Bool>
}

/// `NestedEditorExtension`: makes the editor a child of its parent (by
/// default the active editor when it is built), sharing its theme and,
/// optionally, its editable state.
public let NestedEditorExtension = defineExtension(
  name: "@lexical/extension/NestedEditor", config: NestedEditorConfig(),
  init: { editorConfig, config, _ in
    let parentEditor = try config.getParentEditor()
    editorConfig.parentEditor = parentEditor
    editorConfig.theme = editorConfig.theme ?? parentEditor._config.theme
  },
  build: { _, config, _ in
    NestedEditorOutput(inheritEditableFromParent: signal(config.inheritEditableFromParent))
  },
  register: { editor, _, state in
    try effect { () -> (() -> Void)? in
      guard let parentEditor = editor._parentEditor,
        state.getOutput().inheritEditableFromParent.value
      else { return nil }
      try editor.setEditable(parentEditor.isEditable())
      return parentEditor.registerEditableListener { [unowned editor] editable in
        try editor.setEditable(editable)
      }
    }
  })
