// Port of packages/lexical/src/LexicalSlot.ts.

/// The named slots of a host, in insertion order like the JS `Map`. A value
/// type, so a writable host's copy never aliases the frozen version's (the
/// JS `SLOT_MAP_OWNER` copy-on-write).
public struct SlotMap: Sequence, Equatable, Sendable {
  private var names: [String] = []
  private var keys: [String: NodeKey] = [:]

  public init() {}

  public var count: Int { names.count }
  public var isEmpty: Bool { names.isEmpty }
  public var slotNames: [String] { names }
  public var values: [NodeKey] { names.map { keys[$0]! } }

  public func get(_ name: String) -> NodeKey? { keys[name] }

  public mutating func set(_ name: String, _ key: NodeKey) {
    if keys.updateValue(key, forKey: name) == nil {
      names.append(name)
    }
  }

  public mutating func delete(_ name: String) {
    if keys.removeValue(forKey: name) != nil {
      names.removeAll { $0 == name }
    }
  }

  mutating func sort(by areInIncreasingOrder: (String, String) -> Bool) {
    names.sort(by: areInIncreasingOrder)
  }

  public func makeIterator() -> some IteratorProtocol<(String, NodeKey)> {
    names.lazy.map { ($0, keys[$0]!) }.makeIterator()
  }
}

/// A node that can host slots and be slotted: `ElementNode` and
/// `DecoratorNode`.
package protocol SlotNode: LexicalNode {
  var __slotHost: NodeKey? { get set }
  var __slots: SlotMap? { get set }
}

/// `$isSlotHost`.
public func isSlotHost(_ node: LexicalNode) -> Bool {
  node is ElementNode || node is DecoratorNode
}

/// `$getSlotHostKey`: the host `node` is slotted into, or `nil`.
public func getSlotHostKey(_ node: LexicalNode) throws -> NodeKey? {
  (try node.getLatest() as? any SlotNode)?.__slotHost
}

/// The raw `__slotHost` of this version of `node`, for GC and dirty marking.
package func slotHostKeyField(_ node: LexicalNode) -> NodeKey? {
  (node as? any SlotNode)?.__slotHost
}

/// Whether this version of `node` has slots.
package func hasSlots(_ node: LexicalNode) -> Bool {
  (node as? any SlotNode)?.__slots?.isEmpty == false
}

/// The slot keys of this version of `node`.
package func slotKeys(_ node: LexicalNode) -> [NodeKey] {
  (node as? any SlotNode)?.__slots?.values ?? []
}

/// `$getSlotHost`.
public func getSlotHost(_ node: LexicalNode) throws -> LexicalNode? {
  guard let slotHostKey = try getSlotHostKey(node) else {
    return nil
  }
  let host = try getNodeByKey(slotHostKey)
  try invariant(
    host is ElementNode || host is DecoratorNode,
    "slotHost must be an ElementNode or a DecoratorNode")
  return host
}

/// `$getSlotNameWithinHost`.
public func getSlotNameWithinHost(_ slotChild: LexicalNode) throws -> String? {
  guard let host = try getSlotHost(slotChild) else {
    return nil
  }
  let childKey = try slotChild.getLatest().__key
  for (name, key) in try getSlotMap(host) where key == childKey {
    return name
  }
  return nil
}

/// `$getSlotFrame`: the nearest of `node` and its ancestors that is slotted.
public func getSlotFrame(_ node: LexicalNode) throws -> LexicalNode? {
  var current: LexicalNode? = try node.getLatest()
  while let node = current {
    if try getSlotHostKey(node) != nil {
      return node
    }
    current = try node.getParent()
  }
  return nil
}

/// `$getSelectionSlotFrame`.
public func getSelectionSlotFrame(_ selection: BaseSelection?) throws -> LexicalNode? {
  guard let selection else {
    return nil
  }
  let anchorNode: LexicalNode? =
    try (selection as? RangeSelection)?.anchor.getNode() ?? selection.getNodes().first
  return try anchorNode.flatMap(getSlotFrame)
}

