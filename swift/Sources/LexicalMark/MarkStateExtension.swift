import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-mark/src/MarkStateExtension.ts: the public API,
// the flavors and their extensions, and the registration that maintains
// `markIDNodeKeys`. The inserted-text rule is in MarkStateInsertedText.swift.
//
// Differences from JS, none of which change what a document or the output
// holds:
// - JS keeps the compiled `markableNodes` and the pending edits of each
//   editor in `WeakMap`s on the flavor, which is shared by every editor (and,
//   in Swift, by every thread). Swift keeps them in the flavor extension's
//   init result, which belongs to one editor, and looks it up from the
//   editor.
// - `isMarkIDShown` is a closure, which Swift cannot compare by identity, so
//   every assignment of the output signal re-renders, where JS skips an
//   assignment of the function it already has. The config's default is
//   `nil`, which is JS's `isEveryMarkIDShown`.
// - JS compares two ids arrays by identity to tell the ids `splitText`
//   copied into a piece from ids of the piece's own (`$isSplitOffEntity`).
//   Swift arrays are values; `sameIDsValue` compares their storage, which
//   `arraySetValue` keeps when it reads a canonical value back, as JS keeps
//   the frozen array.

/// `MARK_STATE_KEY` (`@experimental`): the NodeState key that
/// `MarkStateExtension` stores mark ids under: a leaf covered by `c1` and
/// `c2` has `{"markIds": ["c1", "c2"]}` (sorted) in the `$` record of its
/// JSON. The state is an `arraySetValue`, so its schema marks it as a set.
public let MARK_STATE_KEY = "markIds"

/// `MARK_STATE_IDS_ATTRIBUTE` (`@experimental`): the attribute that
/// `MarkStateExtension` renders on the element of each marked leaf, holding
/// its space separated ids. HTML export writes it when `exportIDsToHTML` is
/// set, and HTML import reads it back.
public let MARK_STATE_IDS_ATTRIBUTE = "data-lexical-mark-ids"

/// `MarkableNodeMatch` (`@experimental`): a node class, matched as
/// `instanceof` matches it, or a guard function.
public enum MarkableNodeMatch {
  case klass(LexicalNode.Type)
  case predicate((LexicalNode) throws -> Bool)
}

/// `MarkStateConfig` (`@experimental`): the config of a mark flavor.
public struct MarkStateConfig {
  /// When `true`, mark ids are not rendered, the `markIDNodeKeys` output is
  /// emptied and no longer maintained, and text inserted next to a marked
  /// run no longer has its ids adjusted. The ids stay in the document, the
  /// functions keep working, and HTML export and import still write and read
  /// the ids.
  public var disabled: Bool
  /// When `true`, HTML export writes the ids attribute on marked leaves.
  public var exportIDsToHTML: Bool
  /// The nodes that carry mark ids. Default `DEFAULT_MARKABLE_NODES`.
  public var markableNodes: [MarkableNodeMatch]
  /// Which mark ids are rendered; `nil` renders every id.
  public var isMarkIDShown: ((String) -> Bool)?
  /// The flavor this config belongs to (`__markFlavor` in JS, which is on
  /// the extension).
  package var markFlavor: MarkFlavor?

  package init(
    disabled: Bool = false, exportIDsToHTML: Bool = false,
    markableNodes: [MarkableNodeMatch] = DEFAULT_MARKABLE_NODES,
    isMarkIDShown: ((String) -> Bool)? = nil, markFlavor: MarkFlavor? = nil
  ) {
    self.disabled = disabled
    self.exportIDsToHTML = exportIDsToHTML
    self.markableNodes = markableNodes
    self.isMarkIDShown = isMarkIDShown
    self.markFlavor = markFlavor
  }
}

/// `MarkStateOutput` (`@experimental`).
public final class MarkStateOutput {
  public let disabled: Signal<Bool>
  public let exportIDsToHTML: Signal<Bool>
  /// The `isMarkIDShown` config, which defaults to showing every id. Assign
  /// a new function to re-render the marked leaves with it.
  public let isMarkIDShown: Signal<(String) -> Bool>
  /// Every mark id in the document, mapped to the keys of the leaves that
  /// carry it (in no particular order). A new map is published after each
  /// update that changes it. Empty while the extension is disabled.
  public let markIDNodeKeys: ReadonlySignal<JSMap<String, [NodeKey]>>

  init(
    disabled: Signal<Bool>, exportIDsToHTML: Signal<Bool>,
    isMarkIDShown: Signal<(String) -> Bool>,
    markIDNodeKeys: ReadonlySignal<JSMap<String, [NodeKey]>>
  ) {
    self.disabled = disabled
    self.exportIDsToHTML = exportIDsToHTML
    self.isMarkIDShown = isMarkIDShown
    self.markIDNodeKeys = markIDNodeKeys
  }
}

/// `MarkIDsChange` (`@experimental`): what `registerMarkIDsListener`
/// reports after a commit that changed which ids of a flavor are in the
/// document.
public struct MarkIDsChange {
  /// The ids that no leaf carried before and at least one carries now.
  public let added: Set<String>
  /// Every id of the flavor in the document now.
  public let ids: Set<String>
  /// The ids that some leaf carried before and none carries now.
  public let removed: Set<String>
  /// The tags of the update that made the change; empty for the first call.
  public let tags: Set<String>
}

/// `MarkIDsListener` (`@experimental`).
public typealias MarkIDsListener = (MarkIDsChange) throws -> Void

/// A listener with the ids last reported to it.
package final class MarkIDsListenerEntry {
  let listener: MarkIDsListener
  var reported: Set<String>
  init(_ listener: @escaping MarkIDsListener, _ reported: Set<String>) {
    self.listener = listener
    self.reported = reported
  }
}

/// `MarkStateInit` (`@experimental`): the init result of a flavor
/// extension: state that outlives a `disabled` toggle. It also holds, for
/// its editor, what JS keeps in the flavor's `WeakMap`s.
public final class MarkStateInit {
  public let markIDNodeKeys: Signal<JSMap<String, [NodeKey]>> = signal(JSMap())
  /// The listeners, each with the ids last reported to it.
  package var listeners: [MarkIDsListenerEntry] = []
  /// The compiled `markableNodes` config (`isMarkableByEditor`).
  package var isMarkable: ((LexicalNode) throws -> Bool)?
  /// The pending edits of the inserted-text rule (`editsByEditor`).
  package var edit: PendingEdits?

  init() {}
}

private let MARK_STATE_EXTENSION_NAME = "@lexical/mark/MarkState"

/// The parts of a mark flavor that the internals need, created once per
/// `defineMarkExtension` call.
package final class MarkFlavor: @unchecked Sendable {
  let name: String
  let stateKey: String
  let attribute: String
  /// The set-valued NodeState that stores the ids.
  let state: StateConfig<JSONValue?>
  /// The theme classes of a marked leaf, and of a leaf with several ids.
  let classNames: (Editor) -> (mark: String?, overlap: String?)
  let renderDisabled: RenderStateConfig<Bool>

  init(
    name: String, stateKey: String, attribute: String,
    classNames: @escaping (Editor) -> (mark: String?, overlap: String?),
    renderDisabled: RenderStateConfig<Bool>
  ) {
    self.name = name
    self.stateKey = stateKey
    self.attribute = attribute
    self.state = createMarkIDsState(stateKey)
    self.classNames = classNames
    self.renderDisabled = renderDisabled
  }
}

