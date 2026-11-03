// The static node configuration (`$config()` in JS) and the per-class record
// LexicalUtils.ts compiles from it: the composed serialization schema, the
// transforms, and the NodeState a class requires.

import Synchronization

/// What `$config()` returns in JS: the configuration a class declares for
/// itself. `klass` is the declaring class, so a subclass that inherits the
/// method can be told apart from one that declares its own.
public struct NodeConfig {
  /// The class this configuration belongs to.
  public let klass: LexicalNode.Type
  /// The node type. `nil` for an abstract base class, which JS keys by a
  /// symbol.
  public var type: String?
  /// The exact superclass. Always name it.
  public var extends: LexicalNode.Type?
  /// The node-specific properties of the serialized JSON, in key order.
  public var json: [SchemaField]
  /// `$transform`: registered for this class and its subclasses.
  public var transform: ((LexicalNode) throws -> Void)?
  /// `$importJSON`.
  public var importJSON: ((JSONObject) throws -> LexicalNode)?
  /// `stateConfigs`: the NodeState the node requires.
  public var stateConfigs: [RequiredNodeStateConfig]
  /// `inline`.
  public var inline: Bool?
  /// `slots`: the canonical order of the node's named slots.
  public var slots: [String]?
  /// `children` (`@experimental node-structure`): what an element of this
  /// class may hold.
  public var children: NodeChildrenConfig?
  /// `mergeAdjacent` (`@experimental node-structure`): when adjacent
  /// siblings of this class are the same thing; `.off` is JS `false`.
  public var mergeAdjacent: MergeAdjacentDeclaration?

  public init(
    _ klass: LexicalNode.Type, type: String? = nil, extends: LexicalNode.Type? = nil,
    json: [SchemaField] = [], transform: ((LexicalNode) throws -> Void)? = nil,
    importJSON: ((JSONObject) throws -> LexicalNode)? = nil,
    stateConfigs: [RequiredNodeStateConfig] = [], inline: Bool? = nil, slots: [String]? = nil,
    children: NodeChildrenConfig? = nil, mergeAdjacent: MergeAdjacentDeclaration? = nil
  ) {
    self.klass = klass
    self.type = type
    self.extends = extends
    self.json = json
    self.transform = transform
    self.importJSON = importJSON
    self.stateConfigs = stateConfigs
    self.inline = inline
    self.slots = slots
    self.children = children
    self.mergeAdjacent = mergeAdjacent
  }
}

/// The superclass of a node class, or `nil` for `LexicalNode`.
package func getSuperclassOf(_ klass: LexicalNode.Type) -> LexicalNode.Type? {
  _getSuperclass(klass) as? LexicalNode.Type
}

/// One link of `iterStaticNodeConfigChain`: a class, and the configuration
/// it declares itself (`nil` when it inherits one).
package struct OwnStaticNodeConfig {
  package let klass: LexicalNode.Type
  package let ownNodeConfig: NodeConfig?
  package let ownNodeType: String?
}

/// `NodeClassRecord`: what is compiled once per class.
package final class NodeClassRecord: @unchecked Sendable {
  package let klass: LexicalNode.Type
  /// The config the class declares itself.
  package let ownNodeConfig: NodeConfig?
  /// The node type, its own or inherited.
  package let ownNodeType: String?
  /// The class and its superclasses, most derived first, each with the
  /// config it declares itself.
  package let chain: [OwnStaticNodeConfig]
  /// Export getters, most-derived-first key order.
  package let getters: [SchemaField]
  /// Import setters, base-first key order.
  package let setters: [SchemaField]
  /// The `withField` properties a clone carries, one per key.
  package let carried: [SchemaField]
  /// Every property of the composed schema, most derived first.
  package let fields: [SchemaField]
  /// The flat NodeState configs, read at the top level of the JSON.
  package let flatStates: [AnyStateConfig]
  /// `$importJSON` of the nearest class that declares one.
  package let importJSON: ((JSONObject) throws -> LexicalNode)?
  /// The `inline` declaration of the nearest class that makes one.
  package let inline: Bool?
  /// What `config()` threw for this class or a superclass, thrown again on
  /// registration.
  package let configError: (any Error)?

  init(_ klass: LexicalNode.Type) {
    self.klass = klass
    var configError: (any Error)?
    var config: NodeConfig?
    do {
      config = try klass.config()
      if let own = config, own.klass == klass {
        try checkFieldNames("nodeSchema", own.json.map(\.key), own.json.map(\.schema))
      }
    } catch {
      configError = error
      config = nil
    }
    let own = config.flatMap { $0.klass == klass ? $0 : nil }
    self.ownNodeConfig = own
    // A class without its own config inherits its superclass's, type
    // included.
    self.ownNodeType = config?.type
    var chain = [OwnStaticNodeConfig(klass: klass, ownNodeConfig: own, ownNodeType: ownNodeType)]
    if let next = own?.extends ?? getSuperclassOf(klass) {
      chain += getNodeClassRecord(next).chain
    }
    self.chain = chain
    self.configError =
      configError
      ?? chain.dropFirst().first.flatMap {
        getNodeClassRecord($0.klass).configError
      }

    // composeSchema: most-derived declaration of each key wins, written in
    // most-derived-first order; setters apply base-first.
    var derivedFirst: [SchemaField] = []
    var seen = Set<String>()
    for link in chain {
      for field in link.ownNodeConfig?.json ?? [] where !seen.contains(field.key) {
        seen.insert(field.key)
        derivedFirst.append(field)
      }
    }
    var baseFirst: [SchemaField] = []
    var placed = Set<String>()
    for link in chain.reversed() {
      for field in link.ownNodeConfig?.json ?? [] where !placed.contains(field.key) {
        placed.insert(field.key)
        baseFirst.append(derivedFirst.first { $0.key == field.key }!)
      }
    }
    var flatStates: [AnyStateConfig] = []
    var flatKeys = Set<String>()
    for link in chain.reversed() {
      for required in link.ownNodeConfig?.stateConfigs ?? [] where required.flat {
        if flatKeys.insert(required.stateConfig.key).inserted {
          flatStates.append(required.stateConfig)
        }
      }
    }
    self.getters = derivedFirst.filter { $0.get != nil }
    self.carried = derivedFirst.filter { $0.carry != nil }
    self.fields = derivedFirst
    self.setters = baseFirst.filter { $0.set != nil }
    self.flatStates = flatStates
    self.importJSON = chain.lazy.compactMap { $0.ownNodeConfig?.importJSON }.first
    self.inline = chain.lazy.compactMap { $0.ownNodeConfig?.inline }.first
  }
}

