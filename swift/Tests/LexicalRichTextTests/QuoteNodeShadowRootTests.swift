import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts.

private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "quote-shadow-root-host", dependencies: [RichTextExtension]))
}

@Suite struct QuoteNodeShadowRootTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > isShadowRoot() defaults to false and serializes nothing"
    ))
  func defaultsToFalse() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let quoteNode = try createQuoteNode()
      #expect(quoteNode.isShadowRoot() == false)
      #expect(try quoteNode.exportJSON()["shadowRoot"] == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > opt-in with $createQuoteNode and setIsShadowRoot"
    ))
  func optIn() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let quoteNode = try createQuoteNode(shadowRoot: true)
      #expect(quoteNode.isShadowRoot() == true)
      #expect(try quoteNode.exportJSON()["shadowRoot"] == .bool(true))
      try quoteNode.setIsShadowRoot(false)
      #expect(quoteNode.isShadowRoot() == false)
      #expect(try quoteNode.exportJSON()["shadowRoot"] == nil)
      try quoteNode.setIsShadowRoot(true)
      #expect(quoteNode.isShadowRoot() == true)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > shadow root round-trips through JSON"
    ))
  func roundTripsThroughJSON() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let quoteNode = try createQuoteNode(shadowRoot: true)
      let imported = try QuoteNode.importJSON(quoteNode.exportJSON()) as! QuoteNode
      #expect(imported.isShadowRoot() == true)
      let importedDefault = try QuoteNode.importJSON(createQuoteNode().exportJSON()) as! QuoteNode
      #expect(importedDefault.isShadowRoot() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > collapseAtStart() lifts blocks out of a shadow root quote"
    ))
  func collapseAtStartLiftsBlocks() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot()
      let quoteNode = try createQuoteNode(shadowRoot: true)
      try quoteNode.append(
        createParagraphNode().append(createTextNode("a")),
        createParagraphNode().append(createTextNode("b")))
      try root.clear().append(quoteNode)
      _ = try quoteNode.collapseAtStart(createRangeSelection())
      let children = try root.getChildren()
      #expect(children.count == 2)
      #expect(children.allSatisfy(isParagraphNode))
      #expect(try children.map { try $0.getTextContent() } == ["a", "b"])
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/QuoteNodeShadowRoot.test.ts::QuoteNode shadow root opt-in > getTopLevelElement() stops at a shadow root quote"
    ))
  func topLevelElementStopsAtShadowRoot() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let quoteNode = try createQuoteNode(shadowRoot: true)
      let paragraph = try createParagraphNode().append(createTextNode("a"))
      try quoteNode.append(paragraph)
      try getRoot().clear().append(quoteNode)
      #expect(try paragraph.getTopLevelElementOrThrow().is(paragraph))
      try quoteNode.setIsShadowRoot(false)
      #expect(try paragraph.getTopLevelElementOrThrow().is(quoteNode))
    }
  }
}
