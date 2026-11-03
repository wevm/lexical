// Ported from packages/lexical/src/LexicalStructure.ts: the structure node
// classes declare in `config()` (`inline`, `children`, `mergeAdjacent`),
// and its normalization, which `NodeStructureExtension` turns on.

import Synchronization

// MARK: Declarations

/// One entry of a `children` list: inline content, or a class (which
/// allows its subclasses).
public enum NodeChildEntry: Equatable {
  case inline
  case klass(LexicalNode.Type)

  public static func == (a: NodeChildEntry, b: NodeChildEntry) -> Bool {
    switch (a, b) {
    case (.inline, .inline): true
    case (.klass(let x), .klass(let y)): x == y
    default: false
    }
  }
}

/// `NodeChildrenConfig` (`@experimental node-structure`): what an element
/// may hold. `.inline` is a paragraph's content, `.block` a table cell's,
/// and `.list` only the listed entries.
public enum NodeChildrenConfig: Equatable {
  case inline
  case block
  case list([NodeChildEntry])

  fileprivate var entries: [NodeChildEntry]? {
    if case .list(let entries) = self { return entries }
    return nil
  }

  /// Whether the list names `'inline'`.
  fileprivate var listsInline: Bool { entries?.contains(.inline) ?? false }
}

/// One value `mergeAdjacent.by` names: a schema field of the class, or a
/// NodeState.
public enum MergeAdjacentEntry: Equatable {
  case field(String)
  case state(AnyStateConfig)

  public static func == (a: MergeAdjacentEntry, b: MergeAdjacentEntry) -> Bool {
    switch (a, b) {
    case (.field(let x), .field(let y)): x == y
    case (.state(let x), .state(let y)): x === y
    default: false
    }
  }
}

/// `MergeAdjacentConfig` (`@experimental node-structure`).
public struct MergeAdjacentConfig: Equatable {
  /// Schema field names of the class, or NodeState configs.
  public let by: [MergeAdjacentEntry]
  public init(by: [MergeAdjacentEntry]) { self.by = by }
}

/// A class's own `mergeAdjacent` declaration: `.off` is JS `false`, which
/// turns off a declaration an ancestor made.
public enum MergeAdjacentDeclaration {
  case by([MergeAdjacentEntry])
  case off
}

/// `DeclaredNodeStructure` (`@experimental node-structure`): the structural
/// facts a class declares, each from the nearest class in its config chain
/// that declares it, and the values it declares derived.
public struct DeclaredNodeStructure {
  public struct DeclaredBy {
    public let inline: LexicalNode.Type?
    public let children: LexicalNode.Type?
    public let mergeAdjacent: LexicalNode.Type?
  }
  public let inline: Bool?
  public let children: NodeChildrenConfig?
  public let mergeAdjacent: MergeAdjacentConfig?
  /// The schema fields and NodeState keys of the class declared `derived`.
  public let derived: Set<String>
  /// The class each key came from.
  public let declaredBy: DeclaredBy
}

/// The key `init(key:)` is called with to evaluate a class's `isInline()`
/// on an instance that belongs to no editor (JS calls it on a bare
/// prototype).
package let STRUCTURE_PROBE_KEY: NodeKey = "__lexical_structure_probe__"

/// `probeOwnIsInline`: what `isInline()` returns on an instance no editor
/// holds, or `nil` when it cannot be evaluated without one (it reads the
/// node's state and throws). JS reads only the class's own `isInline()`;
/// Swift cannot tell an override from the inherited one, so the method as
/// the class resolves it is called. One that returns the declaration (the
/// synthesized one included) agrees with it either way.
private func probeIsInline(_ klass: LexicalNode.Type) -> Bool? {
  guard let node = try? klass.init(key: STRUCTURE_PROBE_KEY) else { return nil }
  return try? node.isInline()
}

private enum InvalidMergeBy {
  case unknown(String)
  case derived(String)
  case derivedState(String)
}

/// Why `config.by` is invalid for `klass`, or nil when it is valid.
private func invalidMergeBy(
  _ klass: LexicalNode.Type, _ config: MergeAdjacentConfig, _ derived: Set<String>
) -> InvalidMergeBy? {
  let fields = getComposedSchemaFields(klass)
  for entry in config.by {
    switch entry {
    case .field(let name):
      if fields[name] == nil {
        return .unknown(name)
      }
      if derived.contains(name) {
        return .derived(name)
      }
    case .state(let state):
      if state.derived {
        return .derivedState(state.key)
      }
    }
  }
  return nil
}

/// The nearest `children` declaration, read without the structure cache.
private func declaredChildren(_ klass: LexicalNode.Type) -> NodeChildrenConfig? {
  for link in iterStaticNodeConfigChain(klass) {
    if let children = link.ownNodeConfig?.children {
      return children
    }
  }
  return nil
}

private func isSubclass(_ klass: LexicalNode.Type, _ of: LexicalNode.Type) -> Bool {
  var k: LexicalNode.Type? = klass
  while let current = k {
    if current == of { return true }
    k = getSuperclassOf(current)
  }
  return false
}

/// For a merging class that lists child classes (no `'inline'`): the first
/// listed class that does not hold inline content (possibly with a nested
/// instance of `klass`), or nil.
private func mergingChildNotInline(
  _ klass: LexicalNode.Type, _ children: NodeChildrenConfig?
) -> LexicalNode.Type? {
  guard let entries = children?.entries, !entries.contains(.inline) else {
    return nil
  }
  for entry in entries {
    guard case .klass(let entryKlass) = entry else { continue }
    let item = declaredChildren(entryKlass)
    var ok = item == .inline
    if !ok, let itemEntries = item?.entries, itemEntries.contains(.inline) {
      ok = itemEntries.allSatisfy { e in
        switch e {
        case .inline: true
        case .klass(let k): isSubclass(klass, k)
        }
      }
    }
    if !ok {
      return entryKlass
    }
  }
  return nil
}

/// The names of the schema fields and NodeState keys declared derived.
private func derivedNamesOf(_ klass: LexicalNode.Type) -> Set<String> {
  var derived = Set<String>()
  for field in getNodeClassRecord(klass).fields where field.derived {
    derived.insert(field.key)
  }
  for link in iterStaticNodeConfigChain(klass) {
    for entry in link.ownNodeConfig?.stateConfigs ?? [] where entry.stateConfig.derived {
      derived.insert(entry.stateConfig.key)
    }
  }
  return derived
}

