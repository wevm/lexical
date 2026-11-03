// Port of packages/lexical/src/LexicalEditor.ts (the model; the DOM root, the
// events, and the reconciler belong to the UI layers).

/// A node transform, compared by identity like the JS function.
public final class Transform {
  let fn: (LexicalNode) throws -> Void
  package init(_ fn: @escaping (LexicalNode) throws -> Void) { self.fn = fn }
}

/// `RegisteredNode`: a node class as an editor knows it.
public final class RegisteredNode {
  public let klass: LexicalNode.Type
  package let transforms: JSSet<ObjectIdentifier>
  package var transformsByID: [ObjectIdentifier: Transform] = [:]
  public let replace: ((LexicalNode) throws -> LexicalNode)?
  public let replaceWithKlass: LexicalNode.Type?
  package let sharedNodeState: SharedNodeState
  /// The HTML config's `exportDOM` override for the class.
  public var exportDOM: ((Editor, LexicalNode) throws -> DOMExportOutput)?

  init(
    klass: LexicalNode.Type, transforms: [Transform],
    replace: ((LexicalNode) throws -> LexicalNode)?, replaceWithKlass: LexicalNode.Type?,
    sharedNodeState: SharedNodeState
  ) {
    self.klass = klass
    self.transforms = JSSet()
    self.replace = replace
    self.replaceWithKlass = replaceWithKlass
    self.sharedNodeState = sharedNodeState
    for transform in transforms { addTransform(transform) }
  }

  package func addTransform(_ transform: Transform) {
    let id = ObjectIdentifier(transform)
    transforms.add(id)
    transformsByID[id] = transform
  }

  package func deleteTransform(_ transform: Transform) {
    let id = ObjectIdentifier(transform)
    transforms.delete(id)
    transformsByID[id] = nil
  }

  var transformList: [Transform] { transforms.map { transformsByID[$0]! } }
}

/// `LexicalNodeReplacement`: registers `replace` so that `$create` and the
/// `$create*` functions make `withKlass` instead.
public struct LexicalNodeReplacement {
  public var replace: LexicalNode.Type
  public var with: (LexicalNode) throws -> LexicalNode
  public var withKlass: LexicalNode.Type?

  public init(
    replace: LexicalNode.Type, with: @escaping (LexicalNode) throws -> LexicalNode,
    withKlass: LexicalNode.Type? = nil
  ) {
    self.replace = replace
    self.with = with
    self.withKlass = withKlass
  }
}

/// A node class, or a replacement: `LexicalNodeConfig`.
public enum LexicalNodeConfig {
  case klass(LexicalNode.Type)
  case replacement(LexicalNodeReplacement)

  var klass: LexicalNode.Type {
    switch self {
    case .klass(let klass): klass
    case .replacement(let replacement): replacement.replace
    }
  }
}

public typealias ErrorHandler = (Error) throws -> Void

/// The theme: class names keyed like JS `EditorThemeClasses`, which the UI
/// layers map to attributes.
public typealias EditorThemeClasses = [String: JSONValue]

/// `CreateEditorArgs`.
public struct CreateEditorArgs {
  public var editorState: EditorState?
  public var namespace: String?
  public var nodes: [LexicalNodeConfig]
  public var onError: ErrorHandler?
  public var onWarn: ErrorHandler?
  public var parentEditor: Editor?
  public var editable: Bool?
  public var theme: EditorThemeClasses?
  public var headless: Bool
  public var html: HTMLConfig?
  public var dom: EditorDOMRenderConfig?

  public init(
    editorState: EditorState? = nil, namespace: String? = nil, nodes: [LexicalNodeConfig] = [],
    onError: ErrorHandler? = nil, onWarn: ErrorHandler? = nil, parentEditor: Editor? = nil,
    editable: Bool? = nil, theme: EditorThemeClasses? = nil, headless: Bool = false,
    html: HTMLConfig? = nil, dom: EditorDOMRenderConfig? = nil
  ) {
    self.html = html
    self.dom = dom
    self.editorState = editorState
    self.namespace = namespace
    self.nodes = nodes
    self.onError = onError
    self.onWarn = onWarn
    self.parentEditor = parentEditor
    self.editable = editable
    self.theme = theme
    self.headless = headless
  }
}

/// `EditorUpdateOptions`.
public struct EditorUpdateOptions {
  public var onUpdate: (() throws -> Void)?
  public var skipTransforms: Bool
  public var tag: [UpdateTag]
  public var discrete: Bool

