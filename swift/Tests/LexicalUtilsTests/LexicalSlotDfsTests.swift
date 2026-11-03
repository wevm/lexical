import Lexical
import LexicalExtension
import LexicalTesting
import LexicalUtils
import Testing

// Ports of LexicalSlotDfs.test.ts and LexicalWrapNodeInElementSlots.test.ts
// in packages/lexical-utils/src/__tests__/unit.

private struct Visit: Equatable {
  let depth: Int
  let key: NodeKey
}

private func visits(_ nodes: [DFSNode]) -> [Visit] {
  nodes.map { Visit(depth: $0.depth, key: $0.node.getKey()) }
}

private func slotAwareDfs(_ depth: Int, _ node: LexicalNode) throws -> [Visit] {
  var out = [Visit(depth: depth, key: node.getKey())]
  if let element = node as? ElementNode {
    for name in try getSlotNames(element) {
      if let slot = try getSlot(element, name) { out += try slotAwareDfs(depth + 1, slot) }
    }
    for child in try element.getChildren() { out += try slotAwareDfs(depth + 1, child) }
  }
  return out
}

private func reverseSlotAwareDfs(_ depth: Int, _ node: LexicalNode) throws -> [Visit] {
  var out = [Visit(depth: depth, key: node.getKey())]
  if let element = node as? ElementNode {
    for child in try element.getChildren().reversed() {
      out += try reverseSlotAwareDfs(depth + 1, child)
    }
    for name in try getSlotNames(element).reversed() {
      if let slot = try getSlot(element, name) { out += try reverseSlotAwareDfs(depth + 1, slot) }
    }
  }
  return out
}

private final class Keys {
  var values: [String: NodeKey] = [:]
  subscript(_ name: String) -> NodeKey { values[name]! }
}

private func slotTreeEditor(_ keys: Keys) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[slot-dfs]", nodes: { [.klass(TestShadowRootNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          let host = try createParagraphNode()
          let title = try createTestShadowRootNode()
          let titlePara = try createParagraphNode()
          let titleText = try createTextNode("Title")
          try titlePara.append(titleText)
          try title.append(titlePara)
          let body = try createParagraphNode()
          let bodyText = try createTextNode("Body")
          try body.append(bodyText)
          try getRoot().append(host)
          try host.append(body)
          try setSlot(host, "title", title)
          keys.values = [
            "host": host.getKey(), "title": title.getKey(), "titlePara": titlePara.getKey(),
            "titleText": titleText.getKey(), "body": body.getKey(), "bodyText": bodyText.getKey(),
          ]
        })))
}

private func decoratorSlotEditor(_ keys: Keys) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[slot-dfs-decorator]",
      nodes: { [.klass(TestShadowRootNode.self), .klass(TestDecoratorNode.self)] },
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          let host = try TestDecoratorNode().setIsInline(false)
          let media = try createTestShadowRootNode()
          let mediaPara = try createParagraphNode()
          let mediaText = try createTextNode("Eq")
          try mediaPara.append(mediaText)
          try media.append(mediaPara)
          try getRoot().append(host)
          try setSlot(host, "media", media)
          keys.values = [
            "host": host.getKey(), "media": media.getKey(), "mediaPara": mediaPara.getKey(),
            "mediaText": mediaText.getKey(),
          ]
        })))
}

