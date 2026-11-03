import Lexical

// Port of packages/lexical-extension/src/BlockIdExtension.ts. The positional
// ids are in PositionalBlockId.swift and the handles in BlockHandles.swift.

/// The most candidates an ambiguous `BlockHandleResult` lists, so that a
/// handle matching many blocks (of ids that are not random) stays short.
private let MAX_HANDLE_CANDIDATES = 10

/// Attempts to find a valid, unused id before giving up, which only a broken
/// `generateId` can exhaust.
private let MAX_ID_ATTEMPTS = 32

/// Tag of the follow-up update that repairs ids after a commit applied with
/// `skipTransforms` or tagged `COLLABORATION_TAG` or `HISTORIC_TAG`.
private let BLOCK_ID_REPAIR_TAG: UpdateTag = "block-id-repair"

/// `BLOCK_ID_PATTERN` (`/^[A-Za-z0-9_-]{1,64}$/`): the ids accepted from
/// JSON, HTML, another replica, or `generateId`.
private func matchesBlockIdPattern(_ value: String) -> Bool {
  let units = value.utf16
  return (1...64).contains(units.count)
    && units.allSatisfy { c in
      (c >= 0x41 && c <= 0x5A) || (c >= 0x61 && c <= 0x7A) || (c >= 0x30 && c <= 0x39)
        || c == 0x5F || c == 0x2D
    }
}

private func parseBlockId(_ value: String?) -> String? {
  guard let value, matchesBlockIdPattern(value) else { return nil }
  return value
}

/// `blockIdState` (`@experimental`): the NodeState that stores a block's id,
/// serialized in node JSON as `"$": {"blockId": "..."}`.
///
/// It is ordinary NodeState, so it survives clones and undo, round-trips
/// through `exportJSON`/`importJSON`, and is carried by anything that copies
/// NodeState. It is reset by `copyNode`, so a node split off another one
/// starts without an id instead of with a duplicate.
///
/// A valid id is 1 to 64 characters of the base64url alphabet (`A`–`Z`,
/// `a`–`z`, `0`–`9`, `-`, `_`). Anything else reads as "no id", so the block
/// gets a new one as if it had none.
nonisolated(unsafe) public let blockIdState: StateConfig<String?> = createState(
  "blockId", parse: { parseBlockId($0?.stringValue) }, resetOnCopyNode: true)

/// `$getBlockId(node)` (`@experimental`): the id stored on `node`, or `nil`
/// if it has none.
///
/// This reads the NodeState directly, so it also returns an id left on a
/// node that is not an identified block; such ids are not indexed and
/// `getBlockById` does not find them.
public func getBlockId(_ node: LexicalNode) throws -> String? {
  // A value set directly with setState is not parsed: an invalid one reads
  // as no id here too, so the block gets a valid one.
  parseBlockId(try getState(node, blockIdState))
}

/// `$getBlockById(id)` (`@experimental`): the identified block with the
/// given id in the active editor state, or `nil`. Requires
/// `BlockIdExtension`.
///
/// The lookup goes through an index kept up to date from the dirty nodes of
/// each commit and is verified against the active state, so it never
/// returns a stale node. A block created, replaced or re-keyed earlier in
/// the same update is found once that update has committed. The match is
/// exact: a prefix of an id finds nothing.
public func getBlockById(_ id: String) throws -> LexicalNode? {
  try getExtensionOutput(BlockIdExtension).getBlockById(id)
}

/// `BlockHandleResult` (`@experimental`): what `resolveBlockHandle` found
/// for a handle.
///
/// - `found`: the one identified block that the handle names.
/// - `notFound`: it names no identified block.
/// - `ambiguous`: it names several. `candidates` holds their handles
///   (current ones, or with `read` the read's), at most 10.
/// - `invalid`: neither a handle nor an id.
public enum BlockHandleResult {
  /// Why a handle is `invalid`.
  public enum InvalidReason: String, Sendable {
    /// Fewer than `MIN_BLOCK_HANDLE_LENGTH` characters.
    case tooShort = "too-short"
    /// Not 1 to 64 characters of the base64url alphabet.
    case malformed
  }
  case found(LexicalNode)
  case notFound(handle: String)
  case ambiguous(handle: String, candidates: [String])
  case invalid(handle: String, reason: InvalidReason)

  /// The JS `status`: `'found'`, `'not-found'`, `'ambiguous'` or
  /// `'invalid'`.
  public var status: String {
    switch self {
    case .found: "found"
    case .notFound: "not-found"
    case .ambiguous: "ambiguous"
    case .invalid: "invalid"
    }
  }

  /// The block, when `found`.
  public var node: LexicalNode? {
    if case .found(let node) = self { return node }
    return nil
  }
}

/// `$getBlockHandle(node)` (`@experimental`): the handle of an identified
/// block, a short, opaque name for it in the current document: the shortest
/// prefix, at least `MIN_BLOCK_HANDLE_LENGTH` characters long, of a 64-bit
/// hash of the block's id that no other identified block's hash starts
/// with. `nil` for a node that has no id or that `getBlockById` would not
/// find by its id. Requires `BlockIdExtension`.
///
/// Handles are computed all at once from the blocks of the last committed
/// state: from the index while the extension is enabled (cached until the
/// ids change), and from a scan of that state while it is disabled (cached
/// until the next commit).
public func getBlockHandle(_ node: LexicalNode) throws -> String? {
  try getExtensionOutput(BlockIdExtension).getBlockHandle(node)
}

/// `$getBlockHandles()` (`@experimental`): the handle of every identified
/// block, by id. The map is a new one on each call. Requires
/// `BlockIdExtension`.
public func getBlockHandles() throws -> JSMap<String, String> {
  try getExtensionOutput(BlockIdExtension).getBlockHandles()
}

/// `$resolveBlockHandle(handle, options)` (`@experimental`): the identified
/// block that a handle from `getBlockHandle` names, for input that cannot be
/// trusted: a mistyped, outdated or invalid handle is reported in a
/// `BlockHandleResult` instead of thrown. A string longer than 11
/// characters is taken as a whole id. Pass the handles the model was shown
/// as `BlockHandleOptions.read` whenever the document can change between
/// the read and the edit. Requires `BlockIdExtension`.
public func resolveBlockHandle(
  _ handle: String, _ options: BlockHandleOptions? = nil
) throws -> BlockHandleResult {
  try getExtensionOutput(BlockIdExtension).resolveBlockHandle(handle, options)
}

/// `BlockHandleOptions` (`@experimental`): options of
/// `resolveBlockHandle`.
public struct BlockHandleOptions {
  /// The handles the model was shown, as `getBlockHandles` returned them at
  /// the read (id → handle). With it, a handle names the block it named in
  /// that read, and is `found` only while that block still exists.
  public var read: JSMap<String, String>?
  public init(read: JSMap<String, String>? = nil) { self.read = read }
}

