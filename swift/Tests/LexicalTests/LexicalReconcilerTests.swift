import LexicalTesting
import Testing

@testable import Lexical

// Port of the model half of src/__tests__/unit/LexicalReconciler.test.ts:
// the cached text, `__textFormat` and first-text-key results of the
// reconciler, on a mounted editor. Upstream builds a rich text editor only
// for its paragraphs; LinkNode is used for structure only, so TestLinkNode
// stands in for it, and TestInlineElementNode for the wrapping inline
// element of AUDIT-4 (its DOM wrapper has no model counterpart). The tests
// of rendered DOM (direction, indent, DOM reuse and wrapping parents) are
// dom-only.

private let BOLD = 1

private func mounted() throws -> Editor { try createMountedTestEditor() }

private func cachedText(_ editor: Editor) throws -> String? {
  try editor.read { try getRoot().__cachedText }
}

private func paragraphs(_ texts: [String]) throws {
  let root = try getRoot().clear()
  for text in texts {
    try root.append(try createParagraphNode().append(try createTextNode(text)))
  }
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
}

private func lastText(_ element: ElementNode) throws -> TextNode {
  try #require(try element.getLastChildOrThrow() as? TextNode)
}

/// `editor.getElementByKey(key).__lexicalFirstTextKey`.
private func firstTextKey(_ editor: Editor, _ key: NodeKey) -> NodeKey? {
  guard let cached = editor._keyToDOMMap.get(key)?.firstTextKey else { return nil }
  return cached
}

private func link(_ text: String, _ format: TextFormatType) throws -> TestLinkNode {
  try createTestLinkNode().append(try createTextNode(text).toggleFormat(format))
}

/// Counts `decorate` calls, as upstream spies on `TestDecoratorNode.decorate`.
private final class CountingDecoratorNode: DecoratorNode {
  nonisolated(unsafe) static var decorateCalls = 0
  override class func config() -> NodeConfig? {
    NodeConfig(CountingDecoratorNode.self, type: "counting_decorator", extends: DecoratorNode.self)
  }
  override func decorate(_ editor: Editor) throws -> Any? {
    CountingDecoratorNode.decorateCalls += 1
    return nil
  }
}

