import Lexical
import LexicalTesting
import Testing

// Port of src/__tests__/unit/LexicalSlotSelection.test.ts: the programmatic
// selections, which a commit clamps to one slot frame. The tests that drag
// a DOM selection are skipped as dom-only. Upstream mounts each editor; the
// clamping happens in the model, so the editors here are headless.

/// `$slotFrameKeyOf`: the nearest node that is a slot value.
private func slotFrameKeyOf(_ node: LexicalNode) throws -> NodeKey? {
  var current: LexicalNode? = node
  while let node = current {
    if try getSlotHost(node) != nil {
      return node.getKey()
    }
    current = try node.getParent()
  }
  return nil
}

private typealias Keys = [String: NodeKey]

/// `buildSlotEditor`: a host paragraph with `title` and `subtitle` slots and
/// a body.
private func buildSlotEditor() throws -> (Editor, Keys) {
  let editor = try createTestEditor()
  var keys = Keys()
  try editor.update(discrete: true) {
    let host = try createParagraphNode()
    let title = try createTestShadowRootNode()
    let titleText = try createTextNode("Title")
    try title.append(try createParagraphNode().append(titleText))
    let subtitle = try createTestShadowRootNode()
    let subtitleText = try createTextNode("Subtitle")
    try subtitle.append(try createParagraphNode().append(subtitleText))
    let body = try createParagraphNode()
    let bodyText = try createTextNode("Body")
    try body.append(bodyText)
    try getRoot().clear().append(host)
    try host.append(body)
    _ = try setSlot(host, "title", title)
    _ = try setSlot(host, "subtitle", subtitle)
    keys = [
      "host": host.getKey(), "title": title.getKey(), "titleText": titleText.getKey(),
      "subtitle": subtitle.getKey(), "subtitleText": subtitleText.getKey(),
      "body": body.getKey(), "bodyText": bodyText.getKey(),
    ]
  }
  return (editor, keys)
}

/// `buildRootSlotEditor`: a body and a `footnotes` slot on the root.
private func buildRootSlotEditor() throws -> (Editor, Keys) {
  let editor = try createTestEditor()
  var keys = Keys()
  try editor.update(discrete: true) {
    let bodyText = try createTextNode("Body")
    let footnotes = try createTestShadowRootNode()
    let footnoteText = try createTextNode("Footnote")
    try footnotes.append(try createParagraphNode().append(footnoteText))
    let root = try getRoot().clear()
    try root.append(try createParagraphNode().append(bodyText))
    _ = try setSlot(root, "footnotes", footnotes)
    keys = [
      "bodyText": bodyText.getKey(), "footnotes": footnotes.getKey(),
      "footnoteText": footnoteText.getKey(),
    ]
  }
  return (editor, keys)
}

/// `buildNestedSlotEditor`: a card with `title` and `aside` slots, a
/// heading in the title with an `inner` slot, and a `caption` slot on the
/// title.
private func buildNestedSlotEditor() throws -> (Editor, Keys) {
  let editor = try createTestEditor()
  var keys = Keys()
  try editor.update(discrete: true) {
    let bodyText = try createTextNode("Body")
    let card = try createParagraphNode()
    let title = try createTestShadowRootNode()
    let heading = try createParagraphNode().append(try createTextNode("Heading"))
    let deep = try createTestShadowRootNode()
    let deepText = try createTextNode("Deep")
    try deep.append(try createParagraphNode().append(deepText))
    let caption = try createTestShadowRootNode()
    let captionText = try createTextNode("Caption")
    try caption.append(try createParagraphNode().append(captionText))
    try getRoot().clear().append(try createParagraphNode().append(bodyText), card)
    let introText = try createTextNode("Intro")
    try title.append(
      try createParagraphNode().append(introText), heading,
      try createParagraphNode().append(try createTextNode("Outro")))
    _ = try setSlot(card, "title", title)
    _ = try setSlot(heading, "inner", deep)
    _ = try setSlot(title, "caption", caption)
    let aside = try createTestShadowRootNode()
    let asideText = try createTextNode("Aside")
    try aside.append(try createParagraphNode().append(asideText))
    _ = try setSlot(card, "aside", aside)
    keys = [
      "aside": aside.getKey(), "asideText": asideText.getKey(),
      "captionText": captionText.getKey(), "bodyText": bodyText.getKey(),
      "title": title.getKey(), "introText": introText.getKey(), "deep": deep.getKey(),
      "deepText": deepText.getKey(),
    ]
  }
  return (editor, keys)
}

/// Sets a selection from `anchor` to `focus` and returns the active one.
private func setAndGet(
  _ anchor: (NodeKey, Int, PointType), _ focus: (NodeKey, Int, PointType)
) throws -> RangeSelection {
  let selection = createRangeSelection()
  try selection.anchor.set(anchor.0, anchor.1, anchor.2)
  try selection.focus.set(focus.0, focus.1, focus.2)
  try setSelection(selection)
  return try #require(try getSelection() as? RangeSelection)
}