/// `$isRootChildBlock(node)` (`@experimental`): a `isIdentifiedBlock` that
/// identifies every element and decorator child of the root, and nothing
/// nested.
public func isRootChildBlock(_ node: LexicalNode) throws -> Bool {
  try (node is ElementNode || node is DecoratorNode) && node.getParent() is RootNode
}

/// Whether a class declares that it holds text (`children` allows inline).
private func declaresInlineChildren(_ node: LexicalNode) -> Bool {
  switch getDeclaredNodeStructure(type(of: node)).children {
  case .inline?: true
  case .list(let entries)?: entries.contains(.inline)
  default: false
  }
}

/// `$defaultIsIdentifiedBlock(node)` (`@experimental`): the default
/// `isIdentifiedBlock`:
///
/// - every element and decorator child of the root;
/// - every nested block that holds text: a non-inline element whose class
///   declares `children: .inline` in `config()` (or a list that includes
///   `.inline`);
/// - every nested non-inline decorator.
///
/// Nested wrappers get no id: shadow roots and nested elements whose class
/// declares no `children`. A slot is nested like a shadow root.
public func defaultIsIdentifiedBlock(_ node: LexicalNode) throws -> Bool {
  guard node is ElementNode || node is DecoratorNode else {
    return false
  }
  let parent = try node.getParent()
  if parent is RootNode {
    return true
  }
  if try (parent == nil && getSlotHost(node) == nil) || node.isInline() {
    return false
  }
  guard let element = node as? ElementNode else {
    // A block decorator.
    return true
  }
  return !element.isShadowRoot() && declaresInlineChildren(node)
}

/// `generateBlockId()` (`@experimental`): the default
/// `BlockIdConfig.generateId`: 21 characters of the base64url alphabet (126
/// random bits), from the system's cryptographically secure generator.
public func generateBlockId() -> String {
  var generator = SystemRandomNumberGenerator()
  var id = ""
  for _ in 0..<ID_LENGTH {
    let byte = UInt8.random(in: 0...255, using: &generator)
    id.append(ID_ALPHABET[Int(byte & 63)])
  }
  return id
}

/// `BlockIdConfig` (`@experimental`).
public struct BlockIdConfig {
  /// Set to `true` to stop assigning, repairing, and indexing ids. Ids
  /// already stored on nodes are kept. While disabled, `getBlockById` falls
  /// back to a scan of the document.
  public var disabled: Bool
  /// `$isIdentifiedBlock`: which nodes get an id. Evaluated when a node is
  /// marked dirty, so it should depend only on the node and its ancestors.
  /// Default: `defaultIsIdentifiedBlock`.
  public var isIdentifiedBlock: (LexicalNode) throws -> Bool
  /// Makes a new id: 1 to 64 characters of the base64url alphabet. A value
  /// that is invalid or already used in this editor is discarded and
  /// `generateId` is called again; 32 such values in a row throw. Default:
  /// `generateBlockId`.
  public var generateId: () throws -> String
  /// Whether the follow-up update that names the blocks an update applied
  /// with `skipTransforms`, or tagged `COLLABORATION_TAG` or `HISTORIC_TAG`,
  /// left without a valid id or with a repeated one runs the editor's
  /// transforms (`true`, the default), or only sets their ids (`false`).
  public var repairTransforms: Bool

  public init(
    disabled: Bool = false,
    isIdentifiedBlock: @escaping (LexicalNode) throws -> Bool = defaultIsIdentifiedBlock,
    generateId: @escaping () throws -> String = generateBlockId, repairTransforms: Bool = true
  ) {
    self.disabled = disabled
    self.isIdentifiedBlock = isIdentifiedBlock
    self.generateId = generateId
    self.repairTransforms = repairTransforms
  }
}

/// `BlockIdOutput` (`@experimental`): the output of `BlockIdExtension`.
public struct BlockIdOutput {
  /// See `BlockIdConfig.disabled`.
  public let disabled: Signal<Bool>
  /// The same as `getBlockById` for this editor.
  public let getBlockById: (String) throws -> LexicalNode?
  /// The same as `getBlockHandle` for this editor.
  public let getBlockHandle: (LexicalNode) throws -> String?
  /// The same as `getBlockHandles` for this editor.
  public let getBlockHandles: () throws -> JSMap<String, String>
  /// The same as `resolveBlockHandle` for this editor.
  public let resolveBlockHandle: (String, BlockHandleOptions?) throws -> BlockHandleResult
  /// The configured `BlockIdConfig.isIdentifiedBlock`.
  public let isIdentifiedBlock: (LexicalNode) throws -> Bool
}

/// The keys of the blocks that carry an id: one, or a set only while a
/// duplicate awaits repair.
enum HolderKeys {
  case one(NodeKey)
  case many(JSSet<NodeKey>)
}

/// An index of the identified blocks of the committed editor state: id → the
/// key of the block that carries it, and key → id.
final class BlockIdIndex {
  let keysOfId = JSMap<String, HolderKeys>()
  let idOfKey = JSMap<NodeKey, String>()
  /// Changes whenever the set of indexed ids may have changed.
  var version = 0

  func clear() {
    keysOfId.clear()
    idOfKey.clear()
    version += 1
  }

  func holders(_ id: String) -> [NodeKey] {
    switch keysOfId.get(id) {
    case nil: []
    case .one(let key)?: [key]
    case .many(let keys)?: keys.array
    }
  }

  func isRepeated(_ id: String) -> Bool {
    if case .many? = keysOfId.get(id) { return true }
    return false
  }

  func set(_ key: NodeKey, _ id: String) {
    if idOfKey.get(key) != id {
      remove(key)
      version += 1
      idOfKey.set(key, id)
      switch keysOfId.get(id) {
      case nil: keysOfId.set(id, .one(key))
      case .one(let other)?: keysOfId.set(id, .many(JSSet([other, key])))
      case .many(let keys)?: keys.add(key)
      }
    }
  }

  func remove(_ key: NodeKey) {
    guard let id = idOfKey.get(key) else { return }
    version += 1
    _ = idOfKey.delete(key)
    switch keysOfId.get(id) {
    case .one?, nil:
      _ = keysOfId.delete(id)
    case .many(let keys)?:
      _ = keys.delete(key)
      if keys.size == 1 {
        keysOfId.set(id, .one(keys.array[0]))
      }
    }
  }
}

/// The handles of the blocks `getBlockById` sees, and how to find the live
/// block of an id among them.
struct HandleView {
  let table: HandleTable
  let lookup: (String) throws -> LexicalNode?
}

/// A generated id given to a block created in the same update, and the id
/// it gave up for it, if it was a copy that yielded to the block it was
/// copied from.
private struct ProvisionalId {
  let id: String
  let yielded: String?
}

