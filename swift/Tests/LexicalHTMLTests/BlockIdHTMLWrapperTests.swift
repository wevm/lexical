import Lexical
import LexicalExtension
import LexicalHTML
import LexicalList
import LexicalRichText
import Testing

// Ports of packages/lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts
// and BlockIdHTMLWrapperResolve.test.ts: data-block-wrapper-id (#9606) on
// import. Upstream counts calls of `ElementNode.prototype.getParent` and
// `getChildrenSize` with `vi.spyOn`, which is the work the resolve rule does
// for each waiting id it looks at; Swift reads the runtime's
// `wrapperIdLookups` counter instead. The bounds are the same: a doubled
// input costs less than three times as much.

private func parse(_ html: String) -> DOMDocument {
  DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
}

private func generate(_ dom: DOMDocument) throws -> [LexicalNode] {
  try getExtensionOutput(DOMImportExtension).generateNodesFromDOM(dom)
}

private func lookups(_ editor: Editor) throws -> Int {
  try #require(
    getExtensionDependencyFromEditor(editor, BlockIdHTMLExtension).initResult
      as? BlockIdHTMLRuntime
  ).wrapperIdLookups
}

/// Imports `dom` in a discrete update, checks the nodes with `check`, and
/// returns how many waiting wrapper ids the import looked at.
private func importCost(
  _ editor: Editor, _ dom: DOMDocument, _ check: @escaping ([LexicalNode]) throws -> Void = { _ in }
) throws -> Int {
  let before = try lookups(editor)
  try editor.update(discrete: true) { try check(generate(dom)) }
  return try lookups(editor) - before
}

private func expectLinear(_ costs: [Int], sourceLocation: SourceLocation = #_sourceLocation) {
  #expect(costs[0] > 0, "\(costs)", sourceLocation: sourceLocation)
  #expect(costs[1] < costs[0] * 3, "\(costs)", sourceLocation: sourceLocation)
  #expect(costs[2] < costs[1] * 3, "\(costs)", sourceLocation: sourceLocation)
}

private func plainEditor(_ name: String, _ rules: [DOMImportRule] = []) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name,
      dependencies: [
        RichTextExtension, ListExtension, BlockIdExtension, BlockIdHTMLExtension,
        configExtension(DOMImportExtension) { $0.rules = rules.map(\.entry) },
      ],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
}

private func fresh(_ id: String?) -> String? {
  id?.hasPrefix("fresh") == true ? "<fresh>" : id
}

// MARK: import

/// Runs an import of its own inside the outer one, for `<aside data-reenter>`.
nonisolated(unsafe) private let reenter = defineImportRule(
  name: "test/reenter", match: sel.tag("aside")
) { _, el, _ in
  if el.hasAttribute("data-reenter") {
    _ = try generate(parse("<p>scratch</p>"))
  }
  return []
}

private func buildImport() throws -> Editor {
  var n = 0
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "block-wrapper-id-import",
      dependencies: [
        RichTextExtension, ListExtension,
        configExtension(BlockIdExtension) {
          $0.generateId = {
            n += 1
            return "fresh\(n)"
          }
        },
        BlockIdHTMLExtension,
        configExtension(DOMImportExtension) { $0.rules = [reenter.entry] },
      ],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
  try editor.update(discrete: true) { try getRoot().append(createParagraphNode()) }
  try editor.update(discrete: true) { try getRoot().clear() }
  return editor
}

private func append(_ editor: Editor, _ html: String) throws {
  let dom = parse(html)
  try editor.update(discrete: true) { try getRoot().append(contentsOf: generate(dom)) }
}

/// Every element below the root as `type|text|id`, fresh ids shown as
/// `<fresh>` when `masked`.
private func blocks(_ editor: Editor, masked: Bool = false, root: Bool = false) throws -> [String] {
  try editor.read {
    var out: [String] = []
    func walk(_ node: LexicalNode) throws {
      guard let element = node as? ElementNode else { return }
      if root || node.getType() != "root" {
        let id = try getBlockId(node)
        out.append(
          "\(node.getType())|\(try node.getTextContent())|\((masked ? fresh(id) : id) ?? "null")")
      }
      try element.getChildren().forEach(walk)
    }
    try walk(getRoot())
    return out
  }
}

