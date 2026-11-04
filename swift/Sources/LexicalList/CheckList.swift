import Lexical
import LexicalExtension

// Port of packages/lexical-list/src/checkList.ts and the CheckListExtension
// of LexicalListExtension.ts. The rest of checkList.ts moves DOM focus onto
// a check list `<li>` so that Space, the arrows and Escape operate its
// checkbox, and hit-tests pointer events against the `::before` marker; on
// the native views that is the text view's work (see DESIGN.md).

public let INSERT_CHECK_LIST_COMMAND = createCommand(
  "INSERT_CHECK_LIST_COMMAND", payload: Void.self)

/// `registerCheckList(editor)`: `INSERT_CHECK_LIST_COMMAND` inserts a check
/// list.
public func registerCheckList(_ editor: Editor) throws -> () -> Void {
  try editor.registerCommand(
    INSERT_CHECK_LIST_COMMAND,
    { _, _ in
      try insertList("check")
      return true
    }, COMMAND_PRIORITY_LOW)
}

/// `CheckListConfig`.
public struct CheckListConfig {
  /// Toggling an item by its marker does not focus the editor.
  public var disableTakeFocusOnClick: Bool

  public init(disableTakeFocusOnClick: Bool = false) {
    self.disableTakeFocusOnClick = disableTakeFocusOnClick
  }
}

/// `CheckListExtension`: `ListExtension` and `INSERT_CHECK_LIST_COMMAND`.
public let CheckListExtension = defineExtension(
  name: "@lexical/list/CheckList", config: safeCast(CheckListConfig()),
  dependencies: [ListExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, _ in try registerCheckList(editor) })