private final class CachedNodeStructure: @unchecked Sendable {
  let declared: DeclaredNodeStructure
  init(_ declared: DeclaredNodeStructure) { self.declared = declared }
  var children: NodeChildrenConfig? { declared.children }
  var mergeAdjacent: MergeAdjacentConfig? { declared.mergeAdjacent }
  var derivedNames: Set<String> { declared.derived }
  var declaredBy: DeclaredNodeStructure.DeclaredBy { declared.declaredBy }
}

private let structureCache = Mutex<[ObjectIdentifier: CachedNodeStructure]>([:])

private func cachedNodeStructure(_ klass: LexicalNode.Type) -> CachedNodeStructure {
  let id = ObjectIdentifier(klass)
  if let cached = structureCache.withLock({ $0[id] }) {
    return cached
  }
  var inline: Bool?
  var children: NodeChildrenConfig?
  var mergeAdjacent: MergeAdjacentConfig?
  var byInline: LexicalNode.Type?
  var byChildren: LexicalNode.Type?
  var byMergeAdjacent: LexicalNode.Type?
  var mergeSeen = false
  for link in iterStaticNodeConfigChain(klass) {
    guard let own = link.ownNodeConfig else { continue }
    if byInline == nil, let value = own.inline {
      inline = value
      byInline = link.klass
    }
    if byChildren == nil, let value = own.children {
      children = value
      byChildren = link.klass
    }
    if !mergeSeen, let value = own.mergeAdjacent {
      mergeSeen = true
      if case .by(let entries) = value {
        mergeAdjacent = MergeAdjacentConfig(by: entries)
        byMergeAdjacent = link.klass
      }
    }
  }
  let derivedNames = derivedNamesOf(klass)
  // A declaration the class only inherits never makes it invalid: a key the
  // class contradicts, or may contradict, reads as undeclared for it.
  if let by = byInline, by != klass, probeIsInline(klass) != inline {
    inline = nil
    byInline = nil
  }
  if let by = byMergeAdjacent, by != klass,
    invalidMergeBy(klass, mergeAdjacent!, derivedNames) != nil
      || mergingChildNotInline(klass, children) != nil
  {
    mergeAdjacent = nil
    byMergeAdjacent = nil
  }
  let result = CachedNodeStructure(
    DeclaredNodeStructure(
      inline: inline, children: children, mergeAdjacent: mergeAdjacent, derived: derivedNames,
      declaredBy: DeclaredNodeStructure.DeclaredBy(
        inline: byInline, children: byChildren, mergeAdjacent: byMergeAdjacent)))
  return structureCache.withLock { cache in
    if let existing = cache[id] { return existing }
    cache[id] = result
    return result
  }
}

/// `getDeclaredNodeStructure(klass)` (`@experimental node-structure`): the
/// structural declarations of a node class. Depends on the class only.
public func getDeclaredNodeStructure(_ klass: LexicalNode.Type) -> DeclaredNodeStructure {
  cachedNodeStructure(klass).declared
}

/// Whether a class is `OverflowNode` (`@lexical/overflow`) or a subclass:
/// a class in its chain whose own config names the type `overflow` and
/// extends `ElementNode`.
private func isOverflowClass(_ klass: LexicalNode.Type) -> Bool {
  getNodeClassRecord(klass).chain.contains { link in
    link.ownNodeConfig != nil && link.ownNodeType == "overflow"
      && link.ownNodeConfig?.extends == ElementNode.self
  }
}

private func isOverflowNode(_ node: LexicalNode) -> Bool {
  isOverflowClass(type(of: node))
}

private func isInlineChild(_ node: LexicalNode) -> Bool {
  if node is TextNode || node is LineBreakNode || isOverflowNode(node) {
    return true
  }
  // A class outside the Text/Element/Decorator contract: left alone.
  return (try? node.isInline()) ?? true
}

/// `INTERNAL_isInlineContent`: whether node structure normalization treats
/// `node` as inline content: text, line breaks, inline elements and
/// decorators, and `OverflowNode`.
public func INTERNAL_isInlineContent(_ node: LexicalNode) -> Bool {
  isInlineChild(node)
}

/// A leaf the normalization never removes and that sits in a line.
private func isInlineLeaf(_ node: LexicalNode) -> Bool {
  node is TextNode || node is LineBreakNode || (node is DecoratorNode && isInlineChild(node))
}

private func allows(_ children: NodeChildrenConfig, _ child: LexicalNode) -> Bool {
  if isOverflowNode(child) {
    return true
  }
  switch children {
  case .inline:
    return isInlineChild(child)
  case .block:
    return !isInlineChild(child)
  case .list(let entries):
    if isInlineChild(child) {
      return entries.contains(.inline)
    }
    for entry in entries {
      if case .klass(let klass) = entry, isSubclass(type(of: child), klass) {
        return true
      }
    }
    return false
  }
}

/// What `node` may hold: a root or shadow root holds blocks whatever its
/// class declares, any other node what its class declares.
private func effectiveChildren(_ node: ElementNode) throws -> NodeChildrenConfig? {
  try isRootOrShadowRoot(node) ? .block : cachedNodeStructure(type(of: node)).children
}

/// Whether `node` is a plain paragraph whose content `holderChildren`
/// allows.
private func isJoinableParagraph(
  _ holderChildren: NodeChildrenConfig, _ node: LexicalNode
)
  throws -> Bool
{
  guard let paragraph = node as? ParagraphNode, !paragraph.isShadowRoot() else { return false }
  return try paragraph.getChildren().allSatisfy { allows(holderChildren, $0) }
}

/// A selection point, as a position next to a node that survives repairs.
private enum Anchor {
  case keys([NodeKey], after: Bool)
  case inside(NodeKey)
  case text(NodeKey, offset: Int)
}

/// `node` and its last (or first) descendants down to a leaf.
private func descend(_ node: LexicalNode, _ last: Bool) throws -> [NodeKey] {
  var keys: [NodeKey] = []
  var n: LexicalNode? = node
  while let current = n {
    keys.append(current.getKey())
    if let element = current as? ElementNode {
      n = try last ? element.getLastChild() : element.getFirstChild()
    } else {
      n = nil
    }
  }
  return keys
}