  public init(
    onUpdate: (() throws -> Void)? = nil, skipTransforms: Bool = false, tag: [UpdateTag] = [],
    discrete: Bool = false
  ) {
    self.onUpdate = onUpdate
    self.skipTransforms = skipTransforms
    self.tag = tag
    self.discrete = discrete
  }
}

/// `EditorReadMode`.
public enum EditorReadMode {
  /// Commit pending updates first (the default).
  case forceCommit
  /// Read the pending state, like an update.
  case pending
  /// Read the latest committed state.
  case latest
}

public typealias NodeMutation = String

/// `UpdateListenerPayload`.
public struct UpdateListenerPayload {
  public let dirtyElements: JSMap<NodeKey, Bool>
  public let dirtyLeaves: JSSet<NodeKey>
  public let editorState: EditorState
  public let mutatedNodes: MutatedNodes?
  public let normalizedNodes: JSSet<NodeKey>
  public let prevEditorState: EditorState
  public let tags: JSSet<UpdateTag>
}

public typealias MutatedNodes = JSMap<ObjectIdentifier, JSMap<NodeKey, NodeMutation>>

/// The payload of a mutation listener.
public struct MutationListenerPayload {
  public let updateTags: JSSet<UpdateTag>
  public let dirtyLeaves: JSSet<NodeKey>
  public let prevEditorState: EditorState
}

public typealias UpdateListener = (UpdateListenerPayload) throws -> Void
public typealias TextContentListener = (String) throws -> Void
public typealias EditableListener = (Bool) throws -> Void
public typealias DecoratorListener = ([NodeKey: Any]) throws -> Void
/// A clean-up a listener may return: it runs before the listener's next
/// call, or when the listener is unregistered.
public typealias ListenerCleanup = () -> Void
/// A root element that reports whether it is attached, as a DOM element's
/// `isConnected` does (for a view: whether it is in a window). A root that
/// does not conform counts as connected.
public protocol LexicalRootElement: AnyObject {
  var isConnected: Bool { get }
  /// Gives up the focus and clears the native selection, as `editor.blur()`
  /// does to the DOM root. Does nothing by default.
  func blur()
  /// Whether the root (or a view inside it) has the focus, as
  /// `root.contains(document.activeElement)` says of a DOM root. `false` by
  /// default.
  var hasFocus: Bool { get }
  /// Takes the focus, as `rootElement.focus()` does. Does nothing by
  /// default.
  func focus()
  /// The caret at a point in the root's coordinates, as
  /// `caretFromPoint(x, y, rootElement)` from `@lexical/clipboard` hit-tests
  /// the DOM; read inside an update. `nil` (the default) when no caret
  /// resolves there.
  func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret?
}

extension LexicalRootElement {
  public func blur() {}
  public var hasFocus: Bool { false }
  public func focus() {}
  public func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret? { nil }
}

/// `EditorFocusOptions`.
public struct EditorFocusOptions {
  /// `'rootStart'` or `'rootEnd'` (the default): where the selection goes
  /// when the editor is focused with no selection.
  public var defaultSelection: String?

  public init(defaultSelection: String? = nil) { self.defaultSelection = defaultSelection }
}

/// `RootListener`: the new and previous root elements; it may return a
/// clean-up for the next change.
public typealias RootListener = (_ rootElement: AnyObject?, _ prevRootElement: AnyObject?) -> (
  () -> Void
)?
public typealias MutationListener = (JSMap<NodeKey, NodeMutation>, MutationListenerPayload) throws
  -> Void
public typealias CommandListener<P> = (P, Editor) throws -> Bool

public let COMMAND_PRIORITY_EDITOR = 0
public let COMMAND_PRIORITY_LOW = 1
public let COMMAND_PRIORITY_NORMAL = 2
public let COMMAND_PRIORITY_HIGH = 3
public let COMMAND_PRIORITY_CRITICAL = 4
public let COMMAND_PRIORITY_BEFORE_EDITOR = -8
public let COMMAND_PRIORITY_BEFORE_LOW = -7
public let COMMAND_PRIORITY_BEFORE_NORMAL = -6
public let COMMAND_PRIORITY_BEFORE_HIGH = -5
public let COMMAND_PRIORITY_BEFORE_CRITICAL = -4

/// One registered listener: compared by identity, with the cleanup its last
/// call returned.
package final class ListenerEntry<Listener> {
  let listener: Listener
  var unregister: (() -> Void)?
  init(_ listener: Listener) { self.listener = listener }
}

