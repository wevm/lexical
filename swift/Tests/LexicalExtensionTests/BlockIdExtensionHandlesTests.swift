import Lexical
import LexicalExtension
import LexicalHeadless
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts.
// The editors register ListExtension, as upstream's `build` does.

/// A generateId that hands out `ids` in order, then a counter.
private func fixedIds(_ ids: [String]) -> () -> String {
  var i = 0
  var n = 0
  return {
    if i < ids.count {
      i += 1
      return ids[i - 1]
    }
    n += 1
    return "zz\(n)"
  }
}

private func build(
  _ ids: [String], disabled: Bool = false, headless: Bool = false
) throws
  -> Editor
{
  try asEdited(
    buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-test]",
        dependencies: (headless ? [HeadlessExtension] : []) + [
          RichTextExtension, ListExtension,
          configExtension(BlockIdExtension) {
            $0.disabled = disabled
            $0.generateId = fixedIds(ids)
          },
        ], editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none))))
}

/// An editor holding one paragraph per id, loaded from compact JSON.
private func load(_ ids: [String], disabled: Bool = false) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-handle-load]",
      dependencies: [
        RichTextExtension, configExtension(BlockIdExtension) { $0.disabled = disabled },
      ],
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  let children: [JSONValue] = ids.enumerated().map { i, id in
    [
      "$": ["blockId": .string(id)], "children": [["text": .string("Block \(i)"), "type": "text"]],
      "type": "paragraph",
    ]
  }
  try editor.setEditorState(
    editor.parseEditorState(JSONObject([("root", ["children": .array(children), "type": "root"])])))
  return editor
}

/// Appends one paragraph per text and returns their ids.
@discardableResult
private func appendParagraphs(_ editor: Editor, _ texts: [String]) throws -> [String] {
  try discrete(editor) {
    for text in texts {
      try getRoot().append(paragraph(text))
    }
  }
  return try editor.read {
    try getRoot().getChildren().suffix(texts.count).map { try #require(try getBlockId($0)) }
  }
}

/// Id → handle of every top-level block, through getBlockHandle.
private func handlesOf(_ editor: Editor) throws -> [String: String?] {
  try editor.read {
    var out: [String: String?] = [:]
    for node in try getRoot().getChildren() {
      out[try getBlockId(node) ?? "null"] = .some(try getBlockHandle(node))
    }
    return out
  }
}

private func dictionary(_ map: JSMap<String, String>) -> [String: String] {
  Dictionary(uniqueKeysWithValues: map.map { ($0.0, $0.1) })
}

/// A `BlockHandleResult` as a string: `found:<id>`, `not-found:<handle>`,
/// `ambiguous:<handle>:<candidates>` or `invalid:<handle>:<reason>`.
func describe(_ result: BlockHandleResult) throws -> String {
  switch result {
  case .found(let node): "found:\(try getBlockId(node) ?? "null")"
  case .notFound(let handle): "not-found:\(handle)"
  case .ambiguous(let handle, let candidates):
    "ambiguous:\(handle):\(candidates.joined(separator: ","))"
  case .invalid(let handle, let reason): "invalid:\(handle):\(reason.rawValue)"
  }
}

private func resolve(_ editor: Editor, _ handle: String) throws -> String {
  try editor.read { try describe(resolveBlockHandle(handle)) }
}

/// The shortest prefix of `id`'s key, at least 4 long, no other key starts.
func bruteForceHandle(_ id: String, _ ids: [String]) -> String {
  let key = handleKey(id)
  let others = ids.filter { $0 != id }.map(handleKey)
  var length = MIN_BLOCK_HANDLE_LENGTH
  while length < key.count && others.contains(where: { $0.hasPrefix(String(key.prefix(length))) }) {
    length += 1
  }
  return String(key.prefix(length))
}

let HANDLE_ALPHABET = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")

/// Every string one substitution, deletion or insertion away from `s`, in
/// the order JS's set first sees them.
func oneCharTypos(_ s: String) -> [String] {
  let chars = Array(s)
  var seen = Set<String>()
  var result: [String] = []
  func add(_ value: [Character]) {
    let string = String(value)
    if string != s && seen.insert(string).inserted {
      result.append(string)
    }
  }
  for i in chars.indices {
    add(Array(chars[..<i] + chars[(i + 1)...]))
    for c in HANDLE_ALPHABET {
      add(Array(chars[..<i]) + [c] + Array(chars[(i + 1)...]))
    }
  }
  for i in 0...chars.count {
    for c in HANDLE_ALPHABET {
      add(Array(chars[..<i]) + [c] + Array(chars[i...]))
    }
  }
  return result
}

/// The handle lengths of every block, and the one-character typos of their
/// handles that name another block. A typo may name another block only by
/// being that block's handle or a longer prefix of its key.
private func measure(_ editor: Editor) throws -> (lengths: [Int], tried: Int, hits: Int) {
  try editor.read {
    let handles = try getBlockHandles()
    var tried = 0
    var hits = 0
    for (id, handle) in handles {
      for typo in oneCharTypos(handle) {
        tried += 1
        if case .found(let node) = try resolveBlockHandle(typo) {
          let other = try #require(try getBlockId(node))
          if other != id {
            hits += 1
            #expect(
              handleKey(other).hasPrefix(typo) && typo.hasPrefix(handles.get(other)!),
              "typo \(typo) of \(handle) named \(other)")
          }
        }
      }
    }
    return (handles.map { $0.1.count }, tried, hits)
  }
}

