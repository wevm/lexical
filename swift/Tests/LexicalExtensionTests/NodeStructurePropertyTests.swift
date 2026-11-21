import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMark
import LexicalOverflow
import LexicalRichText
import LexicalTable
import Testing

// Port of `random broken documents (200 seeds)` from
// packages/lexical-extension/src/__tests__/unit/NodeStructureProperty.test.ts.
//
// Property test of NodeStructureExtension over random documents that break
// every declared rule, nested: blocks in paragraphs, headings, quotes, code,
// list items and links; paragraphs in quotes, list items and lists; inline
// content and stray blocks in lists; equal adjacent lists and links; block
// decorators anywhere in ordered list items; tables, shadow root quotes and
// the character limit's OverflowNode; empty links, marks and OverflowNodes,
// which Lexical removes; links, autolinks and marks around blocks; equal
// adjacent lists whose items at the seam hold a nested list, alone or with
// other content.
//
// Upstream draws every choice from the seeded mulberry32 (`rng`) with
// `Math.floor(r() * n)`, in the same order as here, so each seed builds the
// same documents. `NODE_STRUCTURE_SEEDS` and `NODE_STRUCTURE_FIRST_SEED` are
// not ported: the default 200 seeds from 1 run.

private let SEEDS = 200
private let FIRST_SEED = 1

private func rng(_ seed: Int) -> () -> Double {
  mulberry32(UInt32(truncatingIfNeeded: seed))
}

/// `Math.floor(r() * n)`.
private func drawIndex(_ r: () -> Double, _ n: Int) -> Int {
  Int((r() * Double(n)).rounded(.down))
}

/// Editors built for the current seed, disposed after it.
nonisolated(unsafe) private var built: [Editor] = []

private func disposeBuilt() {
  for editor in built { editor.dispose() }
  built = []
}

private enum Structure { case on, off, disabled }

private func build(
  _ structure: Structure, _ extra: [any AnyLexicalExtensionArgument] = []
)
  throws -> Editor
{
  var deps: [any AnyLexicalExtensionArgument] =
    [
      RichTextExtension, ListExtension, LinkExtension,
      configExtension(TableExtension) { $0.hasHorizontalScroll = false }, HorizontalRuleExtension,
    ] + extra
  if structure != .off {
    deps.append(configExtension(NodeStructureExtension) { $0.disabled = structure == .disabled })
  }
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "node-structure-property", dependencies: deps,
      nodes: {
        [
          .klass(AutoLinkNode.self), .klass(CodeNode.self), .klass(MarkNode.self),
          .klass(OverflowNode.self),
        ]
      },
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
  built.append(editor)
  return editor
}

nonisolated(unsafe) private var token = 0

/// `` `[${token++}]` ``.
private func nextToken() -> String {
  defer { token += 1 }
  return "[\(token)]"
}

/// ElementNode's own splice, so that a list can hold what it may not.
@discardableResult
private func put<T: ElementNode>(_ parent: T, _ children: [LexicalNode]) throws -> T {
  try parent.elementSplice(parent.getChildrenSize(), 0, children)
}

/// A random broken subtree. `extended` adds what an editor without the
/// extension throws on or loses content in often enough that its own results
/// cannot serve as a reference: autolinks around blocks, and equal adjacent
/// lists that merge (see `seamLists`).
private func gen(
  _ r: @escaping () -> Double, _ depth: Int, _ extended: Bool = false
) throws
  -> LexicalNode
{
  func pick<T>(_ items: [T]) -> T { items[drawIndex(r, items.count)] }
  func leaf() throws -> LexicalNode {
    let x = r()
    return x < 0.6
      ? try createTextNode(nextToken())
      : x < 0.75 ? try createLineBreakNode() : try createHorizontalRuleNode()
  }
  if depth <= 0 {
    return try leaf()
  }
  func kids(_ n: Int? = nil) throws -> [LexicalNode] {
    try (0..<(n ?? drawIndex(r, 4))).map { _ in try gen(r, depth - 1, extended) }
  }
  switch drawIndex(r, extended ? 19 : 18) {
  case 0, 1:
    return try leaf()
  case 2, 3:
    return try put(createParagraphNode(), kids())
  case 4:
    return try put(createHeadingNode(pick(["h1", "h2"])), kids())
  case 5:
    return try put(createQuoteNode(), kids())
  case 6:
    return try put(createCodeNode(), kids())
  case 7, 8:
    let list = try createListNode(
      pick(["number", "number", "bullet", "check"]), start: 1 + drawIndex(r, 5))
    let n = 1 + drawIndex(r, 3)
    for _ in 0..<n {
      try put(
        list, [r() < 0.85 ? try put(createListItemNode(), kids()) : try gen(r, depth - 1, extended)]
      )
    }
    return list
  case 9, 10:
    return try put(createLinkNode(pick(["https://a", "https://b"])), kids())
  case 11:
    return try put(createAutoLinkNode("https://a"), extended ? kids() : kids(1))
  case 12:
    return try put(createMarkNode([pick(["m1", "m2"])]), kids())
  case 13:
    return try createTableNode().append(
      createTableRowNode().append(
        put(createTableCellNode(), [try put(createParagraphNode(), kids())]),
        put(createTableCellNode(), kids())))
  case 14:
    return try put(createOverflowNode(), kids())
  case 15:
    return try put(createQuoteNode(shadowRoot: true), kids())
  case 16:
    // A list item outside a list.
    return try put(createListItemNode(), kids())
  case 17:
    if !extended {
      return try createParagraphNode()
    }
    // Equal adjacent lists, in a block that does not hold them (they are
    // lifted out next to each other) or in an inline element.
    let makers: [() throws -> ElementNode] = [
      { try createParagraphNode() }, { try createQuoteNode() }, { try createListItemNode() },
      { try createLinkNode("https://a") }, { try createMarkNode(["m1"]) },
    ]
    return try put(pick(makers)(), seamLists(r, depth - 1))
  default:
    return try createParagraphNode()
  }
}