/// A JS `Map<listener, unregister>`: listeners in registration order.
package final class ListenerMap<Listener> {
  let entries = JSMap<ObjectIdentifier, ListenerEntry<Listener>>()
  var size: Int { entries.size }

  func register(_ listener: Listener) -> () -> Void {
    let entry = ListenerEntry(listener)
    let id = ObjectIdentifier(entry)
    entries.set(id, entry)
    return { [weak self] in
      guard let self, let entry = self.entries.get(id) else { return }
      self.entries.delete(id)
      entry.unregister?()
    }
  }
}

/// `DequeSet`: a set that can add at either end, iterated front to back.
package final class DequeSet<T: AnyObject> {
  private var front: [T] = []
  private var back: [T] = []
  var size: Int { front.count + back.count }

  func addBack(_ v: T) {
    if !front.contains(where: { $0 === v }) && !back.contains(where: { $0 === v }) {
      back.append(v)
    }
  }

  func addFront(_ v: T) {
    if !back.contains(where: { $0 === v }) && !front.contains(where: { $0 === v }) {
      front.append(v)
    }
  }

  func delete(_ v: T) {
    front.removeAll { $0 === v }
    back.removeAll { $0 === v }
  }

  var array: [T] { front.reversed() + back }
}

package final class CommandListenerBox {
  let listener: (Any, Editor) throws -> Bool
  init(_ listener: @escaping (Any, Editor) throws -> Bool) { self.listener = listener }
}

/// `createEditor`: an editor with the core nodes and `args.nodes`.
public func createEditor(_ args: CreateEditorArgs = CreateEditorArgs()) throws -> Editor {
  let activeEditor = internalGetActiveEditor()
  let theme = args.theme ?? [:]
  let parentEditor = args.parentEditor
  let editorState = createEmptyEditorState()
  let namespace = args.namespace ?? parentEditor?._config.namespace ?? createUID()
  let nodes: [LexicalNodeConfig] =
    [
      .klass(RootNode.self), .klass(TextNode.self), .klass(LineBreakNode.self),
      .klass(TabNode.self), .klass(ParagraphNode.self), .klass(ArtificialNode__DO_NOT_USE.self),
    ] + args.nodes
  let isEditable = args.editable ?? true
  let registeredNodes = JSMap<String, RegisteredNode>()
  _ = activeEditor
  for nodeConfig in nodes {
    var klass = nodeConfig.klass
    var replace: ((LexicalNode) throws -> LexicalNode)? = nil
    var replaceWithKlass: LexicalNode.Type? = nil
    if case .replacement(let options) = nodeConfig {
      klass = options.replace
      replace = options.with
      replaceWithKlass = options.withKlass
    }
    if let replaceWithKlass {
      try invariant(
        isSubclass(replaceWithKlass, of: klass) && replaceWithKlass != klass,
        "\(replaceWithKlass) doesn't extend the \(klass)")
    } else if replace != nil {
      consoleWarn(
        "Override for \(klass) specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version."
      )
    }
    if let configError = getNodeClassRecord(klass).configError {
      throw configError
    }
    try validateNodeStructure(klass)
    let type = klass.getType()
    try invariant(!type.isEmpty, "LexicalNode: Node \(klass) does not implement .getType().")
    registeredNodes.set(
      type,
      RegisteredNode(
        klass: klass, transforms: getTransformSetFromKlass(klass), replace: replace,
        replaceWithKlass: replaceWithKlass, sharedNodeState: createSharedNodeState(klass)))
    registeredNodes.get(type)?.exportDOM = args.html?.export[ObjectIdentifier(klass)]
  }
  let editor = Editor(
    editorState, parentEditor, registeredNodes,
    EditorConfig(namespace: namespace, theme: theme, dom: args.dom),
    args.onError ?? { _ in },
    args.onWarn ?? { error in throw error },
    isEditable, args)
  editor._headless = args.headless || (parentEditor?._headless ?? false)
  editor._htmlConversions = LexicalNode.conversionCache(registeredNodes, args.html?.import)
  if let initialEditorState = args.editorState {
    editor._pendingEditorState = initialEditorState
    editor._dirtyType = FULL_RECONCILE
  }
  registerDefaultCommandHandlers(editor)
  return editor
}

package func isSubclass(_ klass: LexicalNode.Type, of base: LexicalNode.Type) -> Bool {
  var current: LexicalNode.Type? = klass
  while let c = current {
    if c == base { return true }
    current = getSuperclassOf(c)
  }
  return false
}

/// `EditorConfig`.
public struct EditorConfig {
  public var namespace: String
  public var theme: EditorThemeClasses
  /// `dom`: how nodes become DOM; `nil` for `EditorDOMRenderConfig.default`.
  public var dom: EditorDOMRenderConfig?

