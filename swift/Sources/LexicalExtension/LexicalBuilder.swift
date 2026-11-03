import Lexical

/// `buildEditorFromExtensions(...extensions)`: an editor with every
/// extension (and its dependencies) configured, built and registered.
/// `dispose()` undoes the registration.
public func buildEditorFromExtensions(
  _ extensions: any AnyLexicalExtensionArgument...
) throws -> Editor {
  try LexicalBuilder.fromExtensions(extensions).buildEditor()
}

/// `buildEditorFromExtensions` taking an array.
public func buildEditorFromExtensions(
  _ extensions: [any AnyLexicalExtensionArgument]
) throws -> Editor {
  try LexicalBuilder.fromExtensions(extensions).buildEditor()
}

private let UNSAFE_KEYS: Set<String> = ["__proto__", "constructor", "prototype"]

/// `deepThemeMergeInPlace(a, b)`: `b` merged into `a`, recursing into
/// objects; any other `b` replaces `a`.
public func deepThemeMerge(_ a: JSONValue?, _ b: JSONValue) -> JSONValue {
  guard case .object(let bObj) = b, case .object(var aObj)? = a else { return b }
  for (k, v) in bObj where !UNSAFE_KEYS.contains(k) {
    aObj[k] = deepThemeMerge(aObj[k], v)
  }
  return .object(aObj)
}

/// `deepThemeMergeInPlace` on a theme.
public func deepThemeMergeInPlace(_ a: inout EditorThemeClasses, _ b: EditorThemeClasses) {
  for (k, v) in b where !UNSAFE_KEYS.contains(k) {
    a[k] = deepThemeMerge(a[k], v)
  }
}

/// `LexicalBuilder`: resolves the extension graph, merges configs in
/// dependency order, and builds and registers each extension.
public final class LexicalBuilder {
  public let roots: [AnyNormalizedLexicalExtensionArgument]
  public let extensionNameMap = JSMap<String, ExtensionRep>()
  /// Per extension, the configs it gives each dependency (a
  /// `Partial<Config>` each).
  let outgoingConfigEdges = JSMap<String, JSMap<String, [Any]>>()
  let incomingEdges = JSMap<String, JSSet<String>>()
  let conflicts = JSMap<String, String>()
  var _sortedExtensionReps: [ExtensionRep]?

  init(_ roots: [AnyNormalizedLexicalExtensionArgument]) throws {
    self.roots = roots
    for extension_ in roots {
      try addExtension(extension_)
    }
  }

  public static func fromExtensions(
    _ extensions: [any AnyLexicalExtensionArgument]
  ) throws -> LexicalBuilder {
    var roots = [InitialStateExtension.normalized]
    for extension_ in extensions {
      roots.append(extension_.normalized)
    }
    return try LexicalBuilder(roots)
  }

  public static func maybeFromEditor(_ editor: Editor) -> LexicalBuilder? {
    editor._builder as? LexicalBuilder
  }

  public static func fromEditor(_ editor: Editor) throws -> LexicalBuilder {
    guard let builder = maybeFromEditor(editor) else {
      throw LexicalError(
        "LexicalBuilder.fromEditor: The given editor was not created with LexicalBuilder")
    }
    return builder
  }

  public func constructEditor() throws -> Editor {
    let (args, onError, onWarn) = try buildCreateEditorArgs()
    var createArgs = args
    weak var builtEditor: Editor?
    if let onError {
      createArgs.onError = { error in
        guard let editor = builtEditor else { throw error }
        try onError(error, editor)
      }
    }
    if let onWarn {
      createArgs.onWarn = { error in
        guard let editor = builtEditor else { throw error }
        try onWarn(error, editor)
      }
    }
    let editor = try createEditor(createArgs)
    builtEditor = editor
    editor._builder = self
    for extensionRep in try sortedExtensionReps() {
      try extensionRep.build(editor)
    }
    return editor
  }

  public func buildEditor() throws -> Editor {
    let editor = try constructEditor()
    let unregister = try registerEditor(editor)
    editor._dispose = { [unowned editor] in
      unregister()
      try? editor.setRootElement(nil)
    }
    return editor
  }