/// `BlockIdRuntime`: per-editor state, the init result of
/// `BlockIdExtension` (opaque outside this package).
package final class BlockIdRuntime {
  let config: BlockIdConfig
  let index = BlockIdIndex()
  /// Whether `index` is being maintained (the extension is enabled).
  var active = false
  /// Whether a committed state with content has been seen since the
  /// extension was registered. Until then, updates load content, and get
  /// positional ids.
  var hasContent = false
  /// The ids of the blocks that `placedState` leaves unnamed.
  var placed = JSMap<NodeKey, String>()
  var placedState: EditorState?
  /// What earlier transform passes of the update that is building
  /// `provisionalState` decided.
  fileprivate let provisional = JSMap<NodeKey, ProvisionalId>()
  /// The blocks named by any pass of that update.
  let provisionalSeen = JSSet<NodeKey>()
  var provisionalState: EditorState?
  /// The handles of the indexed ids, for the index version they were built
  /// at.
  var handles: (view: HandleView, version: Int)?
  /// The scan of the last committed state for handles while disabled.
  var scan: (view: HandleView, state: EditorState)?
  /// How many handle tables were built, for the tests of the cache (JS
  /// spies on `buildHandleTable`).
  package var handleTableBuilds = 0

  init(config: BlockIdConfig) {
    self.config = config
  }

  func buildTable<S: Sequence<String>>(_ ids: S) -> HandleTable {
    handleTableBuilds += 1
    return buildHandleTable(ids)
  }

  /// `runtime.$getBlockById`.
  func getBlockById(_ id: String) throws -> LexicalNode? {
    var found: LexicalNode?
    if !active {
      // Not indexed while disabled: the first holder in document order.
      try walk(()) { node, _ in
        if try found == nil && config.isIdentifiedBlock(node) && getBlockId(node) == id {
          found = node
        }
      }
      return found
    }
    // Several holders only until the repair update: the one it keeps.
    for node in try liveHolders(self, id) {
      if try found == nil || compareDocumentOrder(node, found!) < 0 {
        found = node
      }
    }
    return found
  }
}

/// Visits every node below the root except text, in document order (a
/// host's slot values, in canonical slot order, before its children). Each
/// visit returns the context passed to the visits of the node's contents.
private func walk<T>(_ context: T, _ visit: (LexicalNode, T) throws -> T) throws {
  var stack: [(LexicalNode, T)] = []
  func pushContents(_ node: LexicalNode, _ inner: T) throws {
    // Pushed in reverse, so that they pop in document order.
    if let element = node as? ElementNode {
      var child = try element.getLastChild()
      while let current = child {
        if !(current is TextNode) {
          stack.append((current, inner))
        }
        child = try current.getPreviousSibling()
      }
    }
    if isSlotHost(node) {
      let names = try getSlotNames(node)
      for name in names.reversed() {
        if let slot = try getSlot(node, name) {
          stack.append((slot, inner))
        }
      }
    }
  }
  try pushContents(getRoot(), context)
  while let (node, outer) = stack.popLast() {
    try pushContents(node, visit(node, outer))
  }
}

/// An identified block's place among its parent frame's blocks, and a
/// counter for the blocks inside it. The root's frame has no parent.
private final class Frame {
  let parent: Frame?
  let index: Int
  var next = 0
  init(parent: Frame?, index: Int) {
    self.parent = parent
    self.index = index
  }
}

/// Positional ids for the blocks without an id that `isTarget` selects, the
/// same on every read and every replica: SHA-256 of the document key and the
/// block's place, its path among identified blocks. A taken id is retried
/// with `#n` appended to the place. This walks and hashes the whole
/// document.
private func placeBlocks(
  _ runtime: BlockIdRuntime, _ isTarget: (LexicalNode) throws -> Bool
) throws -> JSMap<NodeKey, String> {
  let isIdentifiedBlock = runtime.config.isIdentifiedBlock
  var taken = Set<String>()
  var targets: [(NodeKey, Frame)] = []
  func placeOf(_ frame: Frame) -> String {
    guard let parent = frame.parent, parent.parent != nil else {
      return String(frame.index)
    }
    return "\(placeOf(parent)).\(frame.index)"
  }
  try walk(Frame(parent: nil, index: 0)) { node, frame in
    if try !isIdentifiedBlock(node) {
      return frame
    }
    let inner = Frame(parent: frame, index: frame.next)
    frame.next += 1
    if let id = try getBlockId(node) {
      taken.insert(id)
    } else if try isTarget(node) {
      targets.append((node.getKey(), inner))
    }
    return inner
  }
  let placed = JSMap<NodeKey, String>()
  if targets.isEmpty {
    return placed
  }
  let key = try documentKey()
  for (nodeKey, frame) in targets {
    let path = placeOf(frame)
    var id = derivePositionalId(key, path)
    var attempt = 1
    while taken.contains(id) {
      id = derivePositionalId(key, "\(path)#\(attempt)")
      attempt += 1
    }
    taken.insert(id)
    placed.set(nodeKey, id)
  }
  return placed
}

/// Names the holders of a repeated id that every replica must name alike:
/// the earliest in document order keeps `id`, and the k-th later one gets
/// `deriveDuplicateId(id, k)`, retried with `#n` appended to `k` while
/// taken. Costs O(holders), whatever the document's size.
private func renameLaterHolders(
  _ runtime: BlockIdRuntime, _ id: String, _ holders: [LexicalNode], _ used: JSSet<String>
) throws -> JSMap<NodeKey, String> {
  let renamed = JSMap<NodeKey, String>()
  let holders = try holders.sorted { try compareDocumentOrder($0, $1) < 0 }
  for k in holders.indices.dropFirst() {
    var next = deriveDuplicateId(id, String(k))
    var n = 1
    while try isIdTaken(runtime, next, used) {
      next = deriveDuplicateId(id, "\(k)#\(n)")
      n += 1
    }
    used.add(next)
    renamed.set(holders[k].getKey(), next)
  }
  return renamed
}

/// The ids that the blocks `keys` of the active state should have, for
/// those it leaves unnamed: positional ids for blocks without an id,
/// derived ids for the later holders of a repeated id.
private func nameUnnamed(
  _ runtime: BlockIdRuntime, _ keys: [NodeKey]
) throws -> JSMap<NodeKey, String> {
  let unnamed = JSSet<NodeKey>()
  let repeated = JSSet<String>()
  for key in keys {
    let node = try getNodeByKey(key)
    if let node, try isLiveIdentified(runtime, node) {
      if let id = try getBlockId(node) {
        repeated.add(id)
      } else {
        unnamed.add(key)
      }
    }
  }
  let named =
    unnamed.size > 0
    ? try placeBlocks(runtime) { unnamed.has($0.getKey()) } : JSMap<NodeKey, String>()
  let used = JSSet(named.valuesArray)
  // In code unit order, so that a retry after a collision is decided alike
  // everywhere.
  for id in repeated.array.sorted(by: jsStringLess) {
    let holders = try liveHolders(runtime, id)
    if holders.count > 1 {
      for (key, next) in try renameLaterHolders(runtime, id, holders, used) {
        named.set(key, next)
      }
    }
  }
  return named
}

