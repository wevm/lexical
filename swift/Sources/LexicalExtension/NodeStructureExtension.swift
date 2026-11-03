import Lexical

// Port of packages/lexical-extension/src/NodeStructureExtension.ts. The
// normalization itself is in core (LexicalStructure.swift).

/// `NodeStructureConfig` (`@experimental node-structure`).
public struct NodeStructureConfig {
  /// `true` stops enforcing the declarations (they are then only read).
  /// Turning it back to `false` normalizes the whole document.
  public var disabled: Bool
  public init(disabled: Bool = false) { self.disabled = disabled }
}

/// `registerNodeStructure(editor)` (`@experimental node-structure`):
/// enforces on `editor` the structure node classes declare in `config()`,
/// as `NodeStructureExtension` does, and normalizes the current document.
/// Registering more than once is harmless; the normalization stops when
/// every returned function has been called.
public func registerNodeStructure(_ editor: Editor) throws -> () -> Void {
  try INTERNAL_registerNodeStructure(editor)
}

/// `NodeStructureExtension` (`@experimental node-structure`): enforces the
/// structure node classes declare in `config()` (`children`,
/// `mergeAdjacent`) in the same update as each change. A class that
/// declares `children: .block` is also a block container for the
/// structural commands (see `isBlockContainer`). Opt-in: without it, the
/// declarations change nothing at runtime.
public let NodeStructureExtension = defineExtension(
  name: "@lexical/NodeStructure", config: NodeStructureConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if stores.disabled.value {
        return nil
      }
      return try registerNodeStructure(editor)
    }
  })