/// Two equal adjacent lists (which merge) whose items at the seam each hold a
/// nested list, alone (they are joined) or with other content before or after
/// it (nothing of which may be lost or moved).
private func seamLists(_ r: @escaping () -> Double, _ depth: Int) throws -> [LexicalNode] {
  func pick<T>(_ items: [T]) -> T { items[drawIndex(r, items.count)] }
  let type = pick(["number", "bullet"])
  let nestedType = pick(["number", "bullet"])
  func item() throws -> ListItemNode {
    let node = try createListItemNode()
    return try put(node, (0..<drawIndex(r, 3)).map { _ in try gen(r, depth - 1, true) })
  }
  func seamItem() throws -> ListItemNode {
    let nested = try put(createListNode(nestedType), [try item()])
    if r() < 0.5 {
      return try put(createListItemNode(), [nested])
    }
    let other: LexicalNode =
      r() < 0.7 ? try createTextNode(nextToken()) : try gen(r, depth - 1, true)
    return try put(createListItemNode(), r() < 0.5 ? [nested, other] : [other, nested])
  }
  let first = try createListNode(type)
  try put(first, (r() < 0.5 ? [try item()] : []) + [try seamItem()])
  let second = try createListNode(type)
  let seam = try seamItem()
  try put(second, [seam] + (r() < 0.5 ? [try item()] : []))
  return [first, second]
}

private func isInlineNode(_ node: LexicalNode) throws -> Bool {
  try node is TextNode || node is LineBreakNode || (node is ElementNode && node.isInline())
}

private func rootChildren(_ editor: Editor) throws -> [JSONObject] {
  let root = try #require(try editor.getEditorState().toJSON()["root"]?.objectValue)
  return try #require(root["children"]?.arrayValue).map { try #require($0.objectValue) }
}

/// A random broken document: `plain` as an editor without the extension
/// leaves it (its own transforms ran), `input` the same with empty inline
/// elements put in as they are.
private func makeInput(
  _ seed: Int
) throws -> (
  generated: [JSONObject], input: [JSONObject]?, plain: [JSONObject]?
) {
  let generated = try generateDocument(seed)
  let editor = try build(.off)
  let r = rng(seed)
  token = 0
  do {
    try editor.update(discrete: true) {
      let n = 1 + drawIndex(r, 4)
      for _ in 0..<n {
        var node = try gen(r, 4)
        while try isInlineNode(node) {
          node = try gen(r, 4)
        }
        try getRoot().append(node)
      }
    }
  } catch {
    return (generated, nil, nil)
  }
  let plain = try rootChildren(editor)
  return (generated, try injectEmptyInlines(plain, seed), plain)
}

/// A random broken document from the extended generator, as it is built
/// (before any transform runs), with equal adjacent lists at the root too.
private func generateDocument(_ seed: Int) throws -> [JSONObject] {
  let r = rng(seed * 2741 + 1)
  token = 0
  var out: [JSONObject] = []
  try build(.off).update(discrete: true, skipTransforms: true) {
    let n = 1 + drawIndex(r, 4)
    for _ in 0..<n {
      if r() < 0.15 {
        try put(getRoot(), seamLists(r, 3))
        continue
      }
      // Not a list item either: one at the root cannot be serialized.
      var node = try gen(r, 4, true)
      while try isInlineNode(node) || node is ListItemNode {
        node = try gen(r, 4, true)
      }
      try getRoot().append(node)
    }
    out = try getRoot().getChildren().map(toJSON)
  }
  return out
}

/// An empty inline element that Lexical removes (directly, or once what it
/// holds is removed). Half the time it is the only content of a run next to a
/// block its holder is split around, where whether it is still there decides
/// whether that run's part stays.
private func emptyInline(_ r: @escaping () -> Double, _ holder: Bool) throws -> LexicalNode {
  func pick<T>(_ items: [T]) -> T { items[drawIndex(r, items.count)] }
  func empty() throws -> ElementNode {
    let makers: [() throws -> ElementNode] = [
      { try createLinkNode(pick(["https://a", "https://b"])) },
      { try createMarkNode([pick(["m1", "m2"])]) }, { try createOverflowNode() },
    ]
    return try pick(makers)()
  }
  let outer = try empty()
  let nested = r() < 0.5 ? outer : try put(outer, [try empty()])
  if !holder {
    return nested
  }
  var parts: [LexicalNode] = [
    nested,
    r() < 0.5
      ? try createHorizontalRuleNode()
      : try put(createHeadingNode("h1"), [try createTextNode(nextToken())]),
  ]
  if r() < 0.5 {
    parts.reverse()
  }
  if r() < 0.5 {
    parts.append(try createTextNode(nextToken()))
  }
  let makers: [() throws -> ElementNode] = [
    { try put(createQuoteNode(), parts) }, { try put(createParagraphNode(), parts) },
    { try put(createListNode("number"), [try put(createListItemNode(), parts)]) },
  ]
  return try pick(makers)()
}

private func childObjects(_ tree: JSONObject) -> [JSONObject]? {
  tree["children"]?.arrayValue?.map { $0.objectValue ?? JSONObject() }
}