  public init(namespace: String, theme: EditorThemeClasses = [:], dom: EditorDOMRenderConfig? = nil)
  {
    self.namespace = namespace
    self.theme = theme
    self.dom = dom
  }
}

/// `HTMLConfig`: per-class overrides of HTML export.
public struct HTMLConfig {
  /// `export`: a node class's `exportDOM` replacement.
  public var export: [ObjectIdentifier: (Editor, LexicalNode) throws -> DOMExportOutput]
  /// `import`: importers added after the node classes'.
  public var `import`: DOMConversionMap?

  public init(
    export: [(LexicalNode.Type, (Editor, LexicalNode) throws -> DOMExportOutput)] = [],
    import: DOMConversionMap? = nil
  ) {
    self.export = Dictionary(
      export.map { (ObjectIdentifier($0.0), $0.1) }, uniquingKeysWith: { $1 })
    self.import = `import`
  }
}

/// The editor: owns the current EditorState, the registered nodes, and the
/// listeners and commands. Bound to the thread or actor that created it.
public final class Editor {
  public static let version = "lexical-swift@\(lexicalUpstreamVersion)"

  package var _headless = false
  /// Runs before `setRootElement`; `@lexical/headless` refuses a root with
  /// it, where JS replaces the method on the instance.
  package var _setRootElementGuard: ((AnyObject?) throws -> Void)?
  /// Where a caret moves by a character, word or line, which only text
  /// layout knows: the text view sets it. In JS this is the DOM's
  /// `Selection.modify`. When `nil`, as headless, moves within text stop
  /// where JS stops without a DOM.
  public weak var selectionMovement: (any SelectionMovement)?
  /// The node-structure registration (`INTERNAL_registerNodeStructure`),
  /// which JS keeps in a `WeakMap` keyed by editor.
  package var _nodeStructureRegistration: NodeStructureRegistration?
  package var _parentEditor: Editor?
  /// The native view the editor renders into; `nil` when headless.
  package var _rootElement: AnyObject?
  package var _editorState: EditorState
  package var _pendingEditorState: EditorState?
  package var _compositionKey: NodeKey?
  package var _compositionMergeHeld: NodeKey?
  package var _deferred: [() throws -> Void] = []
  package var _updates: [QueuedUpdate] = []
  package var _updating = false
  package var _cascadeCount = 0
  package let _updateListeners = ListenerMap<
    (UpdateListenerPayload) throws -> ListenerCleanup?
  >()
  package let _textContentListeners = ListenerMap<(String) throws -> ListenerCleanup?>()
  package let _editableListeners = ListenerMap<(Bool) throws -> ListenerCleanup?>()
  package let _decoratorListeners = ListenerMap<([NodeKey: Any]) throws -> ListenerCleanup?>()
  package let _rootListeners = ListenerMap<RootListener>()
  package let _mutationListeners = JSMap<
    ObjectIdentifier, (MutationListener, JSSet<ObjectIdentifier>)
  >()
  package var _commands: [ObjectIdentifier: [DequeSet<CommandListenerBox>]] = [:]
  package let _nodes: JSMap<String, RegisteredNode>
  /// `_htmlConversions`: the importers by DOM node name.
  public internal(set) var _htmlConversions: [String: [(DOMNode) throws -> DOMConversion?]] = [:]
  package var _decorators: [NodeKey: Any] = [:]
  package var _pendingDecorators: [NodeKey: Any]?
  /// `_keyToDOMMap`: what the reconciler keeps per node while a root is
  /// attached (see LexicalReconciler.swift).
  package let _keyToDOMMap = JSMap<NodeKey, ReconciledNode>()
  package var _config: EditorConfig
  package var _dirtyType = NO_DIRTY_NODES
  package var _cloneNotNeeded = JSMap<NodeKey, LexicalNode>()
  package var _dirtyLeaves = JSSet<NodeKey>()
  package var _dirtyElements = JSMap<NodeKey, Bool>()
  package var _normalizedNodes = JSSet<NodeKey>()
  package var _updateTags = JSSet<UpdateTag>()
  package var _inputState = InputState()
  /// `_keyDownShortcuts`: the compiled key-down bindings, for the platform
  /// they were built for.
  var _keyDownShortcuts: (isApple: Bool, shortcuts: CompiledKeyboardShortcuts<KeyDownShortcut>)?
  /// The `LexicalBuilder` that built this editor (JS `editor[builderSymbol]`).
  package var _builder: AnyObject?
  /// Undoes what the builder registered (`LexicalEditorWithDispose`).
  package var _dispose: (() -> Void)?

