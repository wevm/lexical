import Lexical
import LexicalExtension
import LexicalRichText
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts:
// block ids in an editor that also enforces declared node structure, whose
// normalization splits, joins, lifts and merges blocks in the same update as
// the change that needs it. Upstream's editors register ListExtension and
// LinkExtension too; the cases that build lists or links are skipped until
// those packages are ported, and the rest need neither.

/// The order the extensions are listed in, or the structure enabled after
/// the editor is built, or no structure at all.
enum BlockIdStructureOrder: String, CaseIterable, CustomTestStringConvertible {
  case blockIdsFirst = "block ids first"
  case structureFirst = "structure first"
  case structureEnabledLater = "structure enabled later"
  case noStructure = "no structure"
  var testDescription: String { rawValue }
}

private func build(
  _ order: BlockIdStructureOrder = .blockIdsFirst, hadContent: Bool = true
) throws -> Editor {
  var n = 0
  let blockIds = configExtension(BlockIdExtension) {
    $0.generateId = {
      n += 1
      return "g\(n)"
    }
  }
  let structure: [any AnyLexicalExtensionArgument] =
    switch order {
    case .noStructure: [blockIds]
    case .blockIdsFirst: [blockIds, NodeStructureExtension]
    case .structureFirst: [NodeStructureExtension, blockIds]
    case .structureEnabledLater:
      [blockIds, configExtension(NodeStructureExtension) { $0.disabled = true }]
    }
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-structure-test]", dependencies: [RichTextExtension] + structure,
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  if order == .structureEnabledLater {
    try getExtensionDependencyFromEditor(editor, NodeStructureExtension).output.disabled.set(false)
  }
  if hadContent {
    // Have had content once, so that new blocks are edits, not a load.
    try asEdited(editor)
  }
  return editor
}

/// `type#id[children]`, with text in quotes.
private func show(_ node: LexicalNode) throws -> String {
  if node is TextNode {
    return JSONValue.string(try node.getTextContent()).stringify()
  }
  guard let element = node as? ElementNode, !(node is LineBreakNode) else {
    return node.getType()
  }
  let id = try getBlockId(node)
  let children = try element.getChildren().map(show).joined(separator: " ")
  return "\(node.getType())\(id.map { "#\($0)" } ?? "")[\(children)]"
}

private func shape(_ editor: Editor) throws -> [String] {
  try editor.read { try getRoot().getChildren().map(show) }
}

/// Every id is unique and the index finds the block that holds it.
private func expectIndexed(
  _ editor: Editor, sourceLocation: SourceLocation = #_sourceLocation
) throws {
  try editor.read {
    var seen = Set<String>()
    func walk(_ node: LexicalNode) throws {
      if let id = try getBlockId(node) {
        #expect(seen.insert(id).inserted, "\(id)", sourceLocation: sourceLocation)
        #expect(
          try getBlockById(id)?.getKey() == node.getKey(), "\(id)", sourceLocation: sourceLocation)
      }
      if let element = node as? ElementNode {
        try element.getChildren().forEach(walk)
      }
    }
    try walk(getRoot())
  }
}

private func withId<T: LexicalNode>(_ node: T, _ id: String) throws -> T {
  try setState(node, blockIdState, id)
  return node
}

private func t(_ text: String) throws -> TextNode { try createTextNode(text) }
private func p(_ children: LexicalNode...) throws -> ElementNode {
  try createParagraphNode().append(contentsOf: children)
}
private func block(_ id: String) throws -> ElementNode {
  try #require(try getBlockById(id) as? ElementNode)
}

struct BlockIdStructureCase: CustomTestStringConvertible, Sendable {
  let name: String
  let setup: @Sendable () throws -> Void
  let change: @Sendable () throws -> Void
  let expected: [String]
  var testDescription: String { name }
}

