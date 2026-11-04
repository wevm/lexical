import Lexical
import LexicalCodeCore
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-code-core/src/__tests__/unit/CodeNode.test.ts.

private var editorConfig: EditorConfig {
  EditorConfig(namespace: "", theme: ["code": "editor-code"])
}

@Suite struct CodeNodeTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeNode.test.ts::CodeNode > applies and replaces styles through DOM style properties"
    ))
  func appliesAndReplacesStyles() throws {
    let editor = try createCodeTestEditor(theme: editorConfig.theme)
    var dom: DOMElement!
    var prevNode: CodeNode!
    try editor.update {
      let codeNode = try createCodeNode("javascript")
      try codeNode.setStyle("color: red; margin: 0 !important;")
      prevNode = codeNode
      dom = try codeNode.createDOM(editorConfig)
    }
    #expect(dom.style["color"] == "red")
    #expect(dom.style.getPropertyPriority("margin") == "important")

    try editor.update {
      let codeNode = try createCodeNode("javascript")
      try codeNode.setStyle("padding: 1px; --custom: value;")
      #expect(try codeNode.updateDOM(prevNode, dom, editorConfig) == false)
    }
    #expect(dom.style["color"] == "")
    #expect(dom.style["margin"] == "")
    #expect(dom.style["padding"] == "1px")
    #expect(dom.style.getPropertyValue("--custom") == "value")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeNode.test.ts::CodeNode > exports styles through DOM style properties"
    ))
  func exportsStyles() throws {
    let editor = try createCodeTestEditor(theme: editorConfig.theme)
    var exportedElement: DOMElement?
    try editor.update {
      let codeNode = try createCodeNode("javascript")
      try codeNode.setStyle("padding: 1px; color: blue;")
      try getRoot().append(codeNode)
      exportedElement = try codeNode.exportDOM(editor).element as? DOMElement
    }
    let element = try #require(exportedElement)
    #expect(element.style["padding"] == "1px")
    #expect(element.style["color"] == "blue")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeNode.test.ts::CodeNode > round-trips the theme through exportDOM/importDOM"
    ))
  func themeRoundTrip() throws {
    let editor = try createCodeTestEditor(theme: editorConfig.theme)
    var exported: DOMElement?
    try editor.update {
      let codeNode = try createCodeNode("javascript", "poimandres")
      try getRoot().append(codeNode)
      exported = try codeNode.exportDOM(editor).element as? DOMElement
    }
    let exportedElement = try #require(exported)
    #expect(exportedElement.getAttribute("data-language") == "javascript")
    #expect(exportedElement.getAttribute("data-theme") == "poimandres")

    // `document.implementation.createHTMLDocument()`.
    let doc = DOMParser().parseFromString("<!doctype html><html><body></body></html>")
    try #require(doc.body).append(exportedElement)

    try editor.update {
      let node = try #require(
        try generateNodesFromDOM(editor, doc).first as? CodeNode, "expected a CodeNode")
      #expect(try node.getLanguage() == "javascript")
      #expect(try node.getTheme() == "poimandres")
    }
  }
}