  /// `dispose()` of an editor built from extensions: unregisters every
  /// extension and detaches the root, once. Does nothing for other editors.
  public func dispose() {
    let dispose = _dispose
    _dispose = nil
    dispose?()
  }
  package var _key: String
  package var _onError: ErrorHandler
  package var _onWarn: ErrorHandler
  package var _editable: Bool
  package var _slotsUsed = false
  /// Whether node moves are recorded in `EditorState._movedNodes`.
  package var _recordNodeMoves = false
  package var _lastNotifiedSelection: BaseSelection?
  package var _compositionPhase = "idle"
  package var _composedSegmentedKey: NodeKey?
  package var _createEditorArgs: CreateEditorArgs?
  /// Per-editor storage for the packages built on the core (`@lexical/*`
  /// modules keep weak maps keyed by editor in JS).
  package var _extensionStorage: [String: Any] = [:]

  package typealias UpdateFn = () throws -> Void

  init(
    _ editorState: EditorState, _ parentEditor: Editor?, _ nodes: JSMap<String, RegisteredNode>,
    _ config: EditorConfig, _ onError: @escaping ErrorHandler, _ onWarn: @escaping ErrorHandler,
    _ editable: Bool, _ createEditorArgs: CreateEditorArgs?
  ) {
    self._createEditorArgs = createEditorArgs
    self._parentEditor = parentEditor
    self._editorState = editorState
    self._config = config
    self._nodes = nodes
    self._key = createUID()
    self._onError = onError
    self._onWarn = onWarn
    self._editable = editable
  }

  public func isComposing() -> Bool { _compositionKey != nil }

  // Each listener may return a clean-up, as in JS, where a listener that
  // returns a function has it called before its next call or on teardown.
  // A listener returning `Void` takes the first overload; one that returns
  // a clean-up spells its return type (`{ _ -> ListenerCleanup? in ... }`).

  @discardableResult
  public func registerUpdateListener(_ listener: @escaping UpdateListener) -> () -> Void {
    _updateListeners.register {
      try listener($0); return nil
    }
  }

  @_disfavoredOverload @discardableResult
  public func registerUpdateListener(
    _ listener: @escaping (UpdateListenerPayload) throws -> ListenerCleanup?
  ) -> () -> Void {
    _updateListeners.register(listener)
  }

  @discardableResult
  public func registerEditableListener(_ listener: @escaping EditableListener) -> () -> Void {
    _editableListeners.register {
      try listener($0); return nil
    }
  }

  @_disfavoredOverload @discardableResult
  public func registerEditableListener(
    _ listener: @escaping (Bool) throws -> ListenerCleanup?
  ) -> () -> Void {
    _editableListeners.register(listener)
  }

  @discardableResult
  public func registerDecoratorListener(_ listener: @escaping DecoratorListener) -> () -> Void {
    _decoratorListeners.register {
      try listener($0); return nil
    }
  }

  @_disfavoredOverload @discardableResult
  public func registerDecoratorListener(
    _ listener: @escaping ([NodeKey: Any]) throws -> ListenerCleanup?
  ) -> () -> Void {
    _decoratorListeners.register(listener)
  }

  @discardableResult
  public func registerTextContentListener(
    _ listener: @escaping TextContentListener
  )
    -> () -> Void
  {
    _textContentListeners.register {
      try listener($0); return nil
    }
  }

  @_disfavoredOverload @discardableResult
  public func registerTextContentListener(
    _ listener: @escaping (String) throws -> ListenerCleanup?
  ) -> () -> Void {
    _textContentListeners.register(listener)
  }

  /// Registers `listener` for `command` at `priority`; a "before" priority
  /// (`COMMAND_PRIORITY_BEFORE_*`) runs ahead of the listeners already at the
  /// matching priority.
  @discardableResult
  public func registerCommand<P>(
    _ command: LexicalCommand<P>, _ listener: @escaping CommandListener<P>, _ priority: Int
  ) -> () -> Void {
    let id = ObjectIdentifier(command)
    if _commands[id] == nil {
      _commands[id] = (0..<5).map { _ in DequeSet() }
    }
    let normalizedPriority = priority & 7
    let listeners = _commands[id]![normalizedPriority]
    let box = CommandListenerBox { payload, editor in try listener(payload as! P, editor) }
    if normalizedPriority != priority {
      listeners.addFront(box)
    } else {
      listeners.addBack(box)
    }
    return { [weak self] in
      listeners.delete(box)
      if let self, let sets = self._commands[id], sets.allSatisfy({ $0.size == 0 }) {
        self._commands[id] = nil
      }
    }
  }