@Suite(.serialized) struct LexicalReconcilerTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > Cross-parent moves reuse DOM (regression #8420) > Wrapping decorator emits a single \"updated\" listener event and re-decorates"
    ))
  func wrappingDecoratorUpdatedOnce() throws {
    let editor = try createMountedTestEditor(nodes: [.klass(CountingDecoratorNode.self)])
    try update(editor) {
      try getRoot().clear().append(try createParagraphNode().append(try CountingDecoratorNode()))
    }
    CountingDecoratorNode.decorateCalls = 0
    var events: [(String, NodeMutation)] = []
    try editor.registerMutationListener(
      CountingDecoratorNode.self,
      { nodes, _ in
        for (_, mutation) in nodes { events.append(("decorator", mutation)) }
      }, skipInitialization: true)
    try editor.registerMutationListener(
      TestElementNode.self,
      { nodes, _ in
        for (_, mutation) in nodes { events.append(("element", mutation)) }
      }, skipInitialization: true)
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ElementNode)
      let decorator = try #require(try paragraph.getFirstChild() as? DecoratorNode)
      let wrapper = try createTestElementNode()
      try decorator.insertBefore(wrapper)
      try wrapper.append(decorator)
    }
    #expect(events.filter { $0.0 == "decorator" }.map(\.1) == ["updated"])
    #expect(CountingDecoratorNode.decorateCalls > 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > typing at the end of the last paragraph keeps prefix DLB"
    ))
  func typingAtEnd() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["alpha", "beta", "gamma"]) }
    #expect(try cachedText(editor) == "alpha\n\nbeta\n\ngamma")
    try editor.update(discrete: true) {
      let text = try lastText(try #require(try getRoot().getLastChildOrThrow() as? ElementNode))
      try text.setTextContent(try text.getTextContent() + "!")
    }
    #expect(try cachedText(editor) == "alpha\n\nbeta\n\ngamma!")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > multiple contiguous dirty children at the end"
    ))
  func multipleContiguousDirty() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["a", "b", "c", "d"]) }
    try editor.update(discrete: true) {
      let root = try getRoot()
      try lastText(try #require(try root.getChildAtIndex(2) as? ElementNode)).setTextContent("cc")
      try lastText(try #require(try root.getChildAtIndex(3) as? ElementNode)).setTextContent("dd")
    }
    #expect(try cachedText(editor) == "a\n\nb\n\ncc\n\ndd")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > non-contiguous dirty children take the existing fast path"
    ))
  func nonContiguousDirty() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["x", "y", "z"]) }
    try editor.update(discrete: true) {
      let root = try getRoot()
      try lastText(try #require(try root.getChildAtIndex(0) as? ElementNode)).setTextContent("xx")
      try lastText(try #require(try root.getChildAtIndex(2) as? ElementNode)).setTextContent("zz")
    }
    #expect(try cachedText(editor) == "xx\n\ny\n\nzz")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > format toggle on the last paragraph propagates to __textFormat"
    ))
  func formatTogglePropagates() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["head", "foot"]) }
    try editor.update(discrete: true) {
      try lastText(try #require(try getRoot().getLastChildOrThrow() as? ElementNode))
        .toggleFormat("bold")
    }
    try editor.read { () throws in
      let root = try getRoot()
      let head = try #require(try root.getFirstChildOrThrow() as? ParagraphNode)
      let foot = try #require(try root.getLastChildOrThrow() as? ParagraphNode)
      #expect(try head.getTextFormat() == 0)
      #expect(try foot.getTextFormat() != 0)
      #expect(root.__cachedText == "head\n\nfoot")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > empty trailing paragraph contributes zero length"
    ))
  func emptyTrailingParagraph() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("hello")), try createParagraphNode())
    }
    #expect(try cachedText(editor) == "hello\n\n")
    try editor.update(discrete: true) {
      try #require(try getRoot().getLastChildOrThrow() as? ElementNode).append(
        try createTextNode("world"))
    }
    #expect(try cachedText(editor) == "hello\n\nworld")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > linebreak-bounded text nodes update suffix without extra DLB"
    ))
  func lineBreakBounded() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("top"), try createLineBreakNode(), try createTextNode("bottom")))
    }
    #expect(try cachedText(editor) == "top\nbottom")
    try editor.update(discrete: true) {
      try lastText(try firstParagraph()).setTextContent("BOTTOM!")
    }
    #expect(try cachedText(editor) == "top\nBOTTOM!")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > TextNode-direct-child suffix with length change: prefix preserved"
    ))
  func textChildLengthChange() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("hello "), try createTextNode("world").toggleFormat("bold")))
    }
    #expect(try cachedText(editor) == "hello world")
    try editor.update(discrete: true) {
      try lastText(try firstParagraph()).setTextContent("world!!")
    }
    #expect(try cachedText(editor) == "hello world!!")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > append paragraph at end of multi-paragraph root (size+1, K=2)"
    ))
  func appendParagraph() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["a", "b", "c", "d", "e"]) }
    #expect(try cachedText(editor) == "a\n\nb\n\nc\n\nd\n\ne")
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("f")))
    }
    #expect(try cachedText(editor) == "a\n\nb\n\nc\n\nd\n\ne\n\nf")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > remove last paragraph of multi-paragraph root (size-1, K=1)"
    ))
  func removeLastParagraph() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["a", "b", "c", "d", "e"]) }
    #expect(try cachedText(editor) == "a\n\nb\n\nc\n\nd\n\ne")
    try editor.update(discrete: true) { try getRoot().getLastChildOrThrow().remove() }
    #expect(try cachedText(editor) == "a\n\nb\n\nc\n\nd")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained typing on the same paragraph stays correct (cache freshness)"
    ))
  func sustainedTyping() throws {
    let editor = try mounted()
    var textKey = ""
    try editor.update(discrete: true) {
      try paragraphs(["alpha", "beta"])
      let text = try createTextNode("x")
      textKey = text.getKey()
      try getRoot().append(try createParagraphNode().append(text))
    }
    #expect(try cachedText(editor) == "alpha\n\nbeta\n\nx")
    for next in ["xy", "xyz", "xyzz"] {
      try editor.update(discrete: true) {
        try #require(try getNodeByKey(textKey) as? TextNode).setTextContent(next)
      }
      #expect(try cachedText(editor) == "alpha\n\nbeta\n\n\(next)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained appends stay correct across cycles (cache freshness, size+1)"
    ))
  func sustainedAppends() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["a", "b", "c", "d"]) }
    var expectedText = "a\n\nb\n\nc\n\nd"
    #expect(try cachedText(editor) == expectedText)
    for label in ["e", "f", "g", "h", "i"] {
      try editor.update(discrete: true) {
        try getRoot().append(try createParagraphNode().append(try createTextNode(label)))
      }
      expectedText += "\n\n\(label)"
      #expect(try cachedText(editor) == expectedText)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained removes stay correct across cycles (cache freshness, size-1)"
    ))
  func sustainedRemoves() throws {
    let editor = try mounted()
    var remaining = ["a", "b", "c", "d", "e", "f", "g"]
    try editor.update(discrete: true) { try paragraphs(remaining) }
    #expect(try cachedText(editor) == remaining.joined(separator: "\n\n"))
    for _ in 0..<3 {
      try editor.update(discrete: true) { try getRoot().getLastChildOrThrow().remove() }
      remaining.removeLast()
      #expect(try cachedText(editor) == remaining.joined(separator: "\n\n"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1: root __textFormat is not propagated from descendants"
    ))
  func rootTextFormatNotPropagated() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode(), try createParagraphNode(), try createParagraphNode(),
        try createParagraphNode().append(try createTextNode("x")))
    }
    #expect(try editor.read { try getRoot().getTextFormat() } == 0)
    try editor.update(discrete: true) {
      try lastText(try #require(try getRoot().getLastChildOrThrow() as? ElementNode))
        .toggleFormat("bold")
    }
    try editor.read { () throws in
      let root = try getRoot()
      let last = try #require(try root.getLastChildOrThrow() as? ParagraphNode)
      #expect(try last.getTextFormat() != 0)
      #expect(try root.getTextFormat() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 control: prefix with text keeps parent __textFormat stable"
    ))
  func rootTextFormatStable() throws {
    let editor = try mounted()
    try editor.update(discrete: true) { try paragraphs(["a", "b", "c", "d"]) }
    #expect(try editor.read { try getRoot().getTextFormat() } == 0)
    try editor.update(discrete: true) {
      try lastText(try #require(try getRoot().getLastChildOrThrow() as? ElementNode))
        .toggleFormat("bold")
    }
    try editor.read { () throws in
      let root = try getRoot()
      let last = try #require(try root.getLastChildOrThrow() as? ParagraphNode)
      #expect(try last.getTextFormat() != 0)
      #expect(try root.getTextFormat() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 (non-root) control: prefix with text keeps paragraph __textFormat stable"
    ))
  func paragraphTextFormatStable() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("head"), try createLineBreakNode(), try createLineBreakNode(),
          try createTextNode("tail")))
    }
    #expect(try editor.read { try firstParagraph().getTextFormat() } == 0)
    try editor.update(discrete: true) { try lastText(try firstParagraph()).toggleFormat("bold") }
    #expect(try editor.read { try firstParagraph().getTextFormat() } == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 (non-root): paragraph with linebreak-only prefix leaks stale __textFormat"
    ))
  func paragraphLineBreakPrefix() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createLineBreakNode(), try createLineBreakNode(), try createLineBreakNode(),
          try createTextNode("x")))
    }
    #expect(try editor.read { try firstParagraph().getTextFormat() } == 0)
    try editor.update(discrete: true) { try lastText(try firstParagraph()).toggleFormat("bold") }
    #expect(try editor.read { try firstParagraph().getTextFormat() } != 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > general same-size walk caches first-text key on the parent DOM"
    ))
  func sameSizeWalkCachesFirstTextKey() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("a"), try createTextNode("b").toggleFormat("bold"),
          try createTextNode("c").toggleFormat("italic"),
          try createTextNode("d").toggleFormat("code")))
    }
    try editor.update(discrete: true) {
      for child in try firstParagraph().getChildren() {
        try #require(child as? TextNode).setStyle("color: red")
      }
    }
    try editor.read { () throws in
      let para = try firstParagraph()
      #expect(try para.getChildren().count == 4)
      let firstChildKey = try para.getFirstChildOrThrow().__key
      #expect(firstTextKey(editor, para.__key) == firstChildKey)
      #expect(try para.getTextFormat() == 0)
    }
    try editor.update(discrete: true) {
      try lastText(try firstParagraph()).toggleFormat("underline")
    }
    #expect(try editor.read { try firstParagraph().getTextFormat() } == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > suffix walk keeps the leftmost first-text descriptor across element-child iterations"
    ))
  func suffixWalkKeepsLeftmost() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createLineBreakNode(), try createLineBreakNode(), try createLineBreakNode(),
          try link("X", "bold"), try link("Y", "italic")))
    }
    try editor.update(discrete: true) {
      for child in try firstParagraph().getChildren() {
        if let element = child as? ElementNode, let inner = try element.getFirstChild() as? TextNode
        {
          try inner.setStyle("color: red")
        }
      }
    }
    #expect(try editor.read { try firstParagraph().getTextFormat() } == BOLD)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-2: Layer 2 walk writes wrong __lexicalFirstTextKey when middle child is dirty"
    ))
  func middleDirtyKeepsLeftmost() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try link("a", "bold"), try link("b", "italic"), try link("c", "code"),
          try link("d", "underline")))
    }
    // Initial render via createNode doesn't go through reconcileTextFormat.
    #expect(try editor.read { try firstParagraph().getTextFormat() } == 0)
    try editor.update(discrete: true) {
      let linkC = try #require(try firstParagraph().getChildAtIndex(2) as? ElementNode)
      try #require(try linkC.getFirstChildOrThrow() as? TextNode).setStyle("color: red")
    }
    try editor.read { () throws in
      let para = try firstParagraph()
      let linkA = try #require(try para.getChildAtIndex(0) as? ElementNode)
      let textAKey = try linkA.getFirstChildOrThrow().__key
      #expect(firstTextKey(editor, para.__key) == textAKey)
      #expect(try para.getTextFormat() == BOLD)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-3: $reconcileNodeChildren writes wrong cache after middle-insert"
    ))
  func middleInsertKeepsLeftmost() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(
          try link("a", "bold"), try link("b", "italic"), try link("d", "underline")))
    }
    try editor.update(discrete: true) {
      let linkD = try #require(try firstParagraph().getChildAtIndex(2) as? ElementNode)
      try linkD.insertBefore(try link("c", "code"))
    }
    try editor.read { () throws in
      let para = try firstParagraph()
      let linkA = try #require(try para.getChildAtIndex(0) as? ElementNode)
      let textAKey = try linkA.getFirstChildOrThrow().__key
      #expect(firstTextKey(editor, para.__key) == textAKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-4: $bubbleChildFirstText misses cache on elements with wrapping DOM"
    ))
  func cleanInlineElementBubblesFirstText() throws {
    let editor = try mounted()
    try editor.update(discrete: true) {
      let wrapper = try createTestInlineElementNode()
      try wrapper.append(try createTextNode("a").toggleFormat("bold"))
      try getRoot().clear().append(
        try createParagraphNode().append(
          wrapper, try link("b", "italic"), try link("c", "code"), try link("d", "underline")))
    }
    try editor.update(discrete: true) {
      let linkC = try #require(try firstParagraph().getChildAtIndex(2) as? ElementNode)
      try #require(try linkC.getFirstChildOrThrow() as? TextNode).setStyle("color: red")
    }
    try editor.read { () throws in
      let para = try firstParagraph()
      let wrapper = try #require(try para.getChildAtIndex(0) as? ElementNode)
      let textAKey = try wrapper.getFirstChildOrThrow().__key
      #expect(try para.getTextFormat() == BOLD)
      #expect(firstTextKey(editor, para.__key) == textAKey)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-7: K=3 contiguous suffix with sizeDelta=+1 — output sentinel after helper bail"
    ))
  func contiguousSuffixPlusAppend() throws {
    let editor = try mounted()
    var paraCKey = ""
    var paraDKey = ""
    try editor.update(discrete: true) {
      try paragraphs(["pA", "pB", "pC", "pD"])
      paraCKey = try getRoot().getChildAtIndex(2)!.getKey()
      paraDKey = try getRoot().getChildAtIndex(3)!.getKey()
    }
    try editor.update(discrete: true) {
      try lastText(try #require(try getNodeByKey(paraCKey) as? ElementNode)).setTextContent(
        "CHANGED_C")
      try lastText(try #require(try getNodeByKey(paraDKey) as? ElementNode)).setTextContent(
        "CHANGED_D")
      try getRoot().append(try createParagraphNode().append(try createTextNode("pE")))
    }
    let expected = ["pA", "pB", "CHANGED_C", "CHANGED_D", "pE"].joined(separator: "\n\n")
    #expect(try editor.read { try getRoot().getTextContent() } == expected)
    #expect(editor._keyToDOMMap.get("root")?.textContent == expected)
  }
}