  public func hasExtensionByName(_ name: String) -> Bool {
    extensionNameMap.has(name)
  }

  public func getExtensionRep(_ extension_: any AnyLexicalExtension) throws -> ExtensionRep? {
    guard let rep = extensionNameMap.get(extension_.name) else { return nil }
    try invariant(
      rep.extension_ === extension_,
      "LexicalBuilder: A registered extension with name \(extension_.name) exists but does not match the given extension"
    )
    return rep
  }

  func addEdge(_ fromExtensionName: String, _ toExtensionName: String, _ configs: [Any]) {
    if let outgoing = outgoingConfigEdges.get(fromExtensionName) {
      if let existing = outgoing.get(toExtensionName) {
        outgoing.set(toExtensionName, existing + configs)
      } else {
        outgoing.set(toExtensionName, configs)
      }
    } else {
      outgoingConfigEdges.set(fromExtensionName, JSMap([(toExtensionName, configs)]))
    }
    if let incoming = incomingEdges.get(toExtensionName) {
      incoming.add(fromExtensionName)
    } else {
      incomingEdges.set(toExtensionName, JSSet([fromExtensionName]))
    }
  }

  public func addExtension(_ arg: any AnyLexicalExtensionArgument) throws {
    try invariant(
      _sortedExtensionReps == nil, "LexicalBuilder: addExtension called after finalization")
    let normalized = arg.normalized
    let extension_ = normalized.extension_
    let existing = extensionNameMap.get(extension_.name)
    try invariant(
      existing == nil || existing!.extension_ === extension_,
      "LexicalBuilder: Multiple extensions registered with name \(extension_.name), names must be unique"
    )
    guard existing == nil else { return }
    let extensionRep = ExtensionRep(self, extension_)
    extensionNameMap.set(extension_.name, extensionRep)
    if let hasConflict = conflicts.get(extension_.name) {
      throw LexicalError(
        "LexicalBuilder: extension \(extension_.name) conflicts with \(hasConflict)")
    }
    for name in extension_.conflictsWith {
      try invariant(
        !extensionNameMap.has(name),
        "LexicalBuilder: extension \(extension_.name) conflicts with \(name)")
      conflicts.set(name, extension_.name)
    }
    for dep in extension_.dependencies {
      let normDep = dep.normalized
      addEdge(extension_.name, normDep.extension_.name, normDep.configs)
      try addExtension(normDep)
    }
    for peer in extension_.peerDependencies {
      addEdge(extension_.name, peer.name, peer.config.map { [$0] } ?? [])
    }
  }

  public func sortedExtensionReps() throws -> [ExtensionRep] {
    if let sorted = _sortedExtensionReps { return sorted }
    var sortedExtensionReps: [ExtensionRep] = []
    func visit(_ rep: ExtensionRep, _ fromExtensionName: String?) throws {
      if rep.state.id == .permanent { return }
      let extensionName = rep.extension_.name
      try invariant(
        rep.state.id == .unmarked,
        "LexicalBuilder: Circular dependency detected for Extension \(extensionName) from \(fromExtensionName ?? "[unknown]")"
      )
      rep.state.id = .temporary
      if let outgoing = outgoingConfigEdges.get(extensionName) {
        for toExtensionName in outgoing.keysArray {
          if let toRep = extensionNameMap.get(toExtensionName) {
            try visit(toRep, extensionName)
          }
        }
      }
      rep.state.id = .permanent
      sortedExtensionReps.append(rep)
    }
    for rep in extensionNameMap.valuesArray where rep.state.id == .unmarked {
      try visit(rep, nil)
    }
    for rep in sortedExtensionReps {
      guard let outgoing = outgoingConfigEdges.get(rep.extension_.name) else { continue }
      for (toExtensionName, configs) in outgoing where !configs.isEmpty {
        if let toRep = extensionNameMap.get(toExtensionName) {
          toRep.configs.append(contentsOf: configs)
        }
      }
    }
    for root in roots where !root.configs.isEmpty {
      guard let toRep = extensionNameMap.get(root.extension_.name) else {
        throw LexicalError(
          "LexicalBuilder: Expecting existing ExtensionRep for \(root.extension_.name)")
      }
      toRep.configs.append(contentsOf: root.configs)
    }
    _sortedExtensionReps = sortedExtensionReps
    return sortedExtensionReps
  }