/// `n.toString(16)` / `n.toString(36)`.
private func radix(_ n: Int, _ base: Int) -> String { String(n, radix: base) }

/// UUIDv7s in their standard layout, made 0 to 2 ms apart.
func uuidv7s(_ count: Int) -> [String] {
  let random = mulberry32(7)
  func hexDigits(_ digits: Int) -> String {
    (0..<digits).map { _ in radix(Int((random() * 16).rounded(.down)), 16) }.joined()
  }
  var ms = 1_793_357_300_000
  return (0..<count).map { _ in
    ms += Int((random() * 3).rounded(.down))
    let time = String(repeating: "0", count: max(0, 12 - radix(ms, 16).count)) + radix(ms, 16)
    let variant = radix(8 + Int((random() * 4).rounded(.down)), 16)
    let t = Array(time)
    return
      "\(String(t[0..<8]))-\(String(t[8...]))-7\(hexDigits(3))-\(variant)\(hexDigits(3))-\(hexDigits(12))"
  }
}

/// Ids shaped like `@lexical/y`'s derived ids: `y`, a client, `_`, a clock.
func derivedIds(_ count: Int) -> [String] {
  let random = mulberry32(36)
  let client = radix(Int((random() * 9_007_199_254_740_992).rounded(.down)), 36)
  var clock = 0
  return (0..<count).map { _ in
    clock += 10 + Int((random() * 70).rounded(.down))
    return "y\(client)_\(radix(clock, 36))"
  }
}

private let V1ST = "V1StGXR8_Z5jdHi6B-myT"
private let XQE = "9xQeLm2Pq0aK7cTb_wYuA"
private let RK8P = "Rk8pZc41Uo-HsVd2NnFjB"
// Ids picked for their hashes: each of these starts its hash with the same
// 4 characters as the one it is named after.
private let LIKE_V1ST = "qfapyr"
private let LIKE_XQE = "q71spi"
// Pairs whose hashes start with the same 4 characters.
private let PAIR_A = ["p13e", "p20d"]
private let PAIR_B = ["p1i4", "p2pq"]
// Eleven ids whose hashes all start with sclT.
private let ELEVEN = [
  "same0", "same3abof", "same4onh9", "samesnq2r", "samev3hf4", "same1acey1", "same1lwsd9",
  "same1o9hsa", "same1sf64m", "same1yzxvi", "same1zd3zy",
]

