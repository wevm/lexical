import Lexical

/// `getExtensionDependencyFromEditor(editor, extension)`.
public func getExtensionDependencyFromEditor<C, O, I>(
  _ editor: Editor, _ extension_: LexicalExtension<C, O, I>
) throws -> LexicalExtensionDependency<C, O, I> {
  let builder = try LexicalBuilder.fromEditor(editor)
  guard let rep = try builder.getExtensionRep(extension_) else {
    throw LexicalError(
      "getExtensionDependencyFromEditor: Extension \(extension_.name) was not built when creating this editor"
    )
  }
  return try castExtensionDependency(try rep.getExtensionDependency(), extension_.name)
}

func castExtensionDependency<C, O, I>(
  _ dep: AnyExtensionDependency, _ name: String
) throws -> LexicalExtensionDependency<C, O, I> {
  guard let config = dep.config as? C, let initResult = dep.initResult as? I,
    let output = dep.output as? O
  else {
    throw LexicalError("Extension \(name) does not have the requested config, init or output type")
  }
  return LexicalExtensionDependency(config: config, initResult: initResult, output: output)
}

/// `getPeerDependencyFromEditor(editor, name)`, type-erased.
public func getPeerDependencyFromEditor(
  _ editor: Editor, _ extensionName: String
) throws -> AnyExtensionDependency? {
  guard let builder = LexicalBuilder.maybeFromEditor(editor),
    let peer = builder.extensionNameMap.get(extensionName)
  else { return nil }
  return try peer.getExtensionDependency()
}

/// `getPeerDependencyFromEditor<typeof Ext>(editor, name)`.
public func getPeerDependencyFromEditor<C, O, I>(
  _ editor: Editor, _ extensionName: String, as _: LexicalExtension<C, O, I>.Type
) throws -> LexicalExtensionDependency<C, O, I>? {
  guard let dep = try getPeerDependencyFromEditor(editor, extensionName) else { return nil }
  return try castExtensionDependency(dep, extensionName)
}

/// `getPeerDependencyFromEditorOrThrow(editor, name)`.
public func getPeerDependencyFromEditorOrThrow<C, O, I>(
  _ editor: Editor, _ extensionName: String, as type: LexicalExtension<C, O, I>.Type
) throws -> LexicalExtensionDependency<C, O, I> {
  guard let dep = try getPeerDependencyFromEditor(editor, extensionName, as: type) else {
    throw LexicalError(
      "getPeerDependencyFromEditorOrThrow: Editor was not built with Extension \(extensionName)")
  }
  return dep
}

/// `$getExtensionDependency(extension)`.
public func getExtensionDependency<C, O, I>(
  _ extension_: LexicalExtension<C, O, I>
) throws -> LexicalExtensionDependency<C, O, I> {
  try getExtensionDependencyFromEditor(try getEditor(), extension_)
}

/// `$getExtensionOutput(extension)`.
public func getExtensionOutput<C, O, I>(_ extension_: LexicalExtension<C, O, I>) throws -> O {
  try getExtensionDependency(extension_).output
}

/// `$getPeerDependency(name)`.
public func getPeerDependency<C, O, I>(
  _ extensionName: String, as type: LexicalExtension<C, O, I>.Type
) throws -> LexicalExtensionDependency<C, O, I>? {
  try getPeerDependencyFromEditor(try getEditor(), extensionName, as: type)
}