/// `input` with empty inline elements put in at random places, as they are:
/// the transforms of the editor that generated it would have removed them.
private func injectEmptyInlines(_ input: [JSONObject], _ seed: Int) throws -> [JSONObject] {
  let r = rng(seed * 389)
  var out = input
  try build(.off).update(discrete: true, skipTransforms: true) {
    func insert(_ children: inout [JSONObject], _ holder: Bool) throws {
      let at = drawIndex(r, children.count + 1)
      children.insert(try toJSON(emptyInline(r, holder)), at: at)
    }
    func walk(_ tree: inout JSONObject) throws {
      let type = tree["type"]?.stringValue
      guard var children = childObjects(tree), type != "table", type != "tablerow" else {
        return
      }
      for i in children.indices { try walk(&children[i]) }
      if r() < 0.12 {
        try insert(&children, r() < 0.5)
      }
      tree["children"] = .array(children.map { .object($0) })
    }
    for i in out.indices { try walk(&out[i]) }
    if r() < 0.3 {
      try insert(&out, true)
    }
  }
  return out
}

private func load(_ editor: Editor, _ input: [JSONObject]) throws {
  try editor.update(discrete: true) {
    try getRoot().clear().append(contentsOf: input.map { try parseSerializedNode($0) })
  }
}

private func allNodes() throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  func walk(_ node: LexicalNode) throws {
    out.append(node)
    if let element = node as? ElementNode {
      try element.getChildren().forEach(walk)
    }
  }
  try getRoot().getChildren().forEach(walk)
  return out
}

private final class Entry {
  let json: JSONObject
  let children: [Entry]
  init(_ json: JSONObject) {
    self.json = json
    children = (childObjects(json) ?? []).map(Entry.init)
  }
}

/// The orders the nodes of a document are created in (which is the order
/// Lexical runs their transforms in).
private typealias Order = ([Entry], Int) -> [Entry]

/// Each node before its children, in document order: what loading does.
nonisolated(unsafe) private let outerFirst: Order = { all, _ in all }

nonisolated(unsafe) private let orders: [(String, Order)] = [
  // Each node after its children.
  (
    "innerFirst",
    { all, _ in
      var out: [Entry] = []
      func walk(_ e: Entry) {
        e.children.forEach(walk)
        out.append(e)
      }
      let tops = all.filter { e in !all.contains { p in p.children.contains { $0 === e } } }
      tops.forEach(walk)
      return out
    }
  ),
  ("outerFirst", outerFirst),
  ("reverse", { all, _ in all.reversed() }),
  (
    "shuffled",
    { all, orderSeed in
      let r = rng(orderSeed * 7919)
      var c = all
      for i in stride(from: c.count - 1, to: 0, by: -1) {
        c.swapAt(i, drawIndex(r, i + 1))
      }
      return c
    }
  ),
]

private func isElementJSON(_ json: JSONObject) -> Bool {
  json["children"]?.arrayValue != nil
}

/// Load `input` creating its nodes in `order`, then linking them exactly as
/// `parseSerializedNode` does (each child appended after its own children).
private func loadInOrder(
  _ editor: Editor, _ input: [JSONObject], _ order: Order, _ seed: Int, faithful: Bool = false,
  skipTransforms: Bool = false
) throws {
  let tops = input.map(Entry.init)
  var all: [Entry] = []
  func walk(_ e: Entry) {
    all.append(e)
    e.children.forEach(walk)
  }
  tops.forEach(walk)
  let ordered = order(all, seed)
  try editor.update(discrete: true, skipTransforms: skipTransforms) {
    var nodes: [ObjectIdentifier: LexicalNode] = [:]
    for e in ordered {
      var json = e.json
      if isElementJSON(json) {
        json["children"] = .array([])
      }
      nodes[ObjectIdentifier(e)] = try parseSerializedNode(json)
    }
    func link(_ e: Entry) throws -> LexicalNode {
      let node = nodes[ObjectIdentifier(e)]!
      for child in e.children {
        guard let element = node as? ElementNode else {
          throw LexicalError("Cannot append to \(node.getType())")
        }
        if faithful {
          try put(element, [try link(child)])
        } else {
          try element.append(link(child))
        }
      }
      return node
    }
    try getRoot().clear().append(contentsOf: tops.map(link))
  }
}

/// Load `input` as it is, as an update applied with `skipTransforms` does (no
/// transform runs, so nothing is normalized yet), running `then` in the same
/// update.
private func loadAsIs(
  _ editor: Editor, _ input: [JSONObject], _ then: @escaping () throws -> Void = {}
) throws {
  try editor.update(discrete: true, skipTransforms: true) {
    try getRoot().clear().append(contentsOf: input.map(buildAsIs))
    try then()
  }
}

/// The orders in which every node of a loaded document is marked dirty.
nonisolated(unsafe) private let dirtyOrders:
  [(String, ([LexicalNode], Int) throws -> [LexicalNode])] = [
    // Each node before its children, in document order.
    ("forward", { nodes, _ in nodes }),
    // Each node after its children.
    (
      "innerFirst",
      { nodes, _ in
        var out: [LexicalNode] = []
        func walk(_ node: LexicalNode) throws {
          if let element = node as? ElementNode {
            try element.getChildren().forEach(walk)
          }
          out.append(node)
        }
        try getRoot().getChildren().forEach(walk)
        let keys = Set(nodes.map { $0.getKey() })
        return out.filter { keys.contains($0.getKey()) }
      }
    ),
    ("reverse", { nodes, _ in nodes.reversed() }),
    (
      "shuffled",
      { nodes, orderSeed in
        let r = rng(orderSeed * 6271)
        var c = nodes
        for i in stride(from: c.count - 1, to: 0, by: -1) {
          c.swapAt(i, drawIndex(r, i + 1))
        }
        return c
      }
    ),
  ]