/// `$getSlotMap`.
public func getSlotMap(_ node: LexicalNode) throws -> SlotMap {
  (try node.getLatest() as? any SlotNode)?.__slots ?? SlotMap()
}

/// `$getSlotNames`.
public func getSlotNames(_ node: LexicalNode) throws -> [String] {
  try getSlotMap(node).slotNames
}

/// `$getSlot`.
public func getSlot(_ node: LexicalNode, _ name: String) throws -> LexicalNode? {
  guard let key = try getSlotMap(node).get(name) else {
    return nil
  }
  return try getNodeByKey(key)
}

private let RESERVED_SLOT_NAMES = ["__proto__", "constructor", "prototype"]

/// `getDeclaredSlots`: the `slots` of the nearest class in the chain that
/// declares them.
public func getDeclaredSlots(_ klass: LexicalNode.Type) -> [String] {
  for link in getNodeClassRecord(klass).chain {
    if let declared = link.ownNodeConfig?.slots {
      return declared
    }
  }
  return []
}

/// `$getSlotsTextContent`.
public func getSlotsTextContent(_ node: LexicalNode) throws -> String {
  var textContent = ""
  for key in try getSlotMap(node).values {
    if let slot = try getNodeByKey(key) {
      textContent += try slot.getTextContent()
    }
  }
  return textContent
}

/// `$getSlotsTextContentSize`.
public func getSlotsTextContentSize(_ node: LexicalNode) throws -> Int {
  var textContentSize = 0
  for key in try getSlotMap(node).values {
    if let slot = try getNodeByKey(key) {
      textContentSize += try slot.getTextContentSize()
    }
  }
  return textContentSize
}

private func getDeclaredSlotRank(_ klass: LexicalNode.Type) throws -> [String: Int] {
  var built: [String: Int] = [:]
  for name in getDeclaredSlots(klass) {
    try invariant(
      !RESERVED_SLOT_NAMES.contains(name),
      "getDeclaredSlotRank: \(klass) declares reserved slot name \"\(name)\"; __proto__, constructor, and prototype break the plain-object serialization of slots"
    )
    try invariant(
      built[name] == nil,
      "getDeclaredSlotRank: \(klass) declares slot name \"\(name)\" more than once; the canonical order would be ambiguous"
    )
    built[name] = built.count
  }
  return built
}

/// Declared slots in declaration order, then the rest in UTF-16 code unit
/// order (JS `<` on strings).
private func compareSlotNames(_ a: String, _ b: String, _ rank: [String: Int]) -> Int {
  if let rankA = rank[a] {
    if let rankB = rank[b] {
      return rankA - rankB
    }
    return -1
  }
  if rank[b] != nil {
    return 1
  }
  return jsStringLess(a, b) ? -1 : jsStringLess(b, a) ? 1 : 0
}

/// JS `a < b` on strings: UTF-16 code unit order.
package func jsStringLess(_ a: String, _ b: String) -> Bool {
  a.utf16.lexicographicallyPrecedes(b.utf16)
}

private func canonicalizeSlotOrder(_ host: any SlotNode) throws {
  guard var slots = host.__slots, slots.count >= 2 else {
    return
  }
  let rank = try getDeclaredSlotRank(type(of: host))
  let names = slots.slotNames
  if zip(names, names.dropFirst()).allSatisfy({ compareSlotNames($0, $1, rank) <= 0 }) {
    return
  }
  slots.sort { compareSlotNames($0, $1, rank) < 0 }
  host.__slots = slots
}

/// `$markSlotsUsed`.
package func markSlotsUsed() throws {
  try getActiveEditor()._slotsUsed = true
  try getActiveEditorState()._slotsUsed = true
}