/// An element point as the node next to it, so that it keeps its place
/// among the content when its element is split, unwrapped or merged.
private func saveAnchor(_ point: Point) throws -> Anchor? {
  if point.type == .text {
    // Text nodes are moved, never removed, but detaching one moves a point
    // in it to its parent: it is put back afterwards.
    return .text(point.key, offset: point.offset)
  }
  guard let node = try point.getNode() as? ElementNode else {
    return nil
  }
  // A point past the end is at the end.
  let offset = try min(point.offset, node.getChildrenSize())
  let prevNode = try offset > 0 ? node.getChildAtIndex(offset - 1) : nil
  let nextNode = try node.getChildAtIndex(offset)
  if let prevNode, isInlineLeaf(prevNode) {
    return .keys([prevNode.getKey()], after: true)
  }
  if let nextNode, isInlineLeaf(nextNode) {
    return .keys([nextNode.getKey()], after: false)
  }
  var prev = prevNode
  var next = nextNode
  let rule = try effectiveChildren(node)
  if rule == .inline || (rule?.listsInline ?? false) {
    // In a node that holds lines, a block with nothing in it is passed
    // over: a point on either side of it is at the same place in the
    // content.
    while let p = prev, try isPassedOver(p) {
      prev = try p.getPreviousSibling()
    }
    while let n = next, try isPassedOver(n) {
      next = try n.getNextSibling()
    }
    if prev == nil && next == nil {
      prev = prevNode
      next = nextNode
    }
  }
  if let prev, isInlineLeaf(prev) {
    return .keys([prev.getKey()], after: true)
  }
  if let next, isInlineLeaf(next) {
    return .keys([next.getKey()], after: false)
  }
  if let next {
    return .keys(try descend(next, false), after: false)
  }
  if let prev {
    return .keys(try descend(prev, true), after: true)
  }
  return .inside(node.getKey())
}

/// A block a point is passed over: an element that is not inline, not a
/// paragraph and holds no leaf at any depth.
private func isPassedOver(_ node: LexicalNode) throws -> Bool {
  guard let element = node as? ElementNode, !(node is ParagraphNode), !isInlineChild(node) else {
    return false
  }
  func holdsLeaf(_ n: ElementNode) throws -> Bool {
    try n.getChildren().contains { c in
      guard let e = c as? ElementNode else { return true }
      return try holdsLeaf(e)
    }
  }
  return try !holdsLeaf(element)
}

private final class Normalization {
  /// The dirty nodes and all their ancestors, and what the repairs put in a
  /// parent: the only nodes visited (unless `full`).
  var path = Set<NodeKey>()
  /// The keys in `path` by the parent they were added under.
  let pathByParent = JSMap<NodeKey, JSSet<NodeKey>>()
  /// Nodes the repairs put in a parent (lifted blocks, copies, wrappers).
  let touched = JSSet<NodeKey>()
  /// Where a removed childless element's caret goes.
  var substitutes: [NodeKey: Anchor] = [:]
  /// Elements whose children the repairs changed.
  let written = JSSet<NodeKey>()
  var changed = false
  /// Whether every descendant is visited, not only those in `path`.
  var full = false
  /// Whether this is `INTERNAL_normalizeNodeStructure`'s normalization.
  var explicit = false
  /// The roots and shadow roots whose inline children are all wrapped.
  var wrapsAll = Set<NodeKey>()
  private var childrenCache: [NodeKey: (size: Int, nodes: [LexicalNode])] = [:]

  /// Add `node`, under its current parent, to the nodes visited.
  func addPath(_ node: LexicalNode) throws {
    let key = node.getKey()
    path.insert(key)
    if let parent = try node.getLatest().__parent {
      var keys = pathByParent.get(parent)
      if keys == nil {
        keys = JSSet()
        _ = pathByParent.set(parent, keys!)
      }
      _ = keys!.add(key)
    }
  }

  /// The children of `parent` to visit.
  func children(_ parent: ElementNode) throws -> [LexicalNode] {
    if full {
      return try parent.getChildren()
    }
    let parentKey = parent.getKey()
    guard let keys = pathByParent.get(parentKey) else {
      return []
    }
    // The keys of a parent only grow: the same count is the same keys.
    if let cached = childrenCache[parentKey], cached.size == keys.size {
      return cached.nodes
    }
    var nodes: [LexicalNode] = []
    for key in keys {
      if let node = try getNodeByKey(key), try node.getLatest().__parent == parentKey {
        nodes.append(node)
      }
    }
    childrenCache[parentKey] = (keys.size, nodes)
    return nodes
  }

  /// Add `node`, and its ancestors up to one already visited under its
  /// current parent, to the nodes visited.
  func addPathChain(_ node: LexicalNode) throws {
    var n: LexicalNode? = node
    while let current = n {
      if let parent = try current.getLatest().__parent, let keys = pathByParent.get(parent),
        keys.has(current.getKey())
      {
        return
      }
      try addPath(current)
      n = try current.getParent()
    }
  }

  /// After a class's own seam hook: the entries of every removed parent go
  /// to where its children are now.
  func repath() throws {
    for (parentKey, keys) in Array(pathByParent) {
      if let parent = try getNodeByKey(parentKey), try parent.isAttached() {
        continue
      }
      _ = pathByParent.delete(parentKey)
      for key in keys {
        if let node = try getNodeByKey(key), try node.isAttached() {
          try addPathChain(node)
        }
      }
    }
  }

  /// `from`'s children were moved to `to` (a merge): so are their entries.
  func movePath(_ from: NodeKey, _ to: ElementNode) throws {
    if let keys = pathByParent.get(from) {
      _ = pathByParent.delete(from)
      for key in keys {
        if let node = try getNodeByKey(key) {
          try addPath(node)
        }
      }
    }
    try addPath(to)
  }

  func resolve(_ anchor: Anchor, _ depth: Int = 0) throws -> (NodeKey, Int, PointType)? {
    switch anchor {
    case .text(let key, let offset):
      guard let node = try getNodeByKey(key), try node.isAttached() else { return nil }
      return (key, offset, .text)
    case .inside(let key):
      if let node = try getNodeByKey(key), try node.isAttached() {
        return (key, 0, .element)
      }
      if let substitute = substitutes[key], depth < 16 {
        return try resolve(substitute, depth + 1)
      }
      return nil
    case .keys(let keys, let after):
      // From the innermost key out: the leaf the point was next to is
      // moved, never removed.
      for i in stride(from: keys.count - 1, through: 0, by: -1) {
        let node = try getNodeByKey(keys[i])
        guard let node, try node.isAttached(), try node.getParent() != nil else {
          // A node the repairs removed says where its place went.
          if let substitute = substitutes[keys[i]], depth < 16 {
            return try resolve(substitute, depth + 1)
          }
          continue
        }
        // Climb back out while the node is still the last child (or first,
        // for a point before it).
        var at: LexicalNode = node
        for _ in 0..<i {
          guard let parent = try at.getParent(), try !isRootOrShadowRoot(parent),
            try (after ? at.getNextSibling() : at.getPreviousSibling()) == nil
          else {
            break
          }
          at = parent
        }
        if let text = at as? TextNode {
          // Next to a text node in its own parent: a point in it.
          return (text.getKey(), after ? try text.getTextContentSize() : 0, .text)
        }
        return (
          try at.getParentOrThrow().getKey(), try at.getIndexWithinParent() + (after ? 1 : 0),
          .element
        )
      }
      return nil
    }
  }
}