@Suite struct BlockHandleComputationTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > handleKey: 64 bits of a hash of the id in base64url"
    ))
  func handleKeyHashes() {
    #expect(handleKey(V1ST) == "2bNoqlijdOg")
    #expect(handleKey(XQE) == "gTtFDmoxN_s")
    #expect(handleKey(RK8P) == "xt6zBAcASRs")
    #expect(HANDLE_KEY_LENGTH == 11)
    let alphabet = Set(HANDLE_ALPHABET)
    for id in ["", "a", "b1", V1ST, String(repeating: "y", count: 64)] {
      let key = handleKey(id)
      #expect(key.count == 11 && key.allSatisfy { alphabet.contains($0) })
    }
    // The picked ids do share the start of their hashes.
    #expect(handleKey(LIKE_V1ST).prefix(4) == "2bNo")
    #expect(handleKey(LIKE_XQE).prefix(4) == "gTtF")
    for pair in [PAIR_A, PAIR_B] {
      let (a, b) = (Array(handleKey(pair[0])), Array(handleKey(pair[1])))
      #expect(a[..<4] == b[..<4])
      #expect(a[4] != b[4])
    }
    for id in ELEVEN {
      #expect(handleKey(id).prefix(4) == "sclT")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > handles are the shortest unique prefix of the hash, at least 4 characters"
    ))
  func shortestUniquePrefix() {
    #expect(
      dictionary(buildHandleTable([V1ST, XQE, RK8P]).handles) == [
        RK8P: "xt6z", V1ST: "2bNo", XQE: "gTtF",
      ])
    #expect(
      dictionary(buildHandleTable([V1ST, XQE, LIKE_V1ST, LIKE_XQE]).handles) == [
        LIKE_V1ST: "2bNoV", LIKE_XQE: "gTtFc", V1ST: "2bNoq", XQE: "gTtFD",
      ])
    #expect(MIN_BLOCK_HANDLE_LENGTH == 4)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > matches the brute-force definition, whatever the ids look like"
    ))
  func matchesBruteForce() {
    let ids =
      ["b1", "b12", "b123", "c", "cc"] + PAIR_A + PAIR_B + ELEVEN + [V1ST, LIKE_V1ST]
      + derivedIds(40) + uuidv7s(40)
    let table = buildHandleTable(ids)
    for id in ids {
      #expect(table.handles.get(id) == bruteForceHandle(id, ids), "\(id)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > ids of any shape get handles of 4 or 5 characters"
    ))
  func anyShapeGetsShortHandles() {
    let shapes: [(String, [String])] = [
      (
        "counter",
        (0..<1000).map { i in
          "block-\(String(repeating: "0", count: 6 - String(i).count))\(i)"
        }
      ),
      ("derived", derivedIds(1000)),
      ("short", (0..<1000).map { "b\(radix($0 + 1, 36))" }),
      ("uuidv7", uuidv7s(1000)),
    ]
    for (shape, ids) in shapes {
      let lengths = buildHandleTable(ids).handles.map { $0.1.count }
      #expect(lengths.min() == 4, "\(shape)")
      #expect(lengths.max()! <= 5, "\(shape)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > duplicates are counted once"
    ))
  func duplicatesCountedOnce() {
    let table = buildHandleTable([V1ST, V1ST, XQE])
    #expect(table.sorted == [handleKey(V1ST), handleKey(XQE)])
    #expect(table.handles.get(V1ST) == "2bNo")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > the handle of an id outside the table, against the table"
    ))
  func handleOutsideTable() {
    let table = buildHandleTable([V1ST, XQE])
    #expect(handleInTable(table, LIKE_V1ST) == "2bNoV")
    #expect(handleInTable(table, RK8P) == "xt6z")
    #expect(handleInTable(table, XQE) == "gTtF")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::handle computation (pure) > idsWithPrefix lists the ids whose hash starts with a prefix"
    ))
  func idsWithPrefixLists() {
    let table = buildHandleTable([V1ST, LIKE_V1ST, XQE])
    #expect(idsWithPrefix(table, "2bNo") == [LIKE_V1ST, V1ST])
    #expect(idsWithPrefix(table, "2bNoq") == [V1ST])
    #expect(idsWithPrefix(table, "gTtF") == [XQE])
    #expect(idsWithPrefix(table, "zzzz") == [])
  }
}

