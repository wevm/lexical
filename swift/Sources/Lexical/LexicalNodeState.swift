// Port of packages/lexical/src/LexicalNodeState.ts.
//
// JS state values are arbitrary JS values. Here a `StateConfig<V>` is typed,
// and `AnyStateConfig` is its type-erased base, which `NodeState` stores
// values through. Configs are compared by identity, as in JS.

/// `NODE_STATE_DIRECT` / `NODE_STATE_LATEST`.
public enum NodeStateVersion: Sendable {
  /// Read the given version of the node.
  case direct
  /// Read the latest version of the node.
  case latest
}

/// A value that serializes to JSON without an explicit `unparse`.
public protocol JSONValueConvertible {
  var jsonValue: JSONValue? { get }
}

extension String: JSONValueConvertible {
  public var jsonValue: JSONValue? { .string(self) }
}
extension Bool: JSONValueConvertible {
  public var jsonValue: JSONValue? { .bool(self) }
}
extension Int: JSONValueConvertible {
  public var jsonValue: JSONValue? { .number(Double(self)) }
}
extension Double: JSONValueConvertible {
  public var jsonValue: JSONValue? { .number(self) }
}
extension JSONValue: JSONValueConvertible {
  public var jsonValue: JSONValue? { self }
}
extension Optional: JSONValueConvertible where Wrapped: JSONValueConvertible {
  public var jsonValue: JSONValue? { self?.jsonValue }
}

/// `Object.is` for values of unknown type: `==` for `Equatable` values,
/// identity for objects, otherwise unequal.
func jsObjectIs(_ a: Any, _ b: Any) -> Bool {
  func equals<T: Equatable>(_ a: T, _ b: Any) -> Bool {
    guard let b = b as? T else { return false }
    return a == b
  }
  if let a = a as? any Equatable {
    return equals(a, b)
  }
  if let a = a as AnyObject?, let b = b as AnyObject?, type(of: a) is AnyClass {
    return a === b
  }
  return false
}

/// The type-erased base of `StateConfig`: `AnyStateConfig`.
public class AnyStateConfig {
  public let key: String
  public let resetOnCopyNode: Bool
  public let derived: Bool
  /// The schema `parse` was given as, when it was one.
  public let schema: SerializationSchema?

  init(key: String, resetOnCopyNode: Bool, derived: Bool, schema: SerializationSchema?) {
    self.key = key
    self.resetOnCopyNode = resetOnCopyNode
    self.derived = derived
    self.schema = schema
  }

  func parseAny(_ value: JSONValue?) -> Any { fatalError("abstract") }
  func unparseAny(_ value: Any) -> JSONValue? { fatalError("abstract") }
  func isEqualAny(_ a: Any, _ b: Any) -> Bool { fatalError("abstract") }
  var defaultAny: Any { fatalError("abstract") }

  /// Parses `raw` and sets it on `node`: one flat state of `$applyJSONSetters`.
  package func setParsed<T: LexicalNode>(_ node: T, _ raw: JSONValue) throws -> T {
    try setStateAny(node, self, parseAny(raw))
  }
}

/// `StateConfig`: a key and how its value parses, serializes, and compares.
public final class StateConfig<V>: AnyStateConfig {
  public let parse: (JSONValue?) -> V
  public let unparse: (V) -> JSONValue?
  public let isEqual: (V, V) -> Bool
  public let defaultValue: V

  public init(
    _ key: String, parse: @escaping (JSONValue?) -> V, unparse: @escaping (V) -> JSONValue?,
    isEqual: ((V, V) -> Bool)? = nil, resetOnCopyNode: Bool = false, derived: Bool = false,
    schema: SerializationSchema? = nil, defaultValue: V? = nil
  ) {
    self.parse = parse
    self.unparse = unparse
    self.isEqual = isEqual ?? { jsObjectIs($0, $1) }
    self.defaultValue = defaultValue ?? parse(nil)
    super.init(key: key, resetOnCopyNode: resetOnCopyNode, derived: derived, schema: schema)
  }