/// The id of a frozen node of a previous state, without an active state.
private func prevIdOf(_ node: LexicalNode?) throws -> String? {
  guard let node else { return nil }
  return parseBlockId(try getState(node, blockIdState, .direct))
}

/// An attached node that the predicate identifies.
private func isLiveIdentified(_ runtime: BlockIdRuntime, _ node: LexicalNode?) throws -> Bool {
  guard let node else { return false }
  return try node.isAttached() && runtime.config.isIdentifiedBlock(node)
}

/// Attached identified blocks of the active state that carry `id`.
private func liveHolders(
  _ runtime: BlockIdRuntime, _ id: String, _ extraKeys: [NodeKey] = []
) throws -> [LexicalNode] {
  var seen = Set<NodeKey>()
  var result: [LexicalNode] = []
  func visit(_ key: NodeKey) throws {
    if !seen.insert(key).inserted {
      return
    }
    let node = try getNodeByKey(key)
    if let node, try isLiveIdentified(runtime, node), try getBlockId(node) == id {
      result.append(node)
    }
  }
  for key in runtime.index.holders(id) {
    try visit(key)
  }
  for key in extraKeys {
    try visit(key)
  }
  return result
}

/// Whether `id` is in `used` or carried by an identified block of the
/// active state other than the block `except`.
private func isIdTaken(
  _ runtime: BlockIdRuntime, _ id: String, _ used: JSSet<String>, _ except: NodeKey? = nil
) throws -> Bool {
  try used.has(id) || liveHolders(runtime, id).contains { $0.getKey() != except }
}

/// The parent of `node`, or the host of the slot that it occupies.
private func getContainer(_ node: LexicalNode) throws -> LexicalNode? {
  if let parent = try node.getParent() {
    return parent
  }
  return try getSlotHost(node)
}

/// Document order of two nodes of the active state (see `walk`), the
/// tie-break that every replica must resolve alike: node keys cannot be
/// used for it, because each editor numbers the nodes that it receives in
/// its own order. Costs O(depth) plus a walk over the siblings between the
/// two branches.
private func compareDocumentOrder(_ a: LexicalNode, _ b: LexicalNode) throws -> Int {
  if a.is(b) {
    return 0
  }
  // Container key → the node below it on the way up from `a`.
  var below: [NodeKey: LexicalNode] = [:]
  var child = a
  var container = try getContainer(a)
  while let current = container {
    below[current.getKey()] = child
    child = current
    container = try getContainer(current)
  }
  if below[b.getKey()] != nil {
    // `b` is an ancestor of `a`.
    return 1
  }
  var bChild = b
  container = try getContainer(b)
  while let current = container {
    defer {
      bChild = current
    }
    if current.is(a) {
      // `a` is an ancestor of `b`.
      return -1
    }
    guard let aChild = below[current.getKey()] else {
      container = try getContainer(current)
      continue
    }
    // `current` is the closest common one; compare the two branches.
    let aSlot = try getSlotNameWithinHost(aChild)
    let bSlot = try getSlotNameWithinHost(bChild)
    if aSlot != nil || bSlot != nil {
      guard let aSlot, let bSlot else {
        // Slots come before children.
        return aSlot == nil ? 1 : -1
      }
      let names = try getSlotNames(current)
      return (names.firstIndex(of: aSlot) ?? -1) - (names.firstIndex(of: bSlot) ?? -1)
    }
    let aKey = aChild.getKey()
    let bKey = bChild.getKey()
    var na: LexicalNode? = aChild
    var nb: LexicalNode? = bChild
    while let x = na, let y = nb {
      if x.getKey() == bKey {
        return -1
      } else if y.getKey() == aKey {
        return 1
      }
      na = try x.getNextSibling()
      nb = try y.getNextSibling()
    }
    // The walk from `a` reached the end first: `b` comes before it.
    return na == nil ? 1 : -1
  }
  // Not in the same tree (cannot happen for attached nodes).
  return compareKeys(a, b)
}

/// Creation order. Node keys come from a counter that only increases within
/// one process, so this orders nodes that one local update created; it is
/// used only for decisions that one editor makes and other replicas then
/// receive.
private func compareKeys(_ a: LexicalNode, _ b: LexicalNode) -> Int {
  (Int(a.getKey()) ?? 0) - (Int(b.getKey()) ?? 0)
}

private func makeFreshId(_ runtime: BlockIdRuntime, _ used: JSSet<String>) throws -> String {
  for _ in 0..<MAX_ID_ATTEMPTS {
    let id = try runtime.config.generateId()
    if parseBlockId(id) == id, try !isIdTaken(runtime, id, used) {
      used.add(id)
      return id
    }
  }
  throw LexicalError(
    "BlockIdExtension: generateId returned \(MAX_ID_ATTEMPTS) ids in a row that were invalid or already in use"
  )
}

/// Whether the pending update is a load: the first content since the
/// extension was registered, or a state that replaced the previous one
/// wholesale (setEditorState).
private func isLoad(_ editor: Editor, _ runtime: BlockIdRuntime) throws -> Bool {
  let prevRoot = editor.getEditorState()._nodeMap.get("root")
  let prevFirst = (prevRoot as? ElementNode)?.__first
  if !runtime.hasContent {
    return true
  }
  guard let prevFirst else { return false }
  return try getNodeByKey(prevFirst) == nil
}

