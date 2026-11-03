import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// The two tests of packages/lexical-html/src/__tests__/unit/LexicalHtml.test.ts
// that insert with @lexical/clipboard's $insertGeneratedNodes; the rest of
// that file is ported in LexicalHTMLTests, which does not depend on
// LexicalClipboard.

@Suite struct LexicalHtmlInsertGeneratedNodesTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > HTML with <style> tags inlines styles by class"
    ))
  func inlinesStylesByClass() throws {
    let editor = try buildEditorFromExtensions([])
    defer { editor.dispose() }
    // Upstream builds this document with JSDOM, as jsdom's DOMParser has no
    // style sheets (jsdom#3179); the Swift DOMParser reads <style> itself.
    let dom = DOMParser().parseFromString(
      #"<html><head><style>.highlight { font-weight: bold; }</style></head><body><p><span class="highlight">Hello</span></p></body></html>"#
    )
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let nodes = try generateNodesFromDOM(editor, dom)
      try insertGeneratedNodes(editor, nodes, root.select(0))
      #expect(try root.getTextContent() == "Hello")
      let highlightSpan = try #require(
        dom.documentElement?.descendantElements().first { $0.classList.contains("highlight") },
        "span.highlight is in the dom")
      #expect(highlightSpan.style.getPropertyValue("font-weight") == "bold")
      let textNodes = try root.getAllTextNodes()
      try #require(textNodes.count == 1)
      #expect(try textNodes[0].hasFormat("bold"))
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > HTML without <style> tags works as before"
    ))
  func withoutStyleTags() throws {
    let editor = try createEditor(CreateEditorArgs(onError: { throw $0 }, headless: true))
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let dom = DOMParser().parseFromString("<p>Hello world</p>")
      let nodes = try generateNodesFromDOM(editor, dom)
      try insertGeneratedNodes(editor, nodes, root.select(0))
      let texts = try root.getAllTextNodes().map {
        "\(try $0.getTextContent()):\(try $0.hasFormat("bold"))"
      }
      #expect(texts == ["Hello world:false"])
    }
  }
}