private func append(_ parent: ElementNode, _ nodes: [LexicalNode]) throws {
  if !nodes.isEmpty {
    // ElementNode's own splice: an override must not rewrite what the
    // repair moves.
    try parent.elementSplice(parent.getChildrenSize(), 0, nodes)
  }
}

private func detach(_ node: LexicalNode) throws {
  // Never through an override, never removing a parent it empties, never
  // moving the selection (the normalization restores it).
  try removeNode(node, restoreSelection: false, preserveEmptyParent: true)
}

/// The value of a NodeState on `node`, type-erased.
private func getStateAny(_ node: LexicalNode, _ stateConfig: AnyStateConfig) throws -> Any {
  try node.getLatest().__state?.getValueAny(stateConfig) ?? stateConfig.defaultAny
}

/// Whether `a` and its next sibling `b` are the same thing under the
/// `mergeAdjacent` of `a`'s class, which declares it itself.
private func canMergeAdjacent(_ a: LexicalNode, _ b: LexicalNode) throws -> Bool {
  guard type(of: a) == type(of: b), a is ElementNode, b is ElementNode else {
    return false
  }
  let klass = type(of: a)
  let declared = cachedNodeStructure(klass)
  guard let config = declared.mergeAdjacent, declared.declaredBy.mergeAdjacent == klass else {
    return false
  }
  var ja: JSONObject?
  var jb: JSONObject?
  for entry in config.by {
    switch entry {
    case .field(let name):
      ja = try ja ?? a.exportJSON()
      jb = try jb ?? b.exportJSON()
      if ja![name] != jb![name] {
        return false
      }
    case .state(let state):
      if try !state.isEqualAny(getStateAny(a, state), getStateAny(b, state)) {
        return false
      }
    }
  }
  return true
}

/// A class's join of what meets at the seam when two of its nodes merge
/// (`INTERNAL_$afterStructureMerge`, a list joining two items that only
/// wrap nested lists).
public protocol NodeStructureMergeHook: ElementNode {
  func INTERNAL_afterStructureMerge(_ last: LexicalNode, _ first: LexicalNode) throws
}

/// A class's hook for a copy the normalization splits off a node
/// (`INTERNAL_$afterStructureSplit`, an ordered list continuing its
/// numbering).
public protocol NodeStructureSplitHook: ElementNode {
  func INTERNAL_afterStructureSplit(_ original: ElementNode, _ before: [LexicalNode]) throws
}

/// A holder's hook for a child its declared `children` allow that must
/// still be lifted out, given what else it holds
/// (`INTERNAL_$liftsFromStructure`, a list item lifting a nested list that
/// is not all it holds).
public protocol NodeStructureLiftHook: ElementNode {
  func INTERNAL_liftsFromStructure(_ child: LexicalNode, _ explicit: Bool) throws -> Bool
}

/// Append `b`'s children to `a` (its previous sibling) and remove `b`;
/// `a`'s values win. Children that meet at the seam merge in turn.
private func mergeInto(_ a: ElementNode, _ b: ElementNode, _ ctx: Normalization) throws {
  let aLast = try a.getLastChild()
  let bFirst = try b.getFirstChild()
  ctx.substitutes[b.getKey()] =
    try aLast.map { .keys(try descend($0, true), after: true) } ?? .inside(a.getKey())
  try append(a, b.getChildren())
  try detach(b)
  try ctx.movePath(b.getKey(), a)
  ctx.changed = true
  _ = ctx.written.add(a.getKey())
  guard let aLast, let bFirst else {
    return
  }
  if let aLastElement = aLast as? ElementNode, try canMergeAdjacent(aLast, bFirst) {
    try mergeInto(aLastElement, bFirst as! ElementNode, ctx)
    return
  }
  if let hook = a as? NodeStructureMergeHook {
    try hook.INTERNAL_afterStructureMerge(aLast, bFirst)
    if !ctx.full {
      // The hook moves nodes the normalization may not have visited yet.
      try ctx.repath()
    }
  }
}

/// Merge the run of mutually equal siblings around `node` into its first
/// node, and return that node.
private func mergeRun(_ node: LexicalNode, _ ctx: Normalization) throws -> LexicalNode {
  var first = node
  while let prev = try first.getPreviousSibling(), try canMergeAdjacent(prev, first) {
    first = prev
  }
  while let next = try first.getNextSibling(), try canMergeAdjacent(first, next) {
    try mergeInto(first as! ElementNode, next as! ElementNode, ctx)
  }
  return first
}

/// Merge the equal adjacent children of `parent`: every one when `all` (or
/// the normalization is `full`), otherwise those around the children to
/// visit.
private func mergeChildren(
  _ parent: ElementNode, _ ctx: Normalization, _ all: Bool = false
)
  throws
{
  if all || ctx.full {
    var c = try parent.getFirstChild()
    while let current = c {
      c = try mergeRun(current, ctx).getNextSibling()
    }
    return
  }
  for child in try ctx.children(parent) {
    // Only nodes of one class that declares mergeAdjacent itself merge.
    let klass = type(of: child)
    if cachedNodeStructure(klass).declaredBy.mergeAdjacent == klass,
      try child.getLatest().__parent == parent.getKey()
    {
      _ = try mergeRun(child, ctx)
    }
  }
}

/// A run of a holder's content between two lifted blocks.
private final class Segment {
  var content: [LexicalNode] = []
  var hadChildren = false
  /// The holder that received the segment, set by `rebuild`.
  var holder: ElementNode?
}

private enum RebuildPiece {
  case segment(Segment)
  case lifted(LexicalNode)
}