/// JS `\s`, by UTF-16 code unit.
func isJSWhitespace(_ c: UInt16) -> Bool {
  switch c {
  case 0x09...0x0d, 0x20, 0xa0, 0x1680, 0x2000...0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
    0xfeff:
    return true
  default:
    return false
  }
}

/// `value.split(/\s+/).filter(Boolean)`.
func splitOnWhitespace(_ value: String) -> [String] {
  var tokens: [String] = []
  var current: [UInt16] = []
  for c in value.utf16 {
    if isJSWhitespace(c) {
      if !current.isEmpty {
        tokens.append(String(decoding: current, as: UTF16.self))
        current = []
      }
    } else {
      current.append(c)
    }
  }
  if !current.isEmpty {
    tokens.append(String(decoding: current, as: UTF16.self))
  }
  return tokens
}

/// `isValidMarkID(id)`: a non-empty string without whitespace, since the
/// ids of a leaf are written space separated in the ids attribute.
func isValidMarkID(_ id: String) -> Bool {
  !id.isEmpty && !id.utf16.contains(where: isJSWhitespace)
}

/// `markIDValue()`: the item schema of the ids, a string that
/// `isValidMarkID` accepts.
private func markIDValue() -> SerializationSchema {
  SerializationSchema(
    meta: .string, defaultValue: .string(""),
    accepts: { value in
      if case .string(let id)? = value { return isValidMarkID(id) }
      return false
    },
    parse: { value in
      if case .string(let id)? = value, isValidMarkID(id) { return value }
      return .string("")
    })
}

/// `createMarkIDsState(stateKey)`: the NodeState of the ids of a flavor, an
/// array sorted and without duplicates, which is also its JSON.
private func createMarkIDsState(_ stateKey: String) -> StateConfig<JSONValue?> {
  // `arraySetValue` throws only for an item that is not a string schema.
  try! createState(stateKey, schema: arraySetValue(markIDValue()))
}

/// JS `Array.prototype.sort` on strings: by UTF-16 code unit.
func jsSorted(_ ids: [String]) -> [String] {
  ids.sorted { $0.utf16.lexicographicallyPrecedes($1.utf16) }
}

func idsFromJSON(_ value: JSONValue?) -> [String] {
  guard case .array(let items)? = value else { return [] }
  return items.compactMap(\.stringValue)
}

private func idsToJSON(_ ids: [String]) -> JSONValue? {
  .array(ids.map { .string($0) })
}

/// Whether two stored ids values are the same value: JS compares the
/// frozen arrays by identity. A Swift array is a value; its storage is
/// shared by every copy of it (`cloneNodeState`, an unparse and a parse of a
/// canonical value) and by nothing else, so it stands for that identity.
func sameIDsValue(_ a: JSONValue?, _ b: JSONValue?) -> Bool {
  guard case .array(let x)? = a, case .array(let y)? = b else { return a == nil && b == nil }
  if x.isEmpty || y.isEmpty {
    return x.isEmpty && y.isEmpty
  }
  let px = x.withUnsafeBufferPointer { $0.baseAddress }
  let py = y.withUnsafeBufferPointer { $0.baseAddress }
  return px == py
}

/// `$isInlineDecoratorNode(node)` (`@experimental`): inline DecoratorNodes,
/// one of the default `markableNodes`. This includes `DecoratorTextNode`.
public func isInlineDecoratorNode(_ node: LexicalNode?) throws -> Bool {
  guard let node = node as? DecoratorNode else { return false }
  return try node.isInline()
}

/// `DEFAULT_MARKABLE_NODES` (`@experimental`): TextNode, LineBreakNode and
/// inline DecoratorNodes (including `DecoratorTextNode`).
nonisolated(unsafe) public let DEFAULT_MARKABLE_NODES: [MarkableNodeMatch] = [
  .klass(TextNode.self), .klass(LineBreakNode.self), .predicate(isInlineDecoratorNode),
]

private func compileMarkableNodes(
  _ matches: [MarkableNodeMatch]
) -> (LexicalNode) throws -> Bool {
  var klasses: [LexicalNode.Type] = []
  var guards: [(LexicalNode) throws -> Bool] = []
  for match in matches {
    switch match {
    case .klass(let klass): klasses.append(klass)
    case .predicate(let predicate): guards.append(predicate)
    }
  }
  return { node in
    let nodeClass = type(of: node)
    if klasses.contains(where: { isSubclass(nodeClass, of: $0) }) {
      return true
    }
    return try guards.contains { try $0(node) }
  }
}

nonisolated(unsafe) private let defaultIsMarkable = compileMarkableNodes(DEFAULT_MARKABLE_NODES)

/// The init result of `flavor`'s extension in `editor`, when it has it.
func flavorInit(_ flavor: MarkFlavor, _ editor: Editor) throws -> MarkStateInit? {
  try getPeerDependencyFromEditor(editor, flavor.name)?.initResult as? MarkStateInit
}

/// The output of `flavor`'s extension in `editor`, when it has it.
func flavorOutput(_ flavor: MarkFlavor, _ editor: Editor) throws -> MarkStateOutput? {
  try getPeerDependencyFromEditor(editor, flavor.name)?.output as? MarkStateOutput
}

/// `$isMarkableNode(flavor, node)`: whether `node` carries ids of `flavor`
/// in the active editor, according to the `markableNodes` config of that
/// flavor's extension (or the default, when the extension is not
/// installed). Inline ElementNodes such as links are not marked themselves,
/// their leaves are.
func isMarkableNode(_ flavor: MarkFlavor, _ node: LexicalNode?) throws -> Bool {
  guard let node else { return false }
  if let isMarkable = try flavorInit(flavor, getEditor())?.isMarkable {
    return try isMarkable(node)
  }
  return try defaultIsMarkable(node)
}

/// `$getNodeMarkIDs(flavor, node)`: like `getNodeMarkIDs(_:)`, for the ids
/// of `flavor`.
public func getNodeMarkIDs(_ flavor: MarkFlavorExtension, _ node: LexicalNode) throws -> [String] {
  try readNodeMarkIDs(flavor.__markFlavor, node)
}

/// `$getNodeMarkIDs(node)` (`@experimental`): the mark ids stored on a
/// node, sorted: the ids of `MarkStateExtension`. Nodes that cannot carry
/// ids return an empty array.
public func getNodeMarkIDs(_ node: LexicalNode) throws -> [String] {
  try getNodeMarkIDs(MarkStateExtension, node)
}

/// `$readNodeMarkIDs`: the stored value, as JSON.
func readNodeMarkIDsValue(_ flavor: MarkFlavor, _ node: LexicalNode) throws -> JSONValue? {
  try getState(node, flavor.state)
}

/// `$readNodeMarkIDs`.
func readNodeMarkIDs(_ flavor: MarkFlavor, _ node: LexicalNode) throws -> [String] {
  idsFromJSON(try readNodeMarkIDsValue(flavor, node))
}

/// `$getMarkableIDs`: the ids of `node` when it is markable, otherwise none.
func getMarkableIDs(_ flavor: MarkFlavor, _ node: LexicalNode?) throws -> [String] {
  if let node, try isMarkableNode(flavor, node) {
    return try readNodeMarkIDs(flavor, node)
  }
  return NO_MARK_IDS
}