/// The RootNode transform, which Lexical applies after every other
/// transform of the same pass. `candidates` holds the identified blocks
/// marked dirty in this update. The rules are those of the documentation
/// ("How ids follow edits", "Uniqueness", "Documents stored without ids"):
///
/// 0. A block that the previous state left unnamed takes the id every
///    reader of that state derived for it.
/// 1. A new block continues a block of the previous state (and takes its
///    id): one that took the children of a removed block, or all the
///    children of a block now empty (Enter at the start, which re-keys the
///    emptied block), or one that sits where a removed block sat (or in its
///    slot). For the children, a copy that yields its id counts as a block
///    without one.
/// 2. Blocks without an id get one: positional in a load, generated
///    otherwise.
/// 3. Of the blocks that share an id, the one that was identified with it
///    in the previous state keeps it. Several such blocks are named by
///    document order and derived ids, alike on every replica. Otherwise a
///    block that continues a removed holder of the id keeps it, else the
///    first one created; the others get generated ids, in creation order.
private func assignBlockIds(
  _ editor: Editor, _ runtime: BlockIdRuntime, _ candidates: JSSet<NodeKey>
) throws {
  if candidates.isEmpty {
    return
  }
  let keys = candidates.array
  candidates.clear()
  if try hasUpdateTag(COLLABORATION_TAG) || hasUpdateTag(HISTORIC_TAG) {
    // Changes made during updates tagged COLLABORATION_TAG or HISTORIC_TAG
    // are treated as replayed and are not propagated by whoever applied
    // them: the follow-up repair update names these blocks.
    return
  }
  var blocks: [LexicalNode] = []
  for key in keys {
    if let node = try getNodeByKey(key), try isLiveIdentified(runtime, node) {
      blocks.append(node)
    }
  }
  if blocks.isEmpty {
    return
  }
  let blockKeys = JSSet(blocks.map { $0.getKey() })
  let pendingState = editor._pendingEditorState
  if runtime.provisionalState !== pendingState {
    runtime.provisional.clear()
    runtime.provisionalSeen.clear()
    runtime.provisionalState = pendingState
  }
  let provisional = runtime.provisional
  let provisionalSeen = runtime.provisionalSeen
  for key in blockKeys {
    provisionalSeen.add(key)
  }
  // Whether an earlier pass of this update generated the block's id because
  // it had none, so lineage found now may still replace it. (A copy that
  // yielded its id is not: it is decided as a holder of that id, below.)
  func isProvisional(_ node: LexicalNode) throws -> Bool {
    guard let entry = provisional.get(node.getKey()) else { return false }
    return try entry.yielded == nil && entry.id == getBlockId(node)
  }
  // A copy that yielded its id in an earlier pass takes it back when no
  // block holds it any more (the original was merged into it since); the
  // rules below then decide between several such copies.
  for node in blocks {
    if let entry = provisional.get(node.getKey()), let yielded = entry.yielded,
      try entry.id == getBlockId(node),
      try liveHolders(runtime, yielded, provisionalSeen.array).isEmpty
    {
      _ = provisional.delete(node.getKey())
      try setState(node, blockIdState, yielded)
    }
  }
  let prevState = editor.getEditorState()
  let prevNodeMap = prevState._nodeMap
  let index = runtime.index
  let used = JSSet<String>()
  let placedBefore = runtime.placedState === prevState ? runtime.placed : JSMap<NodeKey, String>()
  // The id a block had in the previous state, counting the one its readers
  // derived when that state left it unnamed.
  func prevIdOfKey(_ key: NodeKey) throws -> String? {
    if let placed = placedBefore.get(key) {
      return placed
    }
    return try prevIdOf(prevNodeMap.get(key))
  }
  // Whether the block was identified with `id` in the previous state. An id
  // left on a block that was not identified is not reserved.
  func heldBefore(_ key: NodeKey, _ id: String) -> Bool {
    placedBefore.get(key) == id || index.idOfKey.get(key) == id
  }
  // New key → key of the previous-state block it continues.
  let lineage = JSMap<NodeKey, NodeKey>()
  // Removed in this update: still in the pending node map, detached. A key
  // missing altogether belongs to a state replaced wholesale.
  func isRemoved(_ key: NodeKey?) throws -> Bool {
    guard let key, let node = try getNodeByKey(key) else { return false }
    return try !node.isAttached()
  }
  let load = try isLoad(editor, runtime)
  func inherit(_ node: LexicalNode, _ id: String, _ fromKey: NodeKey) throws {
    lineage.set(node.getKey(), fromKey)
    _ = provisional.delete(node.getKey())
    used.add(id)
    if try getBlockId(node) != id {
      try setState(node, blockIdState, id)
    }
  }

  // 0. Blocks the previous state left unnamed.
  if placedBefore.size > 0 {
    for node in blocks {
      let key = node.getKey()
      if let placed = placedBefore.get(key) {
        let id = try getBlockId(node)
        if try id == nil || id == prevIdOf(prevNodeMap.get(key)),
          try !isIdTaken(runtime, placed, used, key)
        {
          try inherit(node, placed, key)
        }
      }
    }
  }

  // Whether a new block takes lineage by children as if it had no id: its
  // id `id` was generated by an earlier pass of this update, or it is a copy
  // that yields `id` in step 3 to a block that was identified with it before
  // the update and still holds it.
  func isIdLess(_ node: LexicalNode, _ id: String) throws -> Bool {
    if let entry = provisional.get(node.getKey()), entry.id == id {
      return true
    }
    return try liveHolders(runtime, id).contains { holder in
      !holder.is(node) && heldBefore(holder.getKey(), id)
    }
  }

  // 1a. Lineage by children. The first child decides, unless it is new:
  // then the first child that comes from a removed block does.
  let newBlocks = blocks.filter { !prevNodeMap.has($0.getKey()) }
  for node in newBlocks {
    guard let element = node as? ElementNode else {
      continue
    }
    let firstChild = try element.getFirstChild()
    var sourceKey: NodeKey?
    var child = firstChild
    var isFirst = true
    while let current = child, sourceKey == nil {
      if let prevChild = prevNodeMap.get(current.getKey()), let parentKey = prevChild.__parent {
        if try isFirst || isRemoved(parentKey) {
          sourceKey = parentKey
        }
      }
      isFirst = false
      child = try current.getNextSibling()
    }
    guard let sourceKey else {
      continue
    }
    let ownId = try getBlockId(node)
    guard let id = try prevIdOfKey(sourceKey) else {
      continue
    }
    if let ownId, ownId != id, try !isIdLess(node, ownId) {
      continue
    }
    if try isRemoved(sourceKey) {
      // Replaced, e.g. by setBlocksType, or merged into this block.
      if try !isIdTaken(runtime, id, used, sourceKey) {
        try inherit(node, id, sourceKey)
      }
      continue
    }
    let source = try getNodeByKey(sourceKey)
    let prevSource = prevNodeMap.get(sourceKey)
    if let firstChild, let source = source as? ElementNode, try source.isAttached(),
      try source.getChildrenSize() == 0, let prevSource = prevSource as? ElementNode,
      prevSource.__first == firstChild.getKey(), try getBlockId(source) == id, !used.has(id)
    {
      // All of the content moved on (Enter at the start): the id follows it.
      try setState(source, blockIdState, makeFreshId(runtime, used))
      try inherit(node, id, sourceKey)
    }
  }

  // 1b. Lineage by position, for the remaining new blocks without an id.
  let resolved = JSSet<NodeKey>()
  func resolvePosition(_ start: LexicalNode) throws {
    var stack: [LexicalNode] = [start]
    while let node = stack.last {
      let key = node.getKey()
      if resolved.has(key) {
        stack.removeLast()
        continue
      }
      let prevSibling = try node.getPreviousSibling()
      if let prevSibling, !prevNodeMap.has(prevSibling.getKey()),
        blockKeys.has(prevSibling.getKey()), !resolved.has(prevSibling.getKey())
      {
        // A run of replaced blocks is matched against the removed run in
        // order: resolve the new block before this one first.
        stack.append(prevSibling)
        continue
      }
      stack.removeLast()
      resolved.add(key)
      if try lineage.has(key) || (getBlockId(node) != nil && !isProvisional(node)) {
        continue
      }
      var removedKey: NodeKey?
      if let host = try getSlotHost(node) {
        let prevHost = prevNodeMap.get(host.getKey())
        let name = try getSlotNameWithinHost(node)
        if let prevHost = prevHost as? any SlotNode, let slots = prevHost.__slots, let name {
          removedKey = slots.get(name)
        }
      } else if let prevSibling {
        let anchorKey =
          prevNodeMap.has(prevSibling.getKey())
          ? prevSibling.getKey() : lineage.get(prevSibling.getKey())
        let prevAnchor = anchorKey.flatMap { prevNodeMap.get($0) }
        removedKey = prevAnchor?.__next
      } else {
        let parent = try node.getParent()
        let prevParent = parent.flatMap { prevNodeMap.get($0.getKey()) }
        removedKey = (prevParent as? ElementNode)?.__first
      }
      if try isRemoved(removedKey), let removedKey {
        if let id = try prevIdOfKey(removedKey), try !isIdTaken(runtime, id, used, removedKey) {
          try inherit(node, id, removedKey)
        }
      }
    }
  }
  for node in newBlocks {
    try resolvePosition(node)
  }

  // 2 and 3.
  let candidatesById = JSMap<String, [NodeKey]>()
  var needFresh: [LexicalNode] = []
  for node in blocks {
    if let id = try getBlockId(node) {
      candidatesById.set(id, (candidatesById.get(id) ?? []) + [node.getKey()])
    } else {
      needFresh.append(node)
    }
  }
  let repeated = JSMap<String, [LexicalNode]>()
  let yieldedOf = JSMap<NodeKey, String>()
  for (id, keysWithId) in candidatesById {
    let holders = try liveHolders(runtime, id, keysWithId)
    if holders.count > 1 {
      repeated.set(id, holders)
    }
  }
  // In code unit order, like nameUnnamed.
  for id in repeated.keysArray.sorted(by: jsStringLess) {
    let holders = repeated.get(id)!
    used.add(id)
    let existing = holders.filter { heldBefore($0.getKey(), id) }
    let shared = existing.count > 1 ? existing : load ? holders : nil
    let keepers: [LexicalNode]
    if let shared {
      for (key, next) in try renameLaterHolders(runtime, id, shared, used) {
        try setState(getNodeByKey(key)!, blockIdState, next)
      }
      keepers = shared
    } else if existing.count == 1 {
      keepers = existing
    } else {
      // Every holder got the id in this local update.
      let continuing = try holders.filter { node in
        guard let from = lineage.get(node.getKey()) else { return false }
        return try isRemoved(from) && prevIdOfKey(from) == id
      }
      let pool = continuing.isEmpty ? holders : continuing
      keepers = [pool.dropFirst().reduce(pool[0]) { a, b in compareKeys(b, a) < 0 ? b : a }]
    }
    for node in holders where !keepers.contains(where: { $0 === node }) {
      needFresh.append(node)
      yieldedOf.set(node.getKey(), id)
    }
  }
  var random = needFresh
  if load && !needFresh.isEmpty {
    // Every reader that loads the same content names its blocks alike.
    let fresh = JSSet(needFresh.map { $0.getKey() })
    let placed = try placeBlocks(runtime) { fresh.has($0.getKey()) }
    random = []
    for node in needFresh {
      let key = node.getKey()
      if let id = placed.get(key), try !isIdTaken(runtime, id, used, key) {
        used.add(id)
        try setState(node, blockIdState, id)
      } else {
        random.append(node)
      }
    }
  }
  for node in random.sorted(by: { compareKeys($0, $1) < 0 }) {
    let id = try makeFreshId(runtime, used)
    try setState(node, blockIdState, id)
    if !prevNodeMap.has(node.getKey()) {
      provisional.set(node.getKey(), ProvisionalId(id: id, yielded: yieldedOf.get(node.getKey())))
    }
  }
}