  /// Registers `listener` for the creation, update, and destruction of nodes
  /// of `klass` (or its replacement), reported after each reconcile.
  @discardableResult
  public func registerMutationListener(
    _ klass: LexicalNode.Type, _ listener: @escaping MutationListener,
    skipInitialization: Bool = false
  ) throws -> () -> Void {
    let klassToMutate = try resolveRegisteredNodeAfterReplacements(getRegisteredNode(klass)).klass
    // Swift closures have no identity, so each registration is its own
    // entry, keyed by its class set, which the map keeps alive. (A key from
    // an object only the teardown holds could be reused once that teardown
    // is discarded.)
    let klassSet = JSSet<ObjectIdentifier>()
    let id = ObjectIdentifier(klassSet)
    klassSet.add(ObjectIdentifier(klassToMutate))
    _mutationListeners.set(id, (listener, klassSet))
    if !skipInitialization {
      try initializeMutationListener(listener, klassToMutate)
    }
    return { [weak self] in
      klassSet.delete(ObjectIdentifier(klassToMutate))
      if klassSet.size == 0 {
        self?._mutationListeners.delete(id)
      }
    }
  }

  public func getRegisteredNode(_ klass: LexicalNode.Type) throws -> RegisteredNode {
    guard let registeredNode = _nodes.get(klass.getType()) else {
      throw LexicalError(
        "Node \(klass) has not been registered. Ensure node has been passed to createEditor.")
    }
    return registeredNode
  }

  public func resolveRegisteredNodeAfterReplacements(
    _ registeredNode: RegisteredNode
  ) throws
    -> RegisteredNode
  {
    var registeredNode = registeredNode
    while let replaceWithKlass = registeredNode.replaceWithKlass {
      registeredNode = try getRegisteredNode(replaceWithKlass)
    }
    return registeredNode
  }

  private func initializeMutationListener(
    _ listener: MutationListener, _ klass: LexicalNode.Type
  ) throws {
    let prevEditorState = _editorState
    guard let nodeMap = try getCachedTypeToNodeMap(prevEditorState).get(klass.getType()) else {
      return
    }
    let nodeMutationMap = JSMap<NodeKey, NodeMutation>()
    for (k, _) in nodeMap {
      nodeMutationMap.set(k, "created")
    }
    if nodeMutationMap.size > 0 {
      try listener(
        nodeMutationMap,
        MutationListenerPayload(
          updateTags: JSSet(["registerMutationListener"]), dirtyLeaves: JSSet(),
          prevEditorState: prevEditorState))
    }
  }

  /// Registers `listener` to run on every dirty node of `klass` (or its
  /// replacement) during each update, until the tree stops changing.
  @discardableResult
  public func registerNodeTransform<T: LexicalNode>(
    _ klass: T.Type, _ listener: @escaping (T) throws -> Void
  ) throws -> () -> Void {
    let transform = Transform { try listener($0 as! T) }
    let registeredNode = try getRegisteredNode(klass)
    registeredNode.addTransform(transform)
    var registeredNodes = [registeredNode]
    if let replaceWithKlass = registeredNode.replaceWithKlass {
      let registeredReplaceWithNode = try getRegisteredNode(replaceWithKlass)
      registeredReplaceWithNode.addTransform(transform)
      registeredNodes.append(registeredReplaceWithNode)
    }
    try markNodesWithTypesAsDirty(self, registeredNodes.map { $0.klass.getType() })
    return {
      for node in registeredNodes {
        node.deleteTransform(transform)
      }
    }
  }

  public func hasNode(_ node: LexicalNode.Type) -> Bool {
    _nodes.has(node.getType())
  }

  public func hasNodes(_ nodes: [LexicalNode.Type]) -> Bool {
    nodes.allSatisfy(hasNode)
  }

  /// Dispatches `command` to its listeners by priority, until one returns
  /// `true`.
  @discardableResult
  public func dispatchCommand<P>(_ command: LexicalCommand<P>, _ payload: P) throws -> Bool {
    try Lexical.dispatchCommand(self, command, payload)
  }

  @discardableResult
  public func dispatchCommand(_ command: LexicalCommand<Void>) throws -> Bool {
    try Lexical.dispatchCommand(self, command, ())
  }

  public func getDecorators() -> [NodeKey: Any] { _decorators }

  public func getRootElement() -> AnyObject? { _rootElement }

  /// `blur()`: the root gives up the focus and its native selection.
  public func blur() {
    (_rootElement as? LexicalRootElement)?.blur()
  }