let NO_MARK_IDS: [String] = []

func addNodeMarkIDs(_ flavor: MarkFlavor, _ node: LexicalNode, _ ids: [String]) throws {
  // `setState` sorts and dedupes the result
  try setState(node, flavor.state) { prev in
    let prevIDs = idsFromJSON(prev)
    return ids.allSatisfy(prevIDs.contains) ? prev : idsToJSON(prevIDs + ids)
  }
}

private func removeNodeMarkIDs(_ flavor: MarkFlavor, _ node: LexicalNode, _ ids: [String]) throws {
  try setState(node, flavor.state) { prev in
    let prevIDs = idsFromJSON(prev)
    return ids.contains(where: prevIDs.contains)
      ? idsToJSON(prevIDs.filter { !ids.contains($0) }) : prev
  }
}

/// Sets the ids of `node` to `ids`.
func setNodeMarkIDs(_ flavor: MarkFlavor, _ node: LexicalNode, _ ids: [String]) throws {
  try setState(node, flavor.state, idsToJSON(ids))
}

private func assertValidMarkID(_ id: String) throws {
  try invariant(
    isValidMarkID(id),
    "MarkStateExtension: invalid mark id \(JSONValue.string(id).stringify()), ids must be non-empty strings without whitespace"
  )
}

/// `$forEachSlot(node, fn)`: calls `fn` with the value of each slot.
func forEachSlot(_ node: LexicalNode, _ fn: (LexicalNode) throws -> Void) throws {
  for name in try getSlotNames(node) {
    if let slot = try getSlot(node, name) {
      try fn(slot)
    }
  }
}

private func markableNodesInSelection(
  _ flavor: MarkFlavor, _ selection: any BaseSelection
) throws -> [LexicalNode] {
  if let range = selection as? RangeSelection, range.isCollapsed() {
    return []
  }
  // extract() splits the TextNodes at the boundaries of a RangeSelection, so
  // only the selected part of them is returned. It does not include the
  // content of slots, which is collected from each selected slot host.
  var nodes: [LexicalNode] = []
  var seen = Set<ObjectIdentifier>()
  for node in try selection.extract() {
    if try isMarkableNode(flavor, node), seen.insert(ObjectIdentifier(node)).inserted {
      nodes.append(node)
    }
    try forEachSlot(node) { slot in
      try collectMarkableNodes(flavor, slot, &nodes, &seen)
    }
  }
  return nodes
}

/// `$addMarkID(flavor, id, selection)`: like `addMarkID(_:_:)`, for the
/// ids of `flavor`.
@discardableResult
public func addMarkID(
  _ flavor: MarkFlavorExtension, _ id: String, _ selection: any BaseSelection
) throws -> [LexicalNode] {
  let flavor = flavor.__markFlavor
  try assertValidMarkID(id)
  let nodes = try markableNodesInSelection(flavor, selection)
  for node in nodes {
    try addNodeMarkIDs(flavor, node, [id])
  }
  return nodes
}

/// `$addMarkID(id, selection)` (`@experimental`): adds the mark `id` to
/// every markable node in `selection`, including the content of the slots
/// of the nodes in it, splitting the TextNodes at its boundaries. Ids
/// already on those nodes are kept, so marks can overlap freely. A
/// collapsed selection marks nothing. Throws for an id that is empty or
/// contains whitespace. Returns the nodes that now carry the id.
@discardableResult
public func addMarkID(_ id: String, _ selection: any BaseSelection) throws -> [LexicalNode] {
  try addMarkID(MarkStateExtension, id, selection)
}

/// `$collectMarkableNodes`: the markable nodes in the subtree of `node`,
/// looking through the children of ElementNodes and the slots of slot
/// hosts.
func collectMarkableNodes(
  _ flavor: MarkFlavor, _ node: LexicalNode, _ out: inout [LexicalNode],
  _ seen: inout Set<ObjectIdentifier>
) throws {
  if try isMarkableNode(flavor, node), seen.insert(ObjectIdentifier(node)).inserted {
    out.append(node)
  }
  if let element = node as? ElementNode {
    var child = try element.getFirstChild()
    while let current = child {
      try collectMarkableNodes(flavor, current, &out, &seen)
      child = try current.getNextSibling()
    }
  }
  try forEachSlot(node) { slot in
    try collectMarkableNodes(flavor, slot, &out, &seen)
  }
}

func markableNodesIn(_ flavor: MarkFlavor, _ node: LexicalNode) throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  var seen = Set<ObjectIdentifier>()
  try collectMarkableNodes(flavor, node, &out, &seen)
  return out
}

/// `$getMarkIDCandidates(flavor, id)`: the leaves that may carry `id`
/// without a full document walk: the leaves the `markIDNodeKeys` output had
/// for it at the last commit, plus the leaves changed by the pending update.
/// `nil` when the extension is not installed or is disabled.
private func getMarkIDCandidates(_ flavor: MarkFlavor, _ id: String) throws -> [LexicalNode]? {
  let editor = try getEditor()
  guard let output = try flavorOutput(flavor, editor), !output.disabled.peek() else {
    return nil
  }
  let keys = JSSet<NodeKey>(output.markIDNodeKeys.peek().get(id) ?? [])
  // @internal access: the leaves this update made dirty, which the update
  // listener receives as `dirtyLeaves` once it commits
  for key in editor._dirtyLeaves {
    keys.add(key)
  }
  var nodes: [LexicalNode] = []
  for key in keys {
    if let node = try getNodeByKey(key), try isMarkableNode(flavor, node), try node.isAttached() {
      nodes.append(node)
    }
  }
  return nodes
}

/// `$removeMarkID(flavor, id, selection)`: like `removeMarkID(_:_:)`, for
/// the ids of `flavor`.
public func removeMarkID(
  _ flavor: MarkFlavorExtension, _ id: String, _ selection: (any BaseSelection)? = nil
) throws {
  let flavor = flavor.__markFlavor
  let nodes: [LexicalNode]
  if let selection {
    nodes = try markableNodesInSelection(flavor, selection)
  } else {
    nodes = try getMarkIDCandidates(flavor, id) ?? markableNodesIn(flavor, getRoot())
  }
  for node in nodes {
    try removeNodeMarkIDs(flavor, node, [id])
  }
  // The inserted-text rule of this update must not give the id back
  if let edit = try flavorInit(flavor, getEditor())?.edit {
    edit.removed.insert(id)
    forgetSoon(edit)
  }
}

/// `$removeMarkID(id, selection)` (`@experimental`): removes the mark `id`.
/// With a `selection` it is removed from the nodes in that selection only
/// (splitting the TextNodes at its boundaries), otherwise from the whole
/// document. Text that no longer differs from its neighbors is merged back
/// by normalization.
public func removeMarkID(_ id: String, _ selection: (any BaseSelection)? = nil) throws {
  try removeMarkID(MarkStateExtension, id, selection)
}

