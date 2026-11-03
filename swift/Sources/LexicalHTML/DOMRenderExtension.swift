import Lexical
import LexicalExtension

// Port of packages/lexical-html/src/internal: DOMRenderExtension, its
// overrides (`domOverride`, compileDOMRenderConfigOverrides.ts), the
// runtime that toggles them (DOMRenderRuntime.ts) and the render context
// (RenderContext.ts). All `@experimental` upstream.
//
// Core's `EditorDOMRenderConfig` has no `$getSlotTargetElement`: slot DOM
// placement belongs to the DOM reconciler, which the native views replace,
// so overrides have no such hook either.

/// A node an override applies to: instances of a class (and its
/// subclasses), or nodes a predicate accepts.
public enum NodeMatch {
  case klass(LexicalNode.Type)
  case predicate((LexicalNode) -> Bool)
}

/// `RenderContextReader`: reads render context values.
public struct RenderContextReader {
  let record: ContextRecord?
  public func get<V>(_ cfg: RenderStateConfig<V>) -> V { getContextValue(record, cfg) }
}

/// `RenderStateConfig<V>`: a typed key of the render context.
public typealias RenderStateConfig<V> = ContextConfig<V>

/// `DOMOverrideOptions`: whether an override is installed, from the render
/// context. `disabledForEditor` gates it in the editor's render config;
/// `disabledForSession` gates one export session.
public struct DOMOverrideOptions {
  public var disabledForEditor: ((RenderContextReader) -> Bool)?
  public var disabledForSession: ((RenderContextReader) -> Bool)?
  public init(
    disabledForEditor: ((RenderContextReader) -> Bool)? = nil,
    disabledForSession: ((RenderContextReader) -> Bool)? = nil
  ) {
    self.disabledForEditor = disabledForEditor
    self.disabledForSession = disabledForSession
  }
}

