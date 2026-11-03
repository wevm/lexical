import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalSlot.test.ts. The tests of the slot
// DOM (containers, placeholders, managed line breaks, reconciler reuse) are
// skipped as dom-only; the model behavior around them is ported.

/// `$slotContainer(...texts)`: a shadow root with one paragraph per text.
func slotContainer(_ texts: String...) throws -> TestShadowRootNode {
  let container = try createTestShadowRootNode()
  for text in texts {
    try container.append(try createParagraphNode().append(try createTextNode(text)))
  }
  return container
}

/// `DeclaredHostNode`: an element that declares `title` before `body`.
class DeclaredHostNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DeclaredHostNode.self, type: "declared_slot_host", extends: ElementNode.self,
      slots: ["title", "body"])
  }
}

/// `ReorderedHostNode`: a subclass that redeclares the order.
final class ReorderedHostNode: DeclaredHostNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      ReorderedHostNode.self, type: "reordered_slot_host", extends: DeclaredHostNode.self,
      slots: ["body", "title"])
  }
}

final class DupDeclaredHostNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DupDeclaredHostNode.self, type: "dup_declared_slot_host", extends: ElementNode.self,
      slots: ["x", "x"])
  }
}

final class ReservedDeclaredHostNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      ReservedDeclaredHostNode.self, type: "reserved_declared_slot_host",
      extends: ElementNode.self, slots: ["__proto__"])
  }
}

/// `SlotTextHostNode`: a decorator host whose inline-ness is a field.
final class SlotTextHostNode: DecoratorNode {
  var __block = false
  override class func config() -> NodeConfig? {
    NodeConfig(
      SlotTextHostNode.self, type: "slot_text_host", extends: DecoratorNode.self,
      json: [try! SchemaField("block", booleanValue(), field: \SlotTextHostNode.__block)])
  }
  override func isInline() throws -> Bool { try !getLatest().__block }
  func setIsInline(_ inline: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__block = !inline
    return writable
  }
}

/// `createSlotEditor`: upstream's slot editor, headless.
func createSlotEditor() throws -> Editor {
  let editor = try createTestEditor(nodes: [
    .klass(DeclaredHostNode.self), .klass(ReorderedHostNode.self),
    .klass(DupDeclaredHostNode.self), .klass(ReservedDeclaredHostNode.self),
    .klass(SlotTextHostNode.self),
  ])
  try editor.update(discrete: true) { _ = try getRoot().clear() }
  return editor
}

/// Expects `body` to throw, with a message that contains `fragment` when
/// one is given.
func expectThrows(
  _ fragment: String? = nil, sourceLocation: SourceLocation = #_sourceLocation,
  _ body: () throws -> Void
) {
  do {
    try body()
    Issue.record("expected an error", sourceLocation: sourceLocation)
  } catch {
    if let fragment {
      #expect("\(error)".contains(fragment), "\(error)", sourceLocation: sourceLocation)
    }
  }
}

/// Deletes with `selection`: `removeText()`, or `deleteCharacter`,
/// `deleteWord` or `deleteLine` backward.
func deleteAll(_ selection: RangeSelection, _ kind: String) throws {
  switch kind {
  case "removeText": try selection.removeText()
  case "deleteCharacter": try selection.deleteCharacter(true)
  case "deleteWord": try selection.deleteWord(true)
  default: try selection.deleteLine(true)
  }
}

/// `$createLineSlotHost`: a host paragraph `body` with a paragraph
/// `Title` as its `title` slot.
func createLineSlotHost() throws -> (ParagraphNode, ParagraphNode) {
  let host = try createParagraphNode()
  try host.append(try createTextNode("body"))
  try getRoot().append(host)
  let line = try createParagraphNode()
  try line.append(try createTextNode("Title"))
  _ = try setSlot(host, "title", line)
  return (host, line)
}

/// `$appendHostWithSlot`: appends `host` with a `title` slot holding `C`.
func appendHostWithSlot(_ parent: ElementNode, _ host: LexicalNode) throws -> TextNode {
  let text = try createTextNode("C")
  try parent.append(host)
  _ = try setSlot(
    host, "title", try createTestShadowRootNode().append(try createParagraphNode().append(text)))
  return text
}

func editSlot(_ editor: Editor, _ textKey: NodeKey, _ text: String) throws {
  try editor.update(discrete: true) { _ = try (try node(textKey) as TextNode).setTextContent(text) }
}

func rootText(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getTextContent() }
}

func node<T: LexicalNode>(_ key: NodeKey, as type: T.Type = T.self) throws -> T {
  try #require(try getNodeByKey(key) as? T)
}

