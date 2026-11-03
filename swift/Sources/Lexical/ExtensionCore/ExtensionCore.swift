// Port of packages/lexical/src/extension-core: the types and helpers of the
// extension API that live in core. The builder is in LexicalExtension.

/// `Partial<Config>`: the overrides of a config. It records which keys were
/// set, so a `mergeConfig` can tell an override from a default, as JS tells
/// a present key from an absent one. Write it as `{ $0.disabled = true }`
/// where JS writes `{disabled: true}`.
@dynamicMemberLookup
public struct Partial<Config> {
  struct Entry {
    let key: PartialKeyPath<Config>
    let value: Any
    let apply: (inout Config) -> Void
  }
  var entries: [Entry] = []

  public init() {}

  /// A partial built by `fill`, as in `Partial { $0.b = nil }`.
  public init(_ fill: (inout Partial<Config>) -> Void) {
    fill(&self)
  }

  /// The override of `key`, or `nil` when it isn't set. Assigning sets it.
  public subscript<V>(dynamicMember key: WritableKeyPath<Config, V>) -> V? {
    get {
      guard let entry = entries.first(where: { $0.key == key }) else { return nil }
      return .some(entry.value as! V)
    }
    set {
      entries.removeAll { $0.key == key }
      if let newValue {
        entries.append(Entry(key: key, value: newValue) { $0[keyPath: key] = newValue })
      }
    }
  }

  /// Sets `key` to `value`, which may itself be `nil` for an optional key
  /// (JS `{b: null}`).
  public mutating func set<V>(_ key: WritableKeyPath<Config, V>, _ value: V) {
    entries.removeAll { $0.key == key }
    entries.append(Entry(key: key, value: value) { $0[keyPath: key] = value })
  }

  /// Whether `key` is set.
  public func has(_ key: PartialKeyPath<Config>) -> Bool {
    entries.contains { $0.key == key }
  }

  public var isEmpty: Bool { entries.isEmpty }

  /// `config` with every override applied (`{...config, ...overrides}`).
  public func applied(to config: Config) -> Config {
    var result = config
    for entry in entries { entry.apply(&result) }
    return result
  }
}

/// `shallowMergeConfig`: `config` with `overrides` applied over it.
public func shallowMergeConfig<Config>(_ config: Config, _ overrides: Partial<Config>?) -> Config {
  overrides?.applied(to: config) ?? config
}

/// `safeCast`: returns `value` as `T`. Swift infers the type from the
/// annotation, so this only exists to keep ported code close to JS.
@inline(__always)
public func safeCast<T>(_ value: T) -> T { value }

/// The config of an extension with none (JS `ExtensionConfigBase`, `{}`).
public struct EmptyConfig: Equatable, Sendable {
  public init() {}
}

/// `InitialEditorStateType`: how `InitialStateExtension` fills a new
/// editor.
public enum InitialEditorStateType {
  /// `null`: leave the editor empty.
  case none
  /// A serialized editor state.
  case json(String)
  case serialized(JSONObject)
  case editorState(EditorState)
  /// A function run in an update.
  case update((Editor) throws -> Void)
}

/// `InitialEditorConfig`: the editor configuration an extension
/// contributes, and the argument of `init`, which may change it (`inout`)
/// as JS `init` changes the shared object.
public struct InitialEditorConfig {
  public var namespace: String?
  public var nodes: [LexicalNodeConfig]?
  public var theme: EditorThemeClasses?
  public var editable: Bool?
  public var parentEditor: Editor?
  public var onError: ((Error, Editor) throws -> Void)?
  public var onWarn: ((Error, Editor) throws -> Void)?
  public var initialEditorState: InitialEditorStateType?
  /// Swift only: build a headless editor (`createHeadlessEditor`).
  public var headless: Bool?