private func getPointMarkIDs(_ flavor: MarkFlavor, _ point: Point) throws -> [String] {
  let node = try point.getNode()
  if point.type == .text {
    let ids = try readNodeMarkIDs(flavor, node)
    if ids.isEmpty, let text = node as? TextNode, try point.offset == text.getTextContentSize() {
      // Like getMarkIDs: a caret right before a marked run is in it
      return try getMarkableIDs(flavor, getAdjacentLeaf(text, .next))
    }
    return ids
  }
  if let element = node as? ElementNode {
    let before = try getMarkableIDs(flavor, element.getChildAtIndex(point.offset - 1))
    return before.isEmpty
      ? try getMarkableIDs(flavor, element.getChildAtIndex(point.offset)) : before
  }
  return NO_MARK_IDS
}

/// The `mode` of `getMarkIDsAtSelection`, for a selection that is not
/// collapsed: `'any'` or `'all'`.
public enum MarkIDsMode: Sendable {
  case any
  case all
}

/// `$getMarkIDsAtSelection(flavor, selection, mode)`: like
/// `getMarkIDsAtSelection(_:_:)`, for the ids of `flavor`.
public func getMarkIDsAtSelection(
  _ flavor: MarkFlavorExtension, _ selection: (any BaseSelection)?, _ mode: MarkIDsMode = .any
) throws -> [String] {
  let flavor = flavor.__markFlavor
  guard let selection else { return [] }
  if let range = selection as? RangeSelection, range.isCollapsed() {
    return try getPointMarkIDs(flavor, range.anchor)
  }
  var leaves = try selection.getNodes().filter { try isMarkableNode(flavor, $0) }
  if let range = selection as? RangeSelection {
    let (start, end) =
      try range.isBackward() ? (range.focus, range.anchor) : (range.anchor, range.focus)
    leaves = try leaves.filter { node in
      guard let text = node as? TextNode else { return true }
      let key = text.getKey()
      let from = start.type == .text && start.key == key ? start.offset : 0
      let to = end.type == .text && end.key == key ? end.offset : try text.getTextContentSize()
      return to > from
    }
  }
  if leaves.isEmpty {
    return []
  }
  let idSets = try leaves.map { try readNodeMarkIDs(flavor, $0) }
  switch mode {
  case .any:
    return jsSorted(Array(Set(idSets.joined())))
  case .all:
    return idSets[0].filter { id in idSets.allSatisfy { $0.contains(id) } }
  }
}

/// `$getMarkIDsAtSelection(flavor)`: at the current selection.
public func getMarkIDsAtSelection(
  _ flavor: MarkFlavorExtension, _ mode: MarkIDsMode = .any
) throws -> [String] {
  try getMarkIDsAtSelection(flavor, getSelection(), mode)
}

/// `$getMarkIDsAtSelection(selection, mode)` (`@experimental`): the mark
/// ids at a selection, sorted. For a collapsed RangeSelection these are the
/// ids of the leaf holding the caret (or of the next leaf, when the caret
/// is at the end of an unmarked TextNode). Otherwise `.any` returns the ids
/// on any selected leaf, and `.all` the ids shared by every selected leaf.
/// The TextNodes that a RangeSelection only touches at an edge are ignored.
public func getMarkIDsAtSelection(
  _ selection: (any BaseSelection)?, _ mode: MarkIDsMode = .any
) throws -> [String] {
  try getMarkIDsAtSelection(MarkStateExtension, selection, mode)
}

/// `$getMarkIDsAtSelection()`: at the current selection.
public func getMarkIDsAtSelection(_ mode: MarkIDsMode = .any) throws -> [String] {
  try getMarkIDsAtSelection(MarkStateExtension, getSelection(), mode)
}

/// `$migrateMarkNodesToMarkState(flavor, node)`: like
/// `migrateMarkNodesToMarkState(_:)`, with the ids going to `flavor`.
@discardableResult
public func migrateMarkNodesToMarkState(
  _ flavor: MarkFlavorExtension, _ node: LexicalNode? = nil
) throws -> Int {
  let flavor = flavor.__markFlavor
  let node: LexicalNode = try node ?? getRoot()
  var count = 0
  func visit(_ current: LexicalNode, _ inherited: [String]) throws {
    if let mark = current as? MarkNode {
      let ownIDs = try mark.getIDs()
      try ownIDs.forEach(assertValidMarkID)
      var ids: [String] = []
      for id in inherited + ownIDs where !ids.contains(id) {
        ids.append(id)
      }
      for child in try mark.getChildren() {
        try visit(child, ids)
      }
      // Re-read: unwrapping a nested MarkNode replaced it with its children
      for child in try mark.getChildren() {
        try mark.insertBefore(child)
      }
      try mark.remove()
      count += 1
    } else {
      if try isMarkableNode(flavor, current), !inherited.isEmpty {
        try addNodeMarkIDs(flavor, current, inherited)
      }
      if let element = current as? ElementNode {
        for child in try element.getChildren() {
          try visit(child, inherited)
        }
      }
      try forEachSlot(current) { slot in try visit(slot, inherited) }
    }
  }
  try visit(node, [])
  return count
}

/// `$migrateMarkNodesToMarkState(node)` (`@experimental`): converts a
/// document that uses MarkNode into one that uses `MarkStateExtension`: the
/// ids of every MarkNode (including the ids of MarkNodes it is nested in)
/// are added to the leaves inside it, and the MarkNode is unwrapped. Throws
/// on an id that is empty or contains whitespace. Returns the number of
/// MarkNodes that were unwrapped.
@discardableResult
public func migrateMarkNodesToMarkState(_ node: LexicalNode? = nil) throws -> Int {
  try migrateMarkNodesToMarkState(MarkStateExtension, node)
}

/// The default `isMarkIDShown`: every id is rendered.
private func isEveryMarkIDShown(_: String) -> Bool { true }

/// The ids in `ids` that `isMarkIDShown` renders.
private func filterShownMarkIDs(_ ids: [String], _ isMarkIDShown: (String) -> Bool) -> [String] {
  ids.filter(isMarkIDShown)
}

private func decorateMarkIDs(
  _ flavor: MarkFlavor, _ node: LexicalNode, _ dom: DOMElement, _ editor: Editor
) throws {
  var ids = try readNodeMarkIDs(flavor, node)
  if !ids.isEmpty, let output = try flavorOutput(flavor, editor) {
    ids = filterShownMarkIDs(ids, output.isMarkIDShown.peek())
  }
  let (mark, overlap) = flavor.classNames(editor)
  if !ids.isEmpty {
    dom.setAttribute(flavor.attribute, ids.joined(separator: " "))
  } else {
    dom.removeAttribute(flavor.attribute)
  }
  let add = classTokens(ids.count > 0 ? mark : nil) + classTokens(ids.count > 1 ? overlap : nil)
  let remove =
    (classTokens(ids.count > 0 ? nil : mark) + classTokens(ids.count > 1 ? nil : overlap))
    .filter { !add.contains($0) && dom.classList.contains($0) }
  if !add.isEmpty {
    dom.classList.add(contentsOf: add)
  }
  if !remove.isEmpty {
    // A class that another flavor shows on this leaf stays
    let kept = try classTokensOfOtherFlavors(flavor, node, editor)
    let removed = remove.filter { !kept.contains($0) }
    if !removed.isEmpty {
      dom.classList.remove(contentsOf: removed)
    }
  }
}

private func classTokens(_ classNames: String?) -> [String] {
  classNames.map(splitOnWhitespace) ?? []
}