@Suite struct LexicalSlotTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a slotted node is reachable, parentless, and attached"
    ))
  func slottedNodeIsReachable() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer()
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(hostKey)
      let slot: TestShadowRootNode = try node(slotKey)
      #expect(try getSlot(host, "title")?.is(slot) == true)
      #expect(try getSlotNames(host) == ["title"])
      #expect(try getSlotHost(slot)?.is(host) == true)
      #expect(try slot.getParent() == nil)
      #expect(slot.__parent == nil)
      #expect(try getSlotHostKey(slot) == host.getKey())
      #expect(try slot.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTopLevelElement stops at the slot boundary"
    ))
  func topLevelElementStopsAtSlot() throws {
    let editor = try createSlotEditor()
    var keys: (slot: NodeKey, para: NodeKey, text: NodeKey) = ("", "", "")
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      let para = try createParagraphNode()
      let text = try createTextNode("Title")
      try para.append(text)
      try slot.append(para)
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      keys = (slot.getKey(), para.getKey(), text.getKey())
    }
    try editor.read { () throws in
      let slot: TestShadowRootNode = try node(keys.slot)
      let para: ParagraphNode = try node(keys.para)
      let text: TextNode = try node(keys.text)
      #expect(try text.getTopLevelElement()?.is(para) == true)
      #expect(try text.getTopLevelElementOrThrow().is(para))
      #expect(try slot.getTopLevelElement()?.is(slot) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > slot map survives a host mutation (clone)"
    ))
  func slotMapSurvivesClone() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer()
      try getRoot().append(host)
      _ = try setSlot(host, "body", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      _ = try (try node(hostKey) as ParagraphNode).setStyle("color: red")
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(hostKey)
      #expect(try host.getStyle() == "color: red")
      #expect(try getSlot(host, "body")?.getKey() == slotKey)
      #expect(try (try node(slotKey) as TestShadowRootNode).isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot detaches a node that already has a parent"
    ))
  func setSlotDetachesParented() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      let parented = try createTestShadowRootNode()
      try getRoot().append(host).append(parented)
      #expect(try parented.getParent() === (try getRoot()))
      _ = try setSlot(host, "title", parented)
      #expect(try parented.getParent() == nil)
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getSlot(host, "title")?.getKey() == parented.getKey())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > $setSlot moves a node already slotted elsewhere (move semantics)"
    ))
  func setSlotMoves() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let hostA = try createParagraphNode()
      let hostB = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      try getRoot().append(hostA).append(hostB)
      _ = try setSlot(hostA, "title", slot)
      _ = try setSlot(hostB, "title", slot)
      #expect(try getSlotNames(hostA) == [])
      #expect(try getSlot(hostB, "title")?.is(slot) == true)
      #expect(try getSlotHost(slot)?.is(hostB) == true)
      _ = try setSlot(hostB, "subtitle", slot)
      #expect(try getSlotNames(hostB) == ["subtitle"])
      #expect(try getSlot(hostB, "subtitle")?.is(slot) == true)
      #expect(try getSlotHost(slot)?.is(hostB) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects hosting itself or an ancestor (cycle guard)"
    ))
  func setSlotCycleGuard() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let outer = try createTestShadowRootNode()
      let inner = try createTestShadowRootNode()
      try getRoot().append(outer)
      try outer.append(inner)
      expectThrows("cycle") { _ = try setSlot(outer, "x", outer) }
      expectThrows("cycle") { _ = try setSlot(inner, "x", outer) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects an ancestor reachable through a slot up-link"
    ))
  func setSlotCycleThroughUpLink() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let a = try createTestShadowRootNode()
      let b = try createTestShadowRootNode()
      try getRoot().append(a)
      _ = try setSlot(a, "x", b)
      expectThrows("cycle") { _ = try setSlot(b, "y", a) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > moving a slotted node into a child list throws (reverse guard)"
    ))
  func slottedNodeIntoChildListThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let sibling = try createParagraphNode()
      try getRoot().append(host).append(sibling)
      let slot = try createTestShadowRootNode()
      _ = try setSlot(host, "title", slot)
      expectThrows { try host.append(slot) }
      expectThrows { _ = try sibling.insertAfter(slot) }
      expectThrows { _ = try sibling.insertBefore(slot) }
      expectThrows { _ = try sibling.replace(slot) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace() on a slotted node throws (use $setSlot)"
    ))
  func replaceSlottedThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      let slot = try createTestShadowRootNode()
      _ = try setSlot(host, "title", slot)
      do {
        _ = try slot.replace(try createParagraphNode())
        Issue.record("expected replace to throw")
      } catch {
        #expect("\(error)".contains(/\(type paragraph\)[\s\S]*\$setSlot/), "\(error)")
      }
      #expect(try getSlot(host, "title")?.is(slot) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > remove() on a slotted node throws (use $removeSlot)"
    ))
  func removeSlottedThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let slot = try createTestShadowRootNode()
      _ = try setSlot(host, "title", slot)
      expectThrows { try slot.remove() }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot rejects reserved names (__proto__, constructor, prototype)"
    ))
  func setSlotRejectsReservedNames() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      for reserved in ["__proto__", "constructor", "prototype"] {
        let slot = try createTestShadowRootNode()
        expectThrows { _ = try setSlot(host, reserved, slot) }
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > setSlot enforces non-inline element or decorator values"
    ))
  func setSlotValueKinds() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      expectThrows("not a valid slot value") {
        _ = try setSlot(host, "title", try createTextNode("x"))
      }
      expectThrows("not a valid slot value") {
        _ = try setSlot(host, "title", try createTestInlineElementNode())
      }
      expectThrows("not a valid slot value") {
        _ = try setSlot(host, "title", try createTestDecoratorNode())
      }
      let line = try createParagraphNode()
      _ = try setSlot(host, "line", line)
      #expect(try getSlot(host, "line")?.is(line) == true)
      _ = try removeSlot(host, "line")
      let shadow = try createTestShadowRootNode()
      _ = try setSlot(host, "title", shadow)
      #expect(try getSlot(host, "title")?.is(shadow) == true)
      let blockDecorator = try createTestDecoratorNode().setIsInline(false)
      _ = try setSlot(host, "body", blockDecorator)
      #expect(try getSlot(host, "body")?.is(blockDecorator) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > overwriting a slot name orphans the previous occupant (no leak)"
    ))
  func overwritingSlotOrphansOld() throws {
    let editor = try createSlotEditor()
    var oldSlotKey = ""
    var newSlotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let oldSlot = try createTestShadowRootNode()
      let newSlot = try createTestShadowRootNode()
      try getRoot().append(host)
      _ = try setSlot(host, "title", oldSlot)
      oldSlotKey = oldSlot.getKey()
      newSlotKey = newSlot.getKey()
      _ = try setSlot(host, "title", newSlot)
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try getSlot(host, "title")?.getKey() == newSlotKey)
      #expect(try getNodeByKey(oldSlotKey) == nil)
      #expect(try getNodeByKey(newSlotKey)?.isAttached() == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > slots round-trip through serialize -> parse, alongside a normal child"
    ))
  func slotsRoundTrip() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Heading")
      let normalChild = try createParagraphNode()
      try normalChild.append(try createTextNode("Body text"))
      try getRoot().append(host)
      try host.append(normalChild)
      _ = try setSlot(host, "title", title)
    }
    let stringified = try editor.getEditorState().toJSON().stringify()
    try editor.parseEditorState(stringified).read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getChildren().count == 1)
      #expect(try host.getTextContent().contains("Body text"))
      let title = try #require(try getSlot(host, "title"))
      #expect(try title.getTextContent() == "Heading")
      #expect(try getSlotHost(title)?.is(host) == true)
      #expect(try title.getParent() == nil)
      #expect(try title.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a nested host (slot value contains another host with its own slot) round-trips"
    ))
  func nestedHostRoundTrips() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let outerHost = try createParagraphNode()
      try getRoot().append(outerHost)
      let container = try createTestShadowRootNode()
      let innerHost = try createParagraphNode()
      try innerHost.append(try createTextNode("inner body"))
      try container.append(innerHost)
      _ = try setSlot(innerHost, "inner", try slotContainer("InnerSlot"))
      _ = try setSlot(outerHost, "outer", container)
    }
    let stringified = try editor.getEditorState().toJSON().stringify()
    try editor.parseEditorState(stringified).read { () throws in
      let outerHost = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let outerSlot = try #require(try getSlot(outerHost, "outer") as? ElementNode)
      #expect(try getSlotHost(outerSlot)?.is(outerHost) == true)
      let innerHost = try #require(try outerSlot.getFirstChild() as? ElementNode)
      #expect(try innerHost.getChildren().first?.getTextContent() == "inner body")
      let innerSlot = try #require(try getSlot(innerHost, "inner"))
      #expect(try innerSlot.getTextContent() == "InnerSlot")
      #expect(try getSlotHost(innerSlot)?.is(innerHost) == true)
      #expect(try innerSlot.getParent() == nil)
      #expect(try innerSlot.isAttached())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > restoring a prior editor state reverts a slot move (undo-style)"
    ))
  func restoringStateRevertsMove() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let hostA = try createParagraphNode()
      let hostB = try createParagraphNode()
      try getRoot().append(hostA).append(hostB)
      let slot = try slotContainer("Moved")
      _ = try setSlot(hostA, "title", slot)
      slotKey = slot.getKey()
    }
    let beforeMove = editor.getEditorState()
    try editor.update(discrete: true) {
      let hostA = try #require(try getRoot().getFirstChild() as? ElementNode)
      let hostB = try #require(try getRoot().getLastChild() as? ElementNode)
      _ = try setSlot(hostB, "title", try #require(try getSlot(hostA, "title")))
    }
    try editor.read { () throws in
      let hostB = try #require(try getRoot().getLastChild() as? ElementNode)
      #expect(try getSlot(hostB, "title")?.getKey() == slotKey)
    }
    try editor.setEditorState(beforeMove)
    try editor.read { () throws in
      let hostA = try #require(try getRoot().getFirstChild() as? ElementNode)
      let hostB = try #require(try getRoot().getLastChild() as? ElementNode)
      #expect(try getSlotNames(hostB) == [])
      #expect(try getSlot(hostA, "title")?.getKey() == slotKey)
      #expect(try getSlotHost(try #require(try getNodeByKey(slotKey)))?.is(hostA) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > export throws when a slot key resolves to no node"
    ))
  func exportThrowsForGhostSlot() throws {
    let editor = try createTestEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().clear().append(host)
      let writableHost = try host.getWritable()
      var slots = writableHost.__slots ?? SlotMap()
      slots.set("ghost", "nonexistent-key")
      writableHost.__slots = slots
    }
    expectThrows { _ = try editor.getEditorState().toJSON() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTextContent reads slots-first, ahead of children"
    ))
  func textContentSlotsFirst() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Title")
      let body = try createParagraphNode()
      try body.append(try createTextNode("Body"))
      try getRoot().append(host)
      try host.append(body)
      _ = try setSlot(host, "title", title)
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getTextContent() == "TitleBody")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > detaching a host garbage-collects its slot node"
    ))
  func detachingHostCollectsSlot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer()
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      slotKey = slot.getKey()
    }
    #expect(try editor.read { try getNodeByKey(slotKey) } != nil)
    try editor.update(discrete: true) { try getRoot().getFirstChild()!.remove() }
    #expect(try editor.read { try getNodeByKey(slotKey) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a slot value moved to another host survives the old host being removed in the same commit"
    ))
  func movedSlotSurvivesOldHostRemoval() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let hostA = try createParagraphNode()
      let hostB = try createParagraphNode()
      try getRoot().append(hostA).append(hostB)
      let slot = try slotContainer("Moved")
      _ = try setSlot(hostA, "title", slot)
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      let hostA = try #require(try getRoot().getFirstChild() as? ElementNode)
      let hostB = try #require(try getRoot().getLastChild() as? ElementNode)
      _ = try setSlot(hostB, "title", try #require(try getSlot(hostA, "title")))
      try hostA.remove()
    }
    try editor.read { () throws in
      #expect(try getNodeByKey(slotKey) != nil)
      let hostB = try #require(try getRoot().getFirstChild() as? ElementNode)
      #expect(try getSlot(hostB, "title")?.getKey() == slotKey)
      #expect(try getNodeByKey(slotKey)?.isAttached() == true)
    }
    try editor.update(discrete: true) { try getRoot().getFirstChild()!.remove() }
    #expect(try editor.read { try getNodeByKey(slotKey) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > detaching a decorator host garbage-collects its slot node"
    ))
  func detachingDecoratorHostCollectsSlot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    var textKey = ""
    try editor.update(discrete: true) {
      let host = try createTestDecoratorNode().setIsInline(false)
      let slot = try createTestShadowRootNode()
      let para = try createParagraphNode()
      let text = try createTextNode("Slotted")
      try para.append(text)
      try slot.append(para)
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      slotKey = slot.getKey()
      textKey = text.getKey()
    }
    #expect(try editor.read { try getNodeByKey(slotKey) } != nil)
    #expect(try editor.read { try getNodeByKey(textKey) } != nil)
    try editor.update(discrete: true) { try getRoot().getFirstChild()!.remove() }
    #expect(try editor.read { try getNodeByKey(slotKey) } == nil)
    #expect(try editor.read { try getNodeByKey(textKey) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getTextContentSize counts slots-first, like getTextContent"
    ))
  func textContentSizeSlotsFirst() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Title")
      let body = try createParagraphNode()
      try body.append(try createTextNode("Body"))
      try getRoot().append(host)
      try host.append(body)
      _ = try setSlot(host, "title", title)
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getTextContentSize() == host.getTextContent().utf16.count)
      #expect(try host.getTextContentSize() == 9)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getAllTextNodes includes slot text, slots-first"
    ))
  func allTextNodesSlotsFirst() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Title")
      let body = try createParagraphNode()
      try body.append(try createTextNode("Body"))
      try getRoot().append(host)
      try host.append(body)
      _ = try setSlot(host, "title", title)
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getAllTextNodes().map { try $0.getTextContent() } == ["Title", "Body"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a host with slots but no children is not empty"
    ))
  func hostWithSlotsNotEmpty() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      _ = try setSlot(host, "title", try slotContainer("Title"))
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getChildrenSize() == 0)
      #expect(try host.isEmpty() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > reconciler text cache folds slot text in slots-first (RootNode.__cachedText)"
    ))
  func rootTextSlotsFirst() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let body = try createParagraphNode()
      try body.append(try createTextNode("Body"))
      try getRoot().append(host)
      try host.append(body)
      _ = try setSlot(host, "title", try slotContainer("Title"))
    }
    try editor.read { () throws in
      let root = try getRoot()
      let host = try #require(try root.getFirstChild() as? ParagraphNode)
      #expect(try host.getTextContent() == "TitleBody")
      #expect(try root.getTextContent() == "TitleBody")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > descendant navigation stays children-only (slots stay out of selection)"
    ))
  func descendantsChildrenOnly() throws {
    let editor = try createSlotEditor()
    var bodyTextKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let body = try createParagraphNode()
      let bodyText = try createTextNode("Body")
      try body.append(bodyText)
      try getRoot().append(host)
      try host.append(body)
      _ = try setSlot(host, "title", try slotContainer("Title"))
      bodyTextKey = bodyText.getKey()
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try host.getFirstDescendant()?.getKey() == bodyTextKey)
      #expect(try host.getLastDescendant()?.getKey() == bodyTextKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > suffix fast path keeps slot text when a slot and a suffix child are edited together"
    ))
  func suffixEditWithSlotEdit() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var titleTextKey = ""
    var childTextKeys: [NodeKey] = []
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try createTestShadowRootNode()
      let titleText = try createTextNode("Title")
      try title.append(try createParagraphNode().append(titleText))
      try getRoot().append(host)
      for i in 0..<5 {
        let childText = try createTextNode("c\(i)")
        try host.append(try createParagraphNode().append(childText))
        childTextKeys.append(childText.getKey())
      }
      _ = try setSlot(host, "title", title)
      hostKey = host.getKey()
      titleTextKey = titleText.getKey()
    }
    try editor.update(discrete: true) {
      _ = try (try node(titleTextKey) as TextNode).setTextContent("Header")
      _ = try (try node(childTextKeys[4]) as TextNode).setTextContent("c4!")
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(hostKey)
      #expect(try getRoot().getTextContent() == host.getTextContent())
      #expect(try host.getTextContent().hasPrefix("Header"))
      #expect(try host.getTextContent().contains("c4!"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > suffix fast path keeps slot text when only a suffix child is edited"
    ))
  func suffixEditOnly() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var titleTextKey = ""
    var childTextKeys: [NodeKey] = []
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try createTestShadowRootNode()
      let titleText = try createTextNode("Title")
      try title.append(try createParagraphNode().append(titleText))
      try getRoot().append(host)
      for i in 0..<5 {
        let childText = try createTextNode("c\(i)")
        try host.append(try createParagraphNode().append(childText))
        childTextKeys.append(childText.getKey())
      }
      _ = try setSlot(host, "title", title)
      hostKey = host.getKey()
      titleTextKey = titleText.getKey()
    }
    _ = titleTextKey
    try editor.update(discrete: true) {
      _ = try (try node(childTextKeys[4]) as TextNode).setTextContent("c4!")
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(hostKey)
      #expect(try getRoot().getTextContent() == host.getTextContent())
      #expect(try host.getTextContent().hasPrefix("Title"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a document-wide RangeSelection carries slot text"
    ))
  func documentSelectionCarriesSlotText() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let before = try createParagraphNode().append(try createTextNode("BEFORE"))
      let host = try createParagraphNode()
      let after = try createParagraphNode().append(try createTextNode("AFTER"))
      try getRoot().append(before, host, after)
      _ = try setSlot(host, "title", try slotContainer("SLOTTEXT"))
    }
    var text = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let selection = createRangeSelection()
      try selection.anchor.set(root.getKey(), 0, .element)
      try selection.focus.set(root.getKey(), try root.getChildrenSize(), .element)
      try setSelection(selection)
      text = try selection.getTextContent()
    }
    #expect(text.contains("SLOTTEXT"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace(includeChildren) does not transfer slots; they GC with the replaced host"
    ))
  func replaceDoesNotTransferSlots() throws {
    let editor = try createSlotEditor()
    var keys: (newHost: NodeKey, oldHost: NodeKey, slot: NodeKey) = ("", "", "")
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      let newHost = try createParagraphNode()
      _ = try host.replace(newHost, includeChildren: true)
      keys = (newHost.getKey(), host.getKey(), slot.getKey())
      #expect(try getSlot(newHost, "title") == nil)
      #expect(try getSlotNames(newHost) == [])
      #expect(try getSlot(host, "title")?.is(slot) == true)
      #expect(try host.isAttached() == false)
    }
    try editor.read { () throws in
      #expect(try getNodeByKey(keys.oldHost) == nil)
      #expect(try getNodeByKey(keys.slot) == nil)
      #expect(try getNodeByKey(keys.newHost)?.isAttached() == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > replace on a decorator host does not transfer slots; they GC with it"
    ))
  func replaceDecoratorHost() throws {
    let editor = try createSlotEditor()
    var keys: (oldHost: NodeKey, slot: NodeKey) = ("", "")
    try editor.update(discrete: true) { () throws in
      let host = try createTestDecoratorNode().setIsInline(false)
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      _ = try setSlot(host, "media", slot)
      keys = (host.getKey(), slot.getKey())
      let newHost = try createTestDecoratorNode().setIsInline(false)
      _ = try host.replace(newHost)
      #expect(try getSlot(newHost, "media") == nil)
      #expect(try getSlot(host, "media")?.is(slot) == true)
      #expect(try host.isAttached() == false)
    }
    try editor.read { () throws in
      #expect(try getNodeByKey(keys.oldHost) == nil)
      #expect(try getNodeByKey(keys.slot) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a replaced host reattached in the same update keeps its slots (wrap pattern)"
    ))
  func wrapPatternKeepsSlots() throws {
    let editor = try createSlotEditor()
    var keys: (host: NodeKey, slot: NodeKey) = ("", "")
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      keys = (host.getKey(), slot.getKey())
      let wrapper = try createTestShadowRootNode()
      _ = try host.replace(wrapper)
      try wrapper.append(host)
      #expect(try getSlot(host, "title")?.is(slot) == true)
      #expect(try getSlotNames(wrapper) == [])
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(keys.host)
      #expect(try host.isAttached())
      let slot = try #require(try getSlot(host, "title"))
      #expect(slot.getKey() == keys.slot)
      #expect(try slot.getTextContent() == "SLOTTEXT")
      #expect(try getSlotHost(slot)?.is(host) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > NodeSelection.insertNodes leaves a slotted node intact"
    ))
  func nodeSelectionInsertKeepsSlot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createNodeSelection()
      selection.add(slotKey)
      try setSelection(selection)
      try selection.insertNodes([try createParagraphNode()])
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try getSlot(host, "title")?.getTextContent() == "SLOTTEXT")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > NodeSelection.deleteNodes leaves a slotted node intact"
    ))
  func nodeSelectionDeleteKeepsSlot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createNodeSelection()
      selection.add(slotKey)
      try setSelection(selection)
      try selection.deleteNodes()
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try getSlot(host, "title")?.getTextContent() == "SLOTTEXT")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removeText inside a slot stays scoped and does not walk past the slot boundary"
    ))
  func removeTextInsideSlot() throws {
    let editor = try createSlotEditor()
    var textKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      let text = try createTextNode("SLOTTEXT")
      try slot.append(try createParagraphNode().append(text))
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      textKey = text.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(textKey, 0, .text)
      try selection.focus.set(textKey, 4, .text)
      try setSelection(selection)
      try selection.removeText()
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try getSlot(host, "title")?.getTextContent() == "TEXT")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace after a slot-bearing decorator host removes it (#8904)"
    ))
  func backspaceRemovesDecoratorHost() throws {
    let editor = try createSlotEditor()
    var keys: (host: NodeKey, text: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let host = try createTestDecoratorNode().setIsInline(false)
      _ = try setSlot(host, "quote", try slotContainer("QUOTE"))
      let text = try createTextNode("after")
      try getRoot().append(host, try createParagraphNode().append(text))
      keys = (host.getKey(), text.getKey())
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(keys.text, 0, .text)
      try selection.focus.set(keys.text, 0, .text)
      try setSelection(selection)
      try selection.deleteCharacter(true)
    }
    try editor.read { () throws in
      #expect(try getNodeByKey(keys.host) == nil)
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getRoot().getTextContent() == "after")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace in an empty paragraph after a slot-bearing decorator host selects it (#8904)"
    ))
  func backspaceSelectsDecoratorHost() throws {
    let editor = try createSlotEditor()
    var keys: (host: NodeKey, paragraph: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let host = try createTestDecoratorNode().setIsInline(false)
      _ = try setSlot(host, "quote", try slotContainer("QUOTE"))
      let paragraph = try createParagraphNode()
      try getRoot().append(host, paragraph)
      keys = (host.getKey(), paragraph.getKey())
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(keys.paragraph, 0, .element)
      try selection.focus.set(keys.paragraph, 0, .element)
      try setSelection(selection)
      try selection.deleteCharacter(true)
    }
    try editor.read { () throws in
      #expect(try getNodeByKey(keys.paragraph) == nil)
      #expect(try getNodeByKey(keys.host) != nil)
      let selection = try #require(try getSelection() as? NodeSelection)
      #expect(try selection.getNodes().map { $0.getKey() } == [keys.host])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > backspace at the start of a host child does not merge the slot-bearing host away"
    ))
  func backspaceKeepsSlotHost() throws {
    let editor = try createSlotEditor()
    var keys: (host: NodeKey, child: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("AAA")))
      let host = try createParagraphNode()
      _ = try setSlot(host, "title", try slotContainer("TITLE"))
      let child = try createTextNode("BBB")
      try host.append(child)
      try getRoot().append(host)
      keys = (host.getKey(), child.getKey())
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(keys.child, 0, .text)
      try selection.focus.set(keys.child, 0, .text)
      try setSelection(selection)
      try selection.deleteCharacter(true)
    }
    try editor.read { () throws in
      let host: ParagraphNode = try node(keys.host)
      #expect(try getRoot().getChildrenSize() == 2)
      #expect(try getSlotNames(host) == ["title"])
      #expect(try getSlot(host, "title")?.getTextContent() == "TITLE")
      #expect(try host.getTextContent() == "TITLEBBB")
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == keys.child)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > getParentCaret stops at a slot value in every mode"
    ))
  func parentCaretStopsAtSlot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      _ = try setSlot(host, "title", try slotContainer("TITLE"))
      slotKey = try getSlot(host, "title")!.getKey()
    }
    try editor.read { () throws in
      let slot: TestShadowRootNode = try node(slotKey)
      let childCaret: ChildCaret = getChildCaret(slot, .next)
      #expect(try childCaret.getParentCaret(.root) == nil)
      #expect(try childCaret.getParentCaret(.shadowRoot) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for a null selection"
    ))
  func slotFrameOfNil() throws {
    let editor = try createSlotEditor()
    #expect(try editor.read { try getSelectionSlotFrame(nil) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for a selection outside any slot"
    ))
  func slotFrameOutsideSlots() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let text = try createTextNode("plain")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(0, 5)
    }
    #expect(try editor.read { try getSelectionSlotFrame(try getSelection()) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns the slot frame for a RangeSelection inside a slot"
    ))
  func slotFrameOfRangeSelection() throws {
    let editor = try createSlotEditor()
    var containerKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let container = try slotContainer("inside")
      try getRoot().append(host)
      _ = try setSlot(host, "title", container)
      containerKey = container.getKey()
      _ = try container.getFirstChildOrThrow().selectStart()
    }
    #expect(
      try editor.read { try getSelectionSlotFrame(try getSelection())?.getKey() } == containerKey)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns the slot frame for a NodeSelection inside a slot"
    ))
  func slotFrameOfNodeSelection() throws {
    let editor = try createSlotEditor()
    var containerKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let container = try slotContainer("inside")
      try getRoot().append(host)
      _ = try setSlot(host, "title", container)
      containerKey = container.getKey()
      let nodeSelection = createNodeSelection()
      nodeSelection.add(try container.getFirstChildOrThrow().getKey())
      try setSelection(nodeSelection)
    }
    #expect(
      try editor.read { try getSelectionSlotFrame(try getSelection())?.getKey() } == containerKey)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSelectionSlotFrame > returns null for an empty NodeSelection"
    ))
  func slotFrameOfEmptyNodeSelection() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode())
      try setSelection(createNodeSelection())
    }
    #expect(try editor.read { try getSelectionSlotFrame(try getSelection()) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns the slot name for a node sitting in a named slot"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > disambiguates between multiple slots on the same host"
    ),
    arguments: [false, true])
  func slotNameWithinHost(withBody: Bool) throws {
    let editor = try createSlotEditor()
    var keys: (title: NodeKey, body: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer()
      let body = try slotContainer()
      try getRoot().append(host)
      _ = try setSlot(host, "title", title)
      if withBody {
        _ = try setSlot(host, "body", body)
      }
      keys = (title.getKey(), body.getKey())
    }
    try editor.read { () throws in
      #expect(try getSlotNameWithinHost(try node(keys.title)) == "title")
      if withBody {
        #expect(try getSlotNameWithinHost(try node(keys.body)) == "body")
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns null for a regular child (not a slot value)"
    ))
  func slotNameOfRegularChild() throws {
    let editor = try createSlotEditor()
    var childKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let child = try createTextNode("plain")
      try host.append(child)
      try getRoot().append(host)
      childKey = child.getKey()
    }
    #expect(try editor.read { try getSlotNameWithinHost(try node(childKey) as TextNode) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns null for the host itself"
    ))
  func slotNameOfHost() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try setSlot(host, "title", try slotContainer())
      try getRoot().append(host)
      hostKey = host.getKey()
    }
    #expect(
      try editor.read { try getSlotNameWithinHost(try node(hostKey) as ParagraphNode) } == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$getSlotNameWithinHost > returns the immediate slot name when slots are nested"
    ))
  func slotNameWhenNested() throws {
    let editor = try createSlotEditor()
    var innerSlotKey = ""
    try editor.update(discrete: true) {
      let outerHost = try createParagraphNode()
      let innerHost = try slotContainer()
      let innerSlot = try slotContainer()
      try getRoot().append(outerHost)
      _ = try setSlot(outerHost, "outer", innerHost)
      _ = try setSlot(innerHost, "inner", innerSlot)
      innerSlotKey = innerSlot.getKey()
    }
    #expect(
      try editor.read { try getSlotNameWithinHost(try node(innerSlotKey) as TestShadowRootNode) }
        == "inner")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteCharacter selection inside a root slot keeps the selection in that slot"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteWord selection inside a root slot keeps the selection in that slot"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a deleteLine selection inside a root slot keeps the selection in that slot"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a removeText selection inside a root slot keeps the selection in that slot"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting a node selection inside a root slot keeps the selection in that slot"
    ),
    arguments: ["deleteCharacter", "deleteWord", "deleteLine", "removeText", "node"])
  func deleteInsideRootSlot(kind: String) throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let slot = try slotContainer("Footnote")
      _ = try setSlot(root, "footnotes", slot)
      if kind == "node" {
        let selection = createNodeSelection()
        selection.add(try slot.getFirstChildOrThrow().getKey())
        try setSelection(selection)
        try selection.deleteNodes()
      } else {
        try deleteAll(try selectAll(try slot.selectStart()), kind)
      }
      #expect(try getSelectionSlotFrame(try getSelection()) === slot)
      #expect(try root.getChildrenSize() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteCharacter selection restores a paragraph before root slots"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteWord selection restores a paragraph before root slots"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a deleteLine selection restores a paragraph before root slots"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a removeText selection restores a paragraph before root slots"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > deleting all children with a node selection restores a paragraph before root slots"
    ),
    arguments: ["deleteCharacter", "deleteWord", "deleteLine", "removeText", "node"])
  func deleteAllRestoresParagraph(kind: String) throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let paragraph = try createParagraphNode().append(try createTextNode("Body"))
      try root.append(paragraph)
      let slot = try slotContainer("Footnote")
      _ = try setSlot(root, "footnotes", slot)
      if kind == "node" {
        let selection = createNodeSelection()
        selection.add(paragraph.getKey())
        try setSelection(selection)
        try selection.deleteNodes()
      } else {
        try deleteAll(try selectAll(), kind)
      }
      #expect(try root.getChildrenSize() == 1)
      let replacement = try #require(try root.getFirstChild() as? ParagraphNode)
      #expect(try replacement.isEmpty())
      #expect(try getSlot(root, "footnotes") === slot)
      #expect(try slot.getTextContent() == "Footnote")
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.isCollapsed())
      #expect(try selection.anchor.getNode() === replacement)
      try selection.insertText("Replacement")
      #expect(try replacement.getTextContent() == "Replacement")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > $selectAll does not throw when anchor is the slot value root"
    ))
  func selectAllFromSlotRoot() throws {
    let editor = try createSlotEditor()
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer("Title")
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(slotKey, 0, .element)
      try selection.focus.set(slotKey, 0, .element)
      try setSelection(selection)
    }
    try editor.update(discrete: true) {
      _ = try selectAll(try #require(try getSelection() as? RangeSelection))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > $selectAll does not throw when anchor is the root element"
    ))
  func selectAllFromRoot() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { _ = try selectAll(try getRoot().select(0, 0)) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > setEditorState latches _slotsUsed when the state contains slots"
    ))
  func setEditorStateLatchesSlotsUsed() throws {
    let editorA = try createSlotEditor()
    let editorB = try createSlotEditor()
    try editorA.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try setSlot(host, "title", try slotContainer("Hello"))
      try getRoot().append(host)
    }
    #expect(editorA._slotsUsed)
    try editorB.setEditorState(editorA.getEditorState())
    #expect(editorB._slotsUsed)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::$selectAll boundary cases > _slotsUsed stays true after the last slot is removed"
    ))
  func slotsUsedStaysTrue() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      _ = try setSlot(host, "title", try slotContainer("Hello"))
      try getRoot().append(host)
    }
    #expect(editor._slotsUsed)
    try editor.update(discrete: true) {
      if let host = try getRoot().getFirstChild(), isSlotHost(host) {
        _ = try removeSlot(host, "title")
      }
    }
    #expect(editor._slotsUsed)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > insertNodes with the caret on an empty slot value inserts into the slot"
    ))
  func insertNodesIntoEmptySlot() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slotValue = try createTestShadowRootNode()
      try getRoot().append(host)
      _ = try setSlot(host, "title", slotValue)
      try slotValue.select().insertNodes([
        try createParagraphNode().append(try createTextNode("pasted"))
      ])
    }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let slotValue = try #require(try getSlot(host, "title"))
      #expect(try slotValue.getTextContent() == "pasted")
      #expect(try host.getChildrenSize() == 0)
      #expect(try getRoot().getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > closing a cycle through the children channel throws instead of hanging"
    ))
  func cycleThroughChildrenThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      let title = try slotContainer("Title")
      _ = try setSlot(host, "title", title)
      expectThrows("cycle") { try title.append(host) }
      let inner = try #require(try title.getFirstChild() as? ParagraphNode)
      expectThrows("cycle") { try inner.append(host) }
      let innerText = try #require(try inner.getFirstChild())
      expectThrows("cycle") { _ = try innerText.insertBefore(host) }
      expectThrows("cycle") { _ = try innerText.insertAfter(host) }
      expectThrows("cycle") { _ = try innerText.replace(host) }
      #expect(try getSlot(host, "title")?.is(title) == true)
      #expect(try getSlotHost(title)?.is(host) == true)
      #expect(try host.getParent() != nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: audit hardening (insertNodes, cycles, idempotent setSlot) > re-setting the same node into the same slot is a no-op"
    ))
  func resettingSameSlotIsNoOp() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let host = try createParagraphNode()
      try getRoot().append(host)
      let title = try slotContainer("Title")
      _ = try setSlot(host, "title", title)
      _ = try setSlot(host, "title", title)
      #expect(try getSlot(host, "title")?.is(title) == true)
      #expect(try getSlotNames(host) == ["title"])
      #expect(try getSlotNameWithinHost(title) == "title")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > declared slots set in reverse call order come back in declared order"
    ))
  func declaredOrder() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try create(DeclaredHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "body", try slotContainer("Body"))
      _ = try setSlot(host, "title", try slotContainer("Title"))
      hostKey = host.getKey()
    }
    try editor.read { () throws in
      let host: LexicalNode = try node(hostKey)
      #expect(try getSlotNames(host) == ["title", "body"])
      #expect(try host.getTextContent() == "TitleBody")
      #expect(try host.getTextContentSize() == 9)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > mixed declared and undeclared names sort declared-first, then code-unit"
    ))
  func mixedOrder() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try create(DeclaredHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "zeta", try slotContainer("Z"))
      _ = try setSlot(host, "body", try slotContainer("B"))
      _ = try setSlot(host, "alpha", try slotContainer("A"))
      _ = try setSlot(host, "title", try slotContainer("T"))
      hostKey = host.getKey()
    }
    try editor.read { () throws in
      let host: LexicalNode = try node(hostKey)
      #expect(try getSlotNames(host) == ["title", "body", "alpha", "zeta"])
      #expect(try host.getTextContent() == "TBAZ")
      #expect(try host.getTextContentSize() == 4)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > an undeclared host orders slot names in code-unit order"
    ))
  func codeUnitOrder() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      _ = try setSlot(host, "b", try slotContainer("1"))
      _ = try setSlot(host, "A", try slotContainer("2"))
      _ = try setSlot(host, "a", try slotContainer("3"))
      hostKey = host.getKey()
    }
    try editor.read { () throws in
      let host: LexicalNode = try node(hostKey)
      #expect(try getSlotNames(host) == ["A", "a", "b"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a subclass redeclaration overrides the inherited order"
    ))
  func subclassOrder() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try create(ReorderedHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "title", try slotContainer("Title"))
      _ = try setSlot(host, "body", try slotContainer("Body"))
      hostKey = host.getKey()
    }
    try editor.read { () throws in
      let host: LexicalNode = try node(hostKey)
      #expect(try getSlotNames(host) == ["body", "title"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > exportJSON emits slots keys in canonical order"
    ))
  func exportSlotsInCanonicalOrder() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try create(DeclaredHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "body", try slotContainer("Body"))
      _ = try setSlot(host, "alpha", try slotContainer("Alpha"))
      _ = try setSlot(host, "title", try slotContainer("Title"))
    }
    let root = try #require(try editor.getEditorState().toJSON()["root"]?.objectValue)
    let hostJSON = try #require(root["children"]?.arrayValue?.first?.objectValue)
    let slots = try #require(hostJSON["$slots"]?.objectValue)
    #expect(slots.keys == ["title", "body", "alpha"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a duplicate name in a declaration throws when the rank is first computed"
    ))
  func duplicateDeclarationThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try create(DupDeclaredHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "a", try slotContainer("A"))
      expectThrows("more than once") { _ = try setSlot(host, "b", try slotContainer("B")) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: canonical slot order > a reserved name in a declaration throws when the rank is first computed"
    ))
  func reservedDeclarationThrows() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) {
      let host = try create(ReservedDeclaredHostNode.self)
      try getRoot().append(host)
      _ = try setSlot(host, "a", try slotContainer("A"))
      expectThrows("reserved slot name") { _ = try setSlot(host, "b", try slotContainer("B")) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: copy-on-write slot map > a host version cloned without slot changes shares the slot map"
    ))
  func cloneSharesSlotMap() throws {
    // SlotMap is a value type, so "shares" is equality here.
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      _ = try setSlot(host, "title", try slotContainer("Title"))
      hostKey = host.getKey()
    }
    let slotsOf = { () throws -> SlotMap? in
      try editor.getEditorState().read { (try node(hostKey) as ParagraphNode).__slots }
    }
    let mapBefore = try slotsOf()
    #expect(mapBefore != nil)
    try editor.update(discrete: true) {
      _ = try (try node(hostKey) as ParagraphNode).setStyle("color: red")
    }
    #expect(try slotsOf() == mapBefore)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: copy-on-write slot map > a slot mutation clones the map once per version and leaves prior versions intact"
    ))
  func slotMutationLeavesPriorVersions() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      _ = try setSlot(host, "a", try slotContainer("A"))
      hostKey = host.getKey()
    }
    let stateA = editor.getEditorState()
    let mapA = try stateA.read { (try node(hostKey) as ParagraphNode).__slots }
    try editor.update(discrete: true) {
      let host: ParagraphNode = try node(hostKey)
      _ = try setSlot(host, "b", try slotContainer("B"))
      _ = try setSlot(host, "c", try slotContainer("C"))
    }
    #expect(try stateA.read { try getSlotNames(try node(hostKey) as ParagraphNode) } == ["a"])
    #expect(try stateA.read { (try node(hostKey) as ParagraphNode).__slots } == mapA)
    #expect(mapA?.count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > typing and inline insertNodes land inside the value"
    ))
  func typingLandsInsideValue() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) {
      let (_, line) = try createLineSlotHost()
      lineKey = line.getKey()
      let selection = try (try line.getFirstChild() as! TextNode).select(5, 5)
      try selection.insertText("!")
      try selection.insertNodes([try createTextNode("?")])
    }
    try editor.read { () throws in
      #expect(try (try node(lineKey) as ParagraphNode).getTextContent() == "Title!?")
      #expect(try getRoot().getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > Enter inside the value is a no-op (single-block scope)"
    ))
  func enterInsideValueIsNoOp() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    var hostKey = ""
    try editor.update(discrete: true) { () throws in
      let (host, line) = try createLineSlotHost()
      lineKey = line.getKey()
      hostKey = host.getKey()
      #expect(try (try line.getFirstChild() as! TextNode).select(2, 2).insertParagraph() == nil)
    }
    try editor.read { () throws in
      let line: ParagraphNode = try node(lineKey)
      #expect(try line.getTextContent() == "Title")
      #expect(try getSlotHost(line)?.getKey() == hostKey)
      #expect(try getRoot().getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > multi-block paste flattens to inline content like an <input>"
    ))
  func multiBlockPasteFlattens() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) {
      let (_, line) = try createLineSlotHost()
      lineKey = line.getKey()
      try (try line.getFirstChild() as! TextNode).select(5, 5).insertNodes([
        try createParagraphNode().append(
          try createTextNode("A"), try createLineBreakNode(), try createTextNode("B")),
        try createParagraphNode().append(try createTextNode("C")),
        try createTestDecoratorNode().setIsInline(false),
      ])
    }
    try editor.read { () throws in
      #expect(try (try node(lineKey) as ParagraphNode).getTextContent() == "TitleABC")
      #expect(try getRoot().getChildrenSize() == 1)
      #expect(try getRoot().getTextContent() == "TitleABCbody")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > paste into an EMPTY block value inserts without seeding a paragraph"
    ))
  func pasteIntoEmptyValue() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      try getRoot().append(host)
      let line = try createParagraphNode()
      _ = try setSlot(host, "title", line)
      lineKey = line.getKey()
      try line.select().insertNodes([try createParagraphNode().append(try createTextNode("pasted"))]
      )
    }
    try editor.read { () throws in
      let line: ParagraphNode = try node(lineKey)
      #expect(try line.getTextContent() == "pasted")
      #expect(try line.getFirstChild() is TextNode)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > repeated linebreaks into a bare block value stay in insertion order (#8897)"
    ))
  func repeatedLineBreaksInOrder() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) {
      let (_, line) = try createLineSlotHost()
      lineKey = line.getKey()
      try (try line.getFirstChild() as! TextNode).select(5, 5).insertLineBreak()
    }
    try editor.update(discrete: true) {
      for _ in 0..<3 {
        let line: ParagraphNode = try node(lineKey)
        try (try #require(try line.getLastChild())).selectEnd().insertLineBreak()
      }
    }
    try editor.read { () throws in
      let children = try (try node(lineKey) as ParagraphNode).getChildren()
      #expect(try children[0].getTextContent() == "Title")
      #expect(children.filter { $0 is LineBreakNode }.count == 4)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > insertNodes at the end of a bare block value inserts at the end, not the start"
    ))
  func insertNodesAtValueEnd() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) { lineKey = try createLineSlotHost().1.getKey() }
    try editor.update(discrete: true) {
      let line: ParagraphNode = try node(lineKey)
      let size = try line.getChildrenSize()
      try line.select(size, size).insertNodes([try createTextNode("!")])
    }
    #expect(
      try editor.read { try (try node(lineKey) as ParagraphNode).getTextContent() } == "Title!")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > backspace deletes trailing linebreaks in a bare block value one at a time (#8898)"
    ))
  func backspaceDeletesLineBreaksOneAtATime() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) {
      let (_, line) = try createLineSlotHost()
      lineKey = line.getKey()
      try (try line.getFirstChild() as! TextNode).select(5, 5).insertLineBreak()
      try line.getLastChild()!.selectEnd().insertLineBreak()
    }
    let lineBreaks = { () throws -> Int in
      try editor.read {
        try (try node(lineKey) as ParagraphNode).getChildren().filter { $0 is LineBreakNode }.count
      }
    }
    #expect(try lineBreaks() == 2)
    for expected in [1, 0] {
      try editor.update(discrete: true) {
        try (try node(lineKey) as ParagraphNode).getLastChild()!.selectEnd().deleteCharacter(true)
      }
      #expect(try lineBreaks() == expected)
    }
    #expect(
      try editor.read { try (try node(lineKey) as ParagraphNode).getTextContent() } == "Title")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > the boundary still scopes selection and select-all"
    ))
  func boundaryScopesSelectAll() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) { () throws in
      let (_, line) = try createLineSlotHost()
      lineKey = line.getKey()
      let text = try line.getFirstChild() as! TextNode
      #expect(try text.getTopLevelElement()?.is(line) == true)
      let scoped = try selectAll(try text.select(2, 2))
      #expect(try scoped.getTextContent() == "Title")
      #expect(try getSlotFrame(try scoped.anchor.getNode())?.is(line) == true)
      #expect(try getSlotFrame(try scoped.focus.getNode())?.is(line) == true)
    }
    try editor.read { () throws in
      let line: ParagraphNode = try node(lineKey)
      #expect(try getSlotFrame(line)?.is(line) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > $getNearestRootOrShadowRoot stops at the slot value"
    ))
  func nearestRootStopsAtSlotValue() throws {
    let editor = try createSlotEditor()
    try editor.update(discrete: true) { () throws in
      let (_, line) = try createLineSlotHost()
      let text = try #require(try line.getFirstChild())
      #expect(try getNearestRootOrShadowRoot(text).is(line))
      #expect(try getNearestRootOrShadowRoot(line).is(line))
      let host2 = try createParagraphNode()
      try getRoot().append(host2)
      let container = try slotContainer("inside")
      _ = try setSlot(host2, "media", container)
      let innerParagraph = try #require(try container.getFirstChild() as? ParagraphNode)
      #expect(try getNearestRootOrShadowRoot(innerParagraph).is(container))
      #expect(try getNearestRootOrShadowRoot(container).is(container))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: block slot values (virtual shadow root) > backspace at the start of the value stays inside the boundary"
    ))
  func backspaceAtValueStart() throws {
    let editor = try createSlotEditor()
    var lineKey = ""
    try editor.update(discrete: true) { lineKey = try createLineSlotHost().1.getKey() }
    try editor.update(discrete: true) {
      let line: ParagraphNode = try node(lineKey)
      try (try line.getFirstChild() as! TextNode).select(0, 0).deleteCharacter(true)
    }
    try editor.read { () throws in
      #expect(try (try node(lineKey) as ParagraphNode).getTextContent() == "Title")
      #expect(try getRoot().getTextContent() == "Titlebody")
      #expect(try getRoot().getChildrenSize() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: hydrate-time normalize (#8712) > a slot value with a raw TextNode child gets a paragraph wrap on setEditorState"
    ))
  func hydrateWrapsRawText() throws {
    let editor = try createSlotEditor()
    let shadowType = TestShadowRootNode.getType()
    let serialized = """
      {"root":{"children":[{"$slots":{"title":{"children":[{"detail":0,"format":0,\
      "mode":"normal","style":"","text":"raw","type":"text","version":1}],"direction":null,\
      "format":"","indent":0,"type":"\(shadowType)","version":1}},"children":[],\
      "direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"",\
      "type":"paragraph","version":1}],"direction":null,"format":"","indent":0,\
      "type":"root","version":1}}
      """
    try editor.setEditorState(try editor.parseEditorState(serialized))
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let slot = try #require(try getSlot(host, "title") as? TestShadowRootNode)
      let children = try slot.getChildren()
      #expect(children.count == 1)
      let firstChild = try #require(children.first as? ParagraphNode)
      #expect(try firstChild.getTextContent() == "raw")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: typing-path paragraph wrap (#8712) > element-mode caret before a decorator wraps the inserted text in a paragraph"
    ))
  func typingBeforeDecoratorWraps() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      try slot.append(try createTestDecoratorNode().setIsInline(false))
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
    }
    _ = hostKey
    try editor.update(discrete: true) {
      try (try node(slotKey) as ElementNode).select(0, 0).insertText("typed")
    }
    try editor.read { () throws in
      let children = try (try node(slotKey) as ElementNode).getChildren()
      let firstChild = try #require(children.first as? ParagraphNode)
      #expect(try firstChild.getTextContent() == "typed")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: insertNodes redirect termination (#8712) > seeds a paragraph when the slot value starts with a decorator"
    ))
  func insertNodesSeedsParagraph() throws {
    let editor = try createSlotEditor()
    var hostKey = ""
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try createTestShadowRootNode()
      try slot.append(try createTestDecoratorNode().setIsInline(false))
      try getRoot().append(host)
      _ = try setSlot(host, "title", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
    }
    try editor.update(discrete: true) {
      try (try node(slotKey) as ElementNode).select(0, 0).insertNodes([
        try createTextNode("inserted")
      ])
    }
    try editor.read { () throws in
      let slot: ElementNode = try node(slotKey)
      #expect(try slot.getChildren().count >= 2)
      #expect(try slot.getTextContent().contains("inserted"))
      let host: ParagraphNode = try node(hostKey)
      #expect(try getSlot(host, "title")?.is(slot) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline decorator host that ends a paragraph of four children"
    ))
  func inlineDecoratorHostEndsParagraph() throws {
    let editor = try createSlotEditor()
    var textKey = ""
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(
        try createTextNode("ab"), try createLineBreakNode(), try createTextNode("cd "))
      try getRoot().append(paragraph)
      textKey = try appendHostWithSlot(paragraph, try create(SlotTextHostNode.self)).getKey()
    }
    for text in ["CD", "CDE", "CDEF"] {
      try editSlot(editor, textKey, text)
      #expect(try rootText(editor) == "ab\ncd \(text)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a block decorator host that is the last of four blocks"
    ))
  func blockDecoratorHostLastBlock() throws {
    let editor = try createSlotEditor()
    var textKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      for text in ["a", "b", "c"] {
        try root.append(try createParagraphNode().append(try createTextNode(text)))
      }
      textKey = try appendHostWithSlot(
        root, try create(SlotTextHostNode.self).setIsInline(false)
      ).getKey()
    }
    for text in ["CD", "CDE"] {
      try editSlot(editor, textKey, text)
      #expect(try rootText(editor) == "a\n\nb\n\nc\n\n\(text)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a text appended after an edited decorator host"
    ))
  func textAppendedAfterHost() throws {
    let editor = try createSlotEditor()
    var keys: (host: NodeKey, text: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(
        try createTextNode("ab"), try createLineBreakNode(), try createTextNode("cd "))
      try getRoot().append(paragraph)
      let host = try create(SlotTextHostNode.self)
      keys = (host.getKey(), try appendHostWithSlot(paragraph, host).getKey())
    }
    try editSlot(editor, keys.text, "CD")
    try editor.update(discrete: true) {
      _ = try getNodeByKey(keys.host)!.insertAfter(try createTextNode(" ef"))
    }
    #expect(try rootText(editor) == "ab\ncd CD ef")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > a text removed after an edited decorator host"
    ))
  func textRemovedAfterHost() throws {
    let editor = try createSlotEditor()
    var keys: (after: NodeKey, text: NodeKey) = ("", "")
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(
        try createTextNode("ab"), try createLineBreakNode(), try createTextNode("cd "))
      try getRoot().append(paragraph)
      let text = try appendHostWithSlot(paragraph, try create(SlotTextHostNode.self))
      let after = try createTextNode(" ef")
      try paragraph.append(after)
      keys = (after.getKey(), text.getKey())
    }
    try editSlot(editor, keys.text, "CD")
    #expect(try rootText(editor) == "ab\ncd CD ef")
    try editor.update(discrete: true) { try getNodeByKey(keys.after)!.remove() }
    #expect(try rootText(editor) == "ab\ncd CD")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline decorator host that ends a paragraph of three children"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: root text after repeated slot edits > an inline element host that ends a paragraph of four children"
    ),
    arguments: [false, true])
  func inlineHostEndsParagraph(elementHost: Bool) throws {
    let editor = try createSlotEditor()
    var textKey = ""
    try editor.update(discrete: true) {
      let before: [LexicalNode] =
        elementHost
        ? [try createTextNode("ab"), try createLineBreakNode(), try createTextNode("cd ")]
        : [try createTextNode("ab"), try createTextNode("cd ")]
      let paragraph = try createParagraphNode()
      for child in before { try paragraph.append(child) }
      try getRoot().append(paragraph)
      let host: LexicalNode =
        elementHost ? try createTestInlineElementNode() : try create(SlotTextHostNode.self)
      textKey = try appendHostWithSlot(paragraph, host).getKey()
    }
    let prefix = elementHost ? "ab\ncd " : "abcd "
    for text in ["CD", "CDE", "CDEF"] {
      try editSlot(editor, textKey, text)
      #expect(try rootText(editor) == prefix + text)
    }
  }
}

