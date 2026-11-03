import Lexical

// Port of packages/lexical-extension/src/PreventSelectAllExtension.ts.

/// `PreventSelectAllConfig`.
public struct PreventSelectAllConfig {
  public var disabled: Bool
  public init(disabled: Bool = false) { self.disabled = disabled }
}

/// `PreventSelectAllExtension`: keeps a select-all shortcut typed in an
/// `<input>` or `<textarea>` inside the editor from reaching Lexical.
///
/// JS stops the keydown of such a control on the root element. Native key
/// events reach the editor, never a control inside a decorator, so there is
/// nothing to stop: the extension keeps its config and `disabled` signal,
/// which `SelectBlockExtension` drives, and registers nothing.
public let PreventSelectAllExtension = defineExtension(
  name: "@lexical/extension/PreventSelectAll", config: PreventSelectAllConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { _, _, _ in {} })