/// The classes that the other enabled flavors of the editor render on
/// `node`, so that two flavors that share a class do not remove it from
/// each other.
private func classTokensOfOtherFlavors(
  _ flavor: MarkFlavor, _ node: LexicalNode, _ editor: Editor
) throws -> Set<String> {
  var kept = Set<String>()
  guard
    let base = try getPeerDependencyFromEditor(editor, MARK_BASE_EXTENSION_NAME)?.output
      as? MarkBaseOutput
  else {
    return kept
  }
  for (_, extension_) in base.flavors {
    let other = extension_.__markFlavor
    guard other !== flavor, let output = try flavorOutput(other, editor), !output.disabled.peek(),
      try isMarkableNode(other, node)
    else { continue }
    let shown = filterShownMarkIDs(try readNodeMarkIDs(other, node), output.isMarkIDShown.peek())
    let (mark, overlap) = other.classNames(editor)
    for token in classTokens(shown.count > 0 ? mark : nil)
      + classTokens(shown.count > 1 ? overlap : nil)
    {
      kept.insert(token)
    }
  }
  return kept
}

/// `createMarkIDsImportRule(flavor)`: the HTML import rule of a flavor: the
/// ids in its attribute are added to the markable leaves the element
/// imports to.
private func createMarkIDsImportRule(_ flavor: MarkFlavor) -> DOMImportRule {
  defineImportRule(name: "\(flavor.name)/ids", match: sel.any().attr(flavor.attribute)) {
    _, el, next in
    let nodes = try next()
    let ids = splitOnWhitespace(el.getAttribute(flavor.attribute) ?? "").filter(isValidMarkID)
    if !ids.isEmpty {
      var leaves: [LexicalNode] = []
      var seen = Set<ObjectIdentifier>()
      for node in nodes {
        try collectMarkableNodes(flavor, node, &leaves, &seen)
      }
      for leaf in leaves {
        try addNodeMarkIDs(flavor, leaf, ids)
      }
    }
    return nodes
  }
}

/// `PendingEdits`: what the inserted text rule needs to know about the
/// current edits besides the two editor states: the selection that an
/// insertion or paste command replaces (recorded before its handler
/// replaces it), and the ids removed with `removeMarkID`, which the rule
/// must not give back. Both are forgotten when the editor commits and at
/// the next microtask.
package final class PendingEdits {
  var replaced: (PointLike, PointLike)?
  var removed = Set<String>()
}

private func forgetEdits(_ edit: PendingEdits) {
  edit.replaced = nil
  edit.removed.removeAll()
}

/// `forgetSoon(edit)`: forgets the edits at the next microtask.
private func forgetSoon(_ edit: PendingEdits) {
  scheduleMicroTask { forgetEdits(edit) }
}

/// `registerMarkState(flavor, editor, init, splits)`: maintains the
/// `markIDNodeKeys` output, and keeps text inserted next to a marked run
/// consistent with the documented rule (see `applyInsertedTextMarkIDs`).
///
/// The output is updated after each commit from the dirty leaves, so the
/// cost of an update is proportional to what it changed. A commit whose
/// dirty sets do not describe the change (a `setEditorState`, recognized
/// from the root transform of that update or from its tag) is compared with
/// the previous state node by node. After publishing a change, it reports
/// the ids that appeared or disappeared to the listeners of
/// `registerMarkIDsListener`.
private func registerMarkState(
  _ flavor: MarkFlavor, _ editor: Editor, _ initResult: MarkStateInit, _ splits: SplitOrigins
) throws -> () -> Void {
  let output = initResult.markIDNodeKeys
  var idsByKey: [NodeKey: [String]] = [:]
  let keysById = JSMap<String, JSSet<NodeKey>>()
  var changed = false
  var replacedState = false
  var compositionBase: EditorState? = nil
  // Set when an update tagged COLLABORATION_TAG (another editor's update)
  // changed the block of the composition while it was in progress
  var compositionTainted = false
  // The selection the composition started with, once its first update is
  // seen to have replaced it (see getCompositionReplacedRange)
  var compositionReplaced: (PointLike, PointLike)? = nil
  let edit = PendingEdits()
  initResult.edit = edit
  func setKey(_ key: NodeKey, _ ids: [String]) {
    let prev = idsByKey[key] ?? NO_MARK_IDS
    if prev == ids {
      return
    }
    changed = true
    for id in prev {
      if let keys = keysById.get(id), !ids.contains(id) {
        keys.delete(key)
        if keys.isEmpty {
          keysById.delete(id)
        }
      }
    }
    for id in ids {
      let keys: JSSet<NodeKey>
      if let existing = keysById.get(id) {
        keys = existing
      } else {
        keys = JSSet()
        keysById.set(id, keys)
      }
      keys.add(key)
    }
    if !ids.isEmpty {
      idsByKey[key] = ids
    } else {
      idsByKey[key] = nil
    }
  }
  func visit(_ key: NodeKey) throws {
    let node = try getNodeByKey(key)
    setKey(key, try getMarkableIDs(flavor, node))
  }
  func rescan() throws {
    var seen = Set<NodeKey>()
    for leaf in try markableNodesIn(flavor, getRoot()) {
      seen.insert(leaf.getKey())
      setKey(leaf.getKey(), try readNodeMarkIDs(flavor, leaf))
    }
    for key in Array(idsByKey.keys) where !seen.contains(key) {
      setKey(key, NO_MARK_IDS)
    }
  }
  /// Brings the output from `prevState` to `nextState` when the dirty sets
  /// do not describe the change. A node that did not change is the same
  /// object in both states, so only the others are read.
  func rescanChanged(_ prevState: EditorState, _ nextState: EditorState) throws {
    // @internal access: the node maps, compared by identity
    let prevNodes = prevState._nodeMap
    let nextNodes = nextState._nodeMap
    try nextState.read(editor: editor) {
      for (key, node) in nextNodes where prevNodes.get(key) !== node {
        setKey(
          key,
          try isMarkableNode(flavor, node) && node.isAttached()
            ? readNodeMarkIDs(flavor, node) : NO_MARK_IDS)
      }
    }
    for key in Array(idsByKey.keys) where !nextNodes.has(key) {
      setKey(key, NO_MARK_IDS)
    }
  }
  func publish(_ tags: Set<String>) throws {
    if changed {
      changed = false
      let next = JSMap<String, [NodeKey]>()
      for (id, keys) in keysById {
        next.set(id, keys.array)
      }
      try output.set(next)
      try reportMarkIDs(initResult, tags)
    }
  }
  // Set when this registration is disposed (the flavor was disabled, or the
  // editor disposed): an update listener or transform that the editor had
  // already collected for the current commit must then do nothing
  var disposed = false
  let recordReplaced: (Any, Editor) throws -> Bool = { _, _ in
    edit.replaced = try getReplacedRange(getSelection())
    forgetSoon(edit)
    return false
  }
  try editor.getEditorState().read(editor: editor, rescan)
  try publish([])
  // Also when nothing changed: the ids may have gone while it was disabled
  try reportMarkIDs(initResult, [])
  return mergeRegister(
    editor.registerUpdateListener { payload in
      if disposed {
        return
      }
      let editorState = payload.editorState
      let prevEditorState = payload.prevEditorState
      let dirtyElements = payload.dirtyElements
      let dirtyLeaves = payload.dirtyLeaves
      let tags = payload.tags
      forgetEdits(edit)
      splits.clear()
      if let base = compositionBase {
        if !editor.isComposing() {
          compositionBase = nil
          compositionTainted = false
          compositionReplaced = nil
        } else if tags.has(COMPOSITION_START_TAG) {
          compositionReplaced = try getCompositionReplacedRange(editor, base, editorState)
        } else if !compositionTainted, tags.has(COLLABORATION_TAG) {
          compositionTainted = try editorState.read(editor: editor) {
            // @internal access: the node being composed
            let key = editor._compositionKey
            let node = try key.flatMap { try getNodeByKey($0) }
            let composing = try node.flatMap { try getBlock($0) }
            guard let composing else { return true }
            return dirtyElements.has(composing.getKey())
          }
        }
      }
      if replacedState || tags.has(HISTORIC_TAG)
        || (dirtyLeaves.isEmpty && dirtyElements.size == 1 && dirtyElements.has("root"))
      {
        replacedState = false
        try rescanChanged(prevEditorState, editorState)
        try publish(Set(tags))
        return
      }
      if dirtyLeaves.isEmpty && dirtyElements.isEmpty {
        return
      }
      // The dirty nodes that this update removed: the markable nodes under
      // them in the previous state are gone too, unless they were moved
      // (which made them dirty leaves of their own)
      var removed: [NodeKey] = []
      try editorState.read(editor: editor) {
        for key in dirtyLeaves {
          try visit(key)
          if try getNodeByKey(key) == nil {
            removed.append(key)
          }
        }
        for (key, _) in dirtyElements {
          if let node = try getNodeByKey(key) {
            if try isMarkableNode(flavor, node) {
              setKey(key, try readNodeMarkIDs(flavor, node))
            }
          } else {
            removed.append(key)
          }
        }
      }
      if !removed.isEmpty {
        var gone: [NodeKey] = []
        try prevEditorState.read(editor: editor) {
          for key in removed {
            if let node = try getNodeByKey(key) {
              for leaf in try markableNodesIn(flavor, node) {
                gone.append(leaf.getKey())
              }
            }
          }
        }
        try editorState.read(editor: editor) {
          for key in gone where try getNodeByKey(key) == nil {
            setKey(key, NO_MARK_IDS)
          }
        }
      }
      try publish(Set(tags))
    },
    try editor.registerNodeTransform(RootNode.self) { _ in
      if disposed {
        return
      }
      // @internal access: nothing else tells a transform that this update
      // is a setEditorState, whose dirty sets the update listener cannot use
      // and which is not an edit
      if editor._dirtyType == FULL_RECONCILE {
        replacedState = true
        return
      }
      if try editor.isComposing() || hasUpdateTag(COLLABORATION_TAG) || hasUpdateTag(HISTORIC_TAG) {
        return
      }
      if let base = compositionBase {
        // The end of a composition: the composed text is compared with the
        // document from before it, and replaces the selection it had
        if !compositionTainted {
          let replaced =
            try compositionReplaced
            ?? base.read(editor: editor) { try getReplacedRange(getSelection()) }
          try applyInsertedTextMarkIDs(
            flavor, editor, base, replaced, edit.removed, splits, compositionReplaced != nil)
        }
        return
      }
      try applyInsertedTextMarkIDs(
        flavor, editor, editor.getEditorState(), edit.replaced, edit.removed, splits)
    },
    editor.registerCommand(
      COMPOSITION_START_COMMAND,
      { _, _ in
        // Transforms skip the node being composed, so the composed text is
        // compared with the document from before the composition once it
        // ends
        compositionBase = editor.getEditorState()
        compositionTainted = false
        compositionReplaced = nil
        return false
      }, COMMAND_PRIORITY_CRITICAL),
    // The selection that an insertion replaces, before the handler of the
    // command replaces it
    editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND, { try recordReplaced($0, $1) }, COMMAND_PRIORITY_CRITICAL),
    editor.registerCommand(
      PASTE_COMMAND, { try recordReplaced($0, $1) }, COMMAND_PRIORITY_CRITICAL),
    {
      disposed = true
      compositionBase = nil
      compositionReplaced = nil
      forgetEdits(edit)
      if initResult.edit === edit {
        initResult.edit = nil
      }
      // Not reported to the listeners: the ids are still in the document.
      // When the flavor is enabled again, the listeners get the net change.
      output.value = JSMap()
    })
}