/// The slot tests of LexicalSlot.test.ts that assert the reconciled DOM,
/// reworked to assert what the model reconciler keeps: the key map
/// (`getElementByKey`) and the root's cached text.
@Suite struct LexicalSlotReconcilerTests {
  /// `createSlotEditor()`, mounted so commits run the reconciler.
  func createMountedSlotEditor() throws -> Editor {
    let editor = try createSlotEditor()
    try editor.setRootElement(TestRootElement())
    return editor
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > a host with only a slot renders the slot and caches its text"
    ))
  func hostWithOnlySlotCachesText() throws {
    let editor = try createMountedSlotEditor()
    var titleKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Title")
      try getRoot().append(host)
      try setSlot(host, "title", title)
      titleKey = title.getKey()
    }
    #expect(editor._keyToDOMMap.get(titleKey) != nil)
    try editor.read { () throws in
      #expect(try getRoot().getFirstChildOrThrow().getTextContent() == "Title")
      // The childless host still contributes its slot text to the cache.
      #expect(try getRoot().getTextContent() == "Title")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing a slot drops its container and its text from the cache"
    ))
  func removingSlotDropsItsText() throws {
    let editor = try createMountedSlotEditor()
    var hostKey = ""
    var titleKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let title = try slotContainer("Title")
      let body = try createParagraphNode().append(try createTextNode("Body"))
      try getRoot().append(host)
      try host.append(body)
      try setSlot(host, "title", title)
      hostKey = host.getKey()
      titleKey = title.getKey()
    }
    try editor.update(discrete: true) {
      _ = try removeSlot(try #require(try getNodeByKey(hostKey)), "title")
    }
    #expect(editor._keyToDOMMap.get(titleKey) == nil)
    try editor.read { () throws in
      #expect(try #require(try getNodeByKey(hostKey)).getTextContent() == "Body")
      #expect(try getRoot().getTextContent() == "Body")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing the host last child keeps its slot containers in the DOM"
    ))
  func removingLastChildKeepsSlot() throws {
    let editor = try createMountedSlotEditor()
    var childKey = ""
    var hostKey = ""
    var titleKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let child = try createTextNode("A")
      try host.append(child)
      let title = try slotContainer("TT")
      try setSlot(host, "title", title)
      try getRoot().append(host)
      childKey = child.getKey()
      hostKey = host.getKey()
      titleKey = title.getKey()
    }
    try editor.update(discrete: true) { try #require(try getNodeByKey(childKey)).remove() }
    try editor.read { () throws in
      let host = try #require(try getNodeByKey(hostKey))
      #expect(try getSlotNames(host) == ["title"])
      #expect(try host.getTextContent() == "TT")
      #expect(try getRoot().getTextContent() == "TT")
    }
    #expect(editor._keyToDOMMap.get(titleKey) != nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing an element host clears its slot subtree from the DOM map"
    ))
  func removingElementHostClearsSlotSubtree() throws {
    let editor = try createMountedSlotEditor()
    var hostKey = ""
    var slotKey = ""
    var innerKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      try setSlot(host, "title", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
      innerKey = try slot.getFirstChildOrThrow().getKey()
    }
    #expect(editor._keyToDOMMap.get(slotKey) != nil)
    try editor.update(discrete: true) { try #require(try getNodeByKey(hostKey)).remove() }
    #expect(editor._keyToDOMMap.get(slotKey) == nil)
    #expect(editor._keyToDOMMap.get(innerKey) == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlot.test.ts::named-slots: core foundation > removing a decorator host clears its slot subtree from the DOM map"
    ))
  func removingDecoratorHostClearsSlotSubtree() throws {
    let editor = try createMountedSlotEditor()
    var hostKey = ""
    var slotKey = ""
    try editor.update(discrete: true) {
      let host = try createTestDecoratorNode().setIsInline(false)
      let slot = try slotContainer("SLOTTEXT")
      try getRoot().append(host)
      try setSlot(host, "media", slot)
      hostKey = host.getKey()
      slotKey = slot.getKey()
    }
    #expect(editor._keyToDOMMap.get(slotKey) != nil)
    try editor.update(discrete: true) { try #require(try getNodeByKey(hostKey)).remove() }
    #expect(editor._keyToDOMMap.get(slotKey) == nil)
  }
}