/// Split `holder` into its segments: the original keeps the first segment
/// that had children, copies take the others, and the lifted blocks go
/// between them, all in the holder's place in its parent. Returns the
/// holders.
private func rebuild(
  _ holder: ElementNode, _ pieces: [RebuildPiece], _ ctx: Normalization
)
  throws -> [ElementNode]
{
  ctx.changed = true
  for child in try holder.getChildren() {
    try detach(child)
  }
  var placed: [LexicalNode] = []
  var holders: [ElementNode] = []
  var preceding: [LexicalNode] = []
  var holderPlaced = false
  for piece in pieces {
    switch piece {
    case .lifted(let node):
      placed.append(node)
    case .segment(let segment):
      if !segment.hadChildren {
        continue
      }
      let target: ElementNode
      if !holderPlaced {
        holderPlaced = true
        target = holder
      } else {
        target = try copyNode(holder)
        if let hook = target as? NodeStructureSplitHook {
          try hook.INTERNAL_afterStructureSplit(holder, preceding)
        }
      }
      try append(target, segment.content)
      segment.holder = target
      preceding += segment.content
      placed.append(target)
      holders.append(target)
    }
  }
  let parent = try holder.getParentOrThrow()
  _ = ctx.written.add(parent.getKey())
  let at = placed.firstIndex { $0 === holder } ?? -1
  var anchor: LexicalNode = holder
  if at > 0 {
    for i in stride(from: at - 1, through: 0, by: -1) {
      try insertSibling(anchor, .previous, placed[i], false)
      anchor = placed[i]
    }
  }
  anchor = holder
  for i in (at + 1)..<placed.count {
    try insertSibling(anchor, .next, placed[i], false)
    anchor = placed[i]
  }
  for node in placed {
    try ctx.addPath(node)
    _ = ctx.touched.add(node.getKey())
  }
  if at == -1 {
    // Every child was lifted out: the emptied holder goes, and a point in
    // it goes before what was lifted out of it.
    if let first = placed.first {
      ctx.substitutes[holder.getKey()] = .keys(try descend(first, false), after: false)
    }
    try detach(holder)
  }
  return holders
}

private enum InlineKind { case keep, join, lift }
private enum ClassKind { case keep, wrap, flatten, lift }

/// What a holder of inline content does with each child.
private func inlineRuleKinds(
  _ holder: ElementNode, _ rule: NodeChildrenConfig, _ children: [LexicalNode], _ explicit: Bool
) throws -> [InlineKind] {
  let inlineHolder = try holder.isInline()
  let lifts = holder as? NodeStructureLiftHook
  return try children.map { c in
    if allows(rule, c) {
      return try lifts?.INTERNAL_liftsFromStructure(c, explicit) ?? false ? .lift : .keep
    }
    if try !inlineHolder && isJoinableParagraph(rule, c) {
      return .join
    }
    return isInlineChild(c) ? .keep : .lift
  }
}

/// What a holder that lists the classes it allows (no `'inline'`) does
/// with each child.
private func classRuleKinds(
  _ rule: NodeChildrenConfig, _ children: [LexicalNode]
) throws
  -> [ClassKind]
{
  let wrapper = firstListedClass(rule)
  let wrapperChildren = wrapper.flatMap { declaredChildren($0) }
  return try children.map { c in
    if allows(rule, c) {
      return .keep
    }
    if wrapper != nil && (wrapperChildren == nil || allows(wrapperChildren!, c)) {
      return .wrap
    }
    if wrapper != nil, let wrapperChildren, try isJoinableParagraph(wrapperChildren, c) {
      return .flatten
    }
    return isInlineChild(c) ? .keep : .lift
  }
}

/// `rule[0]` as a class (a list that names no `'inline'` lists classes).
private func firstListedClass(_ rule: NodeChildrenConfig) -> ElementNode.Type? {
  guard case .klass(let klass)? = rule.entries?.first else { return nil }
  return klass as? ElementNode.Type
}

/// Before `block` is lifted out of the inline element `holder`, wrap each
/// run of inline content in it, and in every block it holds at any depth,
/// in a copy of `holder`.
private func wrapLiftedContent(
  _ block: ElementNode, _ holder: ElementNode, _ ctx: Normalization
)
  throws
{
  var copies: [ElementNode] = []
  var run: [LexicalNode] = []
  func flush() throws {
    if !run.isEmpty {
      let copy = try copyNode(holder)
      try insertSibling(run[0], .previous, copy, false)
      try append(copy, run)
      copies.append(copy)
      run = []
    }
  }
  for child in try block.getChildren() {
    // An OverflowNode is not wrapped but wrapped inside.
    if isInlineChild(child) && !isOverflowNode(child) {
      run.append(child)
      continue
    }
    try flush()
    if let element = child as? ElementNode {
      try wrapLiftedContent(element, holder, ctx)
    }
  }
  try flush()
  if copies.isEmpty {
    return
  }
  ctx.changed = true
  _ = ctx.written.add(block.getKey())
  if let rule = try effectiveChildren(block), copies.contains(where: { !allows(rule, $0) }) {
    _ = try applyRules(block, rule, true, ctx)
  }
}

/// A holder of inline content: plain paragraphs become lines of it, and
/// other blocks are lifted out, the holder split around them.
private func applyInlineRule(
  _ holder: ElementNode, _ rule: NodeChildrenConfig, _ ctx: Normalization
) throws -> [ElementNode] {
  let inlineHolder = try holder.isInline()
  let children = try holder.getChildren()
  let kinds = try inlineRuleKinds(holder, rule, children, ctx.explicit)
  if kinds.allSatisfy({ $0 == .keep }) {
    return [holder]
  }
  var pieces: [RebuildPiece] = []
  var emptyLines: [(paragraph: ElementNode, segment: Segment, at: Int)] = []
  var segment = Segment()
  enum Last { case none, inline, line, block }
  var last = Last.none
  for (i, child) in children.enumerated() {
    let kind = kinds[i]
    if kind == .lift {
      if inlineHolder, let element = child as? ElementNode {
        // Out of an inline element, the block's inline content is wrapped
        // in copies of it first.
        try wrapLiftedContent(element, holder, ctx)
      }
      pieces.append(.segment(segment))
      pieces.append(.lifted(child))
      segment = Segment()
      last = .none
      continue
    }
    segment.hadChildren = true
    if kind == .join {
      if last == .inline || last == .line {
        segment.content.append(try createLineBreakNode())
      }
      let paragraph = child as! ElementNode
      let content = try paragraph.getChildren()
      if content.isEmpty {
        emptyLines.append((paragraph, segment, segment.content.count))
      }
      segment.content += content
      last = .line
      continue
    }
    let unit: Last = isInlineChild(child) ? .inline : .block
    if unit == .inline && last == .line {
      segment.content.append(try createLineBreakNode())
    }
    segment.content.append(child)
    last = unit
  }
  pieces.append(.segment(segment))
  let holders = try rebuild(holder, pieces, ctx)
  for (paragraph, s, at) in emptyLines {
    // An empty paragraph's line: after what precedes it in its segment, or
    // before what follows, or the holder of its segment.
    let before = at > 0 ? s.content[at - 1] : nil
    let after = at < s.content.count ? s.content[at] : nil
    if let before {
      ctx.substitutes[paragraph.getKey()] = .keys(try descend(before, true), after: true)
    } else if let after {
      ctx.substitutes[paragraph.getKey()] = .keys(try descend(after, false), after: false)
    } else {
      ctx.substitutes[paragraph.getKey()] = .inside((s.holder ?? holder).getKey())
    }
  }
  return holders
}

