import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalSelection
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts.
// The default predicate covers slots like shadow roots: a slot value or a
// block in a slot container gets an id when it holds text or is a block
// decorator; the container itself does not.

/// A shadow-root slot value that holds several blocks.
private final class BlockIdSlotContainerNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      BlockIdSlotContainerNode.self, type: "block-id-slot-container", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

private func build(disabled: Bool = false, history: Bool = false) throws -> Editor {
  var clock = 0.0
  return try asEdited(
    buildEditorFromExtensions(
      defineExtension(
        name: "[block-id-slots-test]",
        dependencies: [RichTextExtension]
          + (history
            ? [
              configExtension(HistoryExtension) {
                $0.disabled = false
                $0.now = {
                  clock += 10000
                  return clock
                }
              }
            ] : [])
          + [
            configExtension(BlockIdExtension) {
              $0.disabled = disabled
              $0.generateId = counter()
            }
          ], nodes: { [.klass(BlockIdSlotContainerNode.self)] },
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none))))
}

private func paragraph(_ text: String, _ id: String) throws -> ElementNode {
  let node = try paragraph(text)
  try setState(node, blockIdState, id)
  return node
}

private func host(_ index: Int) throws -> ElementNode {
  try #require(try getRoot().getChildAtIndex(index) as? ElementNode)
}

private func container(_ index: Int, _ name: String = "title") throws -> ElementNode {
  try #require(try getSlot(host(index), name) as? BlockIdSlotContainerNode)
}

/// Positional ids of places in the active content, as `@place`.
private func positionalAliases() throws -> [String: String] {
  let key = try documentKey()
  var aliases: [String: String] = [:]
  for i in 0..<8 {
    aliases[derivePositionalId(key, String(i))] = "@\(i)"
    for j in 0..<8 {
      aliases[derivePositionalId(key, "\(i).\(j)")] = "@\(i).\(j)"
    }
  }
  return aliases
}

/// Every identified block in document order (a host's slots before its
/// children), as `text:id`, indented by depth.
private func outline(_ editor: Editor) throws -> [String] {
  try editor.read {
    let positional = try positionalAliases()
    var out: [String] = []
    func visit(_ node: LexicalNode, _ depth: Int) throws {
      if try defaultIsIdentifiedBlock(node) {
        // The block's own text, without that of its slots.
        let text =
          if let element = node as? ElementNode {
            try element.getChildren().map { try $0.getTextContent() }.joined()
          } else {
            try node.getTextContent()
          }
        let id = try getBlockId(node)
        // A positional id shows as `@path`.
        let shown = id.map { positional[$0] ?? $0 } ?? "null"
        out.append("\(String(repeating: "  ", count: depth))\(text):\(shown)")
      }
      if node is ElementNode || node is DecoratorNode {
        for name in try getSlotNames(node) {
          if let slot = try getSlot(node, name) {
            try visit(slot, depth + 1)
          }
        }
      }
      if let element = node as? ElementNode {
        for child in try element.getChildren() {
          try visit(child, depth + 1)
        }
      }
    }
    for child in try getRoot().getChildren() {
      try visit(child, 0)
    }
    return out
  }
}

/// A root paragraph `host` with a `title` slot container holding `texts`.
private func hostWithSlot(_ host: String, _ texts: [String]) throws -> ElementNode {
  let node = try paragraph(host)
  let container = try create(BlockIdSlotContainerNode.self).append(
    contentsOf: texts.map { try paragraph($0) })
  try setSlot(node, "title", container)
  return node
}