  /// `focus(callbackFn, options)`: with a root element, marks the selection
  /// dirty so it is applied to the view again, or selects the start or end
  /// of the root when there is none, tagging the update `focus`; the
  /// callback runs after it commits. (JS also turns off `autocapitalize`
  /// on the DOM root around the update, which the views have no need of.)
  public func focus(
    _ callbackFn: (() throws -> Void)? = nil, _ options: EditorFocusOptions = EditorFocusOptions()
  ) throws {
    guard _rootElement != nil else { return }
    try updateEditorSync(self) {
      let selection = try getSelection()
      let root = try getRoot()
      if let selection {
        // Marking the selection dirty will force the selection back to it.
        if !selection.dirty {
          try setSelection(selection.clone())
        }
      } else if try root.getChildrenSize() != 0 {
        if options.defaultSelection == "rootStart" {
          _ = try root.selectStart()
        } else {
          _ = try root.selectEnd()
        }
      }
      try addUpdateTag(FOCUS_TAG)
      try onUpdate {
        try callbackFn?()
      }
    }
  }

  /// Attaches the editor to `nextRootElement`, the native view it renders
  /// into, or detaches it with `nil`. As in JS, the editor is reset and the
  /// pending state (or the current one) is committed again, keeping queued
  /// updates and their tags; root listeners then run.
  public func setRootElement(_ nextRootElement: AnyObject?) throws {
    try _setRootElementGuard?(nextRootElement)
    let prevRootElement = _rootElement
    if nextRootElement === prevRootElement {
      return
    }
    let pendingEditorState = _pendingEditorState ?? _editorState
    _rootElement = nextRootElement
    resetEditor(self, pendingEditorState, preserveUpdateQueue: true)
    if nextRootElement != nil {
      _dirtyType = FULL_RECONCILE
    }
    _updateTags.add(HISTORY_MERGE_TAG)
    try commitPendingUpdates(self)
    let previouslyUpdating = _updating
    _updating = false
    defer { _updating = previouslyUpdating }
    for (id, entry) in Array(_rootListeners.entries) {
      if let unregister = entry.unregister {
        entry.unregister = nil
        unregister()
      }
      let nextUnregister = entry.listener(nextRootElement, prevRootElement)
      if _rootListeners.entries.has(id) {
        entry.unregister = nextUnregister
      } else {
        nextUnregister?()
      }
    }
  }

  /// Registers `listener` for root element changes. It runs at once with
  /// the current root, and with `nil` when unregistered; a clean-up it
  /// returns runs before its next call.
  @discardableResult
  public func registerRootListener(_ listener: @escaping RootListener) -> () -> Void {
    let entry = ListenerEntry(listener)
    entry.unregister = listener(_rootElement, nil)
    let id = ObjectIdentifier(entry)
    _rootListeners.entries.set(id, entry)
    return { [weak self] in
      guard let self, self._rootListeners.entries.has(id) else { return }
      entry.unregister?()
      entry.unregister = listener(nil, self._rootElement)
      self._rootListeners.entries.delete(id)
      entry.unregister?()
      entry.unregister = nil
    }
  }

  public func getKey() -> String { _key }

  public func getEditorState() -> EditorState { _editorState }

  /// Replaces the editor state, committing it at once (unless inside an
  /// update).
  public func setEditorState(_ editorState: EditorState, tag: UpdateTag? = nil) throws {
    let isEmptyEditorState = editorState.isEmpty()
    var writableEditorState = editorState
    if writableEditorState._readOnly {
      writableEditorState = cloneEditorState(editorState)
      writableEditorState._selection = editorState._selection?.clone()
    }
    if let pendingEditorState = _pendingEditorState, !pendingEditorState.isEmpty() {
      if let tag {
        _updateTags.add(tag)
      }
      try commitPendingUpdates(self)
    }
    _pendingEditorState = writableEditorState
    _dirtyType = FULL_RECONCILE
    _dirtyElements.set("root", false)
    _compositionKey = nil
    _slotsUsed = _slotsUsed || editorState._slotsUsed
    let parsedState = writableEditorState
    // Always with options, as JS passes `{discrete: undefined}` while
    // updating, so this never runs inline in the current update.
    try updateEditorSync(
      self,
      { [unowned self] in
        if let tag {
          self._updateTags.add(tag)
        }
        if isEmptyEditorState {
          try devInvariant(
            false,
            "setEditorState: the editor state is empty. Ensure the editor state's root node never becomes empty."
          )
          try getRoot().append(createParagraphNode())
        }
        if editorState._parsed {
          for (key, node) in parsedState._nodeMap {
            if node is ElementNode {
              self._dirtyElements.set(key, true)
            } else {
              self._dirtyLeaves.add(key)
            }
          }
        }
      }, EditorUpdateOptions(discrete: !_updating))
  }