/// A holder that lists the classes it allows and not `'inline'`: a child
/// the first listed class can hold is wrapped in one, a plain paragraph
/// becomes one holding its content, and any other block is lifted out.
private func applyClassRule(
  _ holder: ElementNode, _ rule: NodeChildrenConfig, _ ctx: Normalization
) throws -> [ElementNode] {
  let wrapper = firstListedClass(rule)
  let children = try holder.getChildren()
  let kinds = try classRuleKinds(rule, children)
  if kinds.allSatisfy({ $0 == .keep }) {
    return [holder]
  }
  var pieces: [RebuildPiece] = []
  var segment = Segment()
  var i = 0
  while i < children.count {
    defer { i += 1 }
    let child = children[i]
    let kind = kinds[i]
    if kind == .lift {
      pieces.append(.segment(segment))
      pieces.append(.lifted(child))
      segment = Segment()
      continue
    }
    segment.hadChildren = true
    if kind == .keep {
      segment.content.append(child)
    } else if kind == .flatten {
      let w = try create(wrapper!)
      try append(w, (child as! ElementNode).getChildren())
      ctx.substitutes[child.getKey()] = .inside(w.getKey())
      segment.content.append(w)
    } else {
      var run = [child]
      while isInlineChild(child) && i + 1 < children.count && kinds[i + 1] == .wrap
        && isInlineChild(children[i + 1])
      {
        i += 1
        run.append(children[i])
      }
      let w = try create(wrapper!)
      try append(w, run)
      segment.content.append(w)
    }
  }
  pieces.append(.segment(segment))
  return try rebuild(holder, pieces, ctx)
}

/// A holder of blocks: each run of inline children is wrapped in the
/// element its first node creates as a parent (a paragraph).
private func wrapInlineRun(_ child: LexicalNode, _ ctx: Normalization) throws {
  func blocked(_ n: LexicalNode?) -> Bool { n != nil && !allows(.block, n!) }
  if !blocked(child) {
    return
  }
  var first = child
  while let p = try first.getPreviousSibling(), blocked(p) {
    first = p
  }
  var run: [LexicalNode] = [first]
  var n = try first.getNextSibling()
  while let current = n, blocked(current) {
    run.append(current)
    n = try current.getNextSibling()
  }
  let w = try first.createParentElementNode()
  try insertSibling(first, .previous, w, false)
  try append(w, run)
  ctx.changed = true
  _ = ctx.written.add(w.getKey())
  try ctx.addPath(w)
  _ = ctx.touched.add(w.getKey())
}

/// Whether the rule inputs of a class's nodes can differ between nodes. JS
/// asks whether it overrides `isShadowRoot()` or `isInline()`; Swift cannot
/// detect an override, so every class but the root counts as one that may.
private func hasInstanceRuleInputs(_ klass: LexicalNode.Type) -> Bool {
  klass != RootNode.self
}

/// Whether the children of `holder` must all be checked against its rule:
/// the normalization is `full`, or the holder was never visited in its
/// current or committed version, or what its rule depends on changed since,
/// or the holder has a hook that lifts allowed children.
private func checksAllChildren(_ holder: ElementNode, _ ctx: Normalization) throws -> Bool {
  if holder is NodeStructureLiftHook {
    return true
  }
  if holder is RootNode || !hasInstanceRuleInputs(type(of: holder)) {
    return ctx.full
  }
  let latest = try holder.getLatest()
  let inputs = (try isRootOrShadowRoot(holder) ? 1 : 0) + (isInlineChild(holder) ? 2 : 0)
  var before = latest.__structureRuleInputs
  if before == nil {
    let committed = try getEditor().getEditorState()._nodeMap.get(holder.getKey())
    before = (committed as? ElementNode)?.__structureRuleInputs
  }
  latest.__structureRuleInputs = inputs
  return ctx.full || before != inputs
}

/// Apply `holder`'s rule. Returns the holders it was split into, or nil
/// when it kept its children as they were.
@discardableResult
private func applyRules(
  _ holder: ElementNode, _ rule: NodeChildrenConfig, _ checksAll: Bool, _ ctx: Normalization
) throws -> [ElementNode]? {
  if try rule == .block && checksAll && !holdsInline(holder) {
    // The common case (the root in every update).
    return nil
  }
  let holderKey = holder.getKey()
  let children =
    try checksAll
    ? holder.getChildren()
    : ctx.children(holder).filter { try $0.getLatest().__parent == holderKey }
  if rule == .block {
    for c in children {
      if try c.getLatest().__parent == holderKey {
        try wrapInlineRun(c, ctx)
      }
    }
    return nil
  }
  if rule == .inline || rule.listsInline {
    return try inlineRuleKinds(holder, rule, children, ctx.explicit).allSatisfy { $0 == .keep }
      ? nil : applyInlineRule(holder, rule, ctx)
  }
  return try classRuleKinds(rule, children).allSatisfy { $0 == .keep }
    ? nil : applyClassRule(holder, rule, ctx)
}