  override func parseAny(_ value: JSONValue?) -> Any { parse(value) }
  override func unparseAny(_ value: Any) -> JSONValue? { unparse(value as! V) }
  override func isEqualAny(_ a: Any, _ b: Any) -> Bool { isEqual(a as! V, b as! V) }
  override var defaultAny: Any { defaultValue }
}

/// `createState(key, {parse, unparse, isEqual, resetOnCopyNode})`.
public func createState<V>(
  _ key: String, parse: @escaping (JSONValue?) -> V, unparse: @escaping (V) -> JSONValue?,
  isEqual: ((V, V) -> Bool)? = nil, resetOnCopyNode: Bool = false, derived: Bool = false
) -> StateConfig<V> {
  StateConfig(
    key, parse: parse, unparse: unparse, isEqual: isEqual, resetOnCopyNode: resetOnCopyNode,
    derived: derived)
}

/// `createState` for a value that serializes itself (`unparse` is the JS
/// identity).
public func createState<V: JSONValueConvertible>(
  _ key: String, parse: @escaping (JSONValue?) -> V, isEqual: ((V, V) -> Bool)? = nil,
  resetOnCopyNode: Bool = false, derived: Bool = false
) -> StateConfig<V> {
  StateConfig(
    key, parse: parse, unparse: { $0.jsonValue }, isEqual: isEqual,
    resetOnCopyNode: resetOnCopyNode, derived: derived)
}

/// `createState(key, {parse: schema})`: a state described by a schema, whose
/// default and equality come from it.
public func createState(
  _ key: String, schema: SerializationSchema, resetOnCopyNode: Bool = false,
  derived: Bool = false
) -> StateConfig<JSONValue?> {
  StateConfig(
    key, parse: { schema.parse($0) }, unparse: { $0 },
    isEqual: { schema.isSchemaEqual($0, $1) }, resetOnCopyNode: resetOnCopyNode,
    derived: derived, schema: schema, defaultValue: .some(schema.defaultValue))
}

/// `RequiredNodeStateConfig`: a state a node class declares, optionally
/// `flat` (serialized at the top level of the node's JSON).
public struct RequiredNodeStateConfig {
  public let stateConfig: AnyStateConfig
  public let flat: Bool

  public init(_ stateConfig: AnyStateConfig, flat: Bool = false) {
    self.stateConfig = stateConfig
    self.flat = flat
  }
}

/// `SharedNodeState`: the configs a node type knows, shared by every node
/// of the type.
public final class SharedNodeState {
  package let sharedConfigMap = JSMap<String, AnyStateConfig>()
  package let flatKeys = JSSet<String>()
}

private let UNSAFE_STATE_KEYS: Set<String> = ["__proto__", "constructor", "prototype"]

/// `createSharedNodeState`.
package func createSharedNodeState(_ klass: LexicalNode.Type) -> SharedNodeState {
  let shared = SharedNodeState()
  for link in getNodeClassRecord(klass).chain {
    for required in link.ownNodeConfig?.stateConfigs ?? [] {
      if required.flat {
        shared.flatKeys.add(required.stateConfig.key)
      }
      shared.sharedConfigMap.set(required.stateConfig.key, required.stateConfig)
    }
  }
  return shared
}

typealias KnownStateMap = JSMap<ObjectIdentifier, (config: AnyStateConfig, value: Any)>

/// `NodeState`: the state values of one version of a node.
public final class NodeState {
  /// The node version that owns this state; others copy it before writing.
  weak var node: LexicalNode?
  let knownState: KnownStateMap
  var unknownState: JSONObject?
  let sharedNodeState: SharedNodeState
  var size: Int