struct SlotSelectionCase: Sendable, CustomTestStringConvertible {
  let name: String
  let anchor: (String, Int, PointType)
  let focus: (String, Int, PointType)
  let expected: (String, Int)
  var testDescription: String { name }

  static let host: [SlotSelectionCase] = [
    SlotSelectionCase(
      name: "from just before a host into its slot pushes focus past the host",
      anchor: ("root", 0, .element), focus: ("titleText", 2, .text), expected: ("root", 1)),
    SlotSelectionCase(
      name: "from a slot to just before its host clamps focus to the slot start",
      anchor: ("titleText", 2, .text), focus: ("root", 0, .element), expected: ("title", 0)),
  ]
  static let nested: [SlotSelectionCase] = [
    SlotSelectionCase(
      name: "from a nested slot into an earlier sibling of its outer slot",
      anchor: ("deepText", 1, .text), focus: ("asideText", 2, .text), expected: ("deep", 0)),
    SlotSelectionCase(
      name: "from the end of the root into a nested slot",
      anchor: ("root", 2, .element), focus: ("deepText", 2, .text), expected: ("root", 1)),
    SlotSelectionCase(
      name: "from a nested slot to the end of the root",
      anchor: ("deepText", 1, .text), focus: ("root", 2, .element), expected: ("deep", 1)),
  ]
}

private func runCase(_ testCase: SlotSelectionCase, _ editor: Editor, _ keys: Keys) throws {
  try editor.update(discrete: true) { () throws in
    let keyOf = { (name: String) throws -> NodeKey in
      name == "root" ? try getRoot().getKey() : keys[name]!
    }
    let active = try setAndGet(
      (try keyOf(testCase.anchor.0), testCase.anchor.1, testCase.anchor.2),
      (try keyOf(testCase.focus.0), testCase.focus.1, testCase.focus.2))
    #expect(active.anchor.key == (try keyOf(testCase.anchor.0)))
    #expect(active.focus.key == (try keyOf(testCase.expected.0)))
    #expect(active.focus.offset == testCase.expected.1)
  }
}