/// Normalize `node`'s subtree bottom-up. Returns whether `node` was
/// removed.
@discardableResult
private func normalizeElement(_ node: ElementNode, _ ctx: Normalization) throws -> Bool {
  let rule = try effectiveChildren(node)
  // Decided once per visit: it records what the rule depends on.
  var checksAll = false
  if rule != nil {
    checksAll =
      try checksAllChildren(node, ctx) || (rule == .block && ctx.wrapsAll.contains(node.getKey()))
  }
  if !ctx.full && !ctx.pathByParent.has(node.getKey()) {
    // Written, but none of its children was.
    if try node.isEmpty(), try !isRootOrShadowRoot(node), try isRemovedWhenEmpty(node) {
      try removeEmpty(node, ctx)
      return true
    }
    if !checksAll {
      return false
    }
  }
  try mergeChildren(node, ctx)
  var removedChild = false
  for c in try ctx.children(node) {
    if let element = c as? ElementNode, try c.getLatest().__parent == node.getKey(),
      try normalizeElement(element, ctx)
    {
      removedChild = true
    }
  }
  if try node.isEmpty(), try !isRootOrShadowRoot(node),
    try isRemovedWhenEmpty(node) || (removedChild && !node.canBeEmpty())
  {
    try removeEmpty(node, ctx)
    return true
  }
  let holders = try rule.flatMap { try applyRules(node, $0, checksAll, ctx) }
  if let holders {
    // A rebuilt holder's children were all moved: every pair is new.
    for holder in holders {
      try mergeChildren(holder, ctx, true)
    }
  } else {
    try mergeChildren(node, ctx)
  }
  return false
}

/// Whether Lexical removes `node` once it is empty: an inline element that
/// cannot be empty, or `OverflowNode`.
private func isRemovedWhenEmpty(_ node: ElementNode) throws -> Bool {
  try isOverflowNode(node) || (isInlineChild(node) && !node.canBeEmpty())
}

/// Remove an empty `node`.
private func removeEmpty(_ node: ElementNode, _ ctx: Normalization) throws {
  let prev = try node.getPreviousSibling()
  let next = try node.getNextSibling()
  let parent = try node.getParentOrThrow()
  if let prev {
    ctx.substitutes[node.getKey()] = .keys(try descend(prev, true), after: true)
  } else if let next {
    ctx.substitutes[node.getKey()] = .keys(try descend(next, false), after: false)
  } else {
    ctx.substitutes[node.getKey()] = .inside(parent.getKey())
  }
  try detach(node)
  // The siblings it separated are now adjacent: they may merge.
  for sibling in [prev, next] {
    if let sibling {
      try ctx.addPath(sibling)
      _ = ctx.touched.add(sibling.getKey())
    }
  }
  ctx.changed = true
  _ = ctx.written.add(parent.getKey())
}

/// A class whose rule does not depend on the node and that does not merge.
private func isStructurallyStatic(_ klass: LexicalNode.Type) -> Bool {
  !hasInstanceRuleInputs(klass) && cachedNodeStructure(klass).declaredBy.mergeAdjacent != klass
}

/// Whether a written element is, as far as the normalization is concerned,
/// what it was in the committed state.
private func isStructurallyUnchanged(_ node: LexicalNode, _ committed: LexicalNode?) -> Bool {
  guard let committed = committed as? ElementNode, let n = node as? ElementNode else {
    return false
  }
  let klass = type(of: node)
  if type(of: committed) != klass || !isStructurallyStatic(klass) {
    return false
  }
  return committed.__parent == n.__parent && committed.__prev == n.__prev
    && committed.__next == n.__next && committed.__first == n.__first
    && committed.__last == n.__last && committed.__size == n.__size
}

/// Repair the structure around every node written since the last pass,
/// from the root down.
private func normalizeDirty(
  _ dirty: inout [NodeKey], _ full: Bool, _ committedNormalized: Bool
)
  throws
{
  let root = try getRoot()
  if try dirty.isEmpty && !full && !holdsInline(root) {
    return
  }
  let ctx = Normalization()
  ctx.full = full
  let rootKey = root.getKey()
  // Core's shadow root transform, which the extension turns off, wraps the
  // inline children of the root in every update and those of a shadow root
  // that was written: so does this.
  ctx.wrapsAll.insert(rootKey)
  ctx.path.insert(rootKey)
  let committed = try committedNormalized ? getEditor().getEditorState()._nodeMap : nil
  var chain: [LexicalNode] = []
  for key in full ? [] : dirty {
    chain.removeAll()
    var attached = false
    let node = try getNodeByKey(key)
    if let element = node as? ElementNode, element.isShadowRoot() {
      ctx.wrapsAll.insert(key)
    }
    if let committed, let node, isStructurallyUnchanged(node, committed.get(key)) {
      continue
    }
    var n = node
    while let current = n {
      if ctx.path.contains(current.getKey()) {
        attached = true
        break
      }
      chain.append(current)
      n = try current.getParent()
    }
    if attached {
      for n in chain {
        try ctx.addPath(n)
      }
    }
  }
  dirty.removeAll()
  if try ctx.path.count > 1 || full || holdsInline(root) {
    try normalize(ctx) { try normalizeElement(root, ctx) }
  }
}

/// Whether `node` is an element whose `isInline()` returns false.
private func isBlockElement(_ node: LexicalNode) -> Bool {
  guard let element = node as? ElementNode else { return false }
  return (try? !element.isInline()) ?? false
}

/// Whether `node` holds content that a holder of blocks wraps.
private func holdsInline(_ node: ElementNode) throws -> Bool {
  var c = try node.getFirstChild()
  while let current = c {
    if !isBlockElement(current) && !allows(.block, current) {
      return true
    }
    c = try current.getNextSibling()
  }
  return false
}

/// Run `repair`, then make every ancestor of a change dirty, and put the
/// selection back among the content it was in.
private func normalize(_ ctx: Normalization, _ repair: () throws -> Void) throws {
  var saved: [(anchor: Anchor?, point: Point)] = []
  if let selection = try getSelection() as? RangeSelection {
    for point in [selection.anchor, selection.focus] {
      saved.append((try saveAnchor(point), point))
    }
  }
  try repair()
  for key in ctx.written {
    if let node = try getNodeByKey(key), try node.isAttached() {
      var n: LexicalNode? = node
      while let current = n, !(current is RootNode) {
        try current.markDirty()
        n = try current.getParent()
      }
    }
  }
  if ctx.changed {
    for (anchor, point) in saved {
      if let anchor, let at = try ctx.resolve(anchor) {
        try point.set(at.0, at.1, at.2)
      }
    }
  }
}