  init(
    _ node: LexicalNode, _ sharedNodeState: SharedNodeState, _ unknownState: JSONObject? = nil,
    _ knownState: KnownStateMap = KnownStateMap(), _ size: Int? = nil
  ) throws {
    self.node = node
    self.sharedNodeState = sharedNodeState
    self.unknownState = unknownState
    self.knownState = knownState
    let computedSize =
      size ?? computeSize(sharedNodeState.sharedConfigMap, unknownState, knownState)
    try invariant(
      size == nil || computedSize == size,
      "NodeState: size != computedSize (\(size.map(String.init) ?? "undefined") != \(computedSize))"
    )
    for (_, entry) in knownState {
      try invariant(
        sharedNodeState.sharedConfigMap.has(entry.config.key),
        "NodeState: sharedConfigMap missing knownState key \(entry.config.key)")
    }
    self.size = computedSize
  }

  func getValueAny(_ stateConfig: AnyStateConfig) -> Any {
    if let known = knownState.get(ObjectIdentifier(stateConfig)) {
      return known.value
    }
    sharedNodeState.sharedConfigMap.set(stateConfig.key, stateConfig)
    var parsed = stateConfig.defaultAny
    if let unknownState, unknownState.has(stateConfig.key) {
      if let jsonValue = unknownState[stateConfig.key] {
        parsed = stateConfig.parseAny(jsonValue)
      }
      knownState.set(ObjectIdentifier(stateConfig), (stateConfig, parsed))
    }
    return parsed
  }

  public func getValue<V>(_ stateConfig: StateConfig<V>) -> V {
    getValueAny(stateConfig) as! V
  }

  /// The serialized state: flat keys at the top level, the rest under `$`.
  public func toJSON() throws -> JSONObject {
    var state = unknownState ?? JSONObject()
    var flatState = JSONObject()
    for (_, entry) in knownState {
      if entry.config.isEqualAny(entry.value, entry.config.defaultAny) {
        state.remove(entry.config.key)
      } else {
        state.set(entry.config.key, entry.config.unparseAny(entry.value))
      }
    }
    for key in sharedNodeState.flatKeys where state.has(key) {
      flatState.set(key, state[key])
      state.remove(key)
    }
    if !state.keys.isEmpty {
      flatState[NODE_STATE_KEY] = .object(state)
    }
    return flatState
  }

  /// The state for `node`: this one when `node` owns it, else a copy.
  public func getWritable(_ node: LexicalNode) -> NodeState {
    if self.node === node {
      return self
    }
    let nextKnownState = knownState.copy()
    let nextUnknownState = parseAndPruneNextUnknownState(
      sharedNodeState.sharedConfigMap, nextKnownState, unknownState)
    return try! NodeState(node, sharedNodeState, nextUnknownState, nextKnownState, size)
  }

  @discardableResult
  public func resetOnCopyNode() -> NodeState {
    for (id, entry) in knownState where entry.config.resetOnCopyNode {
      knownState.set(id, (entry.config, entry.config.defaultAny))
    }
    return self
  }

  func updateFromKnown(_ stateConfig: AnyStateConfig, _ value: Any) {
    let key = stateConfig.key
    sharedNodeState.sharedConfigMap.set(key, stateConfig)
    if var unknown = unknownState, unknown.has(key) {
      unknown.remove(key)
      unknownState = unknown.keys.isEmpty ? nil : unknown
    } else if !knownState.has(ObjectIdentifier(stateConfig)) {
      size += 1
    }
    knownState.set(ObjectIdentifier(stateConfig), (stateConfig, value))
  }

  func updateFromUnknown(_ k: String, _ v: JSONValue?) {
    if UNSAFE_STATE_KEYS.contains(k) {
      return
    }
    if let stateConfig = sharedNodeState.sharedConfigMap.get(k) {
      updateFromKnown(stateConfig, stateConfig.parseAny(v))
    } else {
      var unknown = unknownState ?? JSONObject()
      if !unknown.has(k) {
        size += 1
      }
      unknown.set(k, v)
      unknownState = unknown
    }
  }

  func updateFromJSON(_ unknownState: JSONObject?) {
    for (id, entry) in knownState {
      knownState.set(id, (entry.config, entry.config.defaultAny))
    }
    size = knownState.size
    self.unknownState = nil
    if let unknownState {
      for key in unknownState.keys {
        updateFromUnknown(key, unknownState[key])
      }
    }
  }
}