/// The changes of one commit, as an update listener receives them.
private struct CommitChanges {
  let prevEditorState: EditorState
  let dirtyElements: JSMap<NodeKey, Bool>
  let dirtyLeaves: JSSet<NodeKey>
}

/// Brings the index up to date with a committed state: incrementally from a
/// commit's dirty nodes, or rebuilt when `changes` is `nil` or does not say
/// which nodes changed. Returns the keys of identified blocks that lack an
/// id or share one, which need a repair update.
private func syncIndex(
  _ runtime: BlockIdRuntime, _ editorState: EditorState, _ changes: CommitChanges?
) throws -> [NodeKey] {
  let index = runtime.index
  let config = runtime.config
  var needsRepair: [NodeKey] = []
  let touchedIds = JSSet<String>()

  func visit(_ node: LexicalNode) throws {
    let key = node.getKey()
    if try node.isAttached() && config.isIdentifiedBlock(node) {
      if let id = try getBlockId(node) {
        index.set(key, id)
        touchedIds.add(id)
      } else {
        index.remove(key)
        needsRepair.append(key)
      }
    } else {
      index.remove(key)
    }
  }

  if let changes,
    // Only the root is dirty: the state was replaced with setEditorState (an
    // undo or redo, for instance), which Lexical reconciles in full too.
    !(changes.dirtyLeaves.isEmpty && changes.dirtyElements.size == 1
      && changes.dirtyElements.has("root"))
  {
    let nodeMap = editorState._nodeMap
    let prevNodeMap = changes.prevEditorState._nodeMap
    func removeSubtree(_ key: NodeKey) {
      var stack: [NodeKey] = [key]
      while let k = stack.popLast() {
        if nodeMap.has(k) {
          // Moved elsewhere; it is dirty itself if it matters.
          continue
        }
        index.remove(k)
        let prevNode = prevNodeMap.get(k)
        if let prevElement = prevNode as? ElementNode {
          var child = prevElement.__first
          while let current = child {
            stack.append(current)
            child = prevNodeMap.get(current)?.__next
          }
        }
        if let prevHost = prevNode as? any SlotNode, let slots = prevHost.__slots {
          stack.append(contentsOf: slots.values)
        }
      }
    }
    func visitKey(_ key: NodeKey) throws {
      if let node = nodeMap.get(key) {
        if !(node is TextNode) {
          try visit(node)
        }
      } else {
        removeSubtree(key)
      }
    }
    for (key, intentional) in changes.dirtyElements {
      // An element marked dirty only because a descendant changed keeps its
      // id, position and identity, unless it was also created or written in
      // this update: Lexical downgrades the flag when a later transform pass
      // touches only a descendant, but never clones for such a mark.
      if intentional || nodeMap.get(key) !== prevNodeMap.get(key) {
        try visitKey(key)
      }
    }
    for key in changes.dirtyLeaves {
      try visitKey(key)
    }
  } else {
    index.clear()
    try walk(()) { node, _ in try visit(node) }
  }

  for id in touchedIds {
    if index.isRepeated(id) {
      let holders = try liveHolders(runtime, id)
      if holders.count > 1 {
        needsRepair.append(contentsOf: holders.map { $0.getKey() })
      }
    }
  }
  return needsRepair
}