@Suite struct GetBlockHandleTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a node without an id or that is not an identified block"
    ))
  func nullWithoutAnIdOrNotIdentified() throws {
    let ids = ["listid", "item1", "item2"]
    let handles = dictionary(buildHandleTable(ids).handles)
    let editor = try build(ids)
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("one")),
          createListItemNode().append(
            createListNode("bullet").append(createListItemNode().append(createTextNode("nested"))))
        ))
    }
    try editor.read { () throws -> Void in
      let list = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
      let item = try #require(try list.getFirstChildOrThrow() as? ElementNode)
      #expect(try getBlockHandle(list) == handles["listid"])
      #expect(try getBlockHandle(item) == handles["item1"])
      // A text node, and a nested list (a wrapper, not identified).
      #expect(try getBlockHandle(item.getFirstChildOrThrow()) == nil)
      let secondItem = try #require(try list.getLastChildOrThrow() as? ElementNode)
      #expect(try getBlockHandle(secondItem) == handles["item2"])
      let nestedList = try #require(try secondItem.getFirstChildOrThrow() as? ElementNode)
      #expect(try getBlockHandle(nestedList) == nil)
    }
    // Detached.
    try discrete(editor) {
      let list = try getRoot().getFirstChildOrThrow()
      try list.remove()
      #expect(try getBlockHandle(list) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > gives each block its handle"
    ))
  func givesEachBlockItsHandle() throws {
    let editor = try build([V1ST, XQE, RK8P])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["Q3 plan", "Revenue grew 4% in Q3.", "Hiring"])
    #expect(try handlesOf(editor) == [RK8P: "xt6z", V1ST: "2bNo", XQE: "gTtF"])
    #expect(
      dictionary(try editor.read { try getBlockHandles() }) == [
        RK8P: "xt6z", V1ST: "2bNo", XQE: "gTtF",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > short ids, and ids that prefix other ids, get hashed handles too"
    ))
  func shortIdsGetHashedHandles() throws {
    let ids = ["b1", "b12", "b123456", "xy"]
    let editor = try build(ids)
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c", "d"])
    let expected = dictionary(buildHandleTable(ids).handles)
    #expect(try handlesOf(editor) == expected.mapValues { .some($0) })
    for id in ids {
      #expect(try resolve(editor, expected[id]!) == "found:\(id)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a block named since the last commit"
    ))
  func nilForABlockNamedSinceTheLastCommit() throws {
    let editor = try build([V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    try discrete(editor) {
      // Its handle would share 2bNo with V1ST's.
      let block = try createParagraphNode()
      try setState(block, blockIdState, LIKE_V1ST)
      try getRoot().append(block)
      #expect(try getBlockHandle(block) == nil)
      #expect(try getBlockById(LIKE_V1ST) == nil)
    }
    // Once committed, both have longer handles.
    #expect(try handlesOf(editor) == [LIKE_V1ST: "2bNoV", V1ST: "2bNoq", XQE: "gTtF"])
    #expect(try resolve(editor, "2bNoV") == "found:\(LIKE_V1ST)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a block of an earlier state whose id is gone"
    ))
  func nilForAnEarlierStateBlock() throws {
    let editor = try build(PAIR_B + [XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    let earlier = editor.getEditorState()
    try discrete(editor) { try #require(try getBlockById(PAIR_B[0])).remove() }
    let handle = try earlier.read(editor: editor) {
      try getBlockHandle(getRoot().getFirstChildOrThrow())
    }
    #expect(handle == nil)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > null for a copy whose repeated id awaits its repair"
    ))
  func nilForACopyAwaitingRepair() throws {
    let editor = try build([V1ST, XQE, "cccc12345"])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    // Read in the commit that brings the duplicate, before the repair
    // update that the extension's own listener (registered first) schedules.
    var seen: [String] = []
    let unregister = editor.registerUpdateListener { payload in
      if payload.tags.has(COLLABORATION_TAG) {
        try payload.editorState.read(editor: editor) {
          let children = try getRoot().getChildren()
          let (first, last) = (children[0], children[2])
          let result = try resolveBlockHandle("2bNo")
          seen = [
            try getBlockId(last) ?? "null", try getBlockHandle(first) ?? "null",
            try getBlockHandle(last) ?? "null", String(result.node?.is(first) == true),
          ]
        }
      }
    }
    // A duplicate committed without transforms, as a remote merge is.
    try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
      try setState(getRoot().getLastChildOrThrow(), blockIdState, V1ST)
    }
    unregister()
    #expect(seen == [V1ST, "2bNo", "null", "true"])
    // After the repair, the copy has an id and a handle of its own.
    let handles = try handlesOf(editor)
    #expect(handles.count == 3)
    #expect(handles[V1ST] == "2bNo")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$getBlockHandle > handles change only when ids do, and at once"
    ))
  func handlesChangeOnlyWithIds() throws {
    let editor = try build([V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let first = try handlesOf(editor)
    // Typing changes no id: same handles.
    try discrete(editor) {
      try #require(try getRoot().getFirstDescendant() as? TextNode).setTextContent("changed")
    }
    #expect(try handlesOf(editor) == first)
    // A new id whose hash shares 2bNo makes the handle longer at once.
    try appendParagraphs(editor, ["c"])
    try discrete(editor) { try setState(getRoot().getLastChildOrThrow(), blockIdState, LIKE_V1ST) }
    #expect(try handlesOf(editor) == [LIKE_V1ST: "2bNoV", V1ST: "2bNoq", XQE: "gTtF"])
  }
}

@Suite struct ResolveBlockHandleTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > finds the block of a handle, or of a whole id"
    ))
  func findsHandleOrId() throws {
    let editor = try build([V1ST, XQE, RK8P])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    #expect(try resolve(editor, "gTtF") == "found:\(XQE)")
    // A longer prefix of the hash (a handle read while another block's hash
    // shared more of it) still finds the block.
    #expect(try resolve(editor, "gTtFDmo") == "found:\(XQE)")
    #expect(try resolve(editor, handleKey(XQE)) == "found:\(XQE)")
    // A string longer than any handle is a whole id.
    #expect(try resolve(editor, XQE) == "found:\(XQE)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > ids of 11 characters or fewer are found by their handle only"
    ))
  func shortIdsByHandleOnly() throws {
    let ids = ["b1", "b123456", "abcdefghijk", "abcdefghijkl"]
    let editor = try build(ids)
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c", "d"])
    let handles = buildHandleTable(ids).handles
    for id in ids {
      #expect(try resolve(editor, handles.get(id)!) == "found:\(id)")
    }
    #expect(try resolve(editor, "b1") == "invalid:b1:too-short")
    #expect(try resolve(editor, "b123456").hasPrefix("not-found:"))
    #expect(try resolve(editor, "abcdefghijk").hasPrefix("not-found:"))
    // 12 characters: a whole id.
    #expect(try resolve(editor, "abcdefghijkl") == "found:abcdefghijkl")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a typo is not found; fewer than 4 characters is too short"
    ))
  func typoNotFound() throws {
    let editor = try build([V1ST, XQE, RK8P])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    #expect(try resolve(editor, "gTtG") == "not-found:gTtG")
    // Case matters.
    #expect(try resolve(editor, "GTTF") == "not-found:GTTF")
    // Never a handle: a dropped character, not a deleted block.
    for short in ["gTt", "gT", "g"] {
      #expect(try resolve(editor, short) == "invalid:\(short):too-short")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > invalid input"
    ))
  func invalidInput() throws {
    // JS also passes values that are not strings (null, a number, objects
    // that cannot be turned into one); a Swift handle is a String.
    let editor = try build([V1ST])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a"])
    for input in [
      "", "2bNo ", " 2bNo", "2bNo!", "2bNo/Q", String(repeating: "a", count: 65), "2bNo\n", "ß123",
    ] {
      #expect(
        try resolve(editor, input) == "invalid:\(input):malformed", "\(input.debugDescription)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a deleted block is not found"
    ))
  func deletedNotFound() throws {
    let editor = try build([V1ST, XQE, RK8P])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    try discrete(editor) { try #require(try getBlockById(RK8P)).remove() }
    #expect(try resolve(editor, "xt6z") == "not-found:xt6z")
    #expect(try resolve(editor, RK8P) == "not-found:\(RK8P)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a block deleted earlier in the same update is not found"
    ))
  func deletedInTheSameUpdate() throws {
    let editor = try build([V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    try discrete(editor) {
      try #require(try getBlockById(XQE)).remove()
      #expect(try describe(resolveBlockHandle("gTtF")) == "not-found:gTtF")
      #expect(try describe(resolveBlockHandle("2bNo")) == "found:\(V1ST)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > a new block whose hash starts the same way makes an old handle ambiguous"
    ))
  func newBlockMakesHandleAmbiguous() throws {
    let editor = try build([V1ST, XQE, LIKE_V1ST])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["Q3 plan", "Revenue"])
    #expect(try handlesOf(editor)[V1ST] == "2bNo")
    // A collaborator adds a block whose handle would start with 2bNo too.
    try appendParagraphs(editor, ["Q4 plan"])
    #expect(try resolve(editor, "2bNo") == "ambiguous:2bNo:2bNoV,2bNoq")
    // The next handles computed are longer, and each resolves.
    #expect(try handlesOf(editor) == [LIKE_V1ST: "2bNoV", V1ST: "2bNoq", XQE: "gTtF"])
    #expect(try resolve(editor, "2bNoq") == "found:\(V1ST)")
    #expect(try resolve(editor, "2bNoV") == "found:\(LIKE_V1ST)")
    // Once the new block is deleted, the old handle works again.
    try discrete(editor) { try getRoot().getLastChildOrThrow().remove() }
    #expect(try resolve(editor, "2bNo") == "found:\(V1ST)")
    #expect(try resolve(editor, "2bNoq") == "found:\(V1ST)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle > lists at most 10 candidates"
    ))
  func atMostTenCandidates() throws {
    let editor = try build(ELEVEN)
    defer { editor.dispose() }
    try appendParagraphs(editor, ELEVEN)
    let table = buildHandleTable(ELEVEN)
    let result = try editor.read { try resolveBlockHandle("sclT") }
    guard case .ambiguous(_, let candidates) = result else {
      Issue.record("not ambiguous: \(result.status)")
      return
    }
    #expect(
      candidates
        == table.sorted.prefix(10).map { table.handles.get(table.idsOfKey.get($0)![0])! })
  }
}

