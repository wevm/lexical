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
}