@Suite struct BlockIdSlotsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > blocks inside a slot get ids in the commit that creates them"
    ))
  func slotBlocksGetIdsInTheCommit() throws {
    let editor = try build()
    defer { editor.dispose() }
    var ids: [[String?]] = []
    let unregister = editor.registerUpdateListener { payload in
      ids.append(
        try payload.editorState.read(editor: editor) {
          try container(0).getChildren().map { try getBlockId($0) }
        })
    }
    defer { unregister() }
    try discrete(editor) { try getRoot().append(hostWithSlot("host", ["one", "two"])) }
    #expect(ids == [["b2", "b3"]])
    #expect(try outline(editor) == ["host:b1", "    one:b2", "    two:b3"])
    try editor.read { () throws in
      #expect(try getBlockById("b3")?.getTextContent() == "two")
      // The default predicate treats a slot like a shadow root: the
      // container is a wrapper, the blocks in it hold text.
      #expect(try getBlockId(container(0)) == nil)
      #expect(try !defaultIsIdentifiedBlock(container(0)))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a block that is itself a slot value gets an id"
    ))
  func slotValueGetsAnId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      let host = try paragraph("host")
      try getRoot().append(host)
      try setSlot(host, "caption", paragraph("caption"))
    }
    #expect(try outline(editor) == ["host:b1", "  caption:b2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a duplicate inside a slot yields to the existing block"
    ))
  func duplicateInsideASlotYields() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(hostWithSlot("host", ["one"]), paragraph("two"))
    }
    #expect(try outline(editor) == ["host:b1", "    one:b2", "two:b3"])
    // A copy of the last root block pasted into the slot, before the
    // original in document order.
    try discrete(editor) { try container(0).append(paragraph("copy of two", "b3")) }
    #expect(try outline(editor) == ["host:b1", "    one:b2", "    copy of two:b4", "two:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a committed duplicate across slots is repaired in document order"
    ))
  func committedDuplicateAcrossSlots() throws {
    // As in the repair of a merge, the nodes are created in either order;
    // the block first in document order keeps the id either way. A host's
    // slots come before its children, so the block in the slot wins over
    // the host's own child block.
    var results: [[String]] = []
    for slotBlockFirst in [true, false] {
      let editor = try build()
      defer { editor.dispose() }
      try discrete(editor) {
        // A root container whose children are identified, with a slot.
        let host = try create(BlockIdSlotContainerNode.self).append(paragraph("body"))
        try setSlot(host, "title", create(BlockIdSlotContainerNode.self))
        try getRoot().append(host, paragraph("after"))
      }
      try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
        let first = try paragraph(slotBlockFirst ? "in slot" : "in host body", "same")
        let second = try paragraph(slotBlockFirst ? "in host body" : "in slot", "same")
        let (slotBlock, bodyBlock) = slotBlockFirst ? (first, second) : (second, first)
        try host(0).splice(0, 0, [bodyBlock])
        try container(0).append(slotBlock)
      }
      results.append(try outline(editor))
    }
    #expect(
      results[0] == [
        "in host bodybody:b1",
        // The title container is a slot value but not a block; its child is.
        "    in slot:same",
        // The later holder: renamed from the repeated id and its ordinal.
        "  in host body:\(deriveDuplicateId("same", "1"))",
        "  body:b2",
        "after:b3",
      ])
    #expect(results[1] == results[0])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a committed duplicate between two slots follows the slot order"
    ))
  func committedDuplicateBetweenSlots() throws {
    var results: [[String]] = []
    for order in [["caption", "title"], ["title", "caption"]] {
      let editor = try build()
      defer { editor.dispose() }
      try discrete(editor) { try getRoot().append(paragraph("host")) }
      try discrete(editor, COLLABORATION_TAG, skipTransforms: true) {
        // Created (and slotted) in either order.
        for name in order {
          try setSlot(host(0), name, paragraph("in \(name)", "same"))
        }
      }
      results.append(try outline(editor))
    }
    // Slots are kept in a canonical order, so the first one keeps the id on
    // every replica.
    #expect(results[1] == results[0])
    #expect(
      results[0] == [
        "host:b1", "  in caption:same", "  in title:\(deriveDuplicateId("same", "1"))",
      ])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > replacing a slot value with $setSlot keeps the id"
    ))
  func setSlotReplacementKeepsTheId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      let host = try paragraph("host")
      try getRoot().append(host)
      try setSlot(host, "caption", paragraph("old caption"))
    }
    #expect(try outline(editor) == ["host:b1", "  old caption:b2"])
    try discrete(editor) { try setSlot(host(0), "caption", createHeadingNode("h2")) }
    #expect(try outline(editor) == ["host:b1", "  :b2"])
    // Removing the slot and setting a new one in a later update is not a
    // replacement.
    try discrete(editor) { _ = try removeSlot(host(0), "caption") }
    try discrete(editor) { try setSlot(host(0), "caption", paragraph("new caption")) }
    #expect(try outline(editor) == ["host:b1", "  new caption:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a type change inside a slot keeps the id"
    ))
  func typeChangeInsideASlotKeepsTheId() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(hostWithSlot("host", ["one", "two"])) }
    try discrete(editor) {
      let two = try #require(try container(0).getChildren()[1] as? ElementNode)
      _ = try two.select(0, 0)
      try setBlocksType(two.selectStart().clone()) { try createHeadingNode("h1") }
    }
    try editor.read { () throws in #expect(try container(0).getChildren()[1].getType() == "heading")
    }
    #expect(try outline(editor) == ["host:b1", "    one:b2", "    two:b3"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > the index follows a state replaced wholesale (undo) into slots"
    ))
  func indexFollowsUndoIntoSlots() throws {
    let editor = try build(history: true)
    defer { editor.dispose() }
    try discrete(editor) { try getRoot().append(hostWithSlot("host", ["one"])) }
    try discrete(editor) { try container(0).append(paragraph("two")) }
    #expect(try outline(editor) == ["host:b1", "    one:b2", "    two:b3"])
    try editor.dispatchCommand(UNDO_COMMAND)
    #expect(try outline(editor) == ["host:b1", "    one:b2"])
    try editor.read { () throws in
      #expect(try getBlockById("b2")?.getTextContent() == "one")
      #expect(try getBlockById("b3") == nil)
    }
    // A JSON document loaded with setEditorState is indexed too.
    let json = try editor.getEditorState().toJSON().stringify()
    let other = try build()
    defer { other.dispose() }
    try other.setEditorState(other.parseEditorState(json))
    try other.read { () throws in #expect(try getBlockById("b2")?.getTextContent() == "one") }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > removing a host drops the blocks of its slots from the index"
    ))
  func removingAHostDropsItsSlots() throws {
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(hostWithSlot("host", ["one"]), paragraph("two"))
    }
    try discrete(editor) { try host(0).remove() }
    try editor.read { () throws in #expect(try getBlockById("b2") == nil) }
    // The id is free again: a block given it keeps it.
    try discrete(editor) { try getRoot().append(paragraph("restored", "b2")) }
    #expect(try outline(editor) == ["two:b3", "restored:b2"])
    try editor.read { () throws in #expect(try getBlockById("b2")?.getTextContent() == "restored") }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > while disabled, the lookup scans slots too"
    ))
  func disabledLookupScansSlots() throws {
    let editor = try build(disabled: true)
    defer { editor.dispose() }
    try discrete(editor) {
      let host = try hostWithSlot("host", ["one"])
      try getRoot().append(host)
      try setState(host.getLatest(), blockIdState, "h")
      let container = try #require(try getSlot(host, "title") as? ElementNode)
      try setState(container.getFirstChildOrThrow(), blockIdState, "manual")
    }
    try editor.read { () throws in #expect(try getBlockById("manual")?.getTextContent() == "one") }
    try blockIdOutput(editor).disabled.set(false)
    #expect(try outline(editor) == ["host:h", "    one:manual"])
    try editor.read { () throws in #expect(try getBlockById("manual")?.getTextContent() == "one") }
  }
}