private func registerBlockIds(_ editor: Editor, _ runtime: BlockIdRuntime) throws -> () -> Void {
  let candidates = JSSet<NodeKey>()
  let isIdentifiedBlock = runtime.config.isIdentifiedBlock
  func collect(_ node: LexicalNode) throws {
    if try isIdentifiedBlock(node) {
      candidates.add(node.getKey())
    } else if try getBlockId(node) != nil,
      !editor.getEditorState()._nodeMap.has(node.getKey()),
      try !hasUpdateTag(COLLABORATION_TAG), try !hasUpdateTag(HISTORIC_TAG)
    {
      // A block that is not identified has no id: a copy created where it
      // is not one (a list pasted into a list item, which nests it) does not
      // keep the id of the block it was copied from.
      try setState(node, blockIdState, nil)
    }
  }
  func scheduleRepair(_ keys: [NodeKey]) throws {
    let withTransforms = runtime.config.repairTransforms
    try editor.update(
      {
        let attached = JSSet<NodeKey>()
        for key in keys {
          if let node = try getNodeByKey(key), try node.isAttached() {
            if withTransforms {
              try node.markDirty()
            } else {
              attached.add(key)
            }
          }
        }
        if !withTransforms {
          // What the root transform would do with these blocks marked dirty.
          try assignBlockIds(editor, runtime, attached)
        }
      },
      // Discrete, so it commits on its own right after the commit that
      // needed it: batched into a later update tagged COLLABORATION_TAG, its
      // changes would be treated as replayed and not propagated.
      EditorUpdateOptions(
        skipTransforms: !withTransforms, tag: [HISTORY_MERGE_TAG, BLOCK_ID_REPAIR_TAG],
        discrete: true))
  }
  // Notes the ids every reader gives the blocks a committed state leaves
  // unnamed, for the update that repairs it (or any other based on it).
  func sync(_ editorState: EditorState, _ changes: CommitChanges?) throws -> [NodeKey] {
    let repairs = try syncIndex(runtime, editorState, changes)
    if try !runtime.hasContent && !getRoot().isEmpty() {
      runtime.hasContent = true
    }
    runtime.placed = repairs.isEmpty ? JSMap() : try nameUnnamed(runtime, repairs)
    runtime.placedState = editorState
    return repairs
  }

  runtime.index.clear()
  runtime.active = true
  runtime.hasContent = false
  let currentState = editor.getEditorState()
  let initialRepairs = try currentState.read(editor: editor) { try sync(currentState, nil) }

  var cleanups: [() -> Void] = [
    editor.registerUpdateListener { payload in
      let editorState = payload.editorState
      let repairs = try editorState.read(editor: editor) {
        try sync(
          editorState,
          CommitChanges(
            prevEditorState: payload.prevEditorState, dirtyElements: payload.dirtyElements,
            dirtyLeaves: payload.dirtyLeaves))
      }
      if !repairs.isEmpty {
        if !payload.tags.has(BLOCK_ID_REPAIR_TAG) {
          try scheduleRepair(repairs)
        } else {
          consoleWarn(
            "BlockIdExtension: a repair update left blocks without a unique id; is $isIdentifiedBlock stable?"
          )
        }
      }
    }
  ]
  // Registering a transform marks every node of that type dirty, so the
  // first update assigns ids to a document that was opened without them.
  for (_, registered) in editor._nodes {
    let klass = registered.klass
    if klass == RootNode.self || klass is TextNode.Type || klass == LineBreakNode.self {
      continue
    }
    cleanups.append(try editor.registerNodeTransform(klass) { try collect($0) })
  }
  cleanups.append(
    try editor.registerNodeTransform(RootNode.self) { _ in
      // A load is named once a transform pass changes nothing in the
      // document before this transform (the root is not dirty again), so
      // that positional ids are derived from the document the load ends
      // with (equal adjacent lists merged, NodeStructureExtension's repairs
      // made), not from a state in between. A dirty node outside the
      // document does not run the root transform again, so it must not
      // defer naming.
      if try editor._dirtyElements.has("root") && !candidates.isEmpty
        && isLoad(editor, runtime)
      {
        return
      }
      try assignBlockIds(editor, runtime, candidates)
    })
  if !initialRepairs.isEmpty {
    try scheduleRepair(initialRepairs)
  }
  return {
    for cleanup in cleanups.reversed() {
      cleanup()
    }
    runtime.active = false
    runtime.index.clear()
    runtime.placed = JSMap()
    runtime.placedState = nil
    runtime.provisional.clear()
    runtime.provisionalSeen.clear()
    runtime.provisionalState = nil
    runtime.handles = nil
    runtime.scan = nil
    candidates.clear()
  }
}

private func getHandleView(_ editor: Editor, _ runtime: BlockIdRuntime) throws -> HandleView {
  if runtime.active {
    // The ids of the committed state. Ids of blocks that the active update
    // removed are still in it: they can only make a handle longer, and
    // getBlockById, which verifies against the active state, skips them.
    let index = runtime.index
    if let handles = runtime.handles, handles.version == index.version {
      return handles.view
    }
    let view = HandleView(
      table: runtime.buildTable(index.keysOfId.keysArray),
      lookup: frozenLookup { [unowned runtime] in try runtime.getBlockById($0) })
    runtime.handles = (view, index.version)
    return view
  }
  // Not indexed while disabled: a scan of the last committed state, as the
  // index would be, cached until the next commit. Lookups are verified
  // against the active state, as the index's are: the first holder of the id
  // there, in document order, as getBlockById finds it.
  let committed = editor.getEditorState()
  if let scan = runtime.scan, scan.state === committed {
    return scan.view
  }
  let isIdentifiedBlock = runtime.config.isIdentifiedBlock
  let keysOfId = try committed.read(editor: editor) {
    let keys = JSMap<String, [NodeKey]>()
    try walk(()) { node, _ in
      if try isIdentifiedBlock(node), let id = try getBlockId(node) {
        keys.set(id, (keys.get(id) ?? []) + [node.getKey()])
      }
    }
    return keys
  }
  let view = HandleView(
    table: runtime.buildTable(keysOfId.keysArray),
    lookup: frozenLookup { id in
      var found: LexicalNode?
      for key in keysOfId.get(id) ?? [] {
        if let node = try getNodeByKey(key), try node.isAttached(), try isIdentifiedBlock(node),
          try getBlockId(node) == id,
          try found == nil || compareDocumentOrder(node, found!) < 0
        {
          found = node
        }
      }
      return found
    })
  runtime.scan = (view, committed)
  return view
}

