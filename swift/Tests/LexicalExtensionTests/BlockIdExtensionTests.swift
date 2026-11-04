import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHistory
import LexicalList
import LexicalMarkdown
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts.
// Upstream's `build` registers the list and table nodes in every editor; the
// editors here register rich text, the list and the horizontal rule. The
// tests that build tables, code or links are skipped until those
// packages are ported.

private struct BuildOptions {
  var generateId: (() throws -> String)?
  var isIdentifiedBlock: ((LexicalNode) throws -> Bool)?
  var disabled = false
  /// Skip `asEdited`: the first content the editor gets is a load.
  var fresh = false
  var history = false
}

private func build(_ options: BuildOptions = BuildOptions()) throws -> Editor {
  var clock = 0.0
  let generateId = options.generateId ?? counter()
  var dependencies: [any AnyLexicalExtensionArgument] = [
    RichTextExtension, ListExtension, HorizontalRuleExtension,
  ]
  if options.history {
    dependencies.append(
      configExtension(HistoryExtension) {
        $0.disabled = false
        $0.now = {
          clock += 10000
          return clock
        }
      })
  }
  dependencies.append(
    configExtension(BlockIdExtension) {
      $0.generateId = generateId
      if let isIdentifiedBlock = options.isIdentifiedBlock {
        $0.isIdentifiedBlock = isIdentifiedBlock
      }
      $0.disabled = options.disabled
    })
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-test]", dependencies: dependencies,
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  return options.fresh ? editor : try asEdited(editor)
}

private func build(
  generateId: (() throws -> String)? = nil,
  isIdentifiedBlock: ((LexicalNode) throws -> Bool)? = nil, disabled: Bool = false,
  fresh: Bool = false, history: Bool = false
) throws -> Editor {
  try build(
    BuildOptions(
      generateId: generateId, isIdentifiedBlock: isIdentifiedBlock, disabled: disabled,
      fresh: fresh, history: history))
}

private func load(_ editor: Editor, _ json: String) throws {
  try editor.setEditorState(editor.parseEditorState(json))
}

/// An editor built from `dependencies` with the given initial state.
private func buildWith(
  _ name: String, _ dependencies: [any AnyLexicalExtensionArgument],
  _ initialEditorState: InitialEditorStateType? = InitialEditorStateType.none
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name, dependencies: dependencies,
      editorConfig: InitialEditorConfig(initialEditorState: initialEditorState)))
}

private func expectThrows(
  _ fragment: String, sourceLocation: SourceLocation = #_sourceLocation, _ body: () throws -> Void
) {
  let error = #expect(throws: LexicalError.self, sourceLocation: sourceLocation) { try body() }
  #expect(
    error?.message.contains(fragment) == true, "\(String(describing: error))",
    sourceLocation: sourceLocation)
}

/// `$getRoot().getChildren().map($getBlockId)` in a read of `state`.
private func childIds(_ state: EditorState) throws -> [String?] {
  try state.read { try getRoot().getChildren().map { try getBlockId($0) } }
}

@Suite struct BlockIdExtensionAssignmentTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a block gets its id in the commit that creates it"
    ))
  func idInTheCreatingCommit() throws {
    let editor = try build()
    defer { editor.dispose() }
    var committed: [[String?]] = []
    editor.registerUpdateListener { payload in
      committed.append(try childIds(payload.editorState))
    }
    try discrete(editor) {
      try getRoot().append(paragraph("one"), createHorizontalRuleNode())
    }
    // One commit, already carrying both ids: nothing is left for a later
    // update to assign.
    #expect(committed == [["b1", "b2"]])
    #expect(try blocks(editor) == ["paragraph:one:b1", "horizontalrule:\n:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > importJSON assigns ids in the same commit, keeps existing ones"
    ))
  func importJSONAssignsIds() throws {
    let editor = try build(fresh: true)
    defer { editor.dispose() }
    var commits = 0
    editor.registerUpdateListener { _ in commits += 1 }
    try load(
      editor,
      docJSON([
        paragraphJSON("kept", "keep-me"), paragraphJSON("missing"),
        paragraphJSON("also kept", "x_9-Z"),
      ]))
    #expect(commits == 1)
    // A load: the missing id is positional (block 1), not generated.
    #expect(
      try blocks(editor) == [
        "paragraph:kept:keep-me", "paragraph:missing:@1", "paragraph:also kept:x_9-Z",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a document opened without ids gets positional ids in the commit that loads it"
    ))
  func openedWithoutIds() throws {
    let json = docJSON([paragraphJSON("a"), paragraphJSON("b")])
    let editor = try buildWith(
      "[opened]",
      [RichTextExtension, configExtension(BlockIdExtension) { $0.generateId = counter() }],
      .json(json))
    defer { editor.dispose() }
    #expect(try blocks(editor) == ["paragraph:a:@0", "paragraph:b:@1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a node a transform creates but never inserts does not delay the ids of a load"
    ))
  func scratchNodeDoesNotDelayLoad() throws {
    let json = docJSON([paragraphJSON("a"), paragraphJSON("b")])
    func loadIds(_ scratch: Bool) throws -> [[String?]] {
      let editor = try buildWith(
        "[scratch]",
        [RichTextExtension, configExtension(BlockIdExtension) { $0.generateId = counter() }])
      defer { editor.dispose() }
      if scratch {
        try editor.registerNodeTransform(ParagraphNode.self) { _ in
          _ = try createTextNode("scratch")
        }
      }
      var committed: [[String?]] = []
      editor.registerUpdateListener { payload in
        committed.append(try childIds(payload.editorState))
      }
      try load(editor, json)
      return committed
    }
    let expected = try loadIds(false)
    #expect(expected.count == 1)
    #expect(expected.first?.allSatisfy { $0 != nil } == true)
    // One commit, with the ids an editor without the transform assigns.
    #expect(try loadIds(true) == expected)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > a state from an editor without the extension is repaired"
    ))
  func stateWithoutExtensionIsRepaired() throws {
    let plain = try buildWith(
      "[plain]", [RichTextExtension],
      .update { _ in try getRoot().append(paragraph("a"), paragraph("b")) })
    defer { plain.dispose() }
    try plain.read {}
    let editor = try build()
    defer { editor.dispose() }
    // Not parsed: setEditorState marks only the root dirty, so the index is
    // rebuilt and the missing ids are assigned by a follow-up update.
    try editor.setEditorState(plain.getEditorState())
    #expect(try blocks(editor) == ["paragraph:a:@0", "paragraph:b:@1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > headless one-write recipe from the docs"
    ))
  func headlessOneWriteRecipe() throws {
    // Stored JSON without ids (or with some) …
    let stored = docJSON([paragraphJSON("Intro"), paragraphJSON("Body", "b-body")])
    // … is loaded, assigned, and serialized in one synchronous write.
    let editor = try buildWith(
      "[server]",
      [RichTextExtension, configExtension(BlockIdExtension) { $0.generateId = counter("id") }])
    defer { editor.dispose() }
    try load(editor, stored)
    func states() throws -> [JSONValue?] {
      let json = try editor.getEditorState().toJSON()
      return try #require(json["root"]?.objectValue?["children"]?.arrayValue).map {
        $0.objectValue?["$"]
      }
    }
    // The missing id is positional: block 0 of this document.
    let intro = try positionalId(editor, "0")
    #expect(try states() == [["blockId": .string(intro)], ["blockId": "b-body"]])
    // An agent edit addressed by id, also synchronous.
    try discrete(editor) {
      let block = try #require(try getBlockById("b-body"))
      try block.replace(paragraph("New body"))
    }
    // The replacement sits where the removed block sat, so it keeps its id.
    #expect(try states() == [["blockId": .string(intro)], ["blockId": "b-body"]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > invalid stored ids read as absent and are replaced by place"
    ))
  func invalidStoredIds() throws {
    let editor = try build(fresh: true)
    defer { editor.dispose() }
    // JS also stores `'lone\ud800'`; a Swift string holds no lone
    // surrogate (it reads as U+FFFD), which is invalid either way.
    let invalid: [JSONValue] = [
      42, "", .string(String(repeating: "x", count: 65)), "a b", " padded ", "\"><script>",
      "line\r\nbreak", "nul\u{0000}x", "lone\u{FFFD}", "émoji👋", "doc:12",
    ]
    try load(
      editor,
      docJSON(
        invalid.map { paragraphJSON("invalid", $0) } + [
          paragraphJSON("max", .string(String(repeating: "y", count: 64))),
          paragraphJSON("short", "bk3j9x0a2zq"),
          paragraphJSON("uuid", "123e4567-e89b-12d3-a456-426614174000"),
        ]))
    #expect(
      try ids(editor)
        == invalid.indices.map { "@\($0)" } + [
          String(repeating: "y", count: 64), "bk3j9x0a2zq", "123e4567-e89b-12d3-a456-426614174000",
        ])
    // The invalid values are gone from the saved JSON.
    let saved = try editor.getEditorState().toJSON().stringify()
    #expect(!saved.contains("a b"))
    #expect(!saved.contains(String(repeating: "x", count: 65)))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > an invalid id from another replica is repaired like a missing one"
    ))
  func invalidRemoteId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
      try getRoot().append(parseSerializedNode(paragraphJSON("remote", "not valid!")))
    }
    #expect(try ids(editor) == ["b1", "@1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > an invalid id set with $setState reads as absent and is replaced"
    ))
  func invalidIdFromSetState() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    try discrete(editor) {
      let node = try paragraph("two")
      try setState(node, blockIdState, "not valid!")
      try getRoot().append(node)
      #expect(try getBlockId(node) == nil)
    }
    #expect(try ids(editor) == ["b1", "b2"])
    #expect(!(try editor.getEditorState().toJSON().stringify().contains("not valid")))
    try expectAllIndexed(editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > generateId collisions are retried and a broken generator throws"
    ))
  func generateIdCollisions() throws {
    var sequence = ["dup", "dup", "dup", "fresh"]
    let editor = try build(generateId: { sequence.isEmpty ? "dup" : sequence.removeFirst() })
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(paragraph("a"), paragraph("b")) }
    #expect(try ids(editor) == ["dup", "fresh"])
    expectThrows("generateId returned 32 ids in a row") {
      try discrete(editor) { try getRoot().append(paragraph("c")) }
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > invalid generateId values are discarded and generateId is called again"
    ))
  func invalidGenerateIdValues() throws {
    // JS also returns `undefined` (not a string at all); a Swift generateId
    // returns a String, so only invalid strings are tried.
    var sequence: [String] = []
    let editor = try build(generateId: { sequence.isEmpty ? "setup" : sequence.removeFirst() })
    defer { editor.dispose() }
    sequence = ["", String(repeating: "x", count: 65), "a b", "é", "ok"]
    try discrete(editor) { try getRoot().append(paragraph("a")) }
    #expect(try ids(editor) == ["ok"])
    let broken = try build(generateId: { "not valid" })
    defer { broken.dispose() }
    expectThrows("generateId returned 32 ids in a row") {
      try discrete(broken) { try getRoot().append(paragraph("a")) }
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > the default generator makes 126-bit URL-safe ids"
    ))
  func defaultGenerator() {
    var seen = Set<String>()
    var used = Set<Character>()
    let alphabet = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")
    for _ in 0..<1000 {
      let id = generateBlockId()
      #expect(id.count == 21 && id.allSatisfy { alphabet.contains($0) })
      #expect(blockIdState.parse(.string(id)) == id)
      seen.insert(id)
      used.formUnion(id)
    }
    #expect(seen.count == 1000)
    // Every character carries 6 bits: all 64 symbols show up.
    #expect(used.count == 64)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: assignment > markdown import assigns ids"
    ))
  func markdownImportAssignsIds() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try convertFromMarkdownString(
        "# Title\n\nText\n\n> Quote\n\n- one\n- two\n\n1. first",
        [HEADING, QUOTE, UNORDERED_LIST, ORDERED_LIST])
    }
    #expect(
      try outline(editor).map { $0.replacing(/:b\d+$/, with: ":id") } == [
        "heading:id",
        "paragraph:id",
        "quote:id",
        "list:id",
        "  listitem:id",
        "  listitem:id",
        "list:id",
        "  listitem:id",
      ])
    try expectAllIndexed(editor)
  }
}

@Suite struct BlockIdExtensionUniquenessTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > duplicates in imported JSON: the earliest keeps the id"
    ))
  func duplicatesInImportedJSON() throws {
    let editor = try build(fresh: true)
    defer { editor.dispose() }
    try load(
      editor,
      docJSON([
        paragraphJSON("first", "x"), paragraphJSON("second", "y"),
        paragraphJSON("copy of first", "x"), paragraphJSON("another copy", "x"),
        paragraphJSON("copy of second", "y"),
      ]))
    // A load: the later holders of a repeated id get ids derived from it and
    // their ordinal, so every reader of this JSON names the copies alike.
    #expect(
      try ids(editor) == [
        "x", "y", deriveDuplicateId("x", "1"), deriveDuplicateId("x", "2"),
        deriveDuplicateId("y", "1"),
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > deterministic and idempotent"
    ))
  func deterministicAndIdempotent() throws {
    let json = docJSON([
      paragraphJSON("c", "z"), paragraphJSON("a", "z"), paragraphJSON("b"), paragraphJSON("d", "z"),
    ])
    func run() throws -> JSONObject {
      // The default, random generator: the missing and repeated ids are
      // positional, so two loads still agree.
      let editor = try build(generateId: generateBlockId, fresh: true)
      defer { editor.dispose() }
      try load(editor, json)
      return try editor.getEditorState().toJSON()
    }
    let first = try run()
    #expect(try run() == first)
    // Loading the repaired document again changes nothing.
    let editor = try build(generateId: counter("again"))
    defer { editor.dispose() }
    try load(editor, first.stringify())
    #expect(try editor.getEditorState().toJSON() == first)
    // And neither do more updates.
    try discrete(editor) { try nth(0).markDirty() }
    #expect(try editor.getEditorState().toJSON() == first)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a duplicate inserted after the original gets a fresh id"
    ))
  func duplicateAfterOriginal() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      let original = try nth(0)
      let copy = try parseSerializedNode(original.exportJSON())
      #expect(try getBlockId(copy) == "b1")
      try nth(1).insertAfter(copy)
    }
    #expect(try blocks(editor) == ["paragraph:one:b1", "paragraph:two:b2", "paragraph::b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a duplicate inserted above the original yields to it"
    ))
  func duplicateAboveOriginal() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      let copy = try parseSerializedNode(nth(1).exportJSON())
      #expect(try getBlockId(copy) == "b2")
      try nth(0).insertBefore(copy)
    }
    // The original existed before this update, so it keeps b2 even though
    // the copy comes first in the document.
    #expect(try ids(editor) == ["b3", "b1", "b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > duplicating a block upward keeps the id on the original"
    ))
  func duplicatingUpward() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two", "three"])
    try discrete(editor) { try nth(0).insertBefore(deepCopy(nth(2))) }
    #expect(
      try blocks(editor) == [
        "paragraph:three:b4", "paragraph:one:b1", "paragraph:two:b2", "paragraph:three:b3",
      ])
    try editor.read { () throws in #expect(try getBlockById("b3") === nth(3)) }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > blocks that all get an id in one local update: the first created keeps it"
    ))
  func firstCreatedKeepsIt() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    try discrete(editor) {
      func copy(_ text: String) throws -> ElementNode {
        let node = try paragraph(text)
        try setState(node, blockIdState, "same")
        return node
      }
      try nth(0).insertAfter(copy("later"))
      try nth(0).insertAfter(copy("earlier"))
    }
    // Only this editor makes the decision (and shares its result), so the
    // order the nodes were created in is enough, whatever their position.
    #expect(
      try blocks(editor) == ["paragraph:one:b1", "paragraph:earlier:b2", "paragraph:later:same"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > a committed duplicate: the earliest in document order keeps it, whatever the keys"
    ))
  func committedDuplicate() throws {
    // Two editors receive the same duplicate with their nodes created in
    // opposite orders (as two replicas may after their changes merge), and
    // must repair it the same way.
    var repaired: [[String]] = []
    for createLaterFirst in [false, true] {
      let editor = try build()
      defer { editor.dispose() }
      try seed(editor, ["one"])
      try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
        let created =
          createLaterFirst
          ? [try paragraph("later"), try paragraph("earlier")]
          : [try paragraph("earlier"), try paragraph("later")]
        for node in created {
          try setState(node, blockIdState, "same")
        }
        let (earlier, later) =
          createLaterFirst ? (created[1], created[0]) : (created[0], created[1])
        try nth(0).insertAfter(earlier)
        try earlier.insertAfter(later)
      }
      repaired.append(try blocks(editor))
    }
    // The later copy gets the id derived from the repeated id and its
    // ordinal, as every replica that repairs the same duplicate names it.
    #expect(
      repaired[0] == [
        "paragraph:one:b1", "paragraph:earlier:same",
        "paragraph:later:\(deriveDuplicateId("same", "1"))",
      ])
    #expect(repaired[1] == repaired[0])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > $copyNode does not copy the id"
    ))
  func copyNodeDoesNotCopyTheId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    try discrete(editor) {
      let copy = try copyNode(nth(0))
      #expect(try getState(copy, blockIdState) == nil)
      try nth(0).insertBefore(copy)
    }
    // The original keeps its id even though the copy comes first.
    #expect(try ids(editor) == ["b2", "b1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > $copyNode right after loading JSON does not copy the stored id"
    ))
  func copyNodeAfterLoad() throws {
    // NodeState read from JSON is only reset by copyNode once its config is
    // known on the node; the load update reads every identified block's id,
    // so a copy made in the very next update starts without one.
    let editor = try build()
    defer { editor.dispose() }
    try load(editor, docJSON([paragraphJSON("stored", "x")]))
    try discrete(editor) {
      let copy = try copyNode(nth(0))
      #expect(try getState(copy, blockIdState) == nil)
      try getRoot().append(copy)
    }
    #expect(try ids(editor) == ["x", "b1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > an explicit id that is already in use is repaired"
    ))
  func explicitIdInUse() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) { try setState(nth(1), blockIdState, "b1") }
    #expect(try ids(editor) == ["b1", "b3"])
    try discrete(editor) { try setState(nth(0), blockIdState, "b3") }
    // The first block claims the second block's id, but the second block
    // already had it and keeps it.
    #expect(try ids(editor) == ["b4", "b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > copy and paste within one editor and between editors"
    ))
  func copyAndPaste() throws {
    let source = try build(generateId: counter("s"))
    defer { source.dispose() }
    let target = try build(generateId: counter("t"))
    defer { target.dispose() }
    try seed(source, ["alpha", "beta"])
    try seed(target, ["gamma"])
    var copied: SerializedSelectedNodes?
    try discrete(source) {
      try selectBlocks(0, 1)
      copied = try generateJSONFromSelectedNodes(source, getSelection())
    }
    let nodes = try #require(copied).nodes
    #expect(nodes.map { $0["$"] } == [["blockId": "s1"], ["blockId": "s2"]])
    func paste(_ editor: Editor) throws {
      try discrete(editor) {
        _ = try getRoot().selectEnd()
        try insertGeneratedNodes(
          editor, generateNodesFromSerializedNodes(nodes), #require(try getSelection()))
      }
    }
    // Into another editor that does not use the ids: they are kept.
    try paste(target)
    #expect(try blocks(target) == ["paragraph:gammaalpha:t1", "paragraph:beta:s2"])
    // Into the same editor: the pasted copies yield to the originals.
    try paste(source)
    try expectUnique(source)
    #expect(Array(try ids(source).prefix(2)) == ["s1", "s2"])
  }

  private func copyBlocks(
    _ editor: Editor, _ from: Int, _ to: Int
  ) throws -> SerializedSelectedNodes {
    try selectBlocks(from, to)
    return try generateJSONFromSelectedNodes(editor, getSelection())
  }

  private func pasteInto(
    _ editor: Editor, _ copied: SerializedSelectedNodes, _ block: ElementNode
  ) throws {
    _ = try block.select()
    try insertGeneratedNodes(
      editor, generateNodesFromSerializedNodes(copied.nodes), #require(try getSelection()))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > pasting a copy above the original: the original keeps its id"
    ))
  func pastingAboveOriginal() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(createParagraphNode(), paragraph("alpha"), paragraph("beta"))
    }
    #expect(try ids(editor) == ["b1", "b2", "b3"])
    var copied: SerializedSelectedNodes?
    try discrete(editor) { copied = try copyBlocks(editor, 1, 2) }
    let payload = try #require(copied)
    try discrete(editor, PASTE_TAG) { try pasteInto(editor, payload, nth(0)) }
    // The pasted blocks replace the empty paragraph (b1 goes with it). They
    // are new in this update, so they yield to the originals.
    #expect(
      try blocks(editor) == [
        "paragraph:alpha:b4", "paragraph:beta:b5", "paragraph:alpha:b2", "paragraph:beta:b3",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > cut and paste moves the ids with the blocks"
    ))
  func cutAndPaste() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        paragraph("one"), paragraph("two"), paragraph("three"), createParagraphNode())
    }
    var copied: SerializedSelectedNodes?
    // Cut, in its own update…
    try discrete(editor) {
      copied = try copyBlocks(editor, 0, 1)
      try nth(1).remove()
      try nth(0).remove()
    }
    #expect(try ids(editor) == ["b3", "b4"])
    let payload = try #require(copied)
    // …and paste later: nothing else has the ids, so the pasted blocks keep
    // them.
    try discrete(editor, PASTE_TAG) { try pasteInto(editor, payload, nth(1)) }
    #expect(
      try blocks(editor) == ["paragraph:three:b3", "paragraph:one:b1", "paragraph:two:b2"])
    // The same in a single update (a move done as remove + insert of a copy).
    try discrete(editor) {
      let copy = try deepCopy(nth(0))
      try nth(0).remove()
      try nth(1).insertAfter(copy)
    }
    #expect(
      try blocks(editor) == ["paragraph:one:b1", "paragraph:two:b2", "paragraph:three:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: uniqueness > undo of a paste restores the ids, redo the repaired ones"
    ))
  func undoOfAPaste() throws {
    let editor = try build(history: true)
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(createParagraphNode(), paragraph("alpha"), paragraph("beta"))
    }
    let before = try blocks(editor)
    var copied: SerializedSelectedNodes?
    try discrete(editor) { copied = try copyBlocks(editor, 1, 2) }
    let payload = try #require(copied)
    try discrete(editor, PASTE_TAG) { try pasteInto(editor, payload, nth(0)) }
    let after = try blocks(editor)
    #expect(
      after == [
        "paragraph:alpha:b4", "paragraph:beta:b5", "paragraph:alpha:b2", "paragraph:beta:b3",
      ])
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try blocks(editor) == before)
    try editor.read { () throws in
      #expect(try getBlockById("b4") == nil)
      #expect(try getBlockById("b5") == nil)
      #expect(try getBlockById("b2")?.getTextContent() == "alpha")
    }
    try editor.dispatchCommand(REDO_COMMAND)
    #expect(try blocks(editor) == after)
  }
}

