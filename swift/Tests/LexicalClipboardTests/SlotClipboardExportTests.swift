import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts.

// A plain shadow-root ElementNode used as a slot value.
private final class PlainShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainShadowRootNode.self, type: "plain_shadow_root", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
  override func isShadowRoot() -> Bool { true }
}

private func createPlainShadowRootNode() throws -> PlainShadowRootNode {
  try create(PlainShadowRootNode.self)
}

// A shadow-root slot value that also excludes itself from copy, which the
// export guard must reject.
private final class ExcludedShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ExcludedShadowRootNode.self, type: "excluded_shadow_root", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
  override func isShadowRoot() -> Bool { true }
  override func excludeFromCopy(_ destination: String? = nil) throws -> Bool { true }
}

private func createExcludedShadowRootNode() throws -> ExcludedShadowRootNode {
  try create(ExcludedShadowRootNode.self)
}

// A Card-shaped host: a shadow root.
private final class CardLikeNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CardLikeNode.self, type: "card_like", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
  override func isShadowRoot() -> Bool { true }
}

private func createCardLikeNode() throws -> CardLikeNode {
  try create(CardLikeNode.self)
}

private func emptyEditor(_ name: String, _ nodes: [LexicalNodeConfig]) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name, nodes: { nodes },
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
}

private func stringify(_ nodes: [JSONObject]) -> String {
  JSONValue.array(nodes.map { .object($0) }).stringify()
}