/// `lookup`, remembered per id for the frozen state it is asked about (a
/// read of a committed state), so that reading the handle of every holder
/// of a repeated id costs the holders once, not once per holder. While the
/// active state is not frozen (an update), it asks `lookup` every time. JS
/// remembers each frozen state in a `WeakMap`; Swift remembers the last one.
private func frozenLookup(
  _ lookup: @escaping (String) throws -> LexicalNode?
) -> (String) throws -> LexicalNode? {
  final class Cache {
    weak var state: EditorState?
    var found: [String: LexicalNode?] = [:]
  }
  let cache = Cache()
  return { id in
    guard let state = ThreadContext.current.activeEditorState, state._readOnly else {
      return try lookup(id)
    }
    if cache.state !== state {
      cache.state = state
      cache.found = [:]
    }
    if let node = cache.found[id] {
      return node
    }
    let node = try lookup(id)
    cache.found[id] = .some(node)
    return node
  }
}

private func getHandleOf(_ runtime: BlockIdRuntime, _ node: LexicalNode) throws -> String? {
  if try !node.isAttached() || !runtime.config.isIdentifiedBlock(node) {
    return nil
  }
  guard let id = try getBlockId(node) else {
    return nil
  }
  let view = try getHandleView(getEditor(), runtime)
  // Only a block that its id finds has a handle that finds it: not one named
  // after the last commit (its handle is not in the table), nor a later
  // holder of a repeated id, nor a node of another state.
  guard let handle = view.table.handles.get(id) else {
    return nil
  }
  return try view.lookup(id)?.is(node) == true ? handle : nil
}

private func getAllHandles(_ runtime: BlockIdRuntime) throws -> JSMap<String, String> {
  let view = try getHandleView(getEditor(), runtime)
  let result = JSMap<String, String>()
  for (id, handle) in view.table.handles {
    if try view.lookup(id) != nil {
      result.set(id, handle)
    }
  }
  return result
}

/// What `resolveInRead` found: the id the handle names, or why it names
/// none.
private enum ReadResolution {
  case id(String)
  case result(BlockHandleResult)
}

/// The block id that `handle` names among the ids of a read (see
/// `BlockHandleOptions.read`), or why it names none. It reads the map as it
/// is on each call (a scan of its ids, no cached index), so a map changed
/// between calls is never half stale.
private func resolveInRead(_ read: JSMap<String, String>, _ handle: String) -> ReadResolution {
  if handle.utf16.count > HANDLE_KEY_LENGTH {
    return read.has(handle) ? .id(handle) : .result(.notFound(handle: handle))
  }
  if handle.utf16.count < MIN_BLOCK_HANDLE_LENGTH {
    return .result(.invalid(handle: handle, reason: .tooShort))
  }
  var matches: [(key: String, id: String)] = []
  for id in read.keysArray {
    let key = handleKey(id)
    if key.hasPrefix(handle) {
      matches.append((key, id))
    }
  }
  if matches.isEmpty {
    return .result(.notFound(handle: handle))
  }
  if matches.count > 1 {
    return .result(
      .ambiguous(
        handle: handle,
        candidates: matches.sorted { jsStringLess($0.key, $1.key) }.prefix(MAX_HANDLE_CANDIDATES)
          .map { read.get($0.id) ?? $0.key }))
  }
  return .id(matches[0].id)
}

private func resolveHandle(
  _ runtime: BlockIdRuntime, _ handle: String, _ options: BlockHandleOptions?
) throws -> BlockHandleResult {
  // JS first refuses a handle that is not a string; a Swift handle is one.
  if !matchesBlockIdPattern(handle) {
    return .invalid(handle: handle, reason: .malformed)
  }
  if let read = options?.read {
    // What the handle named in the read, if that block still exists: a
    // block added since cannot take the handle, nor make it ambiguous.
    switch resolveInRead(read, handle) {
    case .result(let result):
      return result
    case .id(let id):
      guard let node = try runtime.getBlockById(id) else {
        return .notFound(handle: handle)
      }
      return .found(node)
    }
  }
  if handle.utf16.count > HANDLE_KEY_LENGTH {
    // Longer than any handle: a whole id.
    guard let node = try runtime.getBlockById(handle) else {
      return .notFound(handle: handle)
    }
    return .found(node)
  }
  if handle.utf16.count < MIN_BLOCK_HANDLE_LENGTH {
    // Never a handle: say so, rather than "not found".
    return .invalid(handle: handle, reason: .tooShort)
  }
  let view = try getHandleView(getEditor(), runtime)
  var found: LexicalNode?
  var candidates: [String] = []
  for id in idsWithPrefix(view.table, handle) {
    if let node = try view.lookup(id) {
      found = node
      if candidates.count < MAX_HANDLE_CANDIDATES {
        candidates.append(handleInTable(view.table, id))
      }
    }
  }
  if candidates.count > 1 {
    return .ambiguous(handle: handle, candidates: candidates)
  }
  guard let found else {
    return .notFound(handle: handle)
  }
  return .found(found)
}

/// `BlockIdExtension` (`@experimental`): gives blocks stable ids, stored as
/// NodeState (`blockIdState`), so that code outside the editor (an AI
/// agent, a server pipeline, a comment system) can address a block by id
/// across sessions, replicas, and serializations.
///
/// Every identified block gets an id in the update that creates it. A
/// document stored without ids gets positional ids, which every reader and
/// replica derives alike; blocks created by edits get random ids
/// (`generateBlockId`). Ids are unique: a copy yields to the block it was
/// copied from. While enabled, `getBlockById` looks blocks up in an index
/// maintained from the dirty nodes of each update.
///
/// Showing blocks to a model? Use handles (`getBlockHandle`,
/// `resolveBlockHandle`, or `handles: true` in `BlockIdHTMLExtension`).
///
/// The init result is the editor's `BlockIdRuntime`, opaque outside this
/// package (`unknown` in JS).
public let BlockIdExtension = defineExtension(
  name: "@lexical/extension/BlockId", config: BlockIdConfig(),
  // Where the editor also enforces declared node structure, register after
  // it, so that blocks are named on the structure its normalization leaves.
  peerDependencies: [declarePeerDependency("@lexical/NodeStructure")],
  init: { _, config, _ -> AnyObject in BlockIdRuntime(config: config) },
  build: { _, config, state in
    let runtime = try state.getInitResult() as! BlockIdRuntime
    return BlockIdOutput(
      disabled: signal(config.disabled),
      getBlockById: { try runtime.getBlockById($0) },
      getBlockHandle: { try getHandleOf(runtime, $0) },
      getBlockHandles: { try getAllHandles(runtime) },
      resolveBlockHandle: { try resolveHandle(runtime, $0, $1) },
      isIdentifiedBlock: config.isIdentifiedBlock)
  },
  register: { editor, _, state in
    let output = state.getOutput()
    let runtime = try state.getInitResult() as! BlockIdRuntime
    return try effect { () -> (() -> Void)? in
      if !output.disabled.value {
        return try registerBlockIds(editor, runtime)
      }
      return nil
    }
  })
