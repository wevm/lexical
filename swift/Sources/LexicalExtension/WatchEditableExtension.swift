import Lexical

// Port of packages/lexical-extension/src/WatchEditableExtension.ts.

/// `WatchEditableExtension`: its output is a signal of the editor's
/// editable state, mirroring `editor.isEditable()` through an editable
/// listener while something watches it. Depend on it and read the signal
/// from an `effect` or `computed` to react to editability changes.
public let WatchEditableExtension = defineExtension(
  name: "@lexical/extension/WatchEditable",
  build: { editor, _, _ -> Signal<Bool> in
    watchedSignal(
      { editor.isEditable() },
      { signal in
        editor.registerEditableListener { (editable: Bool) in
          signal.value = editable
        }
      })
  })