/// `$getBlock(node)`: the nearest ancestor of `node` (or `node` itself)
/// that is a block.
func getBlock(_ node: LexicalNode) throws -> ElementNode? {
  var current: LexicalNode? = node
  while let node = current {
    if let element = node as? ElementNode, try !element.isInline() {
      return element
    }
    current = try node.getParent()
  }
  return nil
}

enum LeafDirection {
  case previous
  case next
}

/// `$getAdjacentLeaf(node, direction)`: the leaf next to `node` in
/// `direction` within its block, looking through the boundaries of inline
/// elements such as links. An empty inline element is returned as is.
func getAdjacentLeaf(_ node: LexicalNode, _ direction: LeafDirection) throws -> LexicalNode? {
  var current = node
  while true {
    let sibling =
      try direction == .previous ? current.getPreviousSibling() : current.getNextSibling()
    guard let sibling else {
      if let parent = try current.getParent(), try parent.isInline() {
        current = parent
        continue
      }
      return nil
    }
    var leaf = sibling
    while let element = leaf as? ElementNode, try element.isInline() {
      guard
        let child = try direction == .previous ? element.getLastChild() : element.getFirstChild()
      else { break }
      leaf = child
    }
    return leaf
  }
}

/// `registerMarkIDsState(flavor, editor)`: makes the StateConfig of a
/// flavor known to every node type of the editor, as using it on a node of
/// that type would, so that ids read from JSON or applied with
/// `updateFromUnknown` are parsed right away.
private func registerMarkIDsState(_ flavor: MarkFlavor, _ editor: Editor) {
  // @internal access with precedent (NormalizeInlineElementsExtension)
  for (_, registered) in editor._nodes {
    let sharedConfigMap = registered.sharedNodeState.sharedConfigMap
    if !sharedConfigMap.has(flavor.stateKey) {
      sharedConfigMap.set(flavor.stateKey, flavor.state)
    }
  }
}

/// `SplitOrigins`: where the pieces that a flavor's inserted-text rule cut
/// off a TextNode in the current update came from: piece key to the flavor
/// and the key of the node it was cut from. Shared by the flavors of an
/// editor through `MarkBaseExtension`, and forgotten at each commit.
public final class SplitOrigins {
  var entries: [NodeKey: (flavor: MarkFlavor, origin: NodeKey)] = [:]
  init() {}
  func clear() { entries.removeAll() }
}

/// `getSplitOrigin(splits, flavor, key)`: the key of the node that `key`
/// stands for in `flavor`'s comparison: a piece that another flavor cut off
/// a node in this update stands for that node.
func getSplitOrigin(_ splits: SplitOrigins, _ flavor: MarkFlavor, _ key: NodeKey) -> NodeKey {
  var key = key
  while let entry = splits.entries[key], entry.flavor !== flavor {
    key = entry.origin
  }
  return key
}

let MARK_BASE_EXTENSION_NAME = "@lexical/mark/MarkBase"

