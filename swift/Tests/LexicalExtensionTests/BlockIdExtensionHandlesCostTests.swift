import Lexical
import LexicalExtension
import LexicalHeadless
import LexicalRichText
import Testing

// Ports of packages/lexical-extension/src/__tests__/unit/
// BlockIdExtension.handlesCache.test.ts and BlockIdExtension.handlesCost.test.ts.
// Upstream counts the calls of `buildHandleTable` with `vi.mock`; Swift
// reads the runtime's `handleTableBuilds` counter instead.

@Suite struct BlockHandleCacheTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCache.test.ts::block handle cache > builds the handles once per set of ids"
    ))
  func buildsOncePerSetOfIds() throws {
    let ids = ["V1StGXR8_Z5jdHi6B-myT", "9xQeLm2Pq0aK7cTb_wYuA"]
    var n = 0
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-cache]",
        dependencies: [
          RichTextExtension,
          configExtension(BlockIdExtension) {
            $0.generateId = {
              defer { n += 1 }
              return n < ids.count ? ids[n] : "zz\(n)"
            }
          },
        ], editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try discrete(editor, HISTORY_MERGE_TAG) { try getRoot().append(createParagraphNode()) }
    try discrete(editor, HISTORY_MERGE_TAG) { try getRoot().clear() }
    try discrete(editor) {
      for text in ["a", "b"] {
        try getRoot().append(paragraph(text))
      }
    }
    let runtime = try #require(
      try getExtensionDependencyFromEditor(editor, BlockIdExtension).initResult as? BlockIdRuntime)
    runtime.handleTableBuilds = 0
    func readAll() throws {
      try editor.read {
        for node in try getRoot().getChildren() {
          _ = try getBlockHandle(node)
        }
        _ = try getBlockHandles()
        _ = try resolveBlockHandle("V1St")
        _ = try resolveBlockHandle("9xQe")
        _ = try resolveBlockHandle("zzzz")
      }
    }

    try readAll()
    try readAll()
    #expect(runtime.handleTableBuilds == 1)

    // Typing changes no id: no rebuild.
    try discrete(editor) {
      try #require(try getRoot().getFirstDescendant() as? TextNode).setTextContent("typed")
    }
    try readAll()
    #expect(runtime.handleTableBuilds == 1)

    // A new id: one rebuild.
    try discrete(editor) { try getRoot().append(createParagraphNode()) }
    try readAll()
    try readAll()
    #expect(runtime.handleTableBuilds == 2)

    // A changed id: one rebuild.
    try discrete(editor) {
      try setState(#require(try getBlockById(ids[1])), blockIdState, "renamed")
    }
    try readAll()
    #expect(runtime.handleTableBuilds == 3)
  }
}

/// The calls of `isIdentifiedBlock` made reading the handle of every one
/// of `count` blocks, and how many of them have one.
private func countCalls(
  _ count: Int, disabled: Bool, duplicate: Bool
) throws -> (
  calls: Int, handles: Int
) {
  var calls = 0
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-handle-cost]",
      dependencies: [
        HeadlessExtension,
        configExtension(BlockIdExtension) {
          $0.isIdentifiedBlock = { node in
            calls += 1
            return try isRootChildBlock(node)
          }
          $0.disabled = disabled
        },
      ], editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  defer { editor.dispose() }
  let children: [JSONValue] = (0..<count).map { i in
    let padded = String(repeating: "0", count: 12 - String(i).count) + String(i)
    return [
      "$": ["blockId": .string(duplicate ? "shared-block-id" : "block-\(padded)")],
      "children": [["text": .string("p\(i)"), "type": "text"]], "type": "paragraph",
    ]
  }
  try editor.setEditorState(
    editor.parseEditorState(JSONObject([("root", ["children": .array(children), "type": "root"])])))
  // Build what reading needs once (the index, the scan, the table), so that
  // only the reads are counted.
  _ = try editor.read { try getBlockHandles() }
  calls = 0
  let handles = try editor.read {
    try getRoot().getChildren().filter { try getBlockHandle($0) != nil }.count
  }
  return (calls, handles)
}

@Suite struct BlockHandleCostTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > disabled, unique ids"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > disabled, one id repeated"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > enabled, unique ids"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > enabled, one id repeated"
    ), arguments: [(true, false), (true, true), (false, false), (false, true)])
  func linear(_ disabled: Bool, _ duplicate: Bool) throws {
    // While enabled, loading repairs the repeated id (every block gets an
    // id of its own); while disabled, every block keeps it.
    let rows = try [40, 80, 160].map {
      try countCalls($0, disabled: disabled, duplicate: duplicate)
    }
    for (i, row) in rows.enumerated() {
      // Only the first holder of a repeated id has a handle.
      #expect(row.handles == (duplicate && disabled ? 1 : 40 << i))
    }
    // Doubling the blocks at most doubles the calls (plus slack).
    #expect(Double(rows[2].calls) <= 2.2 * Double(rows[1].calls) + 4)
    #expect(Double(rows[1].calls) <= 2.2 * Double(rows[0].calls) + 4)
  }
}