/// `$setSlot`: slots `node` into `host` under `name`, moving it from where
/// it was and removing the node the slot held.
@discardableResult
public func setSlot(
  _ host: LexicalNode, _ name: String, _ node: LexicalNode
) throws
  -> LexicalNode
{
  try invariant(
    !RESERVED_SLOT_NAMES.contains(name),
    "$setSlot: \"\(name)\" is a reserved slot name; __proto__, constructor, and prototype break the plain-object serialization of slots"
  )
  guard let latestHost = try host.getLatest() as? any SlotNode else {
    throw LexicalError("$setSlot: host \(host.__key) is not an ElementNode or DecoratorNode")
  }
  if try latestHost.__slots?.get(name) == node.getLatest().__key {
    return latestHost
  }
  try invariant(
    try (node is ElementNode || node is DecoratorNode) && !node.isInline(),
    "$setSlot: node \(node.__key) is not a valid slot value; a slot value must be a non-inline ElementNode or DecoratorNode (the slot link itself is the shadow boundary)."
  )
  try invariant(
    !isSlotAncestorOrSelf(node, host),
    "$setSlot: node \(node.__key) cannot be slotted into \(host.__key); a node may not host itself or an ancestor reached through children or slot up-links — the slot up-link would form a cycle that loops isAttached/GC."
  )
  let writableSelf = try latestHost.getWritable()
  if let previousKey = writableSelf.__slots?.get(name) {
    try detachSlottedNode(previousKey)
  }
  guard let writableNode = try node.getWritable() as? any SlotNode else {
    throw LexicalError("$setSlot: node \(node.__key) is not a slot child")
  }
  if let previousHost = try getSlotHost(writableNode) as? any SlotNode {
    if let previousName = try getSlotNameWithinHost(writableNode) {
      let writablePreviousHost = try previousHost.getWritable()
      writablePreviousHost.__slots?.delete(previousName)
    }
    writableNode.__slotHost = nil
  }
  try detachNode(writableNode)
  writableNode.__slotHost = writableSelf.__key
  var slots = writableSelf.__slots ?? SlotMap()
  slots.set(name, writableNode.__key)
  writableSelf.__slots = slots
  try canonicalizeSlotOrder(writableSelf)
  try markSlotsUsed()
  return writableSelf
}

/// `$removeSlot`: removes the node `host` holds under `name`.
@discardableResult
public func removeSlot(_ host: LexicalNode, _ name: String) throws -> LexicalNode {
  guard let writableSelf = try host.getWritable() as? any SlotNode else {
    throw LexicalError("$removeSlot: host \(host.__key) is not an ElementNode or DecoratorNode")
  }
  guard let previousKey = writableSelf.__slots?.get(name) else {
    return writableSelf
  }
  try detachSlottedNode(previousKey)
  writableSelf.__slots?.delete(name)
  return writableSelf
}

private func isSlotAncestorOrSelf(_ node: LexicalNode, _ host: LexicalNode) throws -> Bool {
  var key: NodeKey? = host.__key
  while let current = key {
    if current == node.__key {
      return true
    }
    guard let currentNode = try getNodeByKey(current) else {
      break
    }
    key = try currentNode.__parent ?? getSlotHostKey(currentNode)
  }
  return false
}

/// `$errorOnSlotCycleChild`: refuses to make `child` a child of `parent`
/// when `parent` is reachable from it through slot up-links.
package func errorOnSlotCycleChild(_ parent: LexicalNode, _ child: LexicalNode) throws {
  if try !getEditor()._slotsUsed {
    return
  }
  try invariant(
    !isSlotAncestorOrSelf(child, parent),
    "insert: node \(child.__key) cannot become a child of \(parent.__key); the parent is reachable from the node through slot up-links, so the insertion would form a cycle that loops isAttached/GC."
  )
}

private func detachSlottedNode(_ slotKey: NodeKey) throws {
  guard let previous = try getNodeByKey(slotKey) else {
    return
  }
  guard let writablePrevious = try previous.getWritable() as? any SlotNode else {
    throw LexicalError(
      "detach: slotted node \(slotKey) must be an ElementNode or a DecoratorNode")
  }
  writablePrevious.__slotHost = nil
  try writablePrevious.remove()
}