  public init(
    namespace: String? = nil, nodes: [LexicalNodeConfig]? = nil,
    theme: EditorThemeClasses? = nil, editable: Bool? = nil, parentEditor: Editor? = nil,
    onError: ((Error, Editor) throws -> Void)? = nil,
    onWarn: ((Error, Editor) throws -> Void)? = nil,
    initialEditorState: InitialEditorStateType? = nil, headless: Bool? = nil
  ) {
    self.namespace = namespace
    self.nodes = nodes
    self.theme = theme
    self.editable = editable
    self.parentEditor = parentEditor
    self.onError = onError
    self.onWarn = onWarn
    self.initialEditorState = initialEditorState
    self.headless = headless
  }
}

/// `AbortSignal`: aborted when the editor an extension registered on is
/// disposed.
public final class AbortSignal {
  public private(set) var aborted = false
  private var listeners: [() -> Void] = []

  public init() {}

  /// Calls `listener` once, when the signal aborts (at once if it has).
  public func addAbortListener(_ listener: @escaping () -> Void) {
    if aborted { listener() } else { listeners.append(listener) }
  }

  /// `AbortController.abort()`.
  public func abort() {
    guard !aborted else { return }
    aborted = true
    let listeners = self.listeners
    self.listeners = []
    for listener in listeners { listener() }
  }
}

/// A dependency, or a peer, as the builder hands it out: its config,
/// init result and output, type-erased.
public struct AnyExtensionDependency {
  public let config: Any
  public let initResult: Any
  public let output: Any

  public init(config: Any, initResult: Any, output: Any) {
    self.config = config
    self.initResult = initResult
    self.output = output
  }
}

/// `LexicalExtensionDependency`: a built dependency's config, init result
/// and output.
public struct LexicalExtensionDependency<Config, Output, Init> {
  public let config: Config
  public let initResult: Init
  public let output: Output

  public init(config: Config, initResult: Init, output: Output) {
    self.config = config
    self.initResult = initResult
    self.output = output
  }
}

/// The part of a dependency available before it is built
/// (`Omit<LexicalExtensionDependency, 'output' | 'init'>`).
public struct LexicalExtensionInitDependency<Config> {
  public let config: Config
}

func castDependency<C, O, I>(
  _ dep: AnyExtensionDependency, _ name: String
) throws -> LexicalExtensionDependency<C, O, I> {
  guard let config = dep.config as? C, let initResult = dep.initResult as? I,
    let output = dep.output as? O
  else {
    throw LexicalError("Extension \(name) does not have the requested config, init or output type")
  }
  return LexicalExtensionDependency(config: config, initResult: initResult, output: output)
}

/// `ExtensionInitState`: what `init` can ask of the builder.
public class ExtensionInitState {
  let _getInitDependency: (any AnyLexicalExtension) throws -> Any
  let _getInitPeer: (String) throws -> Any?
  let _getDirectDependentNames: () -> JSSet<String>
  let _getPeerNameSet: () -> JSSet<String>

  package init(
    getInitDependency: @escaping (any AnyLexicalExtension) throws -> Any,
    getInitPeer: @escaping (String) throws -> Any?,
    getDirectDependentNames: @escaping () -> JSSet<String>,
    getPeerNameSet: @escaping () -> JSSet<String>
  ) {
    _getInitDependency = getInitDependency
    _getInitPeer = getInitPeer
    _getDirectDependentNames = getDirectDependentNames
    _getPeerNameSet = getPeerNameSet
  }

  /// The config of a dependency, which is configured before this one.
  public func getDependency<C, O, I>(
    _ dep: LexicalExtension<C, O, I>
  ) throws -> LexicalExtensionInitDependency<C> {
    guard let config = try _getInitDependency(dep) as? C else {
      throw LexicalError("Extension \(dep.name) does not have the requested config type")
    }
    return LexicalExtensionInitDependency(config: config)
  }