  public func registerEditor(_ editor: Editor) throws -> () -> Void {
    let extensionReps = try sortedExtensionReps()
    let controller = AbortSignal()
    var cleanups: [() -> Void] = [{ controller.abort() }]
    for extensionRep in extensionReps {
      if let cleanup = try extensionRep.register(editor, controller) {
        cleanups.append(cleanup)
      }
    }
    for extensionRep in extensionReps {
      if let cleanup = try extensionRep.afterRegistration(editor) {
        cleanups.append(cleanup)
      }
    }
    return mergeRegister(cleanups)
  }

  /// `buildCreateEditorArgs()`: the merged editor configuration, after
  /// running every extension's `init`. Returns the `onError` and `onWarn`
  /// separately, since they take the editor.
  public func buildCreateEditorArgs() throws -> (
    CreateEditorArgs, onError: ((Error, Editor) throws -> Void)?,
    onWarn: ((Error, Editor) throws -> Void)?
  ) {
    var config = InitialEditorConfig()
    var nodes: [LexicalNodeConfig] = []
    var seenKlasses = Set<ObjectIdentifier>()
    var replacedNodes: [ObjectIdentifier: ExtensionRep] = [:]
    var theme: EditorThemeClasses = [:]
    let extensionReps = try sortedExtensionReps()
    for extensionRep in extensionReps {
      let extension_ = extensionRep.extension_
      let editorConfig = extension_.editorConfig
      if let onError = editorConfig.onError { config.onError = onError }
      if let onWarn = editorConfig.onWarn { config.onWarn = onWarn }
      if let parentEditor = editorConfig.parentEditor { config.parentEditor = parentEditor }
      if let editable = editorConfig.editable { config.editable = editable }
      if let namespace = editorConfig.namespace { config.namespace = namespace }
      if let headless = editorConfig.headless { config.headless = headless }
      if let initialEditorState = editorConfig.initialEditorState {
        config.initialEditorState = initialEditorState
      }
      for node in getNodeConfig(extension_) {
        if case .replacement(let replacement) = node {
          let replaced = ObjectIdentifier(replacement.replace)
          if let conflictExtension = replacedNodes[replaced] {
            throw LexicalError(
              "LexicalBuilder: Extension \(extension_.name) can not register replacement for node \(replacement.replace) because \(conflictExtension.extension_.name) already did"
            )
          }
          replacedNodes[replaced] = extensionRep
          nodes.append(node)
        } else if seenKlasses.insert(ObjectIdentifier(node.klassType)).inserted {
          nodes.append(node)
        }
      }
      if let extensionTheme = editorConfig.theme {
        deepThemeMergeInPlace(&theme, extensionTheme)
      }
    }
    if !theme.isEmpty { config.theme = theme }
    if !nodes.isEmpty { config.nodes = nodes }
    for extensionRep in extensionReps {
      try extensionRep.`init`(config)
    }
    let args = CreateEditorArgs(
      namespace: config.namespace, nodes: config.nodes ?? [], parentEditor: config.parentEditor,
      editable: config.editable, theme: config.theme, headless: config.headless ?? false)
    return (args, config.onError, config.onWarn)
  }
}

/// `getNodeConfig(config)`: an extension's nodes.
public func getNodeConfig(_ extension_: any AnyLexicalExtension) -> [LexicalNodeConfig] {
  extension_.nodes ?? []
}

extension LexicalNodeConfig {
  var klassType: LexicalNode.Type {
    switch self {
    case .klass(let klass): klass
    case .replacement(let replacement): replacement.replace
    }
  }
}

/// `getKnownTypesAndNodes(config)`: the node types and classes in a config.
public func getKnownTypesAndNodes(
  _ nodes: [LexicalNodeConfig]
) throws -> (types: JSSet<String>, nodes: [LexicalNode.Type]) {
  let types = JSSet<String>()
  var klasses: [LexicalNode.Type] = []
  for node in nodes {
    let klass = node.klassType
    types.add(klass.getType())
    if !klasses.contains(where: { $0 == klass }) { klasses.append(klass) }
  }
  return (types, klasses)
}
