import Lexical

/// `NormalizeInlineElementsConfig`.
public struct NormalizeInlineElementsConfig {
  public var disabled: Bool
  public init(disabled: Bool = false) { self.disabled = disabled }
}

private func deleteEmptyInline(_ node: LexicalNode) throws {
  guard let element = node as? ElementNode, try element.isInline(), try element.isEmpty() else {
    return
  }
  try element.remove()
  if try element.canBeEmpty() {
    consoleWarn(
      "Empty inline elements are removed from the EditorState, so returning 'true' from \(type(of: element)).canBeEmpty() is not allowed"
    )
  }
}

/// `NormalizeInlineElementsExtension`: removes inline elements that become
/// empty.
///
/// JS adds the transform only to element classes that override `isInline`;
/// Swift can't detect an override, so every element class gets it, and the
/// transform does nothing for a node that isn't inline.
public let NormalizeInlineElementsExtension = defineExtension(
  name: "@lexical/NormalizeInlineElements", config: NormalizeInlineElementsConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      guard !stores.disabled.value else { return nil }
      let transform = Transform(deleteEmptyInline)
      let registered = editor._nodes.valuesArray.filter { $0.klass is ElementNode.Type }
      for node in registered { node.addTransform(transform) }
      return { for node in registered { node.deleteTransform(transform) } }
    }
  })