  /// The config of the peer named `name`, when the editor has it.
  public func getPeer<C>(
    _ name: String, config: C.Type = C.self
  ) throws -> LexicalExtensionInitDependency<C>? {
    guard let config = try _getInitPeer(name) else { return nil }
    guard let config = config as? C else {
      throw LexicalError("Extension \(name) does not have the requested config type")
    }
    return LexicalExtensionInitDependency(config: config)
  }

  /// The names of the extensions that depend on this one directly.
  public func getDirectDependentNames() -> JSSet<String> { _getDirectDependentNames() }

  /// The names of this extension's peer dependencies.
  public func getPeerNameSet() -> JSSet<String> { _getPeerNameSet() }
}

/// `ExtensionBuildState`: what `build` can ask of the builder.
public class ExtensionBuildState<Init> {
  let initState: ExtensionInitState
  let _getDependency: (any AnyLexicalExtension) throws -> AnyExtensionDependency
  let _getPeer: (String) throws -> AnyExtensionDependency?
  let _getInitResult: () throws -> Any

  package init(
    initState: ExtensionInitState,
    getDependency: @escaping (any AnyLexicalExtension) throws -> AnyExtensionDependency,
    getPeer: @escaping (String) throws -> AnyExtensionDependency?,
    getInitResult: @escaping () throws -> Any
  ) {
    self.initState = initState
    _getDependency = getDependency
    _getPeer = getPeer
    _getInitResult = getInitResult
  }

  package init(copying other: ExtensionBuildState<Init>) {
    initState = other.initState
    _getDependency = other._getDependency
    _getPeer = other._getPeer
    _getInitResult = other._getInitResult
  }

  /// A dependency's config, init result and output; dependencies are built
  /// first.
  public func getDependency<C, O, I>(
    _ dep: LexicalExtension<C, O, I>
  ) throws -> LexicalExtensionDependency<C, O, I> {
    try castDependency(try _getDependency(dep), dep.name)
  }

  /// The peer named `name`, when the editor has it, as an extension of the
  /// given types.
  public func getPeer<C, O, I>(
    _ name: String, as _: LexicalExtension<C, O, I>.Type
  ) throws -> LexicalExtensionDependency<C, O, I>? {
    guard let dep = try _getPeer(name) else { return nil }
    return try castDependency(dep, name)
  }

  /// The peer named `name`, type-erased.
  public func getPeer(_ name: String) throws -> AnyExtensionDependency? {
    try _getPeer(name)
  }

  /// What this extension's `init` returned.
  public func getInitResult() throws -> Init {
    guard let result = try _getInitResult() as? Init else {
      throw LexicalError("ExtensionRep: getInitResult() returned an unexpected type")
    }
    return result
  }

  public func getDirectDependentNames() -> JSSet<String> {
    initState.getDirectDependentNames()
  }

  public func getPeerNameSet() -> JSSet<String> { initState.getPeerNameSet() }
}

/// `ExtensionRegisterState`: what `register` and `afterRegistration` can
/// ask of the builder.
public final class ExtensionRegisterState<Init, Output>: ExtensionBuildState<Init> {
  let _getOutput: () -> Output
  let _getSignal: () throws -> AbortSignal

  package init(
    buildState: ExtensionBuildState<Init>, getOutput: @escaping () -> Output,
    getSignal: @escaping () throws -> AbortSignal
  ) {
    _getOutput = getOutput
    _getSignal = getSignal
    super.init(copying: buildState)
  }

  /// What this extension's `build` returned.
  public func getOutput() -> Output { _getOutput() }

  /// Aborted when the editor is disposed.
  public func getSignal() throws -> AbortSignal { try _getSignal() }
}

/// `AnyLexicalExtensionArgument`: an extension, or an extension with config
/// overrides (`configExtension`).
public protocol AnyLexicalExtensionArgument {
  /// `normalizeExtensionArgument`: the extension and its overrides.
  var normalized: AnyNormalizedLexicalExtensionArgument { get }
}