private func computeSize(
  _ sharedConfigMap: JSMap<String, AnyStateConfig>, _ unknownState: JSONObject?,
  _ knownState: KnownStateMap
) -> Int {
  var size = knownState.size
  for key in unknownState?.keys ?? [] {
    let sharedConfig = sharedConfigMap.get(key)
    if sharedConfig == nil || !knownState.has(ObjectIdentifier(sharedConfig!)) {
      size += 1
    }
  }
  return size
}

private func parseAndPruneNextUnknownState(
  _ sharedConfigMap: JSMap<String, AnyStateConfig>, _ nextKnownState: KnownStateMap,
  _ unknownState: JSONObject?
) -> JSONObject? {
  var nextUnknownState: JSONObject? = nil
  guard let unknownState else {
    return nil
  }
  for key in unknownState.keys where !UNSAFE_STATE_KEYS.contains(key) {
    let value = unknownState[key]
    if let stateConfig = sharedConfigMap.get(key) {
      if !nextKnownState.has(ObjectIdentifier(stateConfig)) {
        nextKnownState.set(
          ObjectIdentifier(stateConfig), (stateConfig, stateConfig.parseAny(value)))
      }
    } else {
      var next = nextUnknownState ?? JSONObject()
      next.set(key, value)
      nextUnknownState = next
    }
  }
  return nextUnknownState
}

private func checkCollision(
  _ node: LexicalNode, _ stateConfig: AnyStateConfig, _ state: NodeState
)
  throws
{
  if let collision = state.sharedNodeState.sharedConfigMap.get(stateConfig.key),
    collision !== stateConfig
  {
    throw LexicalError(
      "$setState: State key collision \(JSONValue.string(stateConfig.key).stringify()) detected in \(type(of: node)) node with type \(node.getType()) and key \(node.getKey()). Only one StateConfig with a given key should be used on a node."
    )
  }
}

/// `$getState`: the value of `stateConfig` on `node`.
public func getState<V>(
  _ node: LexicalNode, _ stateConfig: StateConfig<V>, _ version: NodeStateVersion = .latest
) throws -> V {
  let latestOrDirectNode = try version == .latest ? node.getLatest() : node
  if let state = latestOrDirectNode.__state {
    try checkCollision(node, stateConfig, state)
    return state.getValue(stateConfig)
  }
  return stateConfig.defaultValue
}

/// `$getStateChange`: the value and previous value, or `nil` if equal.
public func getStateChange<V>(
  _ node: LexicalNode, _ prevNode: LexicalNode, _ stateConfig: StateConfig<V>
) throws -> (value: V, prevValue: V)? {
  let value = try getState(node, stateConfig, .direct)
  let prevValue = try getState(prevNode, stateConfig, .direct)
  return stateConfig.isEqual(value, prevValue) ? nil : (value, prevValue)
}

func setStateAny<N: LexicalNode>(
  _ node: N, _ stateConfig: AnyStateConfig, _ value: Any
) throws
  -> N
{
  try errorOnReadOnly()
  let writable = try node.getWritable()
  let state = try getWritableNodeState(writable)
  try checkCollision(node, stateConfig, state)
  state.updateFromKnown(stateConfig, value)
  return writable
}

/// `$setState(node, stateConfig, value)`.
@discardableResult
public func setState<N: LexicalNode, V>(
  _ node: N, _ stateConfig: StateConfig<V>, _ value: V
)
  throws -> N
{
  try setStateAny(node, stateConfig, value)
}

/// `$setState(node, stateConfig, updater)`: a no-op when the updater
/// returns an equal value.
@discardableResult
public func setState<N: LexicalNode, V>(
  _ node: N, _ stateConfig: StateConfig<V>, _ updater: (V) throws -> V
) throws -> N {
  try errorOnReadOnly()
  let latest = try node.getLatest()
  let prevValue = try getState(latest, stateConfig)
  let value = try updater(prevValue)
  if stateConfig.isEqual(prevValue, value) {
    return latest
  }
  return try setStateAny(node, stateConfig, value)
}