private let nodeClassCache = Mutex<[ObjectIdentifier: NodeClassRecord]>([:])

/// The compiled record of `klass`, built on first use.
package func getNodeClassRecord(_ klass: LexicalNode.Type) -> NodeClassRecord {
  let id = ObjectIdentifier(klass)
  if let record = nodeClassCache.withLock({ $0[id] }) {
    return record
  }
  // Built outside the lock: building a record builds its superclasses'.
  let record = NodeClassRecord(klass)
  return nodeClassCache.withLock { cache in
    if let existing = cache[id] { return existing }
    cache[id] = record
    return record
  }
}

/// `iterStaticNodeConfigChain`.
package func iterStaticNodeConfigChain(_ klass: LexicalNode.Type) -> [OwnStaticNodeConfig] {
  getNodeClassRecord(klass).chain
}

/// `getStaticNodeConfig`.
package func getStaticNodeConfig(_ klass: LexicalNode.Type) -> OwnStaticNodeConfig {
  getNodeClassRecord(klass).chain[0]
}

/// `getTransformSetFromKlass`: the `$transform` of the class and of each of
/// its superclasses, most derived first. (JS also reads the experimental
/// `static transform()`, which no upstream node declares.)
package func getTransformSetFromKlass(_ klass: LexicalNode.Type) -> [Transform] {
  iterStaticNodeConfigChain(klass).compactMap { link in
    link.ownNodeConfig?.transform.map(Transform.init)
  }
}

/// The export walk (`$walkFromCompiled`): `children` for an element, the
/// schema's properties, then `type` and `version` (legacy) or `type` first
/// (compact).
package func exportNodeJSONOnce(_ node: LexicalNode, _ compact: Bool) throws -> JSONObject {
  let record = getNodeClassRecord(type(of: node))
  var json = JSONObject()
  if compact {
    json["type"] = .string(node.__type)
  }
  if node is ElementNode {
    json["children"] = .array([])
  }
  for entry in record.getters {
    if compact && entry.set == nil {
      continue
    }
    var value = try entry.get!(node)
    if let when = entry.when {
      if try entry.isCompactDefault(value) || !when(node) {
        value = nil
      }
    }
    if compact && entry.isCompactDefault(value) {
      continue
    }
    json.set(entry.key, value)
  }
  if !compact {
    json["type"] = .string(node.__type)
    json["version"] = 1
  }
  return json
}

/// `$applyJSONSetters`: flat NodeState first, then the schema's setters,
/// base first.
package func applyJSONSetters<T: LexicalNode>(_ node: T, _ serializedNode: JSONObject) throws -> T {
  let record = getNodeClassRecord(type(of: node))
  var current = node
  for stateConfig in record.flatStates {
    // Present at the top level wins; absent leaves the nested `$` value.
    if let raw = serializedNode[stateConfig.key] {
      current = try stateConfig.setParsed(current, raw)
    }
  }
  for entry in record.setters {
    try entry.set!(current, entry.schema.parse(serializedNode[entry.key]))
  }
  return current
}

/// `getComposedSchemaFields(klass)`: the schema of every property of
/// `klass`'s JSON, its flat NodeState included, keyed by property.
public func getComposedSchemaFields(_ klass: LexicalNode.Type) -> [String: SerializationSchema] {
  let record = getNodeClassRecord(klass)
  var fields: [String: SerializationSchema] = [:]
  for stateConfig in record.flatStates {
    if let schema = stateConfig.schema {
      fields[stateConfig.key] = schema
    }
  }
  for field in record.fields {
    fields[field.key] = field.schema
  }
  return fields
}