/// `input` with a style of its own on every text node, so that text
/// normalization never merges two of them: Lexical moves an element point
/// between two text nodes it merges, which is not the normalization's doing.
private func withDistinctStyles(_ input: [JSONObject]) -> [JSONObject] {
  var n = 0
  func walk(_ json: inout JSONObject) {
    if json["type"]?.stringValue == "text" {
      json["style"] = .string("--n: \(n)")
      n += 1
    }
    if var children = childObjects(json) {
      for i in children.indices { walk(&children[i]) }
      json["children"] = .array(children.map { .object($0) })
    }
  }
  var copy = input
  for i in copy.indices { walk(&copy[i]) }
  return copy
}

/// What a leaf adds to a position in the content: text, a decorator 1.
private func leafSize(_ node: LexicalNode) throws -> Int {
  if let text = node as? TextNode { return try text.getTextContentSize() }
  return node is DecoratorNode ? 1 : 0
}

private func sizeOf(_ node: LexicalNode) throws -> Int {
  if let element = node as? ElementNode {
    return try element.getChildren().reduce(0) { $0 + (try sizeOf($1)) }
  }
  return try leafSize(node)
}

/// Where a point is in the content: the size of every leaf before it in
/// document order (line breaks count 0, since joining paragraphs adds them).
private func contentOffset(_ key: NodeKey, _ offset: Int) throws -> Int {
  var before = 0
  var found = false
  func walk(_ node: LexicalNode) throws {
    if found {
      return
    }
    if node.getKey() == key {
      found = true
      if let element = node as? ElementNode {
        before += try element.getChildren().prefix(offset).reduce(0) { $0 + (try sizeOf($1)) }
      } else {
        before += offset
      }
      return
    }
    if let element = node as? ElementNode {
      try element.getChildren().forEach(walk)
    } else {
      before += try leafSize(node)
    }
  }
  try walk(getRoot())
  return found ? before : -1
}

private struct Leaf {
  let key: NodeKey
  let desc: String
  let kind: String?
  let inTable: Bool
}

/// The block a text node is a line of: its nearest ancestor that is neither
/// inline nor a plain paragraph (which has no kind of its own and becomes
/// lines of the block around it). Its kind must survive the repairs; only a
/// list's stray inline content gains one (it is wrapped in a list item).
private func blockKindOf(_ node: LexicalNode) throws -> String {
  for p in try node.getParents() {
    if try p.getType() == "paragraph" || p is OverflowNode || p.isInline() {
      continue
    }
    if let heading = p as? HeadingNode {
      return try heading.getTag()
    }
    return p.getType()
  }
  return "root"
}

private func leaves(_ editor: Editor) throws -> [Leaf] {
  try editor.read {
    try allNodes().filter { !($0 is ElementNode) }.map { n in
      Leaf(
        key: n.getKey(), desc: n is TextNode ? try n.getTextContent() : n.getType(),
        kind: n is TextNode ? try blockKindOf(n) : nil,
        inTable: try n.getParents().contains { $0 is TableNode })
    }
  }
}

private func units(_ text: String) -> [String] {
  text.matches(of: /\[\d+\]|<hr>/).map { String($0.output) }
}

/// Whether `result` keeps every piece of content `kept` has (as many times),
/// in the order `created` has it, and nothing else.
private func keepsContent(_ result: String, _ created: String, _ kept: String) -> Bool {
  var counts: [String: Int] = [:]
  for unit in units(result) {
    counts[unit, default: 0] += 1
  }
  for unit in units(kept) {
    let n = counts[unit] ?? 0
    if n == 0 {
      return false
    }
    counts[unit] = n - 1
  }
  // A subsequence of what was created.
  let all = units(created)
  var at = 0
  for unit in units(result) {
    while at < all.count && all[at] != unit {
      at += 1
    }
    if at == all.count {
      return false
    }
    at += 1
  }
  return true
}

private func tokens(_ editor: Editor) throws -> String {
  try editor.read {
    try allNodes().map { n in
      n is TextNode ? try n.getTextContent() : n is DecoratorNode ? "<hr>" : ""
    }.joined()
  }
}

/// Each piece of text (a `[n]` token) with the links, autolinks and marks
/// around it, outermost first: what they say about it (a URL, comment ids)
/// must stay on it.
private func annotations(_ editor: Editor) throws -> [String] {
  try editor.read {
    var out: [String] = []
    for node in try allNodes() where node is TextNode {
      let around = try node.getParents().reversed().flatMap { p -> [String] in
        if let mark = p as? MarkNode {
          return ["mark:\(try mark.getIDs().joined(separator: ","))"]
        }
        if let link = p as? LinkNode {
          return ["\(link.getType()):\(try link.getURL())"]
        }
        return []
      }.joined(separator: ">")
      for unit in try node.getTextContent().matches(of: /\[\d+\]/) {
        out.append("\(unit.output) \(around)")
      }
    }
    return out
  }
}

/// The first difference between two lists of annotated text, for a report.
private func firstDifference(_ a: [String], _ b: [String]) -> String {
  for i in 0..<max(a.count, b.count) {
    let x = i < a.count ? a[i] : nil
    let y = i < b.count ? b[i] : nil
    if x != y {
      return "\(x ?? "undefined") -> \(y ?? "undefined")"
    }
  }
  return ""
}

/// Whether Lexical itself throws on `children` without the extension (some
/// random edits trip transforms of their own, such as a list inside a link).
private func lexicalRejects(_ children: [JSONObject]) -> Bool {
  do {
    try build(.off).update(discrete: true) {
      try getRoot().clear().append(contentsOf: children.map(buildAsIs))
    }
    return false
  } catch {
    return true
  }
}