@Suite struct BlockIdHTMLWrapperImportTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::wrapper ids that go on no block do not make import quadratic"
    ))
  func idsOnNoBlockAreLinear() throws {
    var costs: [Int] = []
    for n in [64, 128, 256] {
      let editor = try buildImport()
      defer { editor.dispose() }
      let dom = parse(
        (0..<n).map { #"<ul data-block-wrapper-id="foreign\#($0)"><li>x</li></ul>"# }.joined())
      costs.append(try importCost(editor, dom) { #expect($0.count == n) })
    }
    // Counting operations, not timing: a doubled linear input costs ~2x,
    // whereas scanning every outstanding wrapper after every element costs
    // ~4x.
    expectLinear(costs)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::an import run inside another keeps the outer import's waiting wrapper id"
    ))
  func nestedImportKeepsTheOuterId() throws {
    let plain = try buildImport()
    defer { plain.dispose() }
    let nested = try buildImport()
    defer { nested.dispose() }
    func fixture(_ inner: String) -> String {
      #"<ul data-block-id="L"><li data-block-id="I">text"#
        + #"<ul data-block-wrapper-id="W"><li data-block-id="N">inner</li></ul>"# + inner
        + "</li></ul>"
    }
    try append(plain, fixture("<aside></aside>"))
    try append(nested, fixture("<aside data-reenter></aside>"))
    #expect(try blocks(nested, masked: true) == blocks(plain, masked: true))
    #expect(try blocks(plain).contains { $0.hasSuffix("|W") })
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::HTML without wrapper ids reads the same with an import run inside it"
    ))
  func withoutWrapperIdsTheSame() throws {
    let plain = try buildImport()
    defer { plain.dispose() }
    let nested = try buildImport()
    defer { nested.dispose() }
    func fixture(_ inner: String) -> String {
      #"<ul data-block-id="L"><li data-block-id="I">text<ul><li data-block-id="N">inner</li></ul>"#
        + inner + "</li></ul>"
    }
    try append(plain, fixture("<aside></aside>"))
    try append(nested, fixture("<aside data-reenter></aside>"))
    #expect(try blocks(nested) == blocks(plain))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::an unknown element that hoists the same list again does not make import look at it again"
    ))
  func hoistedListNotLookedAtAgain() throws {
    var costs: [Int] = []
    for n in [32, 64, 128] {
      let editor = try plainEditor("block-wrapper-id-hoist")
      defer { editor.dispose() }
      // Each unrecognized wrapper hoists the same ListNode, whose n child
      // items have already been constructed. The valid top-level wrapper id
      // remains unresolved, because no wrapping block is created.
      let dom = parse(
        String(repeating: "<x-transparent>", count: n) + #"<ul data-block-wrapper-id="foreign">"#
          + String(repeating: "<li>x</li>", count: n) + "</ul>"
          + String(repeating: "</x-transparent>", count: n))
      costs.append(
        try importCost(editor, dom) { (nodes: [LexicalNode]) throws -> Void in
          #expect(nodes.count == 1)
          #expect(
            try nodes[0].getTextContent().filter { !$0.isWhitespace }
              == String(repeating: "x", count: n))
        })
    }
    expectLinear(costs)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperImport.test.ts::wrapper ids inside many unknown elements are looked at a bounded number of times each"
    ))
  func idsInsideManyUnknownElements() throws {
    var costs: [Int] = []
    for n in [32, 64, 128] {
      let editor = try plainEditor("block-wrapper-id-deep")
      defer { editor.dispose() }
      let dom = parse(
        String(repeating: "<x-transparent>", count: n)
          + (0..<n).map { #"<ul data-block-wrapper-id="w\#($0)"><li>x</li></ul>"# }.joined()
          + String(repeating: "</x-transparent>", count: n))
      costs.append(try importCost(editor, dom))
    }
    expectLinear(costs)
  }
}