/// `AnyNormalizedLexicalExtensionArgument`: `[extension, ...overrides]`.
public struct AnyNormalizedLexicalExtensionArgument: AnyLexicalExtensionArgument {
  public let extension_: any AnyLexicalExtension
  /// Each a `Partial<Config>` of the extension.
  public let configs: [Any]

  public var normalized: AnyNormalizedLexicalExtensionArgument { self }
}

/// `NormalizedPeerDependency`: `[name, overrides?]`.
public struct NormalizedPeerDependency {
  public let name: String
  /// A `Partial<Config>` of the peer, or `nil`.
  public let config: Any?
}

/// `AnyLexicalExtension`: an extension, with its typed parts erased for
/// the builder.
public protocol AnyLexicalExtension: AnyObject, AnyLexicalExtensionArgument {
  var name: String { get }
  var conflictsWith: [String] { get }
  var dependencies: [any AnyLexicalExtensionArgument] { get }
  var peerDependencies: [NormalizedPeerDependency] { get }
  /// The editor configuration this extension contributes.
  var editorConfig: InitialEditorConfig { get }
  /// `getNodeConfig(extension)`: the nodes, calling the deferred form.
  var nodes: [LexicalNodeConfig]? { get }
  /// Whether the extension defines `init` (`getInitResult` requires it).
  var hasInit: Bool { get }

  /// `mergeConfigs`: the default config with each override merged in.
  func _mergeConfigs(_ configs: [Any]) throws -> Any
  func _init(
    _ editorConfig: inout InitialEditorConfig, _ config: Any, _ state: ExtensionInitState
  ) throws -> Any
  func _build(
    _ editor: Editor, _ config: Any,
    _ state: ExtensionBuildState<Any>
  ) throws -> Any
  func _register(
    _ editor: Editor, _ config: Any,
    _ state: ExtensionRegisterState<Any, Any>
  ) throws -> (() -> Void)?
  func _afterRegistration(
    _ editor: Editor, _ config: Any,
    _ state: ExtensionRegisterState<Any, Any>
  ) throws -> (() -> Void)?
}