@Suite struct ResolveBlockHandleWithReadTests {
  private func readOf(_ editor: Editor) throws -> JSMap<String, String> {
    try editor.read { try getBlockHandles() }
  }

  private func resolveRead(
    _ editor: Editor, _ handle: String, _ read: JSMap<String, String>
  ) throws -> String {
    try editor.read { try describe(resolveBlockHandle(handle, BlockHandleOptions(read: read))) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a block deleted since the read is not found, even if a block added since takes its handle"
    ))
  func deletedSinceTheRead() throws {
    let editor = try build([PAIR_A[0], XQE, PAIR_A[1]])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let read = try readOf(editor)
    #expect(read.get(PAIR_A[0]) == "0ydd")
    try discrete(editor) { try #require(try getBlockById(PAIR_A[0])).remove() }
    try appendParagraphs(editor, ["c"])
    // Without the read, the handle now names the block added since.
    #expect(try resolve(editor, "0ydd") == "found:\(PAIR_A[1])")
    // With it, the block the model read is gone.
    #expect(try resolveRead(editor, "0ydd", read) == "not-found:0ydd")
    #expect(try resolveRead(editor, "gTtF", read) == "found:\(XQE)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a block added since cannot make the handle ambiguous"
    ))
  func addedSinceCannotMakeAmbiguous() throws {
    let editor = try build([V1ST, XQE, LIKE_V1ST])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let read = try readOf(editor)
    try appendParagraphs(editor, ["c"])
    #expect(try resolve(editor, "2bNo").hasPrefix("ambiguous:"))
    #expect(try resolveRead(editor, "2bNo", read) == "found:\(V1ST)")
    // The whole id of a block the model did not read is not one of its
    // blocks either.
    try discrete(editor) {
      let node = try createParagraphNode()
      try setState(node, blockIdState, "a-whole-id-not-read")
      try getRoot().append(node)
    }
    #expect(
      try resolveRead(editor, "a-whole-id-not-read", read) == "not-found:a-whole-id-not-read")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > typos fail against the ids of the read"
    ))
  func typosFailAgainstTheRead() throws {
    let editor = try build([V1ST, LIKE_V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b", "c"])
    let read = try readOf(editor)
    #expect(try resolveRead(editor, "gTtG", read) == "not-found:gTtG")
    // Shorter than the handles shown: the read's handles as candidates.
    #expect(try resolveRead(editor, "2bNo", read) == "ambiguous:2bNo:2bNoV,2bNoq")
    #expect(try resolveRead(editor, "gTt", read) == "invalid:gTt:too-short")
    #expect(try resolveRead(editor, "gTtF!", read) == "invalid:gTtF!:malformed")
    // A longer prefix of the hash, and the whole id, work.
    #expect(try resolveRead(editor, "gTtFDm", read) == "found:\(XQE)")
    #expect(try resolveRead(editor, XQE, read) == "found:\(XQE)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > a map changed between calls is read as it is now"
    ))
  func mapChangedBetweenCalls() throws {
    let editor = try build([PAIR_A[0], "wxyz2222", PAIR_A[1]])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let shown = try readOf(editor)
    #expect(try resolveRead(editor, "0ydd", shown) == "found:\(PAIR_A[0])")
    // The same map, refilled from a later read.
    try appendParagraphs(editor, ["c"])
    shown.clear()
    for (id, handle) in try readOf(editor) {
      shown.set(id, handle)
    }
    let fresh = shown.copy()
    for read in [shown, fresh] {
      #expect(try resolveRead(editor, "0ydd", read) == "ambiguous:0ydd:0yddA,0yddC")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::$resolveBlockHandle with the handles of a read > works with a read of another editor state of the same document"
    ))
  func readOfAnotherEditorState() throws {
    let editor = try build([V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let json = try editor.getEditorState().toJSON()
    let read = try readOf(editor)
    let other = try build([])
    defer { other.dispose() }
    try other.setEditorState(other.parseEditorState(json))
    #expect(try resolveRead(other, "gTtF", read) == "found:\(XQE)")
  }
}

@Suite struct BlockHandleTypoSafetyTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > UUIDv7 ids"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > derived-like ids"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::typo safety and handle length for ids of any shape > counter ids"
    ), arguments: ["UUIDv7", "derived-like", "counter"])
  func typoSafety(_ shape: String) throws {
    let ids =
      switch shape {
      case "UUIDv7": uuidv7s(400)
      case "derived-like": derivedIds(400)
      default:
        (0..<400).map { i in "block-\(String(repeating: "0", count: 6 - String(i).count))\(i)" }
      }
    let editor = try load(ids)
    defer { editor.dispose() }
    let (lengths, tried, hits) = try measure(editor)
    #expect(lengths.count == ids.count)
    #expect(lengths.min() == 4)
    #expect(lengths.max()! <= 5)
    // About 400 of 64^4 four-character strings are handles.
    #expect(Double(hits) / Double(tried) < 1e-3)
  }
}