private func enter(_ editor: Editor, _ index: Int, _ offset: Int) throws {
  try discrete(editor) {
    try caret(index, offset)
    try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
  }
}

@Suite struct BlockIdExtensionSplitAndMergeTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a paragraph"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a paragraph"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a paragraph"
    ),
    arguments: [
      (0, ["paragraph::b3", "paragraph:hello:b1"]),
      (2, ["paragraph:he:b1", "paragraph:llo:b3"]),
      (5, ["paragraph:hello:b1", "paragraph::b3"]),
    ])
  func enterInAParagraph(_ offset: Int, _ expected: [String]) throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["hello", "next"])
    try enter(editor, 0, offset)
    #expect(try blocks(editor) == expected + ["paragraph:next:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a heading"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a heading"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a heading"
    ),
    arguments: [
      (0, ["paragraph::b3", "heading:hello:b1"]),
      (2, ["heading:he:b1", "heading:llo:b3"]),
      (5, ["heading:hello:b1", "paragraph::b3"]),
    ])
  func enterInAHeading(_ offset: Int, _ expected: [String]) throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        createHeadingNode("h1").append(createTextNode("hello")), paragraph("next"))
    }
    try enter(editor, 0, offset)
    #expect(try blocks(editor) == expected + ["paragraph:next:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a quote"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a quote"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a quote"
    ),
    arguments: [
      // QuoteNode.insertNewAfter creates a paragraph, so Enter at the start
      // leaves an empty quote above and moves the text into a paragraph.
      // The text keeps its id.
      (0, ["quote::b3", "paragraph:hello:b1"]),
      (2, ["quote:he:b1", "paragraph:llo:b3"]),
      (5, ["quote:hello:b1", "paragraph::b3"]),
    ])
  func enterInAQuote(_ offset: Int, _ expected: [String]) throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(createQuoteNode().append(createTextNode("hello")), paragraph("next"))
    }
    try enter(editor, 0, offset)
    #expect(try blocks(editor) == expected + ["paragraph:next:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace join keeps the surviving block id"
    ))
  func backspaceJoin() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two", "three"])
    try discrete(editor) {
      try caret(1, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try blocks(editor) == ["paragraph:onetwo:b1", "paragraph:three:b3"])
    // Forward delete at the end joins the next block into this one.
    try discrete(editor) {
      try caret(0, 6)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, false)
    }
    #expect(try blocks(editor) == ["paragraph:onetwothree:b1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace into a heading keeps the heading id"
    ))
  func backspaceIntoAHeading() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        createHeadingNode("h2").append(createTextNode("Title")), paragraph("body"))
    }
    try discrete(editor) {
      try caret(1, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try blocks(editor) == ["heading:Titlebody:b1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Backspace at the start of a heading after a paragraph joins into the paragraph"
    ))
  func backspaceAtHeadingStart() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        paragraph("before"), createHeadingNode("h2").append(createTextNode("Title")))
    }
    try discrete(editor) {
      try caret(1, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    #expect(try blocks(editor) == ["paragraph:beforeTitle:b1"])
  }
}

