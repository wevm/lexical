import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalUtils
import Testing

@testable import LexicalPlayground

/// The equation node's JSON and HTML, compared byte for byte with what the
/// web playground's EquationNode writes for the same document (an inline
/// equation, a block one and one KaTeX cannot parse); its HTML import, and
/// the base64 of its `data-lexical-equation`.
@MainActor
struct EquationNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EquationNodeTests",
        dependencies: [RichTextExtension, EquationsExtension]))
  }

  private let webJSON =
    #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Inline ","type":"text","version":1},{"equation":"a^2 + \\text{α} \\le b","inline":true,"type":"equation","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":" done","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"equation":"\\frac{1}{2} \\sum_{i=0}^n x_i","inline":false,"type":"equation","version":1},{"equation":"\\frac{","inline":false,"type":"equation","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
  /// KaTeX's markup is the web's `katex.render`, as the browser serializes
  /// it: `style` properties as `name: value;`, the error color as `rgb()`.
  /// (No `<` in an attribute: Chromium now writes it as `&lt;`, which the
  /// port's serializer, and the other playground vectors, do not.)
  private let webHTML =
    #"<p><span style="white-space: pre-wrap;">Inline </span><span data-lexical-equation="YV4yICsgXHRleHR7zrF9IFxsZSBi" data-lexical-inline="true" role="math" aria-label="Equation: a^2 + \text{α} \le b"><span class="katex"><span class="katex-html" aria-hidden="true"><span class="base"><span class="strut" style="height: 0.8974em; vertical-align: -0.0833em;"></span><span class="mord"><span class="mord mathnormal">a</span><span class="msupsub"><span class="vlist-t"><span class="vlist-r"><span class="vlist" style="height: 0.8141em;"><span class="" style="top: -3.063em; margin-right: 0.05em;"><span class="pstrut" style="height: 2.7em;"></span><span class="sizing reset-size6 size3 mtight"><span class="mord mtight">2</span></span></span></span></span></span></span></span><span class="mspace" style="margin-right: 0.2222em;"></span><span class="mbin">+</span><span class="mspace" style="margin-right: 0.2222em;"></span></span><span class="base"><span class="strut" style="height: 0.7719em; vertical-align: -0.136em;"></span><span class="mord text"><span class="mord">α</span></span><span class="mspace" style="margin-right: 0.2778em;"></span><span class="mrel">≤</span><span class="mspace" style="margin-right: 0.2778em;"></span></span><span class="base"><span class="strut" style="height: 0.6944em;"></span><span class="mord mathnormal">b</span></span></span></span></span><span style="white-space: pre-wrap;"> done</span></p><div data-lexical-equation="XGZyYWN7MX17Mn0gXHN1bV97aT0wfV5uIHhfaQ==" data-lexical-inline="false" role="math" aria-label="Equation: \frac{1}{2} \sum_{i=0}^n x_i"><span class="katex-display"><span class="katex"><span class="katex-html" aria-hidden="true"><span class="base"><span class="strut" style="height: 2.9291em; vertical-align: -1.2777em;"></span><span class="mord"><span class="mopen nulldelimiter"></span><span class="mfrac"><span class="vlist-t vlist-t2"><span class="vlist-r"><span class="vlist" style="height: 1.3214em;"><span class="" style="top: -2.314em;"><span class="pstrut" style="height: 3em;"></span><span class="mord"><span class="mord">2</span></span></span><span class="" style="top: -3.23em;"><span class="pstrut" style="height: 3em;"></span><span class="frac-line" style="border-bottom-width: 0.04em;"></span></span><span class="" style="top: -3.677em;"><span class="pstrut" style="height: 3em;"></span><span class="mord"><span class="mord">1</span></span></span></span><span class="vlist-s">​</span></span><span class="vlist-r"><span class="vlist" style="height: 0.686em;"><span class=""></span></span></span></span></span><span class="mclose nulldelimiter"></span></span><span class="mspace" style="margin-right: 0.1667em;"></span><span class="mop op-limits"><span class="vlist-t vlist-t2"><span class="vlist-r"><span class="vlist" style="height: 1.6514em;"><span class="" style="top: -1.8723em; margin-left: 0em;"><span class="pstrut" style="height: 3.05em;"></span><span class="sizing reset-size6 size3 mtight"><span class="mord mtight"><span class="mord mathnormal mtight">i</span><span class="mrel mtight">=</span><span class="mord mtight">0</span></span></span></span><span class="" style="top: -3.05em;"><span class="pstrut" style="height: 3.05em;"></span><span class=""><span class="mop op-symbol large-op">∑</span></span></span><span class="" style="top: -4.3em; margin-left: 0em;"><span class="pstrut" style="height: 3.05em;"></span><span class="sizing reset-size6 size3 mtight"><span class="mord mathnormal mtight">n</span></span></span></span><span class="vlist-s">​</span></span><span class="vlist-r"><span class="vlist" style="height: 1.2777em;"><span class=""></span></span></span></span></span><span class="mspace" style="margin-right: 0.1667em;"></span><span class="mord"><span class="mord mathnormal">x</span><span class="msupsub"><span class="vlist-t vlist-t2"><span class="vlist-r"><span class="vlist" style="height: 0.3117em;"><span class="" style="top: -2.55em; margin-left: 0em; margin-right: 0.05em;"><span class="pstrut" style="height: 2.7em;"></span><span class="sizing reset-size6 size3 mtight"><span class="mord mathnormal mtight">i</span></span></span></span><span class="vlist-s">​</span></span><span class="vlist-r"><span class="vlist" style="height: 0.15em;"><span class=""></span></span></span></span></span></span></span></span></span></span></div><div data-lexical-equation="XGZyYWN7" data-lexical-inline="false" role="math" aria-label="Equation: \frac{"><span class="katex-error" title="ParseError: KaTeX parse error: Unexpected end of input in a macro argument, expected '}' at end of input: \frac{" style="color:#cc0000">\frac{</span></div>"#

  private func insertEquations(_ editor: Editor) throws {
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("Inline "), createEquationNode(#"a^2 + \text{α} \le b"#, true),
          createTextNode(" done")),
        createEquationNode(#"\frac{1}{2} \sum_{i=0}^n x_i"#, false),
        createEquationNode(#"\frac{"#, false))
    }
  }

  @Test func serializesAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertEquations(editor)
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
  }

  @Test func exportsHTMLAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertEquations(editor)
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html == webHTML)
  }

  /// The web's JSON parses back to the same document, and its HTML imports
  /// the three equations through the `DOMImportExtension` rule and the
  /// legacy `importDOM` alike.
  @Test(arguments: [true, false])
  func parsesTheWebsJSONAndImportsItsHTML(viaExtension: Bool) throws {
    let editor = try makeEditor()
    try editor.setEditorState(editor.parseEditorState(webJSON))
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
    let imported = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EquationNodeTests/Import",
        dependencies: [RichTextExtension, CoreImportExtension, EquationsExtension]))
    try imported.update {
      let dom = DOMParser().parseFromString(webHTML)
      let nodes =
        try viaExtension
        ? generateNodesFromDOMViaExtension(dom) : generateNodesFromDOM(imported, dom)
      try getRoot().clear().append(createParagraphNode())
      try getRoot().selectEnd().insertNodes(nodes)
    }
    let equations = try imported.read {
      try dfs(getRoot()).compactMap { $0.node as? EquationNode }.map {
        "\(try $0.isInline() ? "inline" : "block") \(try $0.getEquation())"
      }
    }
    #expect(
      equations == [
        #"inline a^2 + \text{α} \le b"#, #"block \frac{1}{2} \sum_{i=0}^n x_i"#, #"block \frac{"#,
      ])
  }

  @Test func encodesTheEquationsUTF8() {
    // `\text{α}` holds a code point above U+00FF, which `btoa` alone throws on.
    #expect(encodeEquation(#"a^2 + \text{α} \le b"#) == "YV4yICsgXHRleHR7zrF9IFxsZSBi")
    #expect(decodeEquation("YV4yICsgXHRleHR7zrF9IFxsZSBi") == #"a^2 + \text{α} \le b"#)
    #expect(decodeEquation("not base64!") == nil)
  }
}
