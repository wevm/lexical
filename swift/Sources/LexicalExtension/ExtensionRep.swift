import Lexical

/// `ExtensionRepStateIds`.
public enum ExtensionRepStateId: Int, Comparable, Sendable {
  case unmarked = 0
  case temporary = 1
  case permanent = 2
  case configured = 3
  case initialized = 4
  case built = 5
  case registered = 6
  case afterRegistration = 7

  public static func < (a: Self, b: Self) -> Bool { a.rawValue < b.rawValue }
}

/// `ExtensionRepState`: the state id with the parts each state adds.
public struct ExtensionRepState {
  public internal(set) var id: ExtensionRepStateId
  public internal(set) var config: Any?
  public internal(set) var initResult: Any?
  public internal(set) var output: Any?
  var initState: ExtensionInitState?
  var buildState: ExtensionBuildState<Any>?
  var registerState: ExtensionRegisterState<Any, Any>?

  init(id: ExtensionRepStateId) { self.id = id }

  /// The merged config, as `C`.
  public func config<C>(as _: C.Type = C.self) -> C { config as! C }
}

/// `ExtensionRep`: the builder's record of one extension.
public final class ExtensionRep {
  unowned let builder: LexicalBuilder
  /// Each a `Partial<Config>` of the extension.
  var configs: [Any] = []
  var _dependency: AnyExtensionDependency?
  var _peerNameSet: JSSet<String>?
  public let extension_: any AnyLexicalExtension
  public internal(set) var state = ExtensionRepState(id: .unmarked)
  var _signal: AbortSignal?

  init(_ builder: LexicalBuilder, _ extension_: any AnyLexicalExtension) {
    self.builder = builder
    self.extension_ = extension_
  }

  func mergeConfigs() throws -> Any {
    try extension_._mergeConfigs(configs)
  }

  func `init`(_ editorConfig: InitialEditorConfig) throws {
    try invariant(
      state.id == .permanent, "ExtensionRep: Can not configure from state id \(state.id.rawValue)")
    let initState = ExtensionInitState(
      getInitDependency: { [unowned self] in try self.getInitDependency($0).config },
      getInitPeer: { [unowned self] in try self.getInitPeer($0) },
      getDirectDependentNames: { [unowned self] in self.getDirectDependentNames() },
      getPeerNameSet: { [unowned self] in self.getPeerNameSet() })
    let buildState = ExtensionBuildState<Any>(
      initState: initState,
      getDependency: { [unowned self] in try self.getDependency($0) },
      getPeer: { [unowned self] in try self.getPeer($0) },
      getInitResult: { [unowned self] in try self.getInitResult() })
    state.config = try mergeConfigs()
    state.initState = initState
    state.id = .configured
    let initResult = try extension_._init(editorConfig, state.config!, initState)
    state.initResult = initResult
    state.buildState = buildState
    state.id = .initialized
  }

  func build(_ editor: Editor) throws {
    try invariant(
      state.id == .initialized,
      "ExtensionRep: register called in state id \(state.id.rawValue) (expected \(ExtensionRepStateId.built.rawValue) initialized)"
    )
    let output = try extension_._build(editor, state.config!, state.buildState!)
    state.registerState = ExtensionRegisterState<Any, Any>(
      buildState: state.buildState!, getOutput: { output },
      getSignal: { [unowned self] in try self.getSignal() })
    state.output = output
    state.id = .built
  }

  func register(_ editor: Editor, _ signal: AbortSignal) throws -> (() -> Void)? {
    _signal = signal
    try invariant(
      state.id == .built,
      "ExtensionRep: register called in state id \(state.id.rawValue) (expected \(ExtensionRepStateId.built.rawValue) built)"
    )
    let cleanup = try extension_._register(editor, state.config!, state.registerState!)
    state.id = .registered
    let registeredId = state.id
    return { [unowned self] in
      precondition(
        self.state.id == .afterRegistration,
        "ExtensionRep: rollbackToBuiltState called in state id \(registeredId.rawValue) (expected \(ExtensionRepStateId.afterRegistration.rawValue) afterRegistration)"
      )
      self.state.id = .built
      cleanup?()
    }
  }

  func afterRegistration(_ editor: Editor) throws -> (() -> Void)? {
    try invariant(
      state.id == .registered,
      "ExtensionRep: afterRegistration called in state id \(state.id.rawValue) (expected \(ExtensionRepStateId.registered.rawValue) registered)"
    )
    let result = try extension_._afterRegistration(editor, state.config!, state.registerState!)
    state.id = .afterRegistration
    return result
  }

  func getSignal() throws -> AbortSignal {
    guard let signal = _signal else {
      throw LexicalError("ExtensionRep.getSignal() called before register")
    }
    return signal
  }

  func getInitResult() throws -> Any {
    try invariant(
      extension_.hasInit,
      "ExtensionRep: getInitResult() called for Extension \(extension_.name) that does not define init"
    )
    try invariant(
      state.id >= .initialized,
      "ExtensionRep: getInitResult() called for ExtensionRep in state id \(state.id.rawValue) < \(ExtensionRepStateId.initialized.rawValue) (initialized)"
    )
    return state.initResult as Any
  }

  func getInitPeer(_ name: String) throws -> Any? {
    guard let rep = builder.extensionNameMap.get(name) else { return nil }
    return try rep.getExtensionInitDependency()
  }

  func getExtensionInitDependency() throws -> Any {
    try invariant(
      state.id >= .configured,
      "ExtensionRep: getExtensionInitDependency called in state id \(state.id.rawValue) (expected >= \(ExtensionRepStateId.configured.rawValue) configured)"
    )
    return state.config!
  }

  func getPeer(_ name: String) throws -> AnyExtensionDependency? {
    guard let rep = builder.extensionNameMap.get(name) else { return nil }
    return try rep.getExtensionDependency()
  }

  func getInitDependency(_ dep: any AnyLexicalExtension) throws -> (config: Any, Void) {
    guard let rep = try builder.getExtensionRep(dep) else {
      throw LexicalError(
        "LexicalExtensionBuilder: Extension \(extension_.name) missing dependency extension \(dep.name) to be in registry"
      )
    }
    return (try rep.getExtensionInitDependency(), ())
  }

  func getDependency(_ dep: any AnyLexicalExtension) throws -> AnyExtensionDependency {
    guard let rep = try builder.getExtensionRep(dep) else {
      throw LexicalError(
        "LexicalExtensionBuilder: Extension \(extension_.name) missing dependency extension \(dep.name) to be in registry"
      )
    }
    return try rep.getExtensionDependency()
  }

  /// `getState()`: the state, once registration has finished.
  public func getState() throws -> ExtensionRepState {
    try invariant(
      state.id >= .afterRegistration,
      "ExtensionRep getState called in state id \(state.id.rawValue) (expected \(ExtensionRepStateId.afterRegistration.rawValue) afterRegistration)"
    )
    return state
  }

  public func getDirectDependentNames() -> JSSet<String> {
    builder.incomingEdges.get(extension_.name) ?? JSSet()
  }

  public func getPeerNameSet() -> JSSet<String> {
    if let s = _peerNameSet { return s }
    let s = JSSet(extension_.peerDependencies.map(\.name))
    _peerNameSet = s
    return s
  }

  public func getExtensionDependency() throws -> AnyExtensionDependency {
    if let dependency = _dependency { return dependency }
    try invariant(
      state.id >= .built, "Extension \(extension_.name) used as a dependency before build")
    let dependency = AnyExtensionDependency(
      config: state.config!, initResult: state.initResult as Any, output: state.output as Any)
    _dependency = dependency
    return dependency
  }
}