@Suite struct LexicalSlotDfsTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $dfsWithSlots visits slot subtrees, slots-first, ahead of children"
    ))
  func slotsFirst() throws {
    let keys = Keys()
    let editor = try slotTreeEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      #expect(
        try visits(dfsWithSlots(host)) == [
          Visit(depth: 1, key: keys["host"]), Visit(depth: 2, key: keys["title"]),
          Visit(depth: 3, key: keys["titlePara"]), Visit(depth: 4, key: keys["titleText"]),
          Visit(depth: 2, key: keys["body"]), Visit(depth: 3, key: keys["bodyText"]),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $dfsWithSlots slot traversal matches the reference slots-first walk"
    ))
  func matchesReference() throws {
    let keys = Keys()
    let editor = try slotTreeEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      #expect(try visits(dfsWithSlots(host)) == slotAwareDfs(1, host))
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $reverseDfsWithSlots visits slots-last, mirroring $dfsWithSlots (#7112 invariant)"
    ))
  func reverseSlotsLast() throws {
    let keys = Keys()
    let editor = try slotTreeEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      let visited = try visits(reverseDfsWithSlots(host))
      #expect(
        visited == [
          Visit(depth: 1, key: keys["host"]), Visit(depth: 2, key: keys["body"]),
          Visit(depth: 3, key: keys["bodyText"]), Visit(depth: 2, key: keys["title"]),
          Visit(depth: 3, key: keys["titlePara"]), Visit(depth: 4, key: keys["titleText"]),
        ])
      #expect(try visited == reverseSlotAwareDfs(1, host))
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > endNode slot subtrees are not emitted past the inclusive stop"
    ))
  func endNodeInclusiveStop() throws {
    let keys = Keys()
    let editor = try slotTreeEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let root = try getRoot()
      let host = try #require(try root.getFirstChild())
      let truncated = [Visit(depth: 0, key: root.getKey()), Visit(depth: 1, key: keys["host"])]
      #expect(try visits(dfsWithSlots(root, host)) == truncated)
      #expect(try visits(reverseDfsWithSlots(root, host)) == truncated)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > reconciler text cache includes slot text (RootNode.__cachedText)"
    ))
  func textCacheIncludesSlots() throws {
    let keys = Keys()
    let editor = try slotTreeEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let root = try getRoot()
      let host = try #require(try root.getFirstChild())
      #expect(try host.getTextContent() == "TitleBody")
      #expect(try root.getTextContent() == "TitleBody")
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal into a decorator host > $dfsWithSlots descends into a decorator host's slots"
    ))
  func decoratorHostForward() throws {
    let keys = Keys()
    let editor = try decoratorSlotEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      #expect(
        try visits(dfsWithSlots(host)) == [
          Visit(depth: 1, key: keys["host"]), Visit(depth: 2, key: keys["media"]),
          Visit(depth: 3, key: keys["mediaPara"]), Visit(depth: 4, key: keys["mediaText"]),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal into a decorator host > $reverseDfsWithSlots descends into a decorator host's slots"
    ))
  func decoratorHostReverse() throws {
    let keys = Keys()
    let editor = try decoratorSlotEditor(keys)
    defer { editor.dispose() }
    try editor.read { () throws in
      let host = try #require(try getRoot().getFirstChild())
      #expect(
        try visits(reverseDfsWithSlots(host)) == [
          Visit(depth: 1, key: keys["host"]), Visit(depth: 2, key: keys["media"]),
          Visit(depth: 3, key: keys["mediaPara"]), Visit(depth: 4, key: keys["mediaText"]),
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalWrapNodeInElementSlots.test.ts::$wrapNodeInElement and named slots > wrapping a slot host keeps its slots on the host"
    ))
  func wrapKeepsSlots() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[wrap-slots]", nodes: { [.klass(TestShadowRootNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let host = try createParagraphNode()
            let slot = try createTestShadowRootNode()
            try slot.append(try createParagraphNode().append(try createTextNode("Title")))
            try getRoot().append(host)
            try setSlot(host, "title", slot)
          })))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let host = try getRoot().getFirstChildOrThrow()
      let wrapper = try wrapNodeInElement(host, createTestShadowRootNode)
      #expect(try wrapper.getFirstChild()?.is(host) == true)
      #expect(try getSlotNames(wrapper).isEmpty)
      #expect(try getSlot(host, "title") != nil)
    }
    try editor.read { () throws in
      let wrapper = try #require(try getRoot().getFirstChildOrThrow() as? TestShadowRootNode)
      let host = try #require(try wrapper.getFirstChildOrThrow() as? ParagraphNode)
      #expect(try host.isAttached())
      let slot = try #require(try getSlot(host, "title"))
      #expect(try slot.getTextContent() == "Title")
      #expect(try getSlotHost(slot)?.is(host) == true)
    }
  }
}