/// `INTERNAL_$normalizeNodeStructure` (internal): normalize the subtree
/// rooted at `node` now, as `NodeStructureExtension` does, then merge
/// `node` (or what it was split into) with its equal siblings.
public func INTERNAL_normalizeNodeStructure(_ node: LexicalNode) throws {
  guard let node = node as? ElementNode, try node.isAttached() else {
    return
  }
  let ctx = Normalization()
  ctx.full = true
  ctx.explicit = true
  let parent = try node.getParent()
  try normalize(ctx) {
    if try parent == nil || isRootOrShadowRoot(node) {
      // A root (or shadow root) is never split: normalize it in place.
      try normalizeElement(node, ctx)
      return
    }
    let parentKey = parent!.getKey()
    _ = ctx.touched.add(node.getKey())
    try normalizeElement(node, ctx)
    for key in Array(ctx.touched) {
      if let piece = try getNodeByKey(key), try piece.getLatest().__parent == parentKey {
        _ = try mergeRun(piece, ctx)
      }
    }
  }
}

// MARK: Registration

final class WeakEditorState {
  weak var value: EditorState?
  init(_ value: EditorState) { self.value = value }
}

/// One editor's structure registration, counted.
package final class NodeStructureRegistration {
  var refs = 0
  var dispose: () -> Void = {}
  /// The nodes collected since the last pass, and since the last commit.
  var dirty: [NodeKey] = []
  var collected = Set<NodeKey>()
  /// Committed states that may not be normalized.
  var unnormalized: [WeakEditorState] = []
  /// The first pass after registering normalizes the whole document.
  var full = true

  func isUnnormalized(_ state: EditorState) -> Bool {
    unnormalized.removeAll { $0.value == nil }
    return unnormalized.contains { $0.value === state }
  }
}

/// `INTERNAL_registerNodeStructure(editor)` (internal, for
/// `NodeStructureExtension`): enforce the structure node classes declare on
/// `editor`, and mark every node dirty so the current document is
/// normalized too. Registrations are counted: the normalization stops when
/// the last one is disposed.
public func INTERNAL_registerNodeStructure(_ editor: Editor) throws -> () -> Void {
  let registration: NodeStructureRegistration
  if let existing = editor._nodeStructureRegistration {
    registration = existing
  } else {
    registration = NodeStructureRegistration()
    registration.unnormalized.append(WeakEditorState(editor.getEditorState()))
    let collect = Transform { [unowned registration] node in
      let key = node.__key
      registration.dirty.append(key)
      registration.collected.insert(key)
    }
    let normalizeTransform = Transform { [unowned registration] _ in
      let all = registration.full
      registration.full = false
      try normalizeDirty(
        &registration.dirty, all,
        !registration.isUnnormalized(try getEditor().getEditorState()))
    }
    let disposeListener = editor.registerUpdateListener {
      [unowned registration] (payload: UpdateListenerPayload) in
      let nodeMap = payload.editorState._nodeMap
      func skipped(_ key: NodeKey) -> Bool {
        !registration.collected.contains(key) && nodeMap.has(key)
      }
      var complete = true
      for (key, intentional) in payload.dirtyElements
      where intentional && key != "root" && skipped(key) {
        complete = false
        break
      }
      if complete {
        for key in payload.dirtyLeaves where skipped(key) {
          complete = false
          break
        }
      }
      registration.collected.removeAll()
      if !complete {
        registration.unnormalized.append(WeakEditorState(payload.editorState))
      }
    }
    var added: [(RegisteredNode, Transform)] = []
    var types: [String] = []
    for (type, registered) in editor._nodes {
      let fn = type == "root" ? normalizeTransform : collect
      registered.addTransform(fn)
      added.append((registered, fn))
      if registered.klass is ElementNode.Type || registered.klass is DecoratorNode.Type {
        types.append(type)
      }
    }
    registration.dispose = {
      disposeListener()
      for (registered, fn) in added {
        registered.deleteTransform(fn)
      }
    }
    editor._nodeStructureRegistration = registration
    try markNodesWithTypesAsDirty(editor, types)
  }
  registration.refs += 1
  var disposed = false
  return { [weak editor] in
    if !disposed {
      disposed = true
      registration.refs -= 1
      if registration.refs == 0 {
        registration.dispose()
        if editor?._nodeStructureRegistration === registration {
          editor?._nodeStructureRegistration = nil
        }
      }
    }
  }
}

/// `INTERNAL_isNodeStructureEnforced(editor)` (internal): whether `editor`
/// enforces declared node structure.
public func INTERNAL_isNodeStructureEnforced(_ editor: Editor) -> Bool {
  editor._nodeStructureRegistration != nil
}

/// Whether there is an active editor and it enforces declared structure.
private func isStructureEnforcedOnActiveEditor() -> Bool {
  guard let editor = internalGetActiveEditor() else { return false }
  return INTERNAL_isNodeStructureEnforced(editor)
}

/// `$isBlockContainer` (`@experimental node-structure`): whether structural
/// commands treat `node` as a container of blocks: the root, a shadow root,
/// or, in an editor that enforces declared node structure, a block element
/// whose class declares `children: 'block'`.
public func isBlockContainer(_ node: LexicalNode?) throws -> Bool {
  guard let node = node as? ElementNode else {
    return false
  }
  if try isRootOrShadowRoot(node) {
    return true
  }
  return try cachedNodeStructure(type(of: node)).children == .block && !node.isInline()
    && isStructureEnforcedOnActiveEditor()
}

/// `validateNodeStructure(klass)`: the development check of a class's
/// structural declarations, run when an editor is created.
package func validateNodeStructure(_ klass: LexicalNode.Type) throws {
  let declared = cachedNodeStructure(klass)
  let name = String(describing: klass)
  if let mergeAdjacent = declared.mergeAdjacent, declared.declaredBy.mergeAdjacent == klass {
    switch invalidMergeBy(klass, mergeAdjacent, declared.derivedNames) {
    case .unknown(let entry):
      throw LexicalError(
        "\(name): mergeAdjacent.by names \(entry), which is not a schema field of the class")
    case .derived(let entry):
      throw LexicalError("\(name): mergeAdjacent.by names \(entry), which is derived")
    case .derivedState(let entry):
      throw LexicalError("\(name): mergeAdjacent.by names the derived NodeState \(entry)")
    case nil:
      break
    }
    if let item = mergingChildNotInline(klass, declared.children) {
      throw LexicalError(
        "\(name): a merging class that lists child classes must list classes that hold inline content (\(String(describing: item)) does not)"
      )
    }
  }
  if let inline = declared.declared.inline, declared.declaredBy.inline == klass {
    if let value = probeIsInline(klass), value != inline {
      throw LexicalError("\(name): $config inline is \(inline) but isInline() returns \(value)")
    }
  }
}