@Suite struct LexicalSlotSelectionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection with anchor in a slot, focus in the body, clamps focus into the slot"
    ))
  func slotToBodyClampsIntoSlot() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet((keys["titleText"]!, 2, .text), (keys["bodyText"]!, 2, .text))
      #expect(try slotFrameKeyOf(try active.anchor.getNode()) == keys["title"])
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == keys["title"])
      #expect(active.focus.key != keys["bodyText"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection with anchor in the body, focus in a slot of the host containing it, clamps focus to the start of the host"
    ))
  func bodyToSlotClampsToHostStart() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet((keys["bodyText"]!, 1, .text), (keys["titleText"]!, 1, .text))
      #expect(try slotFrameKeyOf(try active.anchor.getNode()) == nil)
      #expect(active.focus.key == keys["host"])
      #expect(active.focus.offset == 0)
      #expect(try active.getTextContent() == "B")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from just before a host into its slot p…"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from a slot to just before its host cla…"
    ),
    arguments: SlotSelectionCase.host)
  func hostEdgeCases(_ testCase: SlotSelectionCase) throws {
    let (editor, keys) = try buildSlotEditor()
    try runCase(testCase, editor, keys)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection from a slot to a removed node clamps focus to the slot edge"
    ))
  func slotToRemovedNodeClampsToSlotEdge() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let selection = createRangeSelection()
      try selection.anchor.set(keys["titleText"]!, 1, .text)
      try selection.focus.set(keys["bodyText"]!, 1, .text)
      try (try getNodeByKey(keys["bodyText"]!) as! TextNode).remove()
      try setSelection(selection)
      let active = try #require(try getSelection() as? RangeSelection)
      #expect(active.anchor.key == keys["titleText"])
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == keys["title"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection entirely within the body is left untouched"
    ))
  func bodySelectionUntouched() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      _ = try (try getNodeByKey(keys["bodyText"]!) as! TextNode).select(1, 3)
      let active = try #require(try getSelection() as? RangeSelection)
      #expect(active.anchor.key == keys["bodyText"])
      #expect(active.anchor.offset == 1)
      #expect(active.focus.key == keys["bodyText"])
      #expect(active.focus.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > an in-place point mutation that straddles a slot is clamped at commit"
    ))
  func inPlaceMutationClampedAtCommit() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) {
      _ = try (try getNodeByKey(keys["bodyText"]!) as! TextNode).select(1, 2)
    }
    try editor.update(discrete: true) {
      let active = try #require(try getSelection() as? RangeSelection)
      try active.focus.set(keys["titleText"]!, 1, .text)
    }
    try editor.read { () throws in
      let active = try #require(try getSelection() as? RangeSelection)
      #expect(try slotFrameKeyOf(try active.anchor.getNode()) == nil)
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == nil)
      #expect(active.focus.key != keys["titleText"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > $selectAll from inside a slot scopes to the slot, not the whole document"
    ))
  func selectAllInsideSlot() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let selection = try (try getNodeByKey(keys["titleText"]!) as! TextNode).select(1, 1)
      let out = try selectAll(selection)
      #expect(try slotFrameKeyOf(try out.anchor.getNode()) == keys["title"])
      #expect(try slotFrameKeyOf(try out.focus.getNode()) == keys["title"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > $selectAll from outside any slot still scopes to the whole document"
    ))
  func selectAllOutsideSlots() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let selection = try (try getNodeByKey(keys["bodyText"]!) as! TextNode).select(1, 1)
      let out = try selectAll(selection)
      #expect(try slotFrameKeyOf(try out.anchor.getNode()) == nil)
      #expect(try slotFrameKeyOf(try out.focus.getNode()) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection containment (slot isolation) > a programmatic selection across two slots clamps focus into the anchor slot"
    ))
  func acrossTwoSlotsClampsIntoAnchorSlot() throws {
    let (editor, keys) = try buildSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet(
        (keys["titleText"]!, 1, .text), (keys["subtitleText"]!, 1, .text))
      #expect(try slotFrameKeyOf(try active.anchor.getNode()) == keys["title"])
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == keys["title"])
      #expect(active.focus.key != keys["subtitleText"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across a root slot > a programmatic selection with anchor in the body, focus in a root slot, clamps focus to the root edge"
    ))
  func bodyToRootSlotClampsToRootEdge() throws {
    let (editor, keys) = try buildRootSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet(
        (keys["bodyText"]!, 1, .text), (keys["footnoteText"]!, 3, .text))
      #expect(active.anchor.key == keys["bodyText"])
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == nil)
      #expect(active.focus.key == (try getRoot().getKey()))
      #expect(active.focus.offset == 0)
      #expect(try active.getTextContent() == "B")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the body into nested slot deepText pushes focus past the outer host"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the body into nested slot captionText pushes focus past the outer host"
    ),
    arguments: ["deepText", "captionText"])
  func bodyToNestedSlotPushesPastOuterHost(focusKey: String) throws {
    let (editor, keys) = try buildNestedSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet((keys["bodyText"]!, 1, .text), (keys[focusKey]!, 2, .text))
      #expect(active.anchor.key == keys["bodyText"])
      #expect(try slotFrameKeyOf(try active.focus.getNode()) == nil)
      #expect(active.focus.key == (try getRoot().getKey()))
      #expect(active.focus.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a slot into a slot nested in it pushes focus past the host in the anchor slot"
    ))
  func slotIntoNestedSlotPushesPastHost() throws {
    let (editor, keys) = try buildNestedSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet((keys["introText"]!, 1, .text), (keys["deepText"]!, 2, .text))
      #expect(active.anchor.key == keys["introText"])
      #expect(active.focus.key == keys["title"])
      #expect(active.focus.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a slot into its own slot value's slot clamps focus to the slot edge"
    ))
  func slotIntoOwnValueSlotClampsToEdge() throws {
    let (editor, keys) = try buildNestedSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet(
        (keys["introText"]!, 1, .text), (keys["captionText"]!, 2, .text))
      #expect(active.anchor.key == keys["introText"])
      #expect(active.focus.key == keys["title"])
      #expect(active.focus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot into an earlier sibl…"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from the end of the root into a nested …"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot to the end of the ro…"
    ),
    arguments: SlotSelectionCase.nested)
  func nestedEdgeCases(_ testCase: SlotSelectionCase) throws {
    let (editor, keys) = try buildNestedSlotEditor()
    try runCase(testCase, editor, keys)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: selection across nested slots > a programmatic selection from a nested slot into the body clamps focus into the slot"
    ))
  func nestedSlotToBodyClampsIntoSlot() throws {
    let (editor, keys) = try buildNestedSlotEditor()
    try editor.update(discrete: true) { () throws in
      let active = try setAndGet((keys["deepText"]!, 1, .text), (keys["bodyText"]!, 1, .text))
      #expect(active.anchor.key == keys["deepText"])
      #expect(active.focus.key == keys["deep"])
      #expect(active.focus.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: Point.set rejects decorator key targets > Point.set rejects decorator key with text type"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalSlotSelection.test.ts::named-slots: Point.set rejects decorator key targets > Point.set rejects decorator key with element type"
    ),
    arguments: [PointType.text, .element])
  func pointSetRejectsDecoratorKey(type: PointType) throws {
    let editor = try createTestEditor()
    var decoratorKey: NodeKey = ""
    try editor.update(discrete: true) {
      let host = try createParagraphNode()
      let decoratorSlot = try createTestDecoratorNode().setIsInline(false)
      let body = try createParagraphNode()
      try body.append(try createTextNode("Body"))
      try getRoot().clear().append(host)
      try host.append(body)
      _ = try setSlot(host, "media", decoratorSlot)
      decoratorKey = decoratorSlot.getKey()
    }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      #expect(throws: (any Error).self) { try selection.anchor.set(decoratorKey, 0, type) }
    }
  }
}
