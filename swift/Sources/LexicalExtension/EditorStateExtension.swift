import Lexical

// Port of packages/lexical-extension/src/EditorStateExtension.ts.

/// `EditorStateExtension`: its output is a signal of the editor's current
/// `EditorState`, kept up to date while something watches it.
public let EditorStateExtension = defineExtension(
  name: "@lexical/extension/EditorState",
  build: { editor, _, _ -> Signal<EditorState> in
    watchedSignal(
      { editor.getEditorState() },
      { editorStateSignal in
        editor.registerUpdateListener { (payload: UpdateListenerPayload) in
          editorStateSignal.value = payload.editorState
        }
      })
  })