/// A node built from JSON as it is (no override rewrites its children).
private func buildAsIs(_ json: JSONObject) throws -> LexicalNode {
  guard let children = childObjects(json) else {
    return try parseSerializedNode(json)
  }
  var shell = json
  shell["children"] = .array([])
  guard let element = try parseSerializedNode(shell) as? ElementNode else {
    throw LexicalError("\(json["type"]?.stringValue ?? "?") is not an element")
  }
  return try put(element, children.map(buildAsIs))
}

/// Serialize a subtree, children included, before any transform runs.
private func toJSON(_ node: LexicalNode) throws -> JSONObject {
  var json = try node.exportJSON()
  if let element = node as? ElementNode {
    json["children"] = .array(try element.getChildren().map { .object(try toJSON($0)) })
  }
  return json
}

private struct RandomEdit {
  let path: [Int]
  let index: Int
  let node: JSONObject
  let edited: [JSONObject]
}

private func inserting(
  _ tree: JSONObject, _ path: ArraySlice<Int>, _ index: Int, _ node: JSONObject
) -> JSONObject {
  var tree = tree
  var children = childObjects(tree) ?? []
  if let first = path.first {
    children[first] = inserting(children[first], path.dropFirst(), index, node)
  } else {
    children.insert(node, at: index)
  }
  tree["children"] = .array(children.map { .object($0) })
  return tree
}

/// A random broken subtree inserted at a random place of `root`.
private func randomEdit(
  _ root: JSONObject, _ seed: Int, _ extended: Bool = false
) throws
  -> RandomEdit?
{
  let r = rng(seed * 104729)
  var targets: [([Int], JSONObject)] = []
  func walk(_ tree: JSONObject, _ path: [Int]) {
    let type = tree["type"]?.stringValue
    if let children = childObjects(tree), type != "table", type != "tablerow" {
      targets.append((path, tree))
      for (i, c) in children.enumerated() { walk(c, path + [i]) }
    }
  }
  walk(root, [])
  let (path, target) = targets[drawIndex(r, targets.count)]
  let index = drawIndex(r, (childObjects(target) ?? []).count + 1)
  var node: JSONObject?
  let scratch = try build(.off)
  try scratch.update(discrete: true) {
    token = 100000
    var n = try gen(r, 2, extended)
    while try path.isEmpty && isInlineNode(n) {
      n = try gen(r, 2, extended)
    }
    node = try toJSON(n)
  }
  guard let node else {
    return nil
  }
  let edited = inserting(root, path[...], index, node)
  return RandomEdit(path: path, index: index, node: node, edited: childObjects(edited) ?? [])
}

private func stripping(_ value: JSONValue, _ keys: Set<String>) -> JSONValue {
  switch value {
  case .object(let object):
    var out = JSONObject()
    for (key, child) in object where !keys.contains(key) {
      out[key] = stripping(child, keys)
    }
    return .object(out)
  case .array(let items):
    return .array(items.map { stripping($0, keys) })
  default:
    return value
  }
}

private func withoutDerived(_ editor: Editor) throws -> String {
  let derived = getDeclaredNodeStructure(ListItemNode.self).derived
  return stripping(.object(try editor.getEditorState().toJSON()), derived).stringify()
}

private func stateJSON(_ editor: Editor) throws -> String {
  JSONValue.object(try editor.getEditorState().toJSON()).stringify()
}

private func allows(_ children: NodeChildrenConfig, _ child: LexicalNode) throws -> Bool {
  if child is OverflowNode {
    return true
  }
  let inline = try child is TextNode || child is LineBreakNode || child.isInline()
  switch children {
  case .inline:
    return inline
  case .block:
    return !inline
  case .list(let entries):
    if inline {
      return entries.contains(.inline)
    }
    return entries.contains { entry in
      if case .klass(let klass) = entry { return isSubclass(type(of: child), of: klass) }
      return false
    }
  }
}

/// Violations left in the document (it must be a fixed point).
private func violations(_ editor: Editor) throws -> [String] {
  try editor.read {
    var out: [String] = []
    for node in [try getRoot() as LexicalNode] + (try allNodes()) {
      guard let element = node as? ElementNode else {
        continue
      }
      let children: NodeChildrenConfig? =
        try isRootOrShadowRoot(element)
        ? .block : getDeclaredNodeStructure(type(of: element)).children
      if try element.isEmpty(), !(try isRootOrShadowRoot(element)),
        try element is OverflowNode || (element.isInline() && !element.canBeEmpty())
      {
        out.append("empty \(element.getType())")
      }
      for child in try element.getChildren() {
        if let children, !(try allows(children, child)) {
          out.append("\(element.getType()) holds \(child.getType())")
        }
        let next = try child.getNextSibling()
        let declared = getDeclaredNodeStructure(type(of: child))
        if let next, type(of: next) == type(of: child), let mergeAdjacent = declared.mergeAdjacent,
          declared.declaredBy.mergeAdjacent.map(ObjectIdentifier.init)
            == ObjectIdentifier(type(of: child))
        {
          let a = try child.exportJSON()
          let b = try next.exportJSON()
          let equal = mergeAdjacent.by.allSatisfy { entry in
            if case .field(let k) = entry { return a[k] == b[k] }
            return false
          }
          if equal {
            out.append("equal adjacent \(child.getType())")
          }
        }
      }
    }
    return out
  }
}

private typealias Fail = (_ kind: String, _ failedSeed: Int, _ msg: String) -> Void

