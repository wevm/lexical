import Lexical
import LexicalUtils

// Port of packages/lexical-extension/src/SelectionAlwaysOnDisplayExtension.ts.

/// `SelectionAlwaysOnDisplayConfig`. JS's `onReposition`, which receives
/// the elements that mark the selection, has no counterpart: the views
/// draw the mark themselves.
public struct SelectionAlwaysOnDisplayConfig {
  public var disabled: Bool

  public init(disabled: Bool = false) { self.disabled = disabled }
}

/// `SelectionAlwaysOnDisplayExtension`: shows the selection even while the
/// editor does not have the focus (`selectionAlwaysOnDisplay`).
public let SelectionAlwaysOnDisplayExtension = defineExtension(
  name: "@lexical/utils/SelectionAlwaysOnDisplay", config: SelectionAlwaysOnDisplayConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      stores.disabled.value ? nil : selectionAlwaysOnDisplay(editor)
    }
  })