  /// Parses a serialized editor state, in either form.
  public func parseEditorState(
    _ serialized: String, _ updateFn: (() throws -> Void)? = nil
  ) throws
    -> EditorState
  {
    guard case .object(let object) = try JSONValue.parse(serialized) else {
      throw LexicalError("parseEditorState: expected a JSON object")
    }
    return try parseEditorState(object, updateFn)
  }

  public func parseEditorState(
    _ serialized: JSONObject, _ updateFn: (() throws -> Void)? = nil
  )
    throws -> EditorState
  {
    try Lexical.parseEditorState(serialized, self, updateFn)
  }

  /// Reads the editor state in `callback`, committing pending updates
  /// first.
  public func read<T>(_ callback: () throws -> T) throws -> T {
    try read(.forceCommit, callback)
  }

  /// Reads the editor state in `callback`, as `mode` says.
  public func read<T>(_ mode: EditorReadMode, _ callback: () throws -> T) throws -> T {
    if mode == .forceCommit {
      if !isDispatchingCommitListeners(self) {
        try commitPendingUpdates(self)
      } else if _pendingEditorState != nil {
        consoleWarn(
          "editor.read() was called while the editor was notifying listeners "
            + "of a commit, and a listener had started another update. That "
            + "update was not committed early, so that listeners observe "
            + "commits in order, and the read observed the state being "
            + "committed. Inside listeners, read with "
            + "editor.getEditorState().read(fn, {editor}) or "
            + "editor.read('latest', fn) instead.")
      }
    }
    let editorState = mode == .pending ? (_pendingEditorState ?? _editorState) : getEditorState()
    return try editorState.read(editor: self, callback)
  }

  /// Runs `updateFn` in an update: the `$functions` and node mutations act
  /// on a pending state, which is committed in a microtask (or at once with
  /// `discrete`).
  public func update(
    _ updateFn: @escaping () throws -> Void, _ options: EditorUpdateOptions? = nil
  ) throws {
    try updateEditor(self, updateFn, options)
  }

  public func update(
    tag: UpdateTag..., discrete: Bool = false, skipTransforms: Bool = false,
    onUpdate: (() throws -> Void)? = nil, _ updateFn: @escaping () throws -> Void
  ) throws {
    try updateEditor(
      self, updateFn,
      EditorUpdateOptions(
        onUpdate: onUpdate, skipTransforms: skipTransforms, tag: tag, discrete: discrete))
  }

  public func isEditable() -> Bool { _editable }

  public func setEditable(_ editable: Bool) throws {
    if _editable != editable {
      _editable = editable
      try triggerEditableListeners(self, editable)
      if _slotsUsed {
        try update({ fullReconcile() })
      }
    }
  }

  /// `editor.toJSON()`: `{editorState}`, in the form of the export in
  /// progress.
  public func toJSON() throws -> JSONObject {
    JSONObject([("editorState", .object(try _editorState.toJSON(isCompactExport())))])
  }
}

/// `ArtificialNode__DO_NOT_USE`: an element used internally to collect nodes
/// while importing HTML.
public final class ArtificialNode__DO_NOT_USE: ElementNode {
  public override class func config() -> NodeConfig? {
    NodeConfig(ArtificialNode__DO_NOT_USE.self, type: "artificial", extends: ElementNode.self)
  }

  public override func createDOM(
    _ config: EditorConfig, _ editor: Editor? = nil
  ) throws
    -> DOMElement
  {
    getDocument().createElement("div")
  }
}

/// `resetEditor`: an empty current state with `pendingEditorState` pending,
/// and no dirty nodes; queued updates and their tags go too unless
/// `preserveUpdateQueue`. The views own the DOM half of the JS function.
package func resetEditor(
  _ editor: Editor, _ pendingEditorState: EditorState, preserveUpdateQueue: Bool = false
) {
  editor._keyToDOMMap.clear()
  if editor._rootElement != nil {
    editor._keyToDOMMap.set("root", ReconciledNode())
  }
  editor._editorState = createEmptyEditorState()
  editor._pendingEditorState = pendingEditorState
  editor._compositionKey = nil
  editor._dirtyType = NO_DIRTY_NODES
  editor._cloneNotNeeded.clear()
  editor._dirtyLeaves = JSSet()
  editor._dirtyElements.clear()
  editor._normalizedNodes = JSSet()
  if !preserveUpdateQueue {
    editor._updateTags = JSSet()
    editor._updates = []
    editor._cascadeCount = 0
  }
}