/// Three carets placed at random in `doc` as it is (loaded without any
/// transform) keep their place in the content: after the normalization each
/// is at the same offset in document order, and valid.
private func checkCarets(
  _ doc: [JSONObject], _ seed: Int, _ caretSeed: Int, _ label: String, _ fail: Fail
) throws {
  let styled = withDistinctStyles(doc)
  let probe = try build(.off)
  var points: [([Int], Int, PointType)] = []
  try loadAsIs(probe, styled) {
    func walk(_ node: LexicalNode, _ path: [Int]) throws {
      if let text = node as? TextNode {
        for i in 0...(try text.getTextContentSize()) { points.append((path, i, .text)) }
      } else if let element = node as? ElementNode {
        for i in 0...(try element.getChildrenSize()) { points.append((path, i, .element)) }
        for (i, c) in try element.getChildren().enumerated() { try walk(c, path + [i]) }
      }
    }
    try walk(getRoot(), [])
  }
  let r = rng(caretSeed)
  var k = 0
  while k < 3 && !points.isEmpty {
    defer { k += 1 }
    let (path, offset, type) = points[drawIndex(r, points.count)]
    let editor = try build(.on)
    var before = -1
    var place = ""
    do {
      try loadAsIs(editor, styled) {
        var node: LexicalNode = try getRoot()
        for i in path {
          guard let child = try (node as? ElementNode)?.getChildAtIndex(i) else {
            throw LexicalError("no child \(i)")
          }
          node = child
        }
        let selection = createRangeSelection()
        try selection.anchor.set(node.getKey(), offset, type)
        try selection.focus.set(node.getKey(), offset, type)
        try setSelection(selection)
        before = try contentOffset(node.getKey(), offset)
        place = "\(node.getType())[\(path.map(String.init).joined(separator: ","))]@\(offset)"
      }
      try editor.update(discrete: true) { try allNodes().forEach { try $0.markDirty() } }
    } catch {
      fail("throws (caret\(label))", seed, String(describing: error))
      break
    }
    let after = try editor.read { () throws -> String in
      guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed() else {
        return "no collapsed range selection"
      }
      let anchor = selection.anchor
      let node = try anchor.getNode()
      var valid = try node.isAttached()
      if valid {
        if let text = node as? TextNode {
          valid = try anchor.type == .text && anchor.offset <= text.getTextContentSize()
        } else if let element = node as? ElementNode {
          valid = anchor.offset <= (try element.getChildrenSize())
        } else {
          valid = false
        }
      }
      return valid
        ? String(try contentOffset(anchor.key, anchor.offset))
        : "invalid \(anchor.type) point in \(node.getType())"
    }
    if after != String(before) {
      fail("caret moved\(label)", seed, "\(place): \(before) -> \(after)")
    }
  }
}

/// A local edit that breaks the normalized document `state` (inserting a
/// random broken subtree), where only the nodes it writes are dirty, gives
/// what loading the edited document gives, with every node dirty (as if it
/// arrived unrepaired), and leaves a fixed point.
private func checkLocalEdit(
  _ state: String, _ editSeed: Int, _ label: String, _ fail: Fail, _ seed: Int? = nil
) throws {
  let seed = seed ?? editSeed
  guard case .object(let parsed) = try JSONValue.parse(state),
    let root = parsed["root"]?.objectValue
  else {
    throw LexicalError("not a serialized editor state")
  }
  let children = childObjects(root) ?? []
  guard let edit = try randomEdit(root, editSeed, label != ""), !lexicalRejects(edit.edited)
  else {
    return
  }
  let local = try build(.on)
  let fresh = try build(.on)
  do {
    try local.update(discrete: true) {
      try getRoot().clear().append(contentsOf: children.map(buildAsIs))
    }
    try local.update(discrete: true) {
      var target: ElementNode = try getRoot()
      for i in edit.path {
        guard let child = try target.getChildAtIndex(i) as? ElementNode else {
          throw LexicalError("no element at \(i)")
        }
        target = child
      }
      try put(target, [])
      _ = try target.elementSplice(edit.index, 0, [try buildAsIs(edit.node)])
    }
    try fresh.update(discrete: true) {
      try getRoot().clear().append(contentsOf: edit.edited.map(buildAsIs))
    }
  } catch {
    fail("throws on an edit\(label)", seed, String(describing: error))
    return
  }
  // Derived values (a list item's number) are recomputed by their class's
  // own transform when it runs, which this edit (as without the extension)
  // does not always trigger: compare the rest.
  if try withoutDerived(fresh) != withoutDerived(local) {
    fail("a local edit and a fresh load disagree\(label)", seed, "")
  }
  let asBuilt = try build(.off)
  try loadAsIs(asBuilt, edit.edited)
  if try tokens(local) != tokens(asBuilt) {
    fail("content lost by a local edit\(label)", seed, "")
  }
  let remaining = try violations(local)
  if !remaining.isEmpty {
    fail(
      "not a fixed point after a local edit\(label)", seed, remaining.joined(separator: ", "))
  }
}

private func enableStructure(_ editor: Editor) throws {
  try getExtensionDependencyFromEditor(editor, NodeStructureExtension).output.disabled.set(false)
  try editor.read {}
}

private func markAllDirty(_ editor: Editor) throws {
  try editor.update(discrete: true) { try allNodes().forEach { try $0.markDirty() } }
}