@Suite struct SlotClipboardExportTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > throws when a slot value is excluded from copy"
    ))
  func throwsWhenExcluded() throws {
    let editor = try emptyEditor("[slot-exclude]", [.klass(ExcludedShadowRootNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try getRoot().append(host)
      _ = try setSlot(host, "title", createExcludedShadowRootNode())
    }
    try editor.read {
      #expect {
        _ = try generateJSONFromSelectedNodes(editor, nil)
      } throws: { error in
        "\(error)".contains("did not serialize to exactly the slot value node")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a host outside the selection does not gate the export on its slot"
    ))
  func hostOutsideSelection() throws {
    let editor = try emptyEditor("[slot-exclude]", [.klass(ExcludedShadowRootNode.self)])
    defer { editor.dispose() }
    var beforeKey = ""
    try editor.update(discrete: true) {
      let before = try createTextNode("BEFORE")
      _ = try getRoot().append(createParagraphNode().append(before))
      let host = try createParagraphNode()
      _ = try host.append(createTextNode("CHILD"))
      _ = try setSlot(host, "title", createExcludedShadowRootNode())
      _ = try getRoot().append(host)
      beforeKey = before.getKey()
    }
    try editor.update(discrete: true) {
      let before = try #require(try getNodeByKey(beforeKey) as? TextNode)
      _ = try before.select(0, 6)
    }
    try editor.read { () throws in
      // The selection covers only "BEFORE"; the slot-bearing host is
      // outside it, so its excluded slot must not be reached.
      #expect(throws: Never.self) {
        _ = try generateJSONFromSelectedNodes(editor, getSelection())
      }
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a slot-bearing host round-trips through JSON copy + insert"
    ))
  func roundTrip() throws {
    let editor = try emptyEditor("[slot-roundtrip]", [.klass(PlainShadowRootNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try host.append(createTextNode("HostChild"))
      let slot = try createPlainShadowRootNode()
      _ = try slot.append(createParagraphNode().append(createTextNode("SlotText")))
      _ = try getRoot().append(host)
      _ = try setSlot(host, "media", slot)
    }

    // Copy: nil selection, the whole tree.
    let serialized = try editor.read { try generateJSONFromSelectedNodes(editor, nil) }
    try #require(serialized.nodes.count == 1)
    let hostJson = serialized.nodes[0]
    let slots = try #require(hostJson["$slots"]?.objectValue)
    #expect(slots["media"] != nil)

    // Paste into a fresh editor and verify the slot survived.
    let editor2 = try emptyEditor("[slot-roundtrip]", [.klass(PlainShadowRootNode.self)])
    defer { editor2.dispose() }
    try editor2.update(discrete: true) {
      let target = try createParagraphNode()
      _ = try getRoot().append(target)
      _ = try target.select()
      let nodes = try generateNodesFromSerializedNodes(serialized.nodes)
      try insertGeneratedNodes(editor2, nodes, try #require(try getSelection()))
    }

    try editor2.read { () throws in
      let inserted = try #require(
        try getRoot().getChildren().first { try !getSlotNames($0).isEmpty })
      #expect(try getSlotNames(inserted) == ["media"])
      let slot = try #require(try getSlot(inserted, "media"))
      #expect(try slot.getTextContent().contains("SlotText"))
      // The slot up-link points back to the inserted host.
      #expect(try getSlotHost(slot)?.is(inserted) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a selection inside a slot exports its content on both channels"
    ))
  func selectionInsideSlot() throws {
    let editor = try emptyEditor("[slot-inner-copy]", [.klass(PlainShadowRootNode.self)])
    defer { editor.dispose() }
    var slotTextKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try host.append(createTextNode("HostChild"))
      let slot = try createPlainShadowRootNode()
      let slotText = try createTextNode("SlotText")
      _ = try slot.append(createParagraphNode().append(slotText))
      _ = try getRoot().append(host)
      _ = try setSlot(host, "media", slot)
      slotTextKey = slotText.getKey()
    }
    try editor.update(discrete: true) {
      let slotText = try #require(try getNodeByKey(slotTextKey) as? TextNode)
      _ = try slotText.select(0, "SlotText".utf16.count)
    }
    try editor.read { () throws in
      let selection = try getSelection()
      let json = stringify(try generateJSONFromSelectedNodes(editor, selection).nodes)
      #expect(json.contains("SlotText"))
      // The host (and its unselected child) stays out of an in-slot copy.
      #expect(!json.contains("HostChild"))
      let html = try generateHtmlFromNodes(editor, selection)
      #expect(html.contains("SlotText"))
      #expect(!html.contains("HostChild"))
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a partial range over an element host does not over-export"
    ))
  func partialRangeOverHost() throws {
    let editor = try emptyEditor(
      "[slot-partial-range]", [.klass(CardLikeNode.self), .klass(PlainShadowRootNode.self)])
    defer { editor.dispose() }
    var introTextKey = ""
    var bodyTextKey = ""
    try editor.update(discrete: true) {
      let intro = try createTextNode("Intro")
      _ = try getRoot().append(createParagraphNode().append(intro))
      let card = try createCardLikeNode()
      let bodyText = try createTextNode("Body")
      _ = try card.append(createParagraphNode().append(bodyText))
      _ = try card.append(createParagraphNode().append(createTextNode("UNSELECTED")))
      let title = try createPlainShadowRootNode()
      _ = try title.append(createParagraphNode().append(createTextNode("Title")))
      _ = try getRoot().append(card)
      _ = try setSlot(card, "title", title)
      introTextKey = intro.getKey()
      bodyTextKey = bodyText.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(introTextKey, 0, .text)
      try selection.focus.set(bodyTextKey, 2, .text)
      try setSelection(selection)
    }
    try editor.read { () throws in
      let selection = try getSelection()
      let json = stringify(try generateJSONFromSelectedNodes(editor, selection).nodes)
      #expect(json.contains("Intro"))
      #expect(json.contains(#""Bo""#))
      #expect(!json.contains(#""Body""#))
      #expect(!json.contains("UNSELECTED"))
      let html = try generateHtmlFromNodes(editor, selection)
      #expect(html.contains("Bo"))
      #expect(!html.contains("Body"))
      #expect(!html.contains("UNSELECTED"))
    }
  }

  // Regression for meta/lexical#8712: a NodeSelection on a node nested
  // inside a slot must export through the slot's frame.
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a NodeSelection on a node inside a slot exports the node, not an empty list"
    ))
  func nodeSelectionInsideSlot() throws {
    let editor = try emptyEditor("[slot-node-selection]", [.klass(PlainShadowRootNode.self)])
    defer { editor.dispose() }
    var innerParaKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createPlainShadowRootNode()
      let innerPara = try createParagraphNode().append(createTextNode("SlotNodeTarget"))
      _ = try slot.append(innerPara)
      _ = try setSlot(host, "media", slot)
      _ = try getRoot().append(host)
      innerParaKey = innerPara.getKey()
    }
    try editor.update(discrete: true) {
      let innerPara = try #require(try getNodeByKey(innerParaKey))
      let nodeSelection = createNodeSelection()
      nodeSelection.add(innerPara.getKey())
      try setSelection(nodeSelection)
    }
    try editor.read { () throws in
      let json = try generateJSONFromSelectedNodes(editor, getSelection())
      #expect(json.nodes.count > 0)
      #expect(stringify(json.nodes).contains("SlotNodeTarget"))
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > throws when a 1-child excluded slot value would export its child instead"
    ))
  func throwsWhenOneChildExcluded() throws {
    let editor = try emptyEditor("[slot-exclude-one-child]", [.klass(ExcludedShadowRootNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try getRoot().append(host)
      let slot = try createExcludedShadowRootNode()
      _ = try slot.append(createParagraphNode().append(createTextNode("inner")))
      _ = try setSlot(host, "title", slot)
    }
    try editor.read {
      #expect {
        _ = try generateJSONFromSelectedNodes(editor, nil)
      } throws: { error in
        "\(error)".contains("did not serialize to exactly the slot value node")
      }
    }
  }
}