@Suite struct BlockHandlesWithoutTheIndexTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > a disabled extension falls back to a scan"
    ))
  func disabledFallsBackToAScan() throws {
    let editor = try build([V1ST, XQE])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let before = try handlesOf(editor)
    let disabled = try buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-disabled]",
        dependencies: [
          RichTextExtension, configExtension(BlockIdExtension) { $0.disabled = true },
        ],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { disabled.dispose() }
    try disabled.setEditorState(disabled.parseEditorState(editor.getEditorState().toJSON()))
    #expect(try handlesOf(disabled) == before)
    #expect(try resolve(disabled, "gTtF") == "found:\(XQE)")
    #expect(try resolve(disabled, "2bNo") == "found:\(V1ST)")
    #expect(try resolve(disabled, "zzzz") == "not-found:zzzz")
    // A block added while disabled gets no id, and no handle.
    try discrete(disabled) { try getRoot().append(createParagraphNode()) }
    #expect(try disabled.read { try getBlockHandle(getRoot().getLastChildOrThrow()) } == nil)
    // Its id set by hand is seen at once by the scan.
    try discrete(disabled) {
      try setState(getRoot().getLastChildOrThrow(), blockIdState, LIKE_V1ST)
    }
    #expect(try handlesOf(disabled) == [LIKE_V1ST: "2bNoV", V1ST: "2bNoq", XQE: "gTtF"])
    // And an id changed in a nested position, or a block removed.
    try discrete(disabled) { try #require(try getBlockById(LIKE_V1ST)).remove() }
    #expect(try handlesOf(disabled) == [V1ST: "2bNo", XQE: "gTtF"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > disabling and enabling at runtime"
    ))
  func disablingAndEnabling() throws {
    let editor = try build([V1ST, XQE, LIKE_V1ST])
    defer { editor.dispose() }
    try appendParagraphs(editor, ["a", "b"])
    let disabled = try blockIdOutput(editor).disabled
    try disabled.set(true)
    #expect(try handlesOf(editor) == [V1ST: "2bNo", XQE: "gTtF"])
    try discrete(editor) {
      try getRoot().append(createParagraphNode())
      try setState(getRoot().getLastChildOrThrow(), blockIdState, LIKE_V1ST)
    }
    // The scan sees the id set by hand while disabled.
    #expect(try resolve(editor, "2bNo") == "ambiguous:2bNo:2bNoV,2bNoq")
    try disabled.set(false)
    // Indexed again, with the same handles.
    #expect(try handlesOf(editor) == [LIKE_V1ST: "2bNoV", V1ST: "2bNoq", XQE: "gTtF"])
    #expect(try resolve(editor, "2bNoV") == "found:\(LIKE_V1ST)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > while disabled, a repeated id names its first live holder"
    ))
  func repeatedIdWhileDisabled() throws {
    let id = "duplicate-id-0001"
    let editor = try load([id, XQE, id], disabled: true)
    defer { editor.dispose() }
    let handle = try #require(try editor.read { try getBlockHandles() }.get(id))
    #expect(try resolve(editor, handle) == "found:\(id)")
    func holders(_ editor: Editor) throws -> [NodeKey] {
      try editor.read {
        try getRoot().getChildren().filter { try getBlockId($0) == id }.map { $0.getKey() }
      }
    }
    let keys = try holders(editor)
    let (first, second) = (keys[0], keys[1])
    func keyOf(_ input: String) throws -> NodeKey? {
      try resolveBlockHandle(input).node?.getKey()
    }
    #expect(try editor.read { try keyOf(handle) } == first)
    // The first holder removed: the second one now.
    try discrete(editor) {
      try getRoot().getFirstChildOrThrow().remove()
      #expect(try keyOf(handle) == second)
    }
    #expect(try editor.read { try keyOf(handle) } == second)
    #expect(try editor.read { try getBlockHandle(getRoot().getLastChildOrThrow()) } == handle)
    // Two holders reordered in an update: the one first now.
    let reordered = try load([id, XQE, id], disabled: true)
    defer { reordered.dispose() }
    let reorderedKeys = try holders(reordered)
    let (a, b) = (reorderedKeys[0], reorderedKeys[1])
    try discrete(reordered) {
      let root = try getRoot()
      try root.getFirstChildOrThrow().insertBefore(root.getLastChildOrThrow())
      #expect(try keyOf(handle) == b)
      #expect(try getBlockById(id)?.getKey() == b)
    }
    #expect(a != b)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::without the index > enabled or disabled, handles come from the last committed state"
    ))
  func handlesFromTheLastCommittedState() throws {
    for disabled in [false, true] {
      let editor = try load([V1ST, XQE], disabled: disabled)
      defer { editor.dispose() }
      let earlier = editor.getEditorState()
      #expect(try handlesOf(editor) == [V1ST: "2bNo", XQE: "gTtF"])
      try discrete(editor) {
        let node = try createParagraphNode()
        try setState(node, blockIdState, LIKE_V1ST)
        try getRoot().append(node)
        // Not committed yet: no handle for the new block, the old ones.
        #expect(try getBlockHandle(node) == nil)
        #expect(dictionary(try getBlockHandles()) == [V1ST: "2bNo", XQE: "gTtF"])
      }
      #expect(try handlesOf(editor)[V1ST] == "2bNoq")
      // An earlier state, read: its blocks that the committed state still
      // has, with the committed state's handles.
      #expect(
        dictionary(try earlier.read(editor: editor) { try getBlockHandles() }) == [
          V1ST: "2bNoq", XQE: "gTtF",
        ])
      // An id changed below the root (the root itself is not cloned).
      try discrete(editor) {
        let block = try getBlockById(XQE) ?? getRoot().getChildAtIndex(1)
        try setState(#require(block), blockIdState, LIKE_XQE)
      }
      #expect(
        try handlesOf(editor) == [LIKE_V1ST: "2bNoV", LIKE_XQE: "gTtF", V1ST: "2bNoq"],
        "disabled: \(disabled)")
    }
  }
}