let STRUCTURE_CASES: [BlockIdStructureCase] = [
  BlockIdStructureCase(
    name: "a paragraph split around a block lifted out of it: the first piece keeps the id",
    setup: { try getRoot().append(withId(p(t("a")), "A")) },
    change: { try block("A").append(withId(createQuoteNode().append(t("q")), "Q"), t("b")) },
    expected: [#"paragraph#A["a"]"#, #"quote#Q["q"]"#, #"paragraph#g1["b"]"#]),
  BlockIdStructureCase(
    name: "a block lifted out of a paragraph it emptied takes its place and its id",
    setup: { try getRoot().append(withId(p(t("x")), "A")) },
    change: {
      let paragraph = try block("A")
      try paragraph.clear()
      try paragraph.append(createQuoteNode().append(t("q")))
    },
    expected: [#"quote#A["q"]"#]),
  BlockIdStructureCase(
    name: "paragraphs joined as lines: the block that absorbs them keeps its id",
    setup: { try getRoot().append(withId(createHeadingNode("h1").append(t("h")), "H")) },
    change: { try block("H").append(withId(p(t("a")), "PA"), withId(p(t("b")), "PB")) },
    expected: [#"heading#H["h" linebreak "a" linebreak "b"]"#]),
]

@Suite struct BlockIdStructureTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a paragraph split around a block lifted…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > a block lifted out of a paragraph it em…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > block ids first > paragraphs joined as lines: the block t…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a paragraph split around a block lifted…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > a block lifted out of a paragraph it em…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure first > paragraphs joined as lines: the block t…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a paragraph split around a block lifted…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > a block lifted out of a paragraph it em…"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension > structure enabled later > paragraphs joined as lines: the block t…"
    ),
    arguments: [BlockIdStructureOrder.blockIdsFirst, .structureFirst, .structureEnabledLater],
    STRUCTURE_CASES)
  func normalization(_ order: BlockIdStructureOrder, _ testCase: BlockIdStructureCase) throws {
    let editor = try build(order)
    defer { editor.dispose() }
    try discrete(editor) { try testCase.setup() }
    try discrete(editor) { try testCase.change() }
    #expect(try shape(editor) == testCase.expected)
    try expectIndexed(editor)
  }
}

/// A document as an editor without either extension stores it, unrepaired.
private func storedJSON(_ build: @escaping () throws -> Void) throws -> String {
  let writer = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-structure-test-writer]", dependencies: [RichTextExtension],
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  defer { writer.dispose() }
  try discrete(writer, skipTransforms: true) { try build() }
  return try writer.getEditorState().toJSON().stringify()
}

/// The blocks a load of `json` gives, all with positional ids.
private func loadedIds(_ order: BlockIdStructureOrder, _ json: String) throws -> [String] {
  let editor = try build(order, hadContent: false)
  defer { editor.dispose() }
  try editor.setEditorState(editor.parseEditorState(json))
  try expectIndexed(editor)
  let blocks = try shape(editor)
  // No generated (`g<n>`) or missing ids.
  #expect(
    try blocks.joined(separator: " ").firstMatch(
      of: Regex(#"#g\d|(paragraph|quote|list|listitem)\["#)) == nil)
  return blocks
}

/// The ids an editor without NodeStructureExtension derives for the
/// document that `order` loads `json` into, stored again without ids.
private func idsOfLoadedDocument(_ order: BlockIdStructureOrder, _ json: String) throws -> [String]
{
  let editor = try build(order, hadContent: false)
  defer { editor.dispose() }
  try editor.setEditorState(editor.parseEditorState(json))
  func strip(_ node: JSONValue) -> JSONValue {
    guard var object = node.objectValue else { return node }
    if var state = object["$"]?.objectValue {
      state.remove("blockId")
      object["$"] = .object(state)
    }
    if let children = object["children"]?.arrayValue {
      object["children"] = .array(children.map(strip))
    }
    return .object(object)
  }
  var loaded = try editor.getEditorState().toJSON()
  loaded["root"] = strip(try #require(loaded["root"]))
  return try loadedIds(.noStructure, loaded.stringify())
}

private func withoutIds(_ blocks: [String]) throws -> [String] {
  try blocks.map { try $0.replacing(Regex(#"#[\w-]+"#), with: "") }
}

@Suite struct BlockIdStructureLoadTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > block ids first: a split document gets the ids of the document the load ends with"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.structure.test.ts::BlockIdExtension with NodeStructureExtension: loads > structure first: a split document gets the ids of the document the load ends with"
    ),
    arguments: [BlockIdStructureOrder.blockIdsFirst, .structureFirst])
  func splitDocument(_ order: BlockIdStructureOrder) throws {
    let quote = try storedJSON {
      try getRoot().append(
        p(t("a"), createQuoteNode().append(t("q")), t("b")), p(t("c")))
    }
    let ids = try loadedIds(order, quote)
    #expect(try ids == idsOfLoadedDocument(order, quote))
    #expect(
      try withoutIds(ids) == [
        #"paragraph["a"]"#, #"quote["q"]"#, #"paragraph["b"]"#, #"paragraph["c"]"#,
      ])
  }
}
