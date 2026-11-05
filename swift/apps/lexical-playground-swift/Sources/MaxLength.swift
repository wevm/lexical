import Lexical
import LexicalExtension
import LexicalSelection
import LexicalUtils

// Port of packages/lexical-playground/src/plugins/MaxLengthPlugin/index.tsx.

struct MaxLengthConfig {
  var disabled = true
  var maxLength = 30
}

/// `MaxLengthExtension`: while enabled, trims what an edit at a collapsed
/// selection takes past `maxLength` characters, or restores the state before
/// it when that was already full.
let MaxLengthExtension = defineExtension(
  name: "@lexical/playground/MaxLength", config: MaxLengthConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if output.disabled.value { return nil }
      let maxLength = output.maxLength.value
      var lastRestoredEditorState: EditorState?
      return try editor.registerNodeTransform(RootNode.self) { rootNode in
        guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed()
        else { return }
        let prevEditorState = editor.getEditorState()
        let prevTextContentSize = try prevEditorState.read { try rootNode.getTextContentSize() }
        let textContentSize = try rootNode.getTextContentSize()
        if prevTextContentSize != textContentSize {
          let delCount = textContentSize - maxLength
          let anchor = selection.anchor
          if delCount > 0 {
            // Restore the old editor state instead if the last text content
            // was already at the limit.
            if prevTextContentSize == maxLength && lastRestoredEditorState !== prevEditorState {
              lastRestoredEditorState = prevEditorState
              try restoreEditorState(editor, prevEditorState)
            } else {
              try trimTextContentFromAnchor(editor, anchor, delCount)
            }
          }
        }
      }
    }
  })
