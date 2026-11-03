// Port of packages/lexical/src/LexicalEditor.ts (the model; the DOM root, the
// events, and the reconciler belong to the UI layers).

/// A node transform, compared by identity like the JS function.
public final class Transform {
  let fn: (LexicalNode) throws -> Void
  init(_ fn: @escaping (LexicalNode) throws -> Void) { self.fn = fn }
}

/// `RegisteredNode`: a node class as an editor knows it.
public final class RegisteredNode {
  public let klass: LexicalNode.Type
  package let transforms: JSSet<ObjectIdentifier>
  package var transformsByID: [ObjectIdentifier: Transform] = [:]
  public let replace: ((LexicalNode) throws -> LexicalNode)?
  public let replaceWithKlass: LexicalNode.Type?
  package let sharedNodeState: SharedNodeState

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

  func addTransform(_ transform: Transform) {
    let id = ObjectIdentifier(transform)
    transforms.add(id)
    transformsByID[id] = transform
  }

  func deleteTransform(_ transform: Transform) {
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

  public init(
    editorState: EditorState? = nil, namespace: String? = nil, nodes: [LexicalNodeConfig] = [],
    onError: ErrorHandler? = nil, onWarn: ErrorHandler? = nil, parentEditor: Editor? = nil,
    editable: Bool? = nil, theme: EditorThemeClasses? = nil, headless: Bool = false
  ) {
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
    }
    let type = klass.getType()
    try invariant(!type.isEmpty, "LexicalNode: Node \(klass) does not implement .getType().")
    registeredNodes.set(
      type,
      RegisteredNode(
        klass: klass, transforms: getTransformSetFromKlass(klass), replace: replace,
        replaceWithKlass: replaceWithKlass, sharedNodeState: createSharedNodeState(klass)))
  }
  let editor = Editor(
    editorState, parentEditor, registeredNodes,
    EditorConfig(namespace: namespace, theme: theme),
    args.onError ?? { _ in },
    args.onWarn ?? { error in throw error },
    isEditable, args)
  editor._headless = args.headless || (parentEditor?._headless ?? false)
  if let initialEditorState = args.editorState {
    editor._pendingEditorState = initialEditorState
    editor._dirtyType = FULL_RECONCILE
  }
  return editor
}

func isSubclass(_ klass: LexicalNode.Type, of base: LexicalNode.Type) -> Bool {
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
}

/// The editor: owns the current EditorState, the registered nodes, and the
/// listeners and commands. Bound to the thread or actor that created it.
public final class Editor {
  public static let version = "lexical-swift@\(lexicalUpstreamVersion)"

  package var _headless = false
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
  package let _updateListeners = ListenerMap<UpdateListener>()
  package let _textContentListeners = ListenerMap<TextContentListener>()
  package let _editableListeners = ListenerMap<EditableListener>()
  package let _decoratorListeners = ListenerMap<DecoratorListener>()
  package let _mutationListeners = JSMap<
    ObjectIdentifier, (MutationListener, JSSet<ObjectIdentifier>)
  >()
  package var _commands: [ObjectIdentifier: [DequeSet<CommandListenerBox>]] = [:]
  package let _nodes: JSMap<String, RegisteredNode>
  package var _decorators: [NodeKey: Any] = [:]
  package var _pendingDecorators: [NodeKey: Any]?
  package var _config: EditorConfig
  package var _dirtyType = NO_DIRTY_NODES
  package var _cloneNotNeeded = JSMap<NodeKey, LexicalNode>()
  package var _dirtyLeaves = JSSet<NodeKey>()
  package var _dirtyElements = JSMap<NodeKey, Bool>()
  package var _normalizedNodes = JSSet<NodeKey>()
  package var _updateTags = JSSet<UpdateTag>()
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

  @discardableResult
  public func registerUpdateListener(_ listener: @escaping UpdateListener) -> () -> Void {
    _updateListeners.register(listener)
  }

  @discardableResult
  public func registerEditableListener(_ listener: @escaping EditableListener) -> () -> Void {
    _editableListeners.register(listener)
  }

  @discardableResult
  public func registerDecoratorListener(_ listener: @escaping DecoratorListener) -> () -> Void {
    _decoratorListeners.register(listener)
  }

  @discardableResult
  public func registerTextContentListener(
    _ listener: @escaping TextContentListener
  )
    -> () -> Void
  {
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
    let box = ListenerEntry(listener)
    let id = ObjectIdentifier(box)
    let klassSet = JSSet<ObjectIdentifier>()
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
      _ = box
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
      }, _updating ? nil : EditorUpdateOptions(discrete: true))
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

  /// Reads the editor state in `callback`. By default, pending updates are
  /// committed first.
  public func read<T>(_ mode: EditorReadMode = .forceCommit, _ callback: () throws -> T) throws -> T
  {
    if mode == .forceCommit {
      if !isDispatchingCommitListeners(self) {
        try commitPendingUpdates(self)
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
}