@Suite struct BlockIdExtensionTypeChangesTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to heading keeps the id"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to quote keeps the id"
    ),
    arguments: ["heading", "quote"])
  func paragraphToBlock(_ type: String) throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      try caret(1, 1)
      if type == "heading" {
        try setBlocksType(getSelection()) { try createHeadingNode("h1") }
      } else {
        try setBlocksType(getSelection()) { try createQuoteNode() }
      }
    }
    #expect(try blocks(editor) == ["paragraph:one:b1", "\(type):two:b2"])
    // And back.
    try discrete(editor) {
      try caret(1, 1)
      try setBlocksType(getSelection()) { try createParagraphNode() }
    }
    #expect(try blocks(editor) == ["paragraph:one:b1", "paragraph:two:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > several blocks at once, including empty ones, keep their ids"
    ))
  func severalBlocksAtOnce() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        paragraph("a"), createParagraphNode(), createParagraphNode(), paragraph("d"))
    }
    try discrete(editor) {
      _ = try getRoot().select(0, 4)
      try setBlocksType(getSelection()) { try createHeadingNode("h3") }
    }
    #expect(
      try blocks(editor) == ["heading:a:b1", "heading::b2", "heading::b3", "heading:d:b4"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > moving a block (drag and drop) moves its id"
    ))
  func movingABlock() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two", "three"])
    try discrete(editor) { try nth(2).insertAfter(nth(0)) }
    #expect(
      try blocks(editor) == ["paragraph:two:b2", "paragraph:three:b3", "paragraph:one:b1"])
    try discrete(editor) { try nth(0).insertBefore(nth(2)) }
    #expect(try ids(editor) == ["b1", "b2", "b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > replacing a block by hand keeps its id unless the new block has one"
    ))
  func replacingByHand() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) { try nth(0).replace(paragraph("uno")) }
    #expect(try ids(editor) == ["b1", "b2"])
    try discrete(editor) {
      let replacement = try paragraph("dos")
      try setState(replacement, blockIdState, "explicit")
      try nth(1).replace(replacement)
    }
    #expect(try ids(editor) == ["b1", "explicit"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > removing a block and appending a new one elsewhere does not reuse the id"
    ))
  func removingAndAppending() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      try nth(0).remove()
      try getRoot().append(paragraph("three"))
    }
    #expect(try ids(editor) == ["b2", "b3"])
  }
}

@Suite struct BlockIdExtensionUndoTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: undo and redo > restore exact ids"
    ))
  func restoreExactIds() throws {
    let editor = try build(history: true)
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    var snapshots: [[String]] = [try blocks(editor)]
    func step(_ fn: @escaping () throws -> Void) throws {
      try discrete(editor, HISTORY_PUSH_TAG) { try fn() }
      snapshots.append(try blocks(editor))
    }
    try step {
      try caret(0, 1)
      try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    try step {
      try caret(2, 0)
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    try step {
      try caret(1, 0)
      try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    }
    try step { try nth(1).insertBefore(nth(0)) }
    for i in stride(from: snapshots.count - 2, through: 0, by: -1) {
      try editor.dispatchCommand(UNDO_COMMAND)
      #expect(try blocks(editor) == snapshots[i])
      try editor.read { () throws in
        for block in try getRoot().getChildren() {
          #expect(try getBlockById(#require(try getBlockId(block))) === block)
        }
      }
    }
    for i in 1..<snapshots.count {
      try editor.dispatchCommand(REDO_COMMAND)
      #expect(try blocks(editor) == snapshots[i])
    }
  }
}

@Suite struct BlockIdExtensionIndexTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > $getBlockById finds blocks and follows changes"
    ))
  func getBlockByIdFollowsChanges() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try editor.read { () throws in
      #expect(try getBlockById("b1")?.getTextContent() == "one")
      #expect(try getBlockById("b2")?.getTextContent() == "two")
      #expect(try getBlockById("nope") == nil)
    }
    try discrete(editor) {
      try nth(0).remove()
      // Removed in this update: no longer found, even before the commit.
      #expect(try getBlockById("b1") == nil)
    }
    try editor.read { () throws in
      #expect(try getBlockById("b1") == nil)
      #expect(try getBlockById("b2")?.getTextContent() == "two")
    }
    try discrete(editor) { try setState(nth(0), blockIdState, "renamed") }
    try editor.read { () throws in
      #expect(try getBlockById("b2") == nil)
      #expect(try getBlockById("renamed")?.getTextContent() == "two")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > typing does not evaluate $isIdentifiedBlock for untouched blocks"
    ))
  func typingIsODirty() throws {
    final class Calls { var count = 0 }
    let calls = Calls()
    let editor = try build(isIdentifiedBlock: { node in
      calls.count += 1
      return try defaultIsIdentifiedBlock(node)
    })
    defer { editor.dispose() }
    try seed(editor, (0..<1000).map { "block \($0)" })
    #expect(Set(try ids(editor).compactMap { $0 }).count == 1000)
    calls.count = 0
    try discrete(editor) {
      try caret(500, 3)
      try #require(try getSelection() as? RangeSelection).insertText("x")
    }
    // O(dirty), as documented for isIdentifiedBlock: the edited text's
    // block and its ancestors, not the document. The bounds are loose on
    // purpose; a full scan would make 1000+ calls.
    #expect(calls.count < 10)
    calls.count = 0
    try discrete(editor) {
      try caret(500, 3)
      try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(calls.count < 20)
    try expectUnique(editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: index > disabled stops assignment; enabling assigns everything"
    ))
  func disabledStopsAssignment() throws {
    let editor = try build(disabled: true)
    defer { editor.dispose() }
    let disabled = try blockIdOutput(editor).disabled
    try seed(editor, ["one", "two"])
    #expect(try ids(editor) == [nil, nil])
    try discrete(editor) { try setState(nth(1), blockIdState, "manual") }
    try editor.read { () throws in
      // Not indexed while disabled: found by a scan.
      #expect(try getBlockById("manual")?.getTextContent() == "two")
    }
    try disabled.set(false)
    // Blocks left without an id are named by their place.
    #expect(try ids(editor) == ["@0", "manual"])
    let first = try positionalId(editor, "0")
    try editor.read { () throws in #expect(try getBlockById(first)?.getTextContent() == "one") }
    try disabled.set(true)
    try discrete(editor) { try getRoot().append(paragraph("three")) }
    // Existing ids are kept; new blocks are left alone.
    #expect(try ids(editor) == [first, "manual", nil])
  }
}

@Suite struct BlockIdExtensionTransformsSkippedTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: transforms skipped > a change applied without transforms (remote) is repaired in a follow-up"
    ))
  func remoteChangeRepaired() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    var taggedCommits = 0
    var followUps = 0
    editor.registerUpdateListener { payload in
      if payload.tags.has(COLLABORATION_TAG) {
        taggedCommits += 1
      } else {
        followUps += 1
      }
    }
    try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
      let copy = try parseSerializedNode(nth(0).exportJSON())
      try getRoot().append(copy, paragraph("no id"))
    }
    // Reading flushes the follow-up update queued by the listener.
    let after = try blocks(editor)
    #expect(taggedCommits == 1)
    #expect(followUps == 1)
    // The remote blocks left unnamed are named as every replica that
    // receives them names them: the copy by the repeated id and its ordinal,
    // the block without an id by its place.
    #expect(
      after == [
        "paragraph:one:b1",
        // exportJSON does not include children, so the copy is empty.
        "paragraph::\(deriveDuplicateId("b1", "1"))", "paragraph:no id:@2",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: transforms skipped > transforms in an update tagged COLLABORATION_TAG leave the repair to the follow-up"
    ))
  func collaborationTagLeavesRepair() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    var commits: [[String]] = []
    editor.registerUpdateListener { payload in
      commits.append(
        [payload.tags.has(COLLABORATION_TAG) ? "tagged" : "follow-up"]
          + (try childIds(payload.editorState)).map { $0 ?? "null" })
    }
    try discrete(editor, COLLABORATION_TAG) { try getRoot().append(paragraph("remote")) }
    try editor.read {}
    #expect(
      commits == [["tagged", "b1", "null"], ["follow-up", "b1", try positionalId(editor, "1")]])
  }
}

@Suite struct BlockIdExtensionMiscTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: misc > headings created by Enter at start keep ids unique"
    ))
  func headingsCreatedByEnter() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(createHeadingNode("h1").append(createTextNode("A")))
    }
    try enter(editor, 0, 0)
    #expect(try blocks(editor) == ["paragraph::b2", "heading:A:b1"])
    try editor.read { () throws in
      #expect(try nth(0) is ParagraphNode)
      #expect(try nth(1) is HeadingNode)
    }
  }
}

@Suite struct BlockIdExtensionStoredWithoutIdsTests {
  /// A fresh editor with the default, random generator.
  private func reader() throws -> Editor {
    try build(generateId: generateBlockId, fresh: true)
  }

  private let stored = docJSON([
    paragraphJSON("Intro"), paragraphJSON("Body"), paragraphJSON("Outro"),
  ])

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > two reads name the blocks the same way, by place, without a write"
    ))
  func twoReadsAgree() throws {
    let first = try reader()
    defer { first.dispose() }
    let second = try reader()
    defer { second.dispose() }
    try load(first, stored)
    try load(second, stored)
    #expect(try rawIds(first) == rawIds(second))
    #expect(try ids(first) == ["@0", "@1", "@2"])
    // The same format as generated ids.
    for id in try rawIds(first) {
      #expect(id?.utf16.count == 21 && blockIdState.parse(id.map { .string($0) }) == id)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > blocks at the same place in different documents or versions get different ids"
    ))
  func differentDocumentsDifferentIds() throws {
    let v1 = try reader()
    defer { v1.dispose() }
    let v2 = try reader()
    defer { v2.dispose() }
    try load(v1, stored)
    try load(
      v2, docJSON([paragraphJSON("Budget"), paragraphJSON("Body"), paragraphJSON("Outro")]))
    let a = try rawIds(v1)
    let b = try rawIds(v2)
    #expect(a.filter { b.contains($0) } == [])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > stored ids are kept, missing ones are positional, and a taken positional id is retried"
    ))
  func takenPositionalIdRetried() throws {
    let probe = try reader()
    defer { probe.dispose() }
    try load(probe, stored)
    let place0 = try positionalId(probe, "0")
    // Another block already holds the id that block 0's place derives (the
    // document's key ignores ids, so it is the same document).
    let editor = try reader()
    defer { editor.dispose() }
    try load(
      editor,
      docJSON([
        paragraphJSON("Intro"), paragraphJSON("Body", .string(place0)),
        paragraphJSON("Outro", "kept"),
      ]))
    #expect(try ids(editor) == ["@0#1", "@0", "kept"])
    #expect(try rawIds(editor)[1] == place0)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > blocks created later get generated ids"
    ))
  func laterBlocksGetGeneratedIds() throws {
    let probe = try reader()
    defer { probe.dispose() }
    try load(probe, stored)
    let read = try rawIds(probe).map { $0 ?? "null" }
    let editor = try build(fresh: true)
    defer { editor.dispose() }
    try load(editor, stored)
    try enter(editor, 0, 5)
    try discrete(editor) { try nth(3).insertAfter(deepCopy(nth(0))) }
    // The loaded blocks keep the ids they were read with; the split's new
    // block and the copy get ids from `generateId`.
    #expect(
      try blocks(editor) == [
        "paragraph:Intro:\(read[0])", "paragraph::b1", "paragraph:Body:\(read[1])",
        "paragraph:Outro:\(read[2])", "paragraph:Intro:b2",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > the first content of an empty editor is a load: initial state functions and the default paragraph"
    ))
  func firstContentIsALoad() throws {
    func make(_ initial: Bool) throws -> Editor {
      try buildWith(
        "[initial]", [RichTextExtension, BlockIdExtension],
        initial ? .update { _ in try getRoot().append(paragraph("one"), paragraph("two")) } : nil)
    }
    let a = try make(true)
    defer { a.dispose() }
    let b = try make(true)
    defer { b.dispose() }
    #expect(try rawIds(a) == rawIds(b))
    #expect(try ids(a) == ["@0", "@1"])
    let c = try make(false)
    defer { c.dispose() }
    let d = try make(false)
    defer { d.dispose() }
    #expect(try rawIds(c) == rawIds(d))
    #expect(try ids(c) == ["@0"])
    // The next blocks are edits: random.
    try discrete(a) { try getRoot().append(paragraph("three")) }
    try discrete(b) { try getRoot().append(paragraph("three")) }
    #expect(try rawIds(a)[2] != rawIds(b)[2])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a state that replaces the content wholesale is a load; blocks added by updates are edits"
    ))
  func wholesaleReplacementIsALoad() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["old"])
    #expect(try ids(editor) == ["b1"])
    try load(editor, stored)
    #expect(try ids(editor) == ["@0", "@1", "@2"])
    let loaded = try rawIds(editor)
    try discrete(editor) { try getRoot().append(paragraph("new")) }
    #expect(try rawIds(editor) == loaded + ["b2"])
    // Replacing every block in an update is an edit: the new blocks take
    // the places, and so the ids, of the blocks they replaced (as a type
    // change does), and the rest are generated.
    try discrete(editor) {
      _ = try getRoot().clear().append(
        contentsOf: ["x", "y", "z", "w", "v", "u"].map { try paragraph($0) })
    }
    #expect(try rawIds(editor) == loaded + ["b2", "b3", "b4"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a state from an editor without the extension gets the ids its JSON gets"
    ))
  func stateWithoutExtensionGetsJSONIds() throws {
    let plain = try buildWith(
      "[plain]", [RichTextExtension],
      .update { _ in try getRoot().append(paragraph("a"), createHeadingNode("h2")) })
    defer { plain.dispose() }
    try plain.read {}
    let fromState = try reader()
    defer { fromState.dispose() }
    try fromState.setEditorState(plain.getEditorState())
    let fromJSON = try reader()
    defer { fromJSON.dispose() }
    try load(fromJSON, plain.getEditorState().toJSON().stringify())
    #expect(try rawIds(fromState) == rawIds(fromJSON))
    #expect(try ids(fromState) == ["@0", "@1"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a person's first edits keep the ids a reader read"
    ))
  func firstEditsKeepReadIds() throws {
    let probe = try reader()
    defer { probe.dispose() }
    try load(probe, stored)
    let read = try rawIds(probe).map { $0 ?? "null" }
    let editor = try build(fresh: true)
    defer { editor.dispose() }
    try load(editor, stored)
    // Enter at the start of block 0, and block 1 becomes a heading.
    try enter(editor, 0, 0)
    try discrete(editor) {
      try caret(2, 0)
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(
      try blocks(editor) == [
        "paragraph::b1", "paragraph:Intro:\(read[0])", "heading:Body:\(read[1])",
        "paragraph:Outro:\(read[2])",
      ])
    try editor.read { () throws in
      #expect(
        try read.map { try getBlockById($0)?.getTextContent() } == ["Intro", "Body", "Outro"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > nested blocks are named by their path among identified blocks"
    ))
  func nestedBlocksNamedByPath() throws {
    let plain = try buildWith(
      "[plain]", [RichTextExtension, ListExtension],
      .update { _ in
        try convertFromMarkdownString("Title\n\n- one\n- two\n\nEnd", [UNORDERED_LIST])
      })
    defer { plain.dispose() }
    try plain.read {}
    let json = try plain.getEditorState().toJSON().stringify()
    let editor = try reader()
    defer { editor.dispose() }
    try load(editor, json)
    let key = try editor.read { try documentKey() }
    let expected = ["0", "1", "1.0", "1.1", "2"].map { derivePositionalId(key, $0) }
    let actual = try editor.read { () throws -> [String?] in
      var out: [String?] = []
      for block in try getRoot().getChildren() {
        out.append(try getBlockId(block))
        if let element = block as? ElementNode, element.getType() == "list" {
          out.append(contentsOf: try element.getChildren().map { try getBlockId($0) })
        }
      }
      return out
    }
    #expect(actual == expected)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: documents stored without ids > a load assigns every id in its own commit and is idempotent"
    ))
  func loadIsOneCommitAndIdempotent() throws {
    let editor = try reader()
    defer { editor.dispose() }
    var commits = 0
    editor.registerUpdateListener { _ in commits += 1 }
    try load(editor, stored)
    #expect(commits == 1)
    let once = try editor.getEditorState().toJSON()
    let again = try reader()
    defer { again.dispose() }
    try load(again, once.stringify())
    #expect(try again.getEditorState().toJSON() == once)
  }
}