// MARK: resolve

/// Imports its children and adds an item to each list among them.
nonisolated(unsafe) private let growRule = defineImportRule(
  name: "test/grow", match: sel.tag("x-grow")
) { ctx, el, _ in
  let nodes = try ctx.importChildren(el)
  for case let list as ListNode in nodes {
    // Appending at the tail is an insertion-only splice: no scan of old
    // children is needed to construct each new child.
    try list.append(createListItemNode().append(createTextNode("x")))
  }
  return nodes
}

private enum Growth: String, CaseIterable {
  case parent = "parent-growth"
  case holder = "holder-growth"
}

private final class Populate {
  var inner: ListNode?
  var parent: ListNode?
  var holder: ListItemNode?
}

private func buildPopulate(_ mode: Growth) throws -> Editor {
  let built = Populate()
  var i = 0
  let buildRule = defineImportRule(name: "test/build-empty", match: sel.tag("x-build")) {
    ctx, el, _ in
    let inner = try ctx.importChildren(el)[0] as! ListNode
    let parent = try createListNode("bullet")
    let holder = try createListItemNode()
    (built.inner, built.parent, built.holder) = (inner, parent, holder)
    if mode == .holder {
      try parent.append(holder)
    }
    if el.hasAttribute("data-early") {
      try holder.append(inner)
      if mode == .parent {
        try parent.append(holder)
      }
    }
    return [parent]
  }
  let populateRule = defineImportRule(name: "test/populate", match: sel.tag("x-populate")) {
    ctx, el, _ in
    let nodes = try ctx.importChildren(el)
    let holder = built.holder!
    if try holder.getChildrenSize() == 0 {
      try holder.append(built.inner!)
      if mode == .parent {
        try built.parent!.append(holder)
      }
    }
    return nodes
  }
  return try buildEditorFromExtensions(
    defineExtension(
      name: "test-late-populate",
      dependencies: [
        RichTextExtension, ListExtension,
        configExtension(BlockIdExtension) {
          $0.generateId = {
            i += 1
            return "fresh\(i)"
          }
        },
        BlockIdHTMLExtension,
        configExtension(DOMImportExtension) { $0.rules = [buildRule.entry, populateRule.entry] },
      ],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
}

private func populateResult(_ mode: Growth, _ early: Bool) throws -> [String] {
  let editor = try buildPopulate(mode)
  defer { editor.dispose() }
  try append(
    editor,
    "<x-populate><x-build" + (early ? " data-early" : "")
      + #"><ul data-block-wrapper-id="W"><li data-block-id="N">inner</li></ul></x-build></x-populate>"#
  )
  return try blocks(editor, masked: true, root: true)
}

@Suite struct BlockIdHTMLWrapperResolveTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::a list that grows by one item per element around it is resolved in linear work"
    ))
  func growingListIsLinear() throws {
    var costs: [Int] = []
    for n in [32, 64, 128] {
      let editor = try plainEditor("test-changing-count", [growRule])
      defer { editor.dispose() }
      let dom = parse(
        String(repeating: "<x-grow>", count: n)
          + #"<ul data-block-wrapper-id="foreign"><li>x</li></ul>"#
          + String(repeating: "</x-grow>", count: n))
      costs.append(
        try importCost(editor, dom) { (nodes: [LexicalNode]) throws -> Void in
          #expect(nodes.count == 1)
          #expect(try (nodes[0] as! ElementNode).getChildrenSize() == n + 1)
          #expect(
            try nodes[0].getTextContent().filter { !$0.isWhitespace }
              == String(repeating: "x", count: n + 1))
        })
    }
    expectLinear(costs)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::late population resolves ids exactly like early population (parent-growth)"
    ),
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLWrapperResolve.test.ts::late population resolves ids exactly like early population (holder-growth)"
    ),
    arguments: Growth.allCases)
  private func latePopulation(_ mode: Growth) throws {
    #expect(try populateResult(mode, false) == populateResult(mode, true))
  }
}