/// The stored document of the headless tests: one paragraph per id.
private func storedDocument(withIds: Bool = true) -> JSONObject {
  let children: [JSONValue] = [V1ST, XQE, RK8P].enumerated().map { i, id in
    var paragraph = JSONObject()
    if withIds {
      paragraph["$"] = ["blockId": .string(id)]
    }
    paragraph["children"] = [
      [
        "detail": 0, "format": 0, "mode": "normal", "style": "", "text": .string("Block \(i)"),
        "type": "text", "version": 1,
      ]
    ]
    paragraph["direction"] = .null
    paragraph["format"] = ""
    paragraph["indent"] = 0
    paragraph["textFormat"] = 0
    paragraph["textStyle"] = ""
    paragraph["type"] = "paragraph"
    paragraph["version"] = 1
    return .object(paragraph)
  }
  return JSONObject([
    (
      "root",
      [
        "children": .array(children), "direction": .null, "format": "", "indent": 0,
        "type": "root", "version": 1,
      ]
    )
  ])
}

@Suite struct BlockHandlesHeadlessTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > a headless editor reads handles and applies an edit by handle"
    ))
  func headlessEditByHandle() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-headless]",
        dependencies: [HeadlessExtension, RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(storedDocument()))
    #expect(try handlesOf(editor) == [RK8P: "xt6z", V1ST: "2bNo", XQE: "gTtF"])
    try discrete(editor) {
      let node = try #require(try resolveBlockHandle("gTtF").node as? ElementNode)
      try #require(try node.getFirstDescendant() as? TextNode).setTextContent("Revenue grew 6%.")
    }
    let json = try editor.getEditorState().toJSON()
    let second = try #require(json["root"]?.objectValue?["children"]?.arrayValue?[1].objectValue)
    #expect(second["$"]?.objectValue?["blockId"] == .string(XQE))
    #expect(
      second["children"]?.arrayValue?[0].objectValue?["text"] == .string("Revenue grew 6%."))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > reading handles from a parsed state, without a DOM"
    ))
  func handlesFromAParsedState() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-json]", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    let state = try editor.parseEditorState(storedDocument().stringify())
    try editor.setEditorState(state)
    let handles = try editor.getEditorState().read(editor: editor) { try getBlockHandles() }
    #expect(dictionary(handles) == [RK8P: "xt6z", V1ST: "2bNo", XQE: "gTtF"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.test.ts::headless and JSON-only use > a document stored without ids gets handles of its positional ids"
    ))
  func positionalIdsGetHandles() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-positional]", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(storedDocument(withIds: false)))
    let handles = try handlesOf(editor)
    #expect(handles.count == 3)
    for (id, handle) in handles {
      let handle = try #require(handle)
      #expect(id.count == 21)
      #expect(handle.count >= 4)
      #expect(handleKey(id).hasPrefix(handle))
      #expect(try resolve(editor, handle) == "found:\(id)")
    }
  }
}