/// `DOMRenderMatchConfig`: the hooks of an override. Each receives `$next`,
/// the behavior it overrides, except `decorateDOM`, which runs after it.
public struct DOMRenderMatchConfig {
  public var getDOMSlot:
    ((LexicalNode, DOMElement, () throws -> DOMSlot, Editor) throws -> DOMSlot)?
  public var createDOM: ((LexicalNode, () throws -> DOMElement, Editor) throws -> DOMElement)?
  public var updateDOM:
    ((LexicalNode, LexicalNode, DOMElement, () throws -> Bool, Editor) throws -> Bool)?
  public var decorateDOM: ((LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void)?
  public var exportDOM:
    ((LexicalNode, () throws -> DOMExportOutput, Editor) throws -> DOMExportOutput)?
  public var shouldExclude:
    ((LexicalNode, BaseSelection?, () throws -> Bool, Editor) throws -> Bool)?
  public var shouldInclude:
    ((LexicalNode, BaseSelection?, () throws -> Bool, Editor) throws -> Bool)?
  public var extractWithChild:
    (
      (LexicalNode, LexicalNode, BaseSelection?, String, () throws -> Bool, Editor) throws -> Bool
    )?

  public init(
    getDOMSlot: ((LexicalNode, DOMElement, () throws -> DOMSlot, Editor) throws -> DOMSlot)? =
      nil,
    createDOM: ((LexicalNode, () throws -> DOMElement, Editor) throws -> DOMElement)? = nil,
    updateDOM: (
      (LexicalNode, LexicalNode, DOMElement, () throws -> Bool, Editor) throws -> Bool
    )? = nil,
    decorateDOM: ((LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void)? = nil,
    exportDOM: (
      (LexicalNode, () throws -> DOMExportOutput, Editor) throws -> DOMExportOutput
    )? = nil,
    shouldExclude: ((LexicalNode, BaseSelection?, () throws -> Bool, Editor) throws -> Bool)? =
      nil,
    shouldInclude: ((LexicalNode, BaseSelection?, () throws -> Bool, Editor) throws -> Bool)? =
      nil,
    extractWithChild: (
      (LexicalNode, LexicalNode, BaseSelection?, String, () throws -> Bool, Editor) throws -> Bool
    )? = nil
  ) {
    self.getDOMSlot = getDOMSlot
    self.createDOM = createDOM
    self.updateDOM = updateDOM
    self.decorateDOM = decorateDOM
    self.exportDOM = exportDOM
    self.shouldExclude = shouldExclude
    self.shouldInclude = shouldInclude
    self.extractWithChild = extractWithChild
  }
}

/// `DOMRenderMatch`: an override and the nodes it applies to (`nil` for
/// every node, JS `'*'`). A class, because the runtime tells overrides apart
/// by identity, as JS does.
public final class DOMRenderMatch {
  public let nodes: [NodeMatch]?
  public let config: DOMRenderMatchConfig
  public let options: DOMOverrideOptions

  public init(nodes: [NodeMatch]?, config: DOMRenderMatchConfig, options: DOMOverrideOptions) {
    self.nodes = nodes
    self.config = config
    self.options = options
  }
}

/// `domOverride('*', config, options)`: an override of every node.
public func domOverride(
  all config: DOMRenderMatchConfig, _ options: DOMOverrideOptions = DOMOverrideOptions()
) -> DOMRenderMatch {
  DOMRenderMatch(nodes: nil, config: config, options: options)
}

/// `domOverride(nodes, config, options)`.
public func domOverride(
  _ nodes: [NodeMatch], _ config: DOMRenderMatchConfig,
  _ options: DOMOverrideOptions = DOMOverrideOptions()
) -> DOMRenderMatch {
  DOMRenderMatch(nodes: nodes, config: config, options: options)
}

/// `domOverride([Klass, ...], config, options)`.
public func domOverride(
  _ klasses: [LexicalNode.Type], _ config: DOMRenderMatchConfig,
  _ options: DOMOverrideOptions = DOMOverrideOptions()
) -> DOMRenderMatch {
  domOverride(klasses.map { .klass($0) }, config, options)
}

/// `DOMRenderConfig`: the extension's config.
public struct DOMRenderConfig {
  public var overrides: [DOMRenderMatch]
  public var contextDefaults: [ContextPairOrUpdater]
  public init(overrides: [DOMRenderMatch] = [], contextDefaults: [ContextPairOrUpdater] = []) {
    self.overrides = overrides
    self.contextDefaults = contextDefaults
  }
}

// MARK: - Compiling overrides (compileDOMRenderConfigOverrides.ts)

private enum PredicateOrTypes {
  case predicate((LexicalNode) -> Bool)
  case types(JSSet<String>)
}

/// One step of a hook's chain: overrides guarded by a predicate, or
/// overrides by node type (consecutive type overrides share a step).
private enum AnyRender<O> {
  case predicate((LexicalNode) -> Bool, O)
  case types(JSMap<String, [O]>)
}

/// A hook as a function of the node, the other arguments, and the editor.
private typealias HookFn<A, T> = (LexicalNode, A, Editor) throws -> T
private typealias HookOverride<A, T> = (LexicalNode, A, () throws -> T, Editor) throws -> T

private func isInstance(_ node: LexicalNode, _ klass: LexicalNode.Type) -> Bool {
  isSubclass(type(of: node), of: klass)
}

private func getPredicate(
  _ subtypeMap: JSMap<String, JSSet<String>>, _ override: DOMRenderMatch
)
  throws -> PredicateOrTypes
{
  guard let nodes = override.nodes else { return .predicate { _ in true } }
  var types: JSSet<String>? = JSSet()
  var predicates: [(LexicalNode) -> Bool] = []
  for match in nodes {
    switch match {
    case .klass(let klass):
      let type = klass.getType()
      if let set = types {
        guard let subtypes = subtypeMap.get(type) else {
          throw LexicalError(
            "Node class \(klass) with type \(type) not registered in editor")
        }
        for subtype in subtypes { set.add(subtype) }
      }
      predicates.append { isInstance($0, klass) }
    case .predicate(let predicate):
      types = nil
      predicates.append(predicate)
    }
  }
  if let types { return .types(types) }
  if predicates.count == 1 { return .predicate(predicates[0]) }
  return .predicate { node in predicates.contains { $0(node) } }
}

private func addOverride<O>(
  _ arr: inout [AnyRender<O>], _ predicateOrTypes: PredicateOrTypes, _ override: O?
) {
  guard let override else { return }
  switch predicateOrTypes {
  case .predicate(let predicate):
    arr.append(.predicate(predicate, override))
  case .types(let types):
    let typeMap: JSMap<String, [O]>
    if case .types(let last) = arr.last {
      typeMap = last
    } else {
      typeMap = JSMap()
      arr.append(.types(typeMap))
    }
    for type in types {
      typeMap.set(type, (typeMap.get(type) ?? []) + [override])
    }
  }
}

private func merge<A, T>(
  _ acc: @escaping HookFn<A, T>, _ getOverride: @escaping (LexicalNode) -> HookOverride<A, T>?
) -> HookFn<A, T> {
  { node, a, editor in
    let next = { try acc(node, a, editor) }
    if let override = getOverride(node) {
      return try override(node, a, next, editor)
    }
    return try next()
  }
}

private func compileHook<A, T>(
  _ steps: [AnyRender<HookOverride<A, T>>], _ base: @escaping HookFn<A, T>
)
  -> HookFn<A, T>
{
  var acc = base
  for step in steps {
    switch step {
    case .predicate(let predicate, let override):
      acc = merge(acc) { predicate($0) ? override : nil }
    case .types(let typeOverrides):
      var compiled: [String: HookFn<A, T>] = [:]
      for (type, overrides) in typeOverrides {
        compiled[type] = overrides.reduce(acc) { chain, override in merge(chain) { _ in override } }
      }
      acc = merge(acc) { node in
        compiled[node.getType()].map { f in { node, a, _, editor in try f(node, a, editor) } }
      }
    }
  }
  return acc
}

/// `sequence4` for `decorateDOM`: the base, then each override.
private func compileDecorate(
  _ steps: [AnyRender<(LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void>],
  _ base: @escaping (LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void
) -> (LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void {
  var acc = base
  for step in steps {
    let prev = acc
    switch step {
    case .predicate(let predicate, let override):
      acc = { node, prevNode, dom, editor in
        try prev(node, prevNode, dom, editor)
        if predicate(node) { try override(node, prevNode, dom, editor) }
      }
    case .types(let typeOverrides):
      var compiled: [String: (LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void] = [:]
      for (type, overrides) in typeOverrides {
        compiled[type] = overrides.reduce(prev) { chain, override in
          { node, prevNode, dom, editor in
            try chain(node, prevNode, dom, editor)
            try override(node, prevNode, dom, editor)
          }
        }
      }
      acc = { node, prevNode, dom, editor in
        try prev(node, prevNode, dom, editor)
        // The type's chain starts from `prev` too, so `prev` runs twice, as
        // in JS, where `sequence4` runs the accumulator and then the chain
        // `compilePrerenderKey` built over it.
        if let f = compiled[node.getType()] { try f(node, prevNode, dom, editor) }
      }
    }
  }
  return acc
}

/// The class depth of a node class (`iterStaticNodeConfigChain` length).
private func depthOf(_ klass: LexicalNode.Type) -> Int {
  iterStaticNodeConfigChain(klass).count - 1
}

/// `sortedOverrides`: class overrides (one per class, shallowest class
/// first), then predicate overrides, then wildcards.
private func sortedOverrides(_ overrides: [DOMRenderMatch]) -> [DOMRenderMatch] {
  var byWildcard: [DOMRenderMatch] = []
  var byPredicate: [DOMRenderMatch] = []
  var byNode: [(DOMRenderMatch, LexicalNode.Type)] = []
  for override in overrides {
    guard let nodes = override.nodes else {
      byWildcard.append(override)
      continue
    }
    for match in nodes {
      let single =
        nodes.count == 1
        ? override
        : DOMRenderMatch(nodes: [match], config: override.config, options: override.options)
      switch match {
      case .klass(let klass): byNode.append((single, klass))
      case .predicate: byPredicate.append(single)
      }
    }
  }
  // A stable sort by depth, as `Array.prototype.sort` is.
  let sortedNodes = byNode.enumerated().sorted { a, b in
    let (da, db) = (depthOf(a.element.1), depthOf(b.element.1))
    return da != db ? da < db : a.offset < b.offset
  }.map(\.element.0)
  return sortedNodes + byPredicate + byWildcard
}

/// `PreEditorDOMRenderConfig`: each hook's chain before compiling.
private struct Prerender {
  var createDOM: [AnyRender<HookOverride<Void, DOMElement>>] = []
  var exportDOM: [AnyRender<HookOverride<Void, DOMExportOutput>>] = []
  var extractWithChild: [AnyRender<HookOverride<(LexicalNode, BaseSelection?, String), Bool>>] =
    []
  var getDOMSlot: [AnyRender<HookOverride<DOMElement, DOMSlot>>] = []
  var shouldExclude: [AnyRender<HookOverride<BaseSelection?, Bool>>] = []
  var shouldInclude: [AnyRender<HookOverride<BaseSelection?, Bool>>] = []
  var updateDOM: [AnyRender<HookOverride<(LexicalNode, DOMElement), Bool>>] = []
  var decorateDOM: [AnyRender<(LexicalNode, LexicalNode?, DOMElement, Editor) throws -> Void>] =
    []
}

private func precompile(
  _ nodes: [LexicalNodeConfig], _ overrides: [DOMRenderMatch]
) throws
  -> Prerender
{
  let subtypeMap = try getRegisteredSubtypeMap(getKnownTypesAndNodes(nodes).nodes)
  var p = Prerender()
  for override in sortedOverrides(overrides) {
    let pot = try getPredicate(subtypeMap, override)
    let c = override.config
    addOverride(
      &p.createDOM, pot,
      c.createDOM.map { o in { node, _, next, editor in try o(node, next, editor) } })
    addOverride(&p.decorateDOM, pot, c.decorateDOM)
    addOverride(
      &p.exportDOM, pot,
      c.exportDOM.map { o in { node, _, next, editor in try o(node, next, editor) } })
    addOverride(
      &p.extractWithChild, pot,
      c.extractWithChild.map { o in
        { node, a, next, editor in try o(node, a.0, a.1, a.2, next, editor) }
      })
    addOverride(
      &p.getDOMSlot, pot,
      c.getDOMSlot.map { o in { node, dom, next, editor in try o(node, dom, next, editor) } })
    addOverride(
      &p.shouldExclude, pot,
      c.shouldExclude.map { o in
        { node, selection, next, editor in try o(node, selection, next, editor) }
      })
    addOverride(
      &p.shouldInclude, pot,
      c.shouldInclude.map { o in
        { node, selection, next, editor in try o(node, selection, next, editor) }
      })
    addOverride(
      &p.updateDOM, pot,
      c.updateDOM.map { o in { node, a, next, editor in try o(node, a.0, a.1, next, editor) } })
  }
  return p
}

/// `compileDOMRenderConfigOverrides(editorConfig, {overrides})`: the
/// editor's render config (the default, or `dom`) with the overrides
/// layered over it.
func compileDOMRenderConfigOverrides(
  _ nodes: [LexicalNodeConfig], _ dom: EditorDOMRenderConfig?, _ overrides: [DOMRenderMatch]
) throws -> EditorDOMRenderConfig {
  let p = try precompile(nodes, overrides)
  var result = dom ?? .default
  let base = result
  let createDOM = compileHook(p.createDOM) { node, _, editor in try base.createDOM(node, editor) }
  result.createDOM = { node, editor in try createDOM(node, (), editor) }
  let exportDOM = compileHook(p.exportDOM) { node, _, editor in try base.exportDOM(node, editor) }
  result.exportDOM = { node, editor in try exportDOM(node, (), editor) }
  let extractWithChild = compileHook(p.extractWithChild) { node, a, editor in
    try base.extractWithChild(node, a.0, a.1, a.2, editor)
  }
  result.extractWithChild = { node, child, selection, destination, editor in
    try extractWithChild(node, (child, selection, destination), editor)
  }
  let getDOMSlot = compileHook(p.getDOMSlot) { node, dom, editor in
    try base.getDOMSlot(node, dom, editor)
  }
  result.getDOMSlot = { node, dom, editor in try getDOMSlot(node, dom, editor) }
  let shouldExclude = compileHook(p.shouldExclude) { node, selection, editor in
    try base.shouldExclude(node, selection, editor)
  }
  result.shouldExclude = { node, selection, editor in try shouldExclude(node, selection, editor) }
  let shouldInclude = compileHook(p.shouldInclude) { node, selection, editor in
    try base.shouldInclude(node, selection, editor)
  }
  result.shouldInclude = { node, selection, editor in try shouldInclude(node, selection, editor) }
  let updateDOM = compileHook(p.updateDOM) { node, a, editor in
    try base.updateDOM(node, a.0, a.1, editor)
  }
  result.updateDOM = { next, prev, dom, editor in try updateDOM(next, (prev, dom), editor) }
  result.decorateDOM = compileDecorate(p.decorateDOM, base.decorateDOM)
  return result
}

// MARK: - The runtime (DOMRenderRuntime.ts)

private func makeReader(_ record: ContextRecord?) -> RenderContextReader {
  RenderContextReader(record: record)
}

/// `createEditorContextRecord(contextDefaults)`.
private func createEditorContextRecord(_ contextDefaults: [ContextPairOrUpdater]) -> ContextRecord {
  let parent = ContextRecord(parent: nil)
  return contextFromPairs(contextDefaults, parent) ?? parent
}

/// `filterEditorInstalled(overrides, record)`.
private func filterEditorInstalled(
  _ overrides: [DOMRenderMatch], _ record: ContextRecord
)
  -> [DOMRenderMatch]
{
  let reader = makeReader(record)
  return overrides.filter { !($0.options.disabledForEditor?(reader) ?? false) }
}

private func nodeMatcher(_ override: DOMRenderMatch) -> (LexicalNode) -> Bool {
  guard let nodes = override.nodes else { return { _ in true } }
  let matchers: [(LexicalNode) -> Bool] = nodes.map { match in
    switch match {
    case .klass(let klass): { isInstance($0, klass) }
    case .predicate(let predicate): predicate
    }
  }
  return { node in matchers.contains { $0(node) } }
}

/// `DOMRenderRuntime`: the extension's conditional overrides and the
/// editor-level render context.
public final class DOMRenderRuntime {
  let editor: Editor
  let nodes: [LexicalNodeConfig]
  let baseDOM: EditorDOMRenderConfig?
  let overrides: [DOMRenderMatch]
  /// The writable editor-level context record.
  public let editorContext: ContextRecord
  let hasSessionGates: Bool
  var installed: [DOMRenderMatch]
  private var sessionCache: [String: EditorDOMRenderConfig] = [:]

  init(
    editor: Editor, nodes: [LexicalNodeConfig], baseDOM: EditorDOMRenderConfig?,
    overrides: [DOMRenderMatch], editorContext: ContextRecord
  ) {
    self.editor = editor
    self.nodes = nodes
    self.baseDOM = baseDOM
    self.overrides = overrides
    self.editorContext = editorContext
    self.installed = filterEditorInstalled(overrides, editorContext)
    self.hasSessionGates = overrides.contains { $0.options.disabledForSession != nil }
  }

  /// `setContextValue(cfg, value)`: sets an editor-level value, and
  /// recompiles the render config when that changes which overrides are
  /// installed. JS then recreates the DOM of the affected nodes with a full
  /// reconcile; Swift runs the full reconcile, and the views recreate what
  /// they render.
  public func setContextValue<V>(_ cfg: RenderStateConfig<V>, _ value: V) throws {
    let prev = installed
    editorContext.set(cfg, value)
    let next = filterEditorInstalled(overrides, editorContext)
    if prev.count == next.count && zip(prev, next).allSatisfy({ $0 === $1 }) {
      return
    }
    let changed =
      prev.filter { p in !next.contains { $0 === p } }
      + next.filter { n in !prev.contains { $0 === n } }
    installed = next
    sessionCache.removeAll()
    editor._config.dom = try compileDOMRenderConfigOverrides(nodes, baseDOM, next)
    let recreates = changed.contains {
      $0.config.createDOM != nil || $0.config.getDOMSlot != nil || $0.config.decorateDOM != nil
    }
    if recreates {
      try editor.update(discrete: true) { fullReconcile() }
    }
  }

  /// `getSessionConfig()`: the editor's render config without the
  /// overrides the current render context disables for this session.
  public func getSessionConfig() throws -> EditorDOMRenderConfig {
    let resident = editor._config.dom ?? .default
    guard hasSessionGates else { return resident }
    let reader = makeReader(getContextRecord(.render, editor) ?? editorContext)
    var disabledKeys: [String] = []
    var sessionSet: [DOMRenderMatch] = []
    for (i, o) in installed.enumerated() {
      if o.options.disabledForSession?(reader) ?? false {
        disabledKeys.append(String(i))
      } else {
        sessionSet.append(o)
      }
    }
    if disabledKeys.isEmpty { return resident }
    let key = disabledKeys.joined(separator: ",")
    if let cfg = sessionCache[key] { return cfg }
    let cfg = try compileDOMRenderConfigOverrides(nodes, baseDOM, sessionSet)
    sessionCache[key] = cfg
    return cfg
  }
}

/// `DOMRenderExtensionOutput`.
public struct DOMRenderExtensionOutput {
  public let defaults: ContextRecord?
  public let runtime: DOMRenderRuntime
}

/// What `init` captures before replacing the editor's `dom`.
public struct DOMRenderInitResult {
  let nodes: [LexicalNodeConfig]
  let dom: EditorDOMRenderConfig?
}

let DOMRenderExtensionName = "@lexical/html/DOM"

/// `DOMRenderExtension` (`@experimental`): overrides how nodes render to
/// and export as DOM.
public let DOMRenderExtension = defineExtension(
  name: DOMRenderExtensionName, config: DOMRenderConfig(),
  mergeConfig: { config, partial in
    var merged = partial.applied(to: config)
    if let overrides = partial.overrides { merged.overrides = config.overrides + overrides }
    if let contextDefaults = partial.contextDefaults {
      merged.contextDefaults = config.contextDefaults + contextDefaults
    }
    return merged
  },
  editorConfig: {
    var editorConfig = InitialEditorConfig()
    // A RootNode export for generateDOMFromRoot.
    editorConfig.html = HTMLConfig(export: [
      (
        RootNode.self,
        { _, _ in
          let element = getDocument().createElement("div")
          element.setAttribute("role", "textbox")
          return DOMExportOutput(element: element)
        }
      )
    ])
    return editorConfig
  }(),
  init: { editorConfig, config, _ in
    // Capture the base `dom` and `nodes` before replacing `dom`, so the
    // runtime can recompile when overrides toggle.
    let initResult = DOMRenderInitResult(nodes: editorConfig.nodes ?? [], dom: editorConfig.dom)
    let installed = filterEditorInstalled(
      config.overrides, createEditorContextRecord(config.contextDefaults))
    editorConfig.dom = try compileDOMRenderConfigOverrides(
      initResult.nodes, initResult.dom, installed)
    return initResult
  },
  build: { editor, config, state in
    let initResult = try state.getInitResult()
    let editorContext = createEditorContextRecord(config.contextDefaults)
    let runtime = DOMRenderRuntime(
      editor: editor, nodes: initResult.nodes, baseDOM: initResult.dom,
      overrides: config.overrides, editorContext: editorContext)
    return DOMRenderExtensionOutput(defaults: editorContext, runtime: runtime)
  })

// MARK: - The render context (RenderContext.ts)

/// `createRenderState(name, getDefaultValue, isEqual)` (`@experimental`).
public func createRenderState<V>(
  _ name: String, _ getDefaultValue: () -> V, _ isEqual: ((V, V) -> Bool)? = nil
) -> RenderStateConfig<V> {
  createContextState(.render, name, getDefaultValue, isEqual)
}

/// `RenderContextRoot`: `true` while exporting from the root itself.
public let RenderContextRoot = createRenderState("root") { false }

/// `RenderContextExport`: `true` while exporting (rather than rendering).
public let RenderContextExport = createRenderState("isExport") { false }

private func renderOutput(_ editor: Editor) throws -> DOMRenderExtensionOutput? {
  try getPeerDependencyFromEditor(editor, DOMRenderExtensionName, as: type(of: DOMRenderExtension))?
    .output
}

private func getDefaultRenderContext(_ editor: Editor) throws -> ContextRecord? {
  try renderOutput(editor)?.defaults
}

/// `$getRenderContextValue(cfg, editor)`.
public func getRenderContextValue<V>(
  _ cfg: RenderStateConfig<V>, _ editor: Editor? = nil
) throws
  -> V
{
  let editor = try editor ?? getEditor()
  return getContextValue(
    try getContextRecord(.render, editor) ?? getDefaultRenderContext(editor), cfg)
}

/// `$setRenderContextValue(cfg, value, editor)`.
public func setRenderContextValue<V>(
  _ cfg: RenderStateConfig<V>, _ value: V, _ editor: Editor? = nil
) throws {
  let editor = try editor ?? getEditor()
  try renderOutput(editor)?.runtime.setContextValue(cfg, value)
}

/// `$updateRenderContextValue(cfg, updater, editor)`.
public func updateRenderContextValue<V>(
  _ cfg: RenderStateConfig<V>, _ updater: (V) -> V, _ editor: Editor? = nil
) throws {
  let editor = try editor ?? getEditor()
  if let runtime = try renderOutput(editor)?.runtime {
    try runtime.setContextValue(cfg, updater(getContextValue(runtime.editorContext, cfg)))
  }
}

/// `$getSessionDOMRenderConfig(editor)`: the render config for an export
/// session.
public func getSessionDOMRenderConfig(_ editor: Editor? = nil) throws -> EditorDOMRenderConfig {
  let editor = try editor ?? getEditor()
  if let runtime = try renderOutput(editor)?.runtime {
    return try runtime.getSessionConfig()
  }
  return getEditorDOMRenderConfig(editor)
}

/// `$withRenderContext(cfg, editor)(f)`: runs `f` with the render context
/// values set.
public func withRenderContext<T>(
  _ cfg: [ContextPairOrUpdater], _ editor: Editor? = nil, _ f: () throws -> T
) throws -> T {
  let editor = try editor ?? getEditor()
  return try withContext(.render, getDefaultRenderContext, cfg, editor, f)
}