/// `MarkBaseInit` (`@experimental`): the init result of `MarkBaseExtension`.
public final class MarkBaseInit {
  /// The flavors of the editor, by stateKey.
  package let flavors = JSMap<String, MarkFlavorExtension>()
  package let splits = SplitOrigins()
  init() {}
}

/// `MarkBaseOutput` (`@experimental`).
public struct MarkBaseOutput {
  /// The mark flavors of the editor, by their `stateKey`.
  public let flavors: JSMap<String, MarkFlavorExtension>
}

/// `MarkBaseExtension` (`@experimental`): the shared base of the mark
/// flavors (`MarkStateExtension` and every extension made with
/// `defineMarkExtension`), which all depend on it. It keeps the registry of
/// the editor's flavors (the `flavors` output), rejects two flavors that
/// share a `stateKey` or an attribute when the editor is built, and lets
/// the inserted-text rules of the flavors tell each other's work from
/// inserted content.
public let MarkBaseExtension: LexicalExtension<EmptyConfig, MarkBaseOutput, MarkBaseInit> =
  defineExtension(
    name: MARK_BASE_EXTENSION_NAME,
    init: { _, _, _ in MarkBaseInit() },
    build: { _, _, state in MarkBaseOutput(flavors: try state.getInitResult().flavors) },
    register: { editor, _, state in
      let splits = try state.getInitResult().splits
      return mergeRegister(
        editor.registerUpdateListener { _ in splits.clear() },
        { splits.clear() })
    })

/// `MarkFlavorExtension` (`@experimental`): a mark flavor, an extension that
/// stores one kind of annotation ids in its own set-valued NodeState key,
/// renders them with its own attribute and classes, and reports them to its
/// own listeners. Made with `defineMarkExtension`; `MarkStateExtension` is
/// the default flavor.
public typealias MarkFlavorExtension = LexicalExtension<
  MarkStateConfig, MarkStateOutput, MarkStateInit
>

extension LexicalExtension
where Config == MarkStateConfig, Output == MarkStateOutput, Init == MarkStateInit {
  /// The attribute that marked leaves are rendered with.
  public var attribute: String { __markFlavor.attribute }
  /// The NodeState key that the ids are stored under.
  public var stateKey: String { __markFlavor.stateKey }
  package var __markFlavor: MarkFlavor { config.markFlavor! }
}

/// `commentIds` to `data-lexical-comment-ids`.
private func defaultMarkAttribute(_ stateKey: String) -> String {
  var result = "data-lexical-"
  for c in stateKey {
    if c.isASCII, c.isUppercase {
      result += "-" + c.lowercased()
    } else {
      result.append(c)
    }
  }
  return result
}

/// `reportMarkIDs(init, tags)`: reports to each listener the ids that
/// appeared or disappeared since the last report to it.
private func reportMarkIDs(_ initResult: MarkStateInit, _ tags: Set<String>) throws {
  if initResult.listeners.isEmpty {
    return
  }
  let ids = Set(initResult.markIDNodeKeys.peek().keysArray)
  for entry in initResult.listeners {
    let prev = entry.reported
    let added = ids.subtracting(prev)
    let removed = prev.subtracting(ids)
    // Unregistered by an earlier listener of this report
    if (!added.isEmpty || !removed.isEmpty)
      && initResult.listeners.contains(where: { $0 === entry })
    {
      entry.reported = ids
      try entry.listener(MarkIDsChange(added: added, ids: ids, removed: removed, tags: tags))
    }
  }
}

/// A reference to the extension a flavor's closures add to the registry.
private final class ExtensionRef {
  weak var extension_: MarkFlavorExtension?
}

/// `createMarkFlavorExtension(...)`: builds a mark flavor extension.
private func createMarkFlavorExtension(
  _ name: String, _ stateKey: String, _ attribute: String,
  _ classNames: @escaping (Editor) -> (mark: String?, overlap: String?),
  _ renderStateName: String, _ configDefaults: Partial<MarkStateConfig>
) -> MarkFlavorExtension {
  let flavor = MarkFlavor(
    name: name, stateKey: stateKey, attribute: attribute, classNames: classNames,
    renderDisabled: createRenderState(renderStateName) { false })
  let ref = ExtensionRef()
  let isMarkableNodeMatch: (LexicalNode) -> Bool = { node in
    (try? isMarkableNode(flavor, node)) ?? false
  }
  // A guard rather than [TextNode, LineBreakNode, DecoratorNode]: the class
  // form requires every class to be registered.
  let decorateOverride = domOverride(
    [.predicate(isMarkableNodeMatch)],
    DOMRenderMatchConfig(decorateDOM: { node, _, dom, editor in
      try decorateMarkIDs(flavor, node, dom, editor)
    }),
    DOMOverrideOptions(disabledForEditor: { ctx in ctx.get(flavor.renderDisabled) }))
  let exportOverride = domOverride(
    [.predicate(isMarkableNodeMatch)],
    DOMRenderMatchConfig(exportDOM: { node, next, editor in
      let output = try next()
      guard let flavorOutput = try flavorOutput(flavor, editor),
        flavorOutput.exportIDsToHTML.peek()
      else {
        return output
      }
      let ids = try readNodeMarkIDs(flavor, node)
      if !ids.isEmpty, let element = output.element as? DOMElement {
        element.setAttribute(attribute, ids.joined(separator: " "))
      }
      return output
    }))
  let importRule = createMarkIDsImportRule(flavor)
  let extension_: MarkFlavorExtension = defineExtension(
    name: name,
    config: configDefaults.applied(to: MarkStateConfig(markFlavor: flavor)),
    dependencies: [
      configExtension(DOMRenderExtension) { $0.overrides = [decorateOverride, exportOverride] },
      configExtension(DOMImportExtension) { $0.rules = [importRule.entry] },
      MarkBaseExtension,
    ],
    init: { _, _, _ in MarkStateInit() },
    build: { editor, config, state in
      let flavors = try state.getDependency(MarkBaseExtension).initResult.flavors
      for (_, other) in flavors {
        try invariant(
          other.stateKey != stateKey,
          "MarkBaseExtension: \(other.name) and \(name) both use stateKey \(JSONValue.string(stateKey).stringify()). Give each flavor its own stateKey."
        )
        try invariant(
          other.attribute != attribute,
          "MarkBaseExtension: \(other.name) and \(name) both render the attribute \(attribute). Give each flavor its own attribute."
        )
      }
      if let extension_ = ref.extension_ {
        flavors.set(stateKey, extension_)
      }
      let initResult = try state.getInitResult()
      initResult.isMarkable = compileMarkableNodes(config.markableNodes)
      registerMarkIDsState(flavor, editor)
      return MarkStateOutput(
        disabled: signal(config.disabled), exportIDsToHTML: signal(config.exportIDsToHTML),
        isMarkIDShown: signal(config.isMarkIDShown ?? isEveryMarkIDShown),
        markIDNodeKeys: initResult.markIDNodeKeys)
    },
    register: { editor, _, state in
      let output = state.getOutput()
      let initResult = try state.getInitResult()
      let splits = try state.getDependency(MarkBaseExtension).initResult.splits
      return mergeRegister(
        try effect { () -> (() -> Void)? in
          let isDisabled = output.disabled.value
          try setRenderContextValue(flavor.renderDisabled, isDisabled, editor)
          if !isDisabled {
            return try registerMarkState(flavor, editor, initResult, splits)
          }
          return nil
        },
        try registerMarkIDShownRerender(
          editor, output.isMarkIDShown, output.disabled, initResult.markIDNodeKeys))
    })
  ref.extension_ = extension_
  return extension_
}