@Suite struct NodeStructurePropertyTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureProperty.test.ts::NodeStructureExtension properties > random broken documents (200 seeds)"
    ))
  func randomBrokenDocuments() throws {
    var failures: [(kind: String, messages: [String])] = []
    let fail: Fail = { kind, failedSeed, msg in
      let line = String("\(failedSeed) \(msg)".prefix(300))
      if let i = failures.firstIndex(where: { $0.kind == kind }) {
        failures[i].messages.append(line)
      } else {
        failures.append((kind, [line]))
      }
    }
    var checked = 0
    var generatedChecked = 0
    defer { disposeBuilt() }
    for seed in FIRST_SEED..<(FIRST_SEED + SEEDS) {
      disposeBuilt()
      let (generated, input, plain) = try makeInput(seed)
      generatedChecked += 1
      // The document as generated, before any transform ran: links,
      // autolinks and marks around blocks (which an editor without the
      // extension lifts blocks out of only in part, or throws on), and equal
      // adjacent lists whose items at the seam hold a nested list. Nothing is
      // lost, duplicated or reordered, every piece of text keeps the links,
      // autolinks and marks around it, and the result is a fixed point that
      // depends on the document only. (`INTERNAL_normalizeNodeStructure` is
      // checked on the documents above only: in an update without
      // transforms, Lexical's own, such as the one that puts a list item
      // outside a list into a list, do not run.)
      do {
        let asBuilt = try build(.off)
        try loadAsIs(asBuilt, generated)
        let rawTokens = try tokens(asBuilt)
        let rawAnnotations = try annotations(asBuilt)
        let editor = try build(.on)
        var reference: String?
        do {
          try editor.update(discrete: true) {
            try getRoot().clear().append(contentsOf: generated.map(buildAsIs))
          }
          reference = try stateJSON(editor)
        } catch {
          fail("throws (generated)", seed, String(describing: error))
        }
        if let reference {
          if try tokens(editor) != rawTokens {
            fail("content lost, duplicated or reordered (generated)", seed, "")
          }
          let got = try annotations(editor)
          if got != rawAnnotations {
            fail("annotation lost (generated)", seed, firstDifference(rawAnnotations, got))
          }
          let remaining = try violations(editor)
          if !remaining.isEmpty {
            fail("not a fixed point (generated)", seed, remaining.joined(separator: ", "))
          }
          try markAllDirty(editor)
          if try stateJSON(editor) != reference {
            fail("not idempotent (generated)", seed, "")
          }
          for (name, order) in orders {
            let other = try build(.on)
            do {
              try loadInOrder(other, generated, order, seed, faithful: true)
            } catch {
              fail("throws (generated, \(name))", seed, String(describing: error))
              continue
            }
            if try stateJSON(other) != reference {
              fail("result depends on dirty order (generated, \(name))", seed, "")
            }
          }
          for (name, order) in dirtyOrders {
            let other = try build(.on)
            do {
              try loadAsIs(other, generated)
              try other.update(discrete: true) {
                try order(allNodes(), seed).forEach { try $0.markDirty() }
              }
            } catch {
              fail("throws (generated, dirty \(name))", seed, String(describing: error))
              continue
            }
            if try stateJSON(other) != reference {
              fail("result depends on dirty order (generated, dirty \(name))", seed, "")
            }
          }
          let later = try build(.disabled)
          try loadAsIs(later, generated)
          try enableStructure(later)
          if try stateJSON(later) != reference {
            fail("result differs when enforcement is turned on (generated)", seed, "")
          }
        }
        try checkCarets(generated, seed, seed * 7741, " (generated)", fail)
        if let reference {
          try checkLocalEdit(reference, seed * 31 + 7, " (generated)", fail, seed)
        }
      }
      guard let plain, let input else {
        // Lexical's own transforms reject the document without the extension
        // (a list inside a link): there is no reference to compare with.
        continue
      }
      // Loading with the extension: everything is created, outer nodes
      // first. A document Lexical's own transforms reject without the
      // extension (a list inside a link) is skipped.
      let off = try build(.off)
      do {
        try load(off, plain)
      } catch {
        continue
      }
      checked += 1
      let loaded = try build(.on)
      do {
        try load(loaded, plain)
      } catch {
        fail("throws on load", seed, String(describing: error))
        continue
      }
      if try tokens(loaded) != tokens(off) {
        fail("content lost, duplicated or reordered on load", seed, "")
      }
      // What a link, autolink or mark says about the text it holds stays on
      // it, as it does without the extension.
      if try annotations(loaded) != annotations(off) {
        fail(
          "annotation lost on load", seed,
          firstDifference(try annotations(off), try annotations(loaded)))
      }
      let want = try stateJSON(loaded)
      let left = try violations(loaded)
      if !left.isEmpty {
        fail("not a fixed point", seed, left.joined(separator: ", "))
      }
      // Idempotence: everything dirty again changes nothing.
      try markAllDirty(loaded)
      if try stateJSON(loaded) != want {
        fail("not idempotent", seed, "")
      }
      // Idempotence of the whole normalization on its own result.
      try loaded.update(discrete: true) { try INTERNAL_normalizeNodeStructure(getRoot()) }
      if try stateJSON(loaded) != want {
        fail("not idempotent (INTERNAL_$normalizeNodeStructure)", seed, "")
      }
      // The same document with its nodes created in other orders.
      for (name, order) in orders {
        let editor = try build(.on)
        do {
          try loadInOrder(editor, plain, order, seed)
        } catch {
          fail("throws (\(name))", seed, String(describing: error))
          continue
        }
        if try stateJSON(editor) != want {
          fail("result depends on dirty order (\(name))", seed, "")
        }
      }
      // The document with empty inline elements put in, loaded as it is (as
      // an update applied with skipTransforms leaves it), then every node
      // marked dirty, in several orders: the same result whichever
      // transforms run first (the normalization's, or Lexical's own, such as
      // the removal of an empty inline element), with the same content.
      let raw = try build(.off)
      try loadAsIs(raw, input)
      let content = try tokens(raw)
      var asIsReference: Editor?
      do {
        var reference: String?
        for (name, order) in dirtyOrders {
          let editor = try build(.on)
          do {
            try loadAsIs(editor, input)
            try editor.update(discrete: true) {
              try order(allNodes(), seed).forEach { try $0.markDirty() }
            }
          } catch {
            fail("throws (dirty \(name))", seed, String(describing: error))
            break
          }
          let result = try stateJSON(editor)
          if reference == nil {
            reference = result
            asIsReference = editor
            if try tokens(editor) != content {
              fail("content lost, duplicated or reordered (as is)", seed, "")
            }
            let remaining = try violations(editor)
            if !remaining.isEmpty {
              fail("not a fixed point (dirty)", seed, remaining.joined(separator: ", "))
            }
          } else if result != reference {
            fail("result depends on dirty order (dirty \(name))", seed, "")
          }
        }
      }
      // The same document created in every order, through the methods
      // loading uses: the same result, keeping the content loading builds
      // (before any transform runs) in its order, except what Lexical's own
      // transforms drop without the extension too (the rows of a table put
      // in a list).
      do {
        let asBuilt = try build(.off)
        let injectedOff = try build(.off)
        var rejected = false
        do {
          try loadInOrder(asBuilt, input, outerFirst, seed, faithful: false, skipTransforms: true)
          try load(injectedOff, input)
        } catch {
          rejected = true
        }
        var reference: String?
        for (name, order) in rejected ? [] : orders {
          let editor = try build(.on)
          do {
            try loadInOrder(editor, input, order, seed)
          } catch {
            fail("throws (empty inline, \(name))", seed, String(describing: error))
            break
          }
          if reference == nil {
            reference = try stateJSON(editor)
            if try !keepsContent(tokens(editor), tokens(asBuilt), tokens(injectedOff)) {
              fail("content lost (empty inline)", seed, "")
            }
          } else if try stateJSON(editor) != reference {
            fail("result depends on dirty order (empty inline, \(name))", seed, "")
          }
        }
      }
      // A caret anywhere in the document keeps its place in the content.
      try checkCarets(input, seed, seed * 7727, "", fail)
      // The document as an editor without enforcement leaves it, which is
      // then turned on: every leaf kept exactly once, in order, in a block of
      // the same kind, and the same result as loading it with enforcement.
      do {
        let editor = try build(.disabled)
        try load(editor, plain)
        let before = try leaves(editor)
        try enableStructure(editor)
        if try stateJSON(editor) != want {
          fail("result differs when enforcement is turned on later", seed, "")
        }
        let beforeKeys = Set(before.map(\.key))
        let after = try leaves(editor).filter {
          $0.desc != "linebreak" || beforeKeys.contains($0.key)
        }
        func describeLeaves(_ ls: [Leaf]) -> String {
          ls.map { "\($0.key):\($0.desc)" }.joined(separator: " ")
        }
        if describeLeaves(after) != describeLeaves(before) {
          fail("leaf lost, duplicated or reordered", seed, "")
        }
        var byKey: [NodeKey: Leaf] = [:]
        for l in after { byKey[l.key] = l }
        for b in before {
          let a = byKey[b.key]
          if let a, a.kind != b.kind, !(b.kind == "list" && a.kind == "listitem") {
            fail(
              "block kind lost", seed,
              "\(b.desc): \(b.kind ?? "null") -> \(a.kind ?? "null")")
            break
          }
          if let a, b.inTable, !a.inTable {
            fail("left its table", seed, b.desc)
            break
          }
        }
      }
      // The document as it is, in an editor that turns enforcement on
      // before any transform ran: the same result.
      do {
        let editor = try build(.disabled)
        try loadAsIs(editor, input)
        try enableStructure(editor)
        if let asIsReference, try stateJSON(editor) != stateJSON(asIsReference) {
          fail("result differs when enforcement is turned on (as is)", seed, "")
        }
      }
      // The internal entry point that normalizes a subtree now: on the root
      // of the document as an editor without enforcement leaves it, and on
      // the document as it is, in an update that runs no transform (list
      // numbers, which the list's own transform recomputes, are left out).
      do {
        let editor = try build(.off)
        try load(editor, plain)
        try editor.update(discrete: true) { try INTERNAL_normalizeNodeStructure(getRoot()) }
        if try stateJSON(editor) != want {
          fail("INTERNAL_$normalizeNodeStructure differs", seed, "")
        }
        let asIs = try build(.off)
        try loadAsIs(asIs, input) { try INTERNAL_normalizeNodeStructure(getRoot()) }
        if let asIsReference, try withoutDerived(asIs) != withoutDerived(asIsReference) {
          fail("INTERNAL_$normalizeNodeStructure differs (as is)", seed, "")
        }
        let remaining = try violations(asIs)
        if !remaining.isEmpty {
          fail(
            "not a fixed point (INTERNAL_$normalizeNodeStructure, as is)", seed,
            remaining.joined(separator: ", "))
        }
      }
      // A local edit that breaks the normalized document, where only the
      // nodes it writes are dirty, gives what loading the edited document
      // gives, with every node dirty (as if it arrived unrepaired).
      try checkLocalEdit(want, seed, "", fail)
      do {
        let withInline = try build(.on, [NormalizeInlineElementsExtension])
        try load(withInline, plain)
        if try tokens(withInline) != tokens(off) {
          fail("content lost with NormalizeInlineElements", seed, "")
        }
      } catch {
        fail("throws with NormalizeInlineElements", seed, String(describing: error))
      }
    }
    let summary = failures.map {
      "\($0.kind): \($0.messages.count), first \($0.messages.prefix(3))"
    }
    #expect(Double(checked) > Double(SEEDS) * 0.75, "\(checked) documents checked")
    #expect(summary == [], "\(summary.joined(separator: "\n"))")
  }
}
