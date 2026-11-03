import Lexical

// Port of packages/lexical-extension/src/AutoFocusExtension.ts.

/// `AutoFocusConfig`.
public struct AutoFocusConfig {
  /// Where to move the selection when the editor is focused and there is no
  /// existing selection: `"rootStart"` or `"rootEnd"` (the default).
  public var defaultSelection: String
  /// The initial state of disabled.
  public var disabled: Bool

  public init(defaultSelection: String = "rootEnd", disabled: Bool = false) {
    self.defaultSelection = defaultSelection
    self.disabled = disabled
  }
}

/// `AutoFocusExtension`: focuses the editor when a root element is set.
///
/// JS asks the DOM whether the active element is inside the root; Swift
/// asks a root that conforms to `LexicalRootElement` (`hasFocus`) and
/// focuses it (`focus()`). Any other root is left as it is.
public let AutoFocusExtension = defineExtension(
  name: "@lexical/extension/AutoFocus", config: AutoFocusConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if stores.disabled.value {
        return nil
      }
      return editor.registerRootListener { rootElement, _ in
        do {
          try editor.focus(
            {
              // If the selection moves to the same point, the view is not
              // focused again, so it is focused here.
              if let root = rootElement as? LexicalRootElement, !root.hasFocus {
                root.focus()
              }
            }, EditorFocusOptions(defaultSelection: stores.defaultSelection.peek()))
        } catch {
          reportUncaughtError(error)
        }
        return nil
      }
    }
  })
