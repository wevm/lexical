import Lexical
import LexicalExtension

// Port of packages/lexical-list/src/LexicalListExtension.ts.

/// `ListConfig`.
public struct ListConfig {
  /// Enforces strict indentation (`registerListStrictIndentTransform`).
  public var hasStrictIndent: Bool
  /// A list split by Enter in an empty item continues the numbering.
  public var shouldPreserveNumbering: Bool

  public init(hasStrictIndent: Bool = false, shouldPreserveNumbering: Bool = false) {
    self.hasStrictIndent = hasStrictIndent
    self.shouldPreserveNumbering = shouldPreserveNumbering
  }
}

/// `ListExtension`: registers `ListNode` and `ListItemNode`, the list
/// commands, and the strict indent transform when `hasStrictIndent` is set.
public let ListExtension = defineExtension(
  name: "@lexical/list/List", config: safeCast(ListConfig()),
  nodes: { [.klass(ListNode.self), .klass(ListItemNode.self)] },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return mergeRegister(
      try effect { () -> (() -> Void)? in
        try registerList(
          editor, RegisterListOptions(restoreNumbering: stores.shouldPreserveNumbering.value))
      },
      try effect { () -> (() -> Void)? in
        stores.hasStrictIndent.value ? try registerListStrictIndentTransform(editor) : nil
      })
  })