// MARK: lists

/// `outline(editor)`: every element and decorator below the root as
/// `type`, with `:id` when it has one (positional ids aliased), indented by
/// depth.
private func outline(_ editor: Editor) throws -> [String] {
  let shown = try aliases(editor)
  return try editor.read {
    var out: [String] = []
    func walk(_ node: LexicalNode, _ depth: Int) throws {
      if node is TextNode { return }
      if depth >= 0 {
        let id = try getBlockId(node)
        out.append(
          String(repeating: "  ", count: depth) + node.getType()
            + (id.map { ":\(shown[$0] ?? $0)" } ?? ""))
      }
      if let element = node as? ElementNode {
        for child in try element.getChildren() { try walk(child, depth + 1) }
      }
    }
    try walk(getRoot(), -1)
    return out
  }
}

private func bulletList(_ texts: String...) throws -> ListNode {
  try createListNode("bullet").append(
    contentsOf: texts.map { try createListItemNode().append(createTextNode($0)) })
}

@Suite struct BlockIdExtensionListTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the start of a list item (nested ids)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the middle of a list item (nested ids)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: split and merge > Enter at the end of a list item (nested ids)"
    ),
    arguments: [
      (0, [":b4", "one:b1"]),
      (1, ["o:b1", "ne:b4"]),
      (3, ["one:b1", ":b4"]),
    ])
  func enterInAListItem(_ offset: Int, _ expected: [String]) throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      try selectBlocks(0, 1)
      try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
    }
    func items() throws -> [String] {
      try editor.read {
        try nth(0).getChildren().map {
          "\(try $0.getTextContent()):\(try getBlockId($0) ?? "null")"
        }
      }
    }
    // The items took the paragraphs' content, and with it their ids; the
    // list is a new block.
    #expect(try ids(editor) == ["b3"])
    #expect(try items() == ["one:b1", "two:b2"])
    try discrete(editor) {
      let item = try #require(try nth(0).getFirstChildOrThrow() as? ElementNode)
      let text = try #require(try item.getFirstDescendant() as? TextNode)
      _ = try text.select(offset, offset)
      try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    }
    #expect(try items() == expected + ["two:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to list: the item keeps the id, the list is new"
    ))
  func paragraphToList() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      try caret(1, 0)
      try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
    }
    #expect(try outline(editor) == ["paragraph:b1", "list:b3", "  listitem:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: type changes and moves > paragraph to list with $isRootChildBlock keeps the id on the list"
    ))
  func paragraphToListRootChildBlock() throws {
    let editor = try build(isIdentifiedBlock: isRootChildBlock)
    defer { editor.dispose() }
    try seed(editor, ["one", "two"])
    try discrete(editor) {
      try caret(1, 0)
      try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
    }
    #expect(try outline(editor) == ["paragraph:b1", "list:b2", "  listitem"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a list given an id with $setState before it is inserted"
    ))
  func listGivenAnIdBeforeInsert() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["p"])
    try discrete(editor) {
      let block = try bulletList("item")
      try setState(block, blockIdState, "my-block")
      try getRoot().append(block)
    }
    #expect(try ids(editor) == ["b1", "my-block"])
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      #expect(try getBlockById("my-block") === nth(1).getLatest())
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > a copy of a list whose items got ids alongside it yields to it"
    ))
  func copyOfListYields() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      let list = try bulletList("item")
      try setState(list, blockIdState, "my-list")
      try getRoot().append(paragraph("before"), list, paragraph("after"))
    }
    try discrete(editor) {
      let item = try #require(try nth(1).getFirstChildOrThrow() as? ElementNode)
      try #require(try item.getFirstChildOrThrow() as? TextNode).setTextContent("item!")
    }
    let originalKey = try editor.read { try nth(1).getKey() }
    try discrete(editor) { try nth(0).insertBefore(deepCopy(nth(1))) }
    try expectUnique(editor)
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      #expect(try getBlockById("my-list")?.getKey() == originalKey)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: the index finds every identified block > setEditorState of a list whose items repeat an id"
    ))
  func listItemsRepeatAnId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try seed(editor, ["one"])
    var json = try editor.getEditorState().toJSON()
    func item(_ text: String) -> JSONValue {
      .object(
        JSONObject([
          ("$", .object(JSONObject([("blockId", "X")]))),
          (
            "children",
            [
              .object(
                JSONObject([
                  ("detail", 0), ("format", 0), ("mode", "normal"), ("style", ""),
                  ("text", .string(text)), ("type", "text"), ("version", 1),
                ]))
            ]
          ),
          ("direction", .null), ("format", ""), ("indent", 0), ("type", "listitem"),
          ("value", 1), ("version", 1),
        ]))
    }
    var root = try #require(json["root"]?.objectValue)
    var children = try #require(root["children"]?.arrayValue)
    children.append(
      .object(
        JSONObject([
          ("$", .object(JSONObject([("blockId", "L")]))), ("children", [item("a"), item("b")]),
          ("direction", .null), ("format", ""), ("indent", 0), ("listType", "bullet"),
          ("start", 1), ("tag", "ul"), ("type", "list"), ("version", 1),
        ])))
    root["children"] = .array(children)
    json["root"] = .object(root)
    try editor.setEditorState(editor.parseEditorState(json))
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      #expect(try getBlockById("L")?.getType() == "list")
      #expect(try getBlockById("X")?.getTextContent() == "a")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: copies of lists that merge > pasting copies, one of them right above the original: the merged list keeps the id"
    ))
  func copyRightAboveOriginal() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(paragraph("p"), bulletList("a")) }
    let (listId, itemId) = try editor.read {
      (try getBlockId(nth(1))!, try getBlockId(nth(1).getFirstChildOrThrow())!)
    }
    try discrete(editor) {
      let list = try nth(1)
      try nth(0).insertBefore(deepCopy(list))
      // ListNode merges the original into this adjacent copy.
      try list.insertBefore(deepCopy(list))
    }
    try expectUnique(editor)
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      let holder = try #require(try getBlockById(listId) as? ElementNode)
      // The list that holds the original item keeps the list's id.
      #expect(try holder.getChildren().contains { try getBlockId($0) == itemId })
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: copies of lists that merge > copies on both sides of the original, merged over several transform passes: the merged list keeps the id"
    ))
  func copiesOnBothSides() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(paragraph("p"), bulletList("a")) }
    let (listId, itemId) = try editor.read {
      (try getBlockId(nth(1))!, try getBlockId(nth(1).getFirstChildOrThrow())!)
    }
    try discrete(editor) {
      let list = try nth(1)
      try list.insertBefore(deepCopy(list))
      try list.insertBefore(deepCopy(list))
      try list.insertAfter(deepCopy(list))
    }
    // ListNode merges the four lists one pair per transform pass. The copies
    // yield the id to the original in the first pass; once the original is
    // merged into the first copy, that copy continues it.
    #expect(try ids(editor) == ["b1", listId])
    try expectUnique(editor)
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      #expect(try nth(1).getChildren().map { try getBlockId($0) }.contains(itemId))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: a copy of a list pasted above another list > the list it merges into keeps its id"
    ))
  func copyAboveAnotherList() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(bulletList("l"), paragraph("sep"), bulletList("m"))
    }
    let (copiedId, copiedItemId, listId, itemId) = try editor.read {
      (
        try getBlockId(nth(0))!, try getBlockId(nth(0).getFirstChildOrThrow())!,
        try getBlockId(nth(2))!, try getBlockId(nth(2).getFirstChildOrThrow())!
      )
    }
    try discrete(editor) {
      // ListNode merges the list below into the copy, which yields the
      // copied ids: the merged list continues the list below, as a new list
      // without an id does.
      try nth(2).insertBefore(deepCopy(nth(0)))
    }
    try expectUnique(editor)
    try expectAllIndexed(editor)
    try editor.read { () throws -> Void in
      #expect(try getBlockId(nth(0)) == copiedId)
      #expect(try getBlockId(nth(0).getFirstChildOrThrow()) == copiedItemId)
      let merged = try nth(2)
      #expect(try getBlockId(merged) == listId)
      let itemIds = try merged.getChildren().map { try getBlockId($0) }
      #expect(itemIds.count == 2)
      #expect(itemIds.first! != copiedItemId)
      #expect(itemIds.first! != nil)
      #expect(itemIds.last! == itemId)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > copy and paste of nested list items through the Lexical clipboard"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.test.ts::BlockIdExtension: more predicates and nesting > cut and paste of nested list items through the Lexical clipboard"
    ),
    arguments: ["copy", "cut"])
  func pasteNestedListItems(_ mode: String) throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(bulletList("one", "two"), createParagraphNode()) }
    func itemIds() throws -> [String] {
      try editor.read {
        try getRoot().getChildren().flatMap { block -> [String] in
          guard let list = block as? ElementNode, block.getType() == "list" else { return [] }
          return try list.getChildren().map {
            "\(try $0.getTextContent()):\(try getBlockId($0) ?? "null")"
          }
        }
      }
    }
    let before = try itemIds()
    var copied: SerializedSelectedNodes?
    try discrete(editor) {
      let list = try nth(0)
      let first = try #require(try list.getFirstChildOrThrow() as? ElementNode)
      let last = try #require(try list.getLastChildOrThrow() as? ElementNode)
      let selection = try first.select(0, 0)
      try selection.focus.set(last.getKey(), last.getChildrenSize(), .element)
      copied = try generateJSONFromSelectedNodes(editor, getSelection())
      if mode == "cut" {
        try list.remove()
      }
    }
    try discrete(editor) {
      _ = try getRoot().getLastChildOrThrow().selectEnd()
      let selection = try #require(try getSelection() as? RangeSelection)
      try insertGeneratedNodes(
        editor, generateNodesFromSerializedNodes(#require(copied).nodes), selection)
    }
    let after = try itemIds()
    let all = after.map { $0.split(separator: ":").last.map(String.init) ?? "" }
    #expect(Set(all).count == all.count)
    if mode == "copy" {
      // The originals keep their ids; the copies get new ones.
      #expect(Array(after.prefix(2)) == before)
      #expect(after.count == 4)
    } else {
      #expect(after == before)
    }
    try expectAllIndexed(editor)
  }
}