/// `defineMarkExtension(definition)` (`@experimental`): defines a mark
/// flavor: an extension for one kind of annotation, such as comments,
/// suggestions or highlights, that works like `MarkStateExtension` on its
/// own NodeState key. Two flavors on the same text never see each other's
/// ids. Pass the flavor first to the functions:
/// `addMarkID(CommentMarks, "c1", selection)`.
///
/// `stateKey` must be non-empty and without whitespace, other than
/// `"markIds"` (MarkStateExtension's) and `"__proto__"`; this throws
/// otherwise. `attribute` defaults to `data-lexical-` followed by the key in
/// kebab case, and is lowercased.
public func defineMarkExtension(
  name: String, stateKey: String, attribute: String? = nil, className: String? = nil,
  overlapClassName: String? = nil, config: Partial<MarkStateConfig> = Partial()
) throws -> MarkFlavorExtension {
  try invariant(
    isValidMarkID(stateKey) && stateKey != MARK_STATE_KEY && stateKey != "__proto__",
    "defineMarkExtension: invalid stateKey \(JSONValue.string(stateKey).stringify()) for \(name). Use a non-empty name without whitespace, other than \"markIds\" (the key of MarkStateExtension) and \"__proto__\", such as \"commentIds\"."
  )
  let classes = (mark: className, overlap: overlapClassName)
  return createMarkFlavorExtension(
    name, stateKey,
    // Lowercase, as HTML stores attribute names, so that a clash is caught
    (attribute.flatMap { $0.isEmpty ? nil : $0 } ?? defaultMarkAttribute(stateKey)).lowercased(),
    { _ in classes }, "markStateDisabled:\(stateKey)", config)
}

/// `registerMarkIDsListener(editor, flavor, listener)` (`@experimental`):
/// calls `listener` after each commit that changes which ids of `flavor`
/// are in the document. If the document already has ids, it is also called
/// right away, with all of them in `added`. Nothing is reported while the
/// flavor is `disabled`; when it is enabled again, the listener gets the
/// net change. Returns a function that unregisters the listener.
public func registerMarkIDsListener(
  _ editor: Editor, _ flavor: MarkFlavorExtension, _ listener: @escaping MarkIDsListener
) throws -> () -> Void {
  let dep = try getPeerDependencyFromEditor(editor, flavor.name)
  guard let dep, let initResult = dep.initResult as? MarkStateInit,
    let output = dep.output as? MarkStateOutput
  else {
    throw LexicalError(
      "registerMarkIDsListener: the editor was not built with \(flavor.name). Add it to the dependencies of the extension the editor is built from."
    )
  }
  // While disabled the ids are not tracked: the first report comes when the
  // flavor is enabled again
  let ids: Set<String> =
    output.disabled.peek() ? [] : Set(output.markIDNodeKeys.peek().keysArray)
  // Called before it is registered, so that a listener that throws here is
  // not left registered
  if !ids.isEmpty {
    try listener(MarkIDsChange(added: ids, ids: ids, removed: [], tags: []))
  }
  let entry = MarkIDsListenerEntry(listener, ids)
  initResult.listeners.append(entry)
  return { [weak initResult] in
    initResult?.listeners.removeAll { $0 === entry }
  }
}

/// `$getMarkIDsByFlavor(node)` (`@experimental`): the ids of every mark
/// flavor of the active editor on a node, by `stateKey`, sorted. A flavor
/// whose `markableNodes` do not include the node has an empty array.
public func getMarkIDsByFlavor(_ node: LexicalNode) throws -> [String: [String]] {
  var out: [String: [String]] = [:]
  if let base = try getPeerDependencyFromEditor(getEditor(), MARK_BASE_EXTENSION_NAME)?.output
    as? MarkBaseOutput
  {
    for (stateKey, extension_) in base.flavors {
      out[stateKey] = try getMarkableIDs(extension_.__markFlavor, node)
    }
  }
  return out
}

/// `MarkStateExtension` (`@experimental`): an alternative to `MarkNode` for
/// comments and other annotations: the mark ids are stored as a set-valued
/// NodeState (`markIds`) on the leaves they cover, instead of wrapping them
/// in a MarkNode. Marks overlap freely, adding or removing an id changes
/// NodeState only, and the ids travel with the text. Text inserted at the
/// edge of a marked run does not join it, like MarkNode; text inserted
/// inside a run does; text that replaces a selection also gets the ids all
/// of the replaced text had, and pasted content keeps its own ids.
///
/// It is the default mark flavor: `defineMarkExtension` makes more.
public let MarkStateExtension: MarkFlavorExtension = createMarkFlavorExtension(
  MARK_STATE_EXTENSION_NAME, MARK_STATE_KEY, MARK_STATE_IDS_ATTRIBUTE,
  // @internal access with precedent: the render overrides receive the
  // editor, not its theme
  { editor in
    (editor._config.theme["mark"]?.stringValue, editor._config.theme["markOverlap"]?.stringValue)
  }, "markStateDisabled", Partial())

/// `registerMarkIDShownRerender(...)`: re-renders the marked leaves when a
/// new `isMarkIDShown` is assigned, with a full reconcile in an update of
/// its own (which changes no node), unless the document has no marks. When
/// it is assigned while an update is pending, or in a listener that may
/// queue edits, it waits for them to commit. It does not move the DOM
/// selection.
private func registerMarkIDShownRerender(
  _ editor: Editor, _ isMarkIDShown: ReadonlySignal<(String) -> Bool>,
  _ disabled: ReadonlySignal<Bool>, _ markIDNodeKeys: ReadonlySignal<JSMap<String, [NodeKey]>>
) throws -> () -> Void {
  var scheduled = false
  var disposed = false
  func rerender() throws {
    scheduled = false
    if disposed || disabled.peek() || editor.getRootElement() == nil
      || markIDNodeKeys.peek().isEmpty
    {
      return
    }
    // @internal access: whether an update is in progress or waiting to
    // commit, which the full reconcile would otherwise join
    if editor._pendingEditorState != nil {
      // Joins the pending update, and runs once it commits; the re-render
      // is then queued as an update of its own
      scheduled = true
      try editor.update({}, EditorUpdateOptions(onUpdate: rerender))
      return
    }
    // @internal access: in an update or command listener, or an `onUpdate`
    // callback, an update is queued and joins the updates queued before it:
    // wait until they have run
    if editor._updating {
      scheduled = true
      scheduleMicroTask { try? rerender() }
      return
    }
    try editor.update(
      { fullReconcile() }, EditorUpdateOptions(tag: [SKIP_DOM_SELECTION_TAG], discrete: true))
  }
  // A closure cannot be compared with the one it replaces, so every
  // assignment after the first run counts as a new function
  var initial = true
  let unregister = try effect {
    _ = isMarkIDShown.value
    if initial {
      initial = false
      return
    }
    // A scheduled re-render reads the latest function when it runs
    if !scheduled {
      try rerender()
    }
  }
  return {
    disposed = true
    unregister()
  }
}
