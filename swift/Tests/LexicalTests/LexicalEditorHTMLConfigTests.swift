import LexicalHTML
import LexicalTesting
import Testing

@testable import Lexical

// Port of the `html config` tests of src/__tests__/unit/LexicalEditor.test.tsx,
// which export and import through @lexical/html.

private func htmlConfigEditor(_ html: HTMLConfig, onError: @escaping ErrorHandler) throws -> Editor
{
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "", nodes: defaultTestNodes, onError: onError, theme: [:], html: html))
}

@Suite(.serialized) struct LexicalEditorHTMLConfigTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > html config > should override export output function"
    ))
  func overrideExportOutput() throws {
    let onError = Calls()
    let editor = try htmlConfigEditor(
      HTMLConfig(export: [
        (
          TextNode.self,
          { _, target in
            let text = try #require(target as? TextNode)
            return DOMExportOutput(
              element: getDocument().createElement(try text.hasFormat("bold") ? "bor" : "foo"))
          }
        )
      ]),
      onError: { onError.record($0) })
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update { () throws in
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let text = try createTextNode()
      try root.append(paragraph)
      try paragraph.append(text)

      let selection = createNodeSelection()
      selection.add(text.getKey())

      #expect(try generateHtmlFromNodes(editor, selection) == "<foo></foo>")

      try text.toggleFormat("bold")

      #expect(try generateHtmlFromNodes(editor, selection) == "<bor></bor>")
    }
    #expect(onError.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > html config > should override import conversion function"
    ))
  func overrideImportConversion() throws {
    let onError = Calls()
    let editor = try htmlConfigEditor(
      HTMLConfig(import: [
        "figure": { _ in
          DOMConversion(priority: 4) { _ in DOMConversionOutput(node: try createTextNode("yolo")) }
        }
      ]),
      onError: { onError.record($0) })
    defer { editor.dispose() }
    try editor.setRootElement(TestRootElement())
    try editor.update { () throws in
      let dom = DOMParser().parseFromString("<figure></figure>", "text/html")
      let node = try #require(try generateNodesFromDOM(editor, dom).first as? TextNode)
      #expect(node.__type == "text")
      #expect(node.__text == "yolo")
      #expect(node.__detail == 0)
      #expect(node.__format == 0)
      #expect(node.__mode == 0)
      #expect(node.__style == "")
      #expect(node.__parent == nil)
      #expect(node.__prev == nil)
      #expect(node.__next == nil)
    }
    #expect(onError.count == 0)
  }
}
