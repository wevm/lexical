import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts.

private func parent(
  _ name: String = "parent", editable: Bool? = nil, theme: EditorThemeClasses? = nil
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name, editorConfig: InitialEditorConfig(theme: theme, editable: editable)))
}

private func child(
  _ parentEditor: Editor, _ dependency: any AnyLexicalExtensionArgument = NestedEditorExtension,
  editable: Bool? = nil, theme: EditorThemeClasses? = nil
) throws -> Editor {
  try parentEditor.read {
    try buildEditorFromExtensions(
      defineExtension(
        name: "child", dependencies: [dependency],
        editorConfig: InitialEditorConfig(theme: theme, editable: editable)))
  }
}

private func textTheme(_ editor: Editor, _ key: String) -> String? {
  editor._config.theme["text"]?.objectValue?[key]?.stringValue
}

private let TEST_COMMAND = createCommand("TEST_COMMAND", payload: String.self)

private final class Calls {
  var child = 0
  var parent = 0
}

private func registerListeners(
  _ parentEditor: Editor, _ childEditor: Editor, _ calls: Calls
)
  -> () -> Void
{
  mergeRegister([
    childEditor.registerCommand(
      TEST_COMMAND,
      { _, editor in
        calls.child += 1
        #expect(editor === childEditor)
        #expect((try? getEditor()) === childEditor)
        return false
      }, COMMAND_PRIORITY_EDITOR),
    parentEditor.registerCommand(
      TEST_COMMAND,
      { _, editor in
        calls.parent += 1
        #expect(editor === childEditor)
        #expect((try? getEditor()) === parentEditor)
        return true
      }, COMMAND_PRIORITY_EDITOR),
  ])
}

@Suite struct NestedEditorExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > it sets _parentEditor implicitly"
    ))
  func setsParentEditorImplicitly() throws {
    let editor = try parent()
    defer { editor.dispose() }
    let childEditor = try child(editor)
    #expect(childEditor._parentEditor === editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > $getParentEditor can be overridden"
    ))
  func getParentEditorCanBeOverridden() throws {
    let editor = try parent()
    defer { editor.dispose() }
    let childEditor = try buildEditorFromExtensions(
      defineExtension(
        name: "child",
        dependencies: [configExtension(NestedEditorExtension) { $0.getParentEditor = { editor } }]))
    #expect(childEditor._parentEditor === editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > The theme is inherited by default"
    ))
  func themeIsInherited() throws {
    let editor = try parent(theme: ["text": ["bold": "bold"]])
    let childEditor = try child(editor)
    #expect(textTheme(childEditor, "bold") == "bold")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > If the child has a theme it is not inherited"
    ))
  func childThemeIsNotMerged() throws {
    let editor = try parent(theme: ["text": ["bold": "bold", "italic": "italic"]])
    defer { editor.dispose() }
    let childEditor = try child(editor, theme: ["text": ["italic": "child-italic"]])
    #expect(textTheme(childEditor, "bold") == nil)
    #expect(textTheme(childEditor, "italic") == "child-italic")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > inheritEditableFromParent defaults false but can be enabled later"
    ))
  func inheritEditableEnabledLater() throws {
    let editor = try parent()
    defer { editor.dispose() }
    let childEditor = try child(editor, editable: false)
    #expect(editor.isEditable())
    #expect(!childEditor.isEditable())
    try getExtensionDependencyFromEditor(childEditor, NestedEditorExtension).output
      .inheritEditableFromParent.value = true
    #expect(childEditor.isEditable())
    try editor.setEditable(false)
    #expect(!editor.isEditable())
    #expect(!childEditor.isEditable())
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > inheritEditableFromParent works when configured true"
    ))
  func inheritEditableConfiguredTrue() throws {
    let editor = try parent(editable: false)
    defer { editor.dispose() }
    let childEditor = try child(
      editor, configExtension(NestedEditorExtension) { $0.inheritEditableFromParent = true },
      editable: false)
    #expect(!editor.isEditable())
    #expect(!childEditor.isEditable())
    try editor.setEditable(true)
    #expect(editor.isEditable())
    #expect(childEditor.isEditable())
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > Commands delegate to parent synchronously when parent is not updating"
    ))
  func commandsDelegateSynchronously() throws {
    let parentEditor = try parent()
    defer { parentEditor.dispose() }
    let childEditor = try child(parentEditor)
    defer { childEditor.dispose() }
    let calls = Calls()
    let unregister = registerListeners(parentEditor, childEditor, calls)
    defer { unregister() }
    try childEditor.update(discrete: true) {
      let result = try childEditor.dispatchCommand(TEST_COMMAND, "test-payload")
      #expect(calls.child == 1)
      #expect(calls.parent == 1)
      #expect(result)
    }
    #expect(calls.child == 1)
    #expect(calls.parent == 1)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > Commands delegate to parent asynchronously when parent is updating"
    ))
  func commandsDelegateAfterParentUpdate() throws {
    let parentEditor = try parent()
    defer { parentEditor.dispose() }
    let childEditor = try child(parentEditor)
    defer { childEditor.dispose() }
    let calls = Calls()
    let unregister = registerListeners(parentEditor, childEditor, calls)
    defer { unregister() }
    try parentEditor.update(discrete: true) {
      let result = try childEditor.dispatchCommand(TEST_COMMAND, "test-payload")
      #expect(calls.child == 1)
      #expect(!result)
      #expect(calls.parent == 0)
    }
    #expect(calls.parent == 1)
  }
}