/// `LexicalExtension`: a named unit of editor configuration and behavior.
/// A class, because the builder tells extensions apart by identity as JS
/// does; `dependencies` can be changed after definition. It is
/// `@unchecked Sendable` so an extension can be a global `let`, as JS
/// defines them at module scope: a definition is set up once, and the
/// editors it configures stay on their own thread.
public final class LexicalExtension<Config, Output, Init>: AnyLexicalExtension, @unchecked Sendable
{
  public let name: String
  public var conflictsWith: [String]
  public var dependencies: [any AnyLexicalExtensionArgument]
  public var peerDependencies: [NormalizedPeerDependency]
  public var config: Config
  public var mergeConfig: ((Config, Partial<Config>) throws -> Config)?
  public var initFn: ((inout InitialEditorConfig, Config, ExtensionInitState) throws -> Init)?
  public var buildFn: ((Editor, Config, ExtensionBuildState<Init>) throws -> Output)?
  public var registerFn:
    ((Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void)?
  public var afterRegistrationFn:
    ((Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void)?
  public var editorConfig: InitialEditorConfig
  private let _nodes: (() -> [LexicalNodeConfig])?

  public init(
    name: String, config: Config, conflictsWith: [String] = [],
    dependencies: [any AnyLexicalExtensionArgument] = [],
    peerDependencies: [NormalizedPeerDependency] = [],
    mergeConfig: ((Config, Partial<Config>) throws -> Config)? = nil,
    nodes: (() -> [LexicalNodeConfig])? = nil,
    editorConfig: InitialEditorConfig = InitialEditorConfig(),
    init initFn: ((inout InitialEditorConfig, Config, ExtensionInitState) throws -> Init)?,
    build: ((Editor, Config, ExtensionBuildState<Init>) throws -> Output)?,
    register: ((Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void)? =
      nil,
    afterRegistration: (
      (Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void
    )? = nil
  ) {
    self.name = name
    self.config = config
    self.conflictsWith = conflictsWith
    self.dependencies = dependencies
    self.peerDependencies = peerDependencies
    self.mergeConfig = mergeConfig
    self._nodes = nodes
    self.editorConfig = editorConfig
    self.initFn = initFn
    self.buildFn = build
    self.registerFn = register
    self.afterRegistrationFn = afterRegistration
  }

  public var normalized: AnyNormalizedLexicalExtensionArgument {
    AnyNormalizedLexicalExtensionArgument(extension_: self, configs: [])
  }

  public var hasInit: Bool { initFn != nil }

  /// `getNodeConfig(extension)`: the nodes, calling the deferred form.
  public var nodes: [LexicalNodeConfig]? { _nodes?() ?? editorConfig.nodes }

  public func _mergeConfigs(_ configs: [Any]) throws -> Any {
    var merged = config
    for partial in configs {
      guard let partial = partial as? Partial<Config> else {
        throw LexicalError("LexicalBuilder: a config for \(name) has the wrong type")
      }
      merged = try mergeConfig?(merged, partial) ?? shallowMergeConfig(merged, partial)
    }
    return merged
  }

  public func _init(
    _ editorConfig: inout InitialEditorConfig, _ config: Any, _ state: ExtensionInitState
  ) throws -> Any {
    guard let initFn else { return () }
    return try initFn(&editorConfig, config as! Config, state)
  }

  public func _build(
    _ editor: Editor, _ config: Any, _ state: ExtensionBuildState<Any>
  ) throws -> Any {
    guard let buildFn else { return () }
    return try buildFn(editor, config as! Config, ExtensionBuildState<Init>(erased: state))
  }

  public func _register(
    _ editor: Editor, _ config: Any, _ state: ExtensionRegisterState<Any, Any>
  ) throws -> (() -> Void)? {
    guard let registerFn else { return nil }
    return try registerFn(editor, config as! Config, ExtensionRegisterState(erased: state))
  }

  public func _afterRegistration(
    _ editor: Editor, _ config: Any, _ state: ExtensionRegisterState<Any, Any>
  ) throws -> (() -> Void)? {
    guard let afterRegistrationFn else { return nil }
    return try afterRegistrationFn(
      editor, config as! Config, ExtensionRegisterState(erased: state))
  }
}

extension ExtensionBuildState {
  convenience init(erased state: ExtensionBuildState<Any>) {
    self.init(
      initState: state.initState, getDependency: state._getDependency, getPeer: state._getPeer,
      getInitResult: state._getInitResult)
  }
}

extension ExtensionRegisterState {
  convenience init(erased state: ExtensionRegisterState<Any, Any>) {
    let getOutput = state._getOutput
    self.init(
      buildState: ExtensionBuildState<Init>(erased: state),
      getOutput: { getOutput() as! Output }, getSignal: state._getSignal)
  }
}

/// `NormalizedLexicalExtensionArgument`: an extension with overrides.
public struct ConfiguredExtension<Config, Output, Init>: AnyLexicalExtensionArgument {
  public let extension_: LexicalExtension<Config, Output, Init>
  public let configs: [Partial<Config>]

  public var normalized: AnyNormalizedLexicalExtensionArgument {
    AnyNormalizedLexicalExtensionArgument(extension_: extension_, configs: configs)
  }
}

/// `configExtension(extension, ...overrides)`.
public func configExtension<C, O, I>(
  _ extension_: LexicalExtension<C, O, I>, _ configs: Partial<C>...
) -> ConfiguredExtension<C, O, I> {
  ConfiguredExtension(extension_: extension_, configs: configs)
}

/// `configExtension(extension, {...})`, writing the overrides in a
/// closure: `configExtension(Ext) { $0.disabled = true }`.
public func configExtension<C, O, I>(
  _ extension_: LexicalExtension<C, O, I>, _ fill: (inout Partial<C>) -> Void
) -> ConfiguredExtension<C, O, I> {
  ConfiguredExtension(extension_: extension_, configs: [Partial(fill)])
}

/// `declarePeerDependency(name)`.
public func declarePeerDependency(_ name: String) -> NormalizedPeerDependency {
  NormalizedPeerDependency(name: name, config: nil)
}

/// `declarePeerDependency<typeof Ext>(name, overrides)`: `config` names the
/// peer's config type, which JS gives as a type argument.
public func declarePeerDependency<C>(
  _ name: String, config _: C.Type = C.self, _ fill: (inout Partial<C>) -> Void
) -> NormalizedPeerDependency {
  NormalizedPeerDependency(name: name, config: Partial(fill))
}

/// `defineExtension({...})`. The overloads cover the combinations of
/// `config`, `init` and `build` an extension may leave out, because Swift
/// generic parameters have no defaults.
public func defineExtension<Config, Output, Init>(
  name: String, config: Config, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  mergeConfig: ((Config, Partial<Config>) throws -> Config)? = nil,
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  init initFn: @escaping (inout InitialEditorConfig, Config, ExtensionInitState) throws -> Init,
  build: @escaping (Editor, Config, ExtensionBuildState<Init>) throws -> Output,
  register: ((Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void)? = nil,
  afterRegistration: (
    (Editor, Config, ExtensionRegisterState<Init, Output>) throws -> () -> Void
  )? =
    nil
) -> LexicalExtension<Config, Output, Init> {
  LexicalExtension(
    name: name, config: config, conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, mergeConfig: mergeConfig, nodes: nodes,
    editorConfig: editorConfig, init: initFn, build: build, register: register,
    afterRegistration: afterRegistration)
}

/// `defineExtension` without `init`.
public func defineExtension<Config, Output>(
  name: String, config: Config, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  mergeConfig: ((Config, Partial<Config>) throws -> Config)? = nil,
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  build: @escaping (Editor, Config, ExtensionBuildState<Void>) throws -> Output,
  register: ((Editor, Config, ExtensionRegisterState<Void, Output>) throws -> () -> Void)? = nil,
  afterRegistration: (
    (Editor, Config, ExtensionRegisterState<Void, Output>) throws -> () -> Void
  )? =
    nil
) -> LexicalExtension<Config, Output, Void> {
  LexicalExtension(
    name: name, config: config, conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, mergeConfig: mergeConfig, nodes: nodes,
    editorConfig: editorConfig, init: nil, build: build, register: register,
    afterRegistration: afterRegistration)
}

/// `defineExtension` without `build`.
public func defineExtension<Config, Init>(
  name: String, config: Config, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  mergeConfig: ((Config, Partial<Config>) throws -> Config)? = nil,
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  init initFn: @escaping (inout InitialEditorConfig, Config, ExtensionInitState) throws -> Init,
  register: ((Editor, Config, ExtensionRegisterState<Init, Void>) throws -> () -> Void)? = nil,
  afterRegistration: ((Editor, Config, ExtensionRegisterState<Init, Void>) throws -> () -> Void)? =
    nil
) -> LexicalExtension<Config, Void, Init> {
  LexicalExtension(
    name: name, config: config, conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, mergeConfig: mergeConfig, nodes: nodes,
    editorConfig: editorConfig, init: initFn, build: nil, register: register,
    afterRegistration: afterRegistration)
}

/// `defineExtension` without `init` or `build`.
public func defineExtension<Config>(
  name: String, config: Config, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  mergeConfig: ((Config, Partial<Config>) throws -> Config)? = nil,
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  register: ((Editor, Config, ExtensionRegisterState<Void, Void>) throws -> () -> Void)? = nil,
  afterRegistration: ((Editor, Config, ExtensionRegisterState<Void, Void>) throws -> () -> Void)? =
    nil
) -> LexicalExtension<Config, Void, Void> {
  LexicalExtension(
    name: name, config: config, conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, mergeConfig: mergeConfig, nodes: nodes,
    editorConfig: editorConfig, init: nil, build: nil, register: register,
    afterRegistration: afterRegistration)
}

/// `defineExtension` without `config`.
public func defineExtension<Output, Init>(
  name: String, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  init initFn:
    @escaping (inout InitialEditorConfig, EmptyConfig, ExtensionInitState) throws -> Init,
  build: @escaping (Editor, EmptyConfig, ExtensionBuildState<Init>) throws -> Output,
  register: ((Editor, EmptyConfig, ExtensionRegisterState<Init, Output>) throws -> () -> Void)? =
    nil,
  afterRegistration: (
    (Editor, EmptyConfig, ExtensionRegisterState<Init, Output>) throws -> () -> Void
  )? = nil
) -> LexicalExtension<EmptyConfig, Output, Init> {
  LexicalExtension(
    name: name, config: EmptyConfig(), conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, nodes: nodes, editorConfig: editorConfig, init: initFn,
    build: build, register: register, afterRegistration: afterRegistration)
}

/// `defineExtension` without `config` or `init`.
public func defineExtension<Output>(
  name: String, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  build: @escaping (Editor, EmptyConfig, ExtensionBuildState<Void>) throws -> Output,
  register: ((Editor, EmptyConfig, ExtensionRegisterState<Void, Output>) throws -> () -> Void)? =
    nil,
  afterRegistration: (
    (Editor, EmptyConfig, ExtensionRegisterState<Void, Output>) throws -> () -> Void
  )? = nil
) -> LexicalExtension<EmptyConfig, Output, Void> {
  LexicalExtension(
    name: name, config: EmptyConfig(), conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, nodes: nodes, editorConfig: editorConfig, init: nil,
    build: build, register: register, afterRegistration: afterRegistration)
}

/// `defineExtension` without `config` or `build`.
public func defineExtension<Init>(
  name: String, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  init initFn:
    @escaping (inout InitialEditorConfig, EmptyConfig, ExtensionInitState) throws -> Init,
  register: ((Editor, EmptyConfig, ExtensionRegisterState<Init, Void>) throws -> () -> Void)? =
    nil,
  afterRegistration: (
    (Editor, EmptyConfig, ExtensionRegisterState<Init, Void>) throws -> () -> Void
  )? = nil
) -> LexicalExtension<EmptyConfig, Void, Init> {
  LexicalExtension(
    name: name, config: EmptyConfig(), conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, nodes: nodes, editorConfig: editorConfig, init: initFn,
    build: nil, register: register, afterRegistration: afterRegistration)
}

/// `defineExtension` with none of `config`, `init` or `build`.
public func defineExtension(
  name: String, conflictsWith: [String] = [],
  dependencies: [any AnyLexicalExtensionArgument] = [],
  peerDependencies: [NormalizedPeerDependency] = [],
  nodes: (() -> [LexicalNodeConfig])? = nil,
  editorConfig: InitialEditorConfig = InitialEditorConfig(),
  register: ((Editor, EmptyConfig, ExtensionRegisterState<Void, Void>) throws -> () -> Void)? =
    nil,
  afterRegistration: (
    (Editor, EmptyConfig, ExtensionRegisterState<Void, Void>) throws -> () -> Void
  )? = nil
) -> LexicalExtension<EmptyConfig, Void, Void> {
  LexicalExtension(
    name: name, config: EmptyConfig(), conflictsWith: conflictsWith, dependencies: dependencies,
    peerDependencies: peerDependencies, nodes: nodes, editorConfig: editorConfig, init: nil,
    build: nil, register: register, afterRegistration: afterRegistration)
}