/// `$getWritableNodeState`.
package func getWritableNodeState(_ node: LexicalNode) throws -> NodeState {
  let writable = try node.getWritable()
  let state =
    try writable.__state?.getWritable(writable)
    ?? NodeState(writable, getSharedNodeState(writable))
  writable.__state = state
  return state
}

/// `$getSharedNodeState`.
package func getSharedNodeState(_ node: LexicalNode) throws -> SharedNodeState {
  if let state = node.__state {
    return state.sharedNodeState
  }
  return try getRegisteredNodeOrThrow(getEditor(), node.getType()).sharedNodeState
}

/// `$updateStateFromJSON`: replaces `node`'s state with the nested `$`
/// object of `serialized`.
package func updateStateFromJSON<T: LexicalNode>(_ node: T, _ serialized: JSONObject) throws -> T {
  let writable = try node.getWritable()
  let unknownState = serialized[NODE_STATE_KEY]?.objectValue
  if writable.__state != nil || unknownState != nil {
    try getWritableNodeState(node).updateFromJSON(unknownState)
  }
  return writable
}

/// `nodeStatesAreEquivalent`.
package func nodeStatesAreEquivalent(_ a: NodeState?, _ b: NodeState?) -> Bool {
  if a === b {
    return true
  }
  var keys = Set<String>()
  if let a, hasUnequalMapEntry(&keys, a, b) { return false }
  if let b, hasUnequalMapEntry(&keys, b, a) { return false }
  if let a, hasUnequalRecordEntry(&keys, a, b) { return false }
  if let b, hasUnequalRecordEntry(&keys, b, a) { return false }
  return true
}

private func hasUnequalMapEntry(
  _ keys: inout Set<String>, _ sourceState: NodeState, _ otherState: NodeState?
) -> Bool {
  for (_, entry) in sourceState.knownState {
    let key = entry.config.key
    if !keys.insert(key).inserted {
      continue
    }
    let otherValue = otherState?.getValueAny(entry.config) ?? entry.config.defaultAny
    if !entry.config.isEqualAny(otherValue, entry.value) {
      return true
    }
  }
  return false
}

private func hasUnequalRecordEntry(
  _ keys: inout Set<String>, _ sourceState: NodeState, _ otherState: NodeState?
) -> Bool {
  guard let unknownState = sourceState.unknownState else {
    return false
  }
  for key in unknownState.keys {
    if !keys.insert(key).inserted {
      continue
    }
    if unknownState[key] != otherState?.unknownState?[key] {
      return true
    }
  }
  return false
}

/// `$carryNodeState`: copies onto `to` the state of `from` that `to` does
/// not have.
public func carryNodeState(_ from: LexicalNode, _ to: LexicalNode) throws {
  guard let fromState = try from.getLatest().__state, fromState.size != 0 else {
    return
  }
  var done = Set<String>()
  if let toState = try to.getLatest().__state {
    for (_, entry) in toState.knownState {
      done.insert(entry.config.key)
    }
    for key in toState.unknownState?.keys ?? [] {
      done.insert(key)
    }
  }
  let state = try getWritableNodeState(to)
  for (_, entry) in fromState.knownState {
    let config = entry.config
    if !done.insert(config.key).inserted {
      continue
    }
    if config.isEqualAny(entry.value, config.defaultAny) {
      continue
    }
    let own = state.sharedNodeState.sharedConfigMap.get(config.key)
    if own == nil || own === config {
      state.updateFromKnown(config, entry.value)
    } else {
      state.updateFromUnknown(config.key, config.unparseAny(entry.value))
    }
  }
  if let unknownState = fromState.unknownState {
    for key in unknownState.keys where !done.contains(key) {
      state.updateFromUnknown(key, unknownState[key])
    }
  }
}

/// `$cloneNodeState`: the state for `to`, a split or copy of `from`.
package func cloneNodeState(_ from: LexicalNode, _ to: LexicalNode) -> NodeState? {
  guard let state = from.__state else {
    return nil
  }
  return state.node === from ? state.getWritable(to) : state
}
