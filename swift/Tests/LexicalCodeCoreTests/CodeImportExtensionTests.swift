import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts.
// The test with TableExtension is skipped until @lexical/table is ported.

/// CodeExtension registers its own import rules (and the shared
/// CoreImportExtension baseline); no dedicated import extension is needed.
private func buildEditor() throws -> Editor {
  try buildEditorFromExtensions(defineExtension(name: "code-host", dependencies: [CodeExtension]))
}

private func generate(_ html: String) throws -> [LexicalNode] {
  let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom)
}

private func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) {
    let nodes = try generate(html)
    try getRoot().clear().splice(0, 0, nodes)
  }
}

private func rootCode() throws -> CodeNode {
  try #require(try getRoot().getFirstChild() as? CodeNode, "expected CodeNode at root")
}

private func codeNodes() throws -> [CodeNode] {
  try getRoot().getChildren().compactMap { $0 as? CodeNode }
}

@Suite struct CodeImportExtensionTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre> imports as CodeNode"
    ))
  func preImports() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<pre>const x = 1;</pre>")
    try editor.read { () throws -> Void in
      #expect(try rootCode().getTextContent().contains("const x = 1;"))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre data-language=\"ts\"> sets the language"
    ))
  func preLanguage() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<pre data-language="ts">x</pre>"#)
    try editor.read { () throws -> Void in
      #expect(try rootCode().getLanguage() == "ts")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre data-theme=\"poimandres\"> restores the theme"
    ))
  func preTheme() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<pre data-language="ts" data-theme="poimandres">x</pre>"#)
    try editor.read { () throws -> Void in
      let node = try rootCode()
      #expect(try node.getLanguage() == "ts")
      #expect(try node.getTheme() == "poimandres")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > multi-line <code data-theme> restores the theme"
    ))
  func multilineCodeTheme() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<code data-language=\"ts\" data-theme=\"poimandres\">a\nb</code>")
    try editor.read { () throws -> Void in
      let node = try rootCode()
      #expect(try node.getLanguage() == "ts")
      #expect(try node.getTheme() == "poimandres")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > multi-line <code> imports as CodeNode (not inline)"
    ))
  func multilineCode() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<code>line1\nline2</code>")
    try editor.read { () throws -> Void in
      let text = try rootCode().getTextContent()
      #expect(text.contains("line1"))
      #expect(text.contains("line2"))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > single-line <code> defers to inline-format (no CodeNode)"
    ))
  func singleLineCode() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<p><code>inline</code></p>")
    try editor.read { () throws -> Void in
      let para = try #require(try getRoot().getFirstChild() as? ParagraphNode, "expected paragraph")
      let text = try #require(try para.getAllTextNodes().first, "expected text node")
      #expect(try text.getTextContent() == "inline")
      #expect(try text.hasFormat("code"))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <div style=\"font-family: monospace\"> imports as CodeNode"
    ))
  func monospaceDiv() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<div style=\"font-family: Menlo, monospace\">a\nb</div>")
    try editor.read { () throws -> Void in
      let text = try rootCode().getTextContent()
      #expect(text.contains("a"))
      #expect(text.contains("b"))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > GitHub raw-file-view table imports as CodeNode"
    ))
  func gitHubTable() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor,
      [
        #"<table class="js-file-line-container">"#,
        #"<tr><td class="js-file-line">line 1</td></tr>"#,
        #"<tr><td class="js-file-line">line 2</td></tr>"#,
        "</table>",
      ].joined())
    try editor.read { () throws -> Void in
      let text = try rootCode().getTextContent()
      #expect(text.contains("line 1"))
      #expect(text.contains("line 2"))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > plain <table> falls through (no CodeNode)"
    ))
  func plainTable() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<table><tr><td>a</td></tr></table>")
    try editor.read { () throws -> Void in
      #expect(
        !(try getRoot().getFirstChild() is CodeNode), "plain table should not become a CodeNode")
    }
  }
}

// VS Code → browser paste fixtures: Chrome wraps every line in one outer
// monospace `white-space: pre` <div>; Safari emits flat sibling <div>s and
// <br>s, each with that style. Both should give one CodeNode.

private let VSCODE_EXPECTED = """
  ## 📱 iOS alpha four

  Biggest alpha update!

  - Animate on sending message and delete
  - Set online/offline
  - Show all users when @ is typed

  [Get it from TestFlight.](https://testflight.apple.com/join/yKKmzbhD)

  ## Smaller improvements
  """

private let VSCODE_CHROME_HTML =
  #"<meta charset='utf-8'><div style="color: #cccccc;background-color: #1f1f1f;font-family: Menlo, Monaco, 'Courier New', monospace;font-weight: normal;font-size: 12px;line-height: 18px;white-space: pre;"><div><span style="color: #569cd6;font-weight: bold;">## 📱 iOS alpha four</span></div><br><div><span style="color: #cccccc;">Biggest alpha update!</span></div><br><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Animate on sending message and delete</span></div><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Set online/offline</span></div><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Show all users when @ is typed</span></div><br><div><span style="color: #cccccc;">[</span><span style="color: #ce9178;">Get it from TestFlight.</span><span style="color: #cccccc;">](</span><span style="color: #cccccc;text-decoration: underline;">https://testflight.apple.com/join/yKKmzbhD</span><span style="color: #cccccc;">)</span></div><br><div><span style="color: #569cd6;font-weight: bold;">## Smaller improvements</span></div></div>"#

// Trimmed Safari payload, with only the inline styles the rules read.
private let SAFARI_LINE_STYLE =
  #"font-family: Menlo, Monaco, "Courier New", monospace; white-space: pre;"#
private let VSCODE_SAFARI_HTML = [
  #"<div style='\#(SAFARI_LINE_STYLE)'><span style="color: rgb(86, 156, 214); font-weight: bold;">## 📱 iOS alpha four</span></div>"#,
  #"<br style='\#(SAFARI_LINE_STYLE)'>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span style="color: rgb(204, 204, 204);">Biggest alpha update!</span></div>"#,
  #"<br style='\#(SAFARI_LINE_STYLE)'>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span>-</span><span> Animate on sending message and delete</span></div>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span>-</span><span> Set online/offline</span></div>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span>-</span><span> Show all users when @ is typed</span></div>"#,
  #"<br style='\#(SAFARI_LINE_STYLE)'>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span>[</span><span>Get it from TestFlight.</span><span>](</span><span style="text-decoration: underline;">https://testflight.apple.com/join/yKKmzbhD</span><span>)</span></div>"#,
  #"<br style='\#(SAFARI_LINE_STYLE)'>"#,
  #"<div style='\#(SAFARI_LINE_STYLE)'><span style="color: rgb(86, 156, 214); font-weight: bold;">## Smaller improvements</span></div>"#,
].joined()

@Suite struct CodeImportExtensionVSCodePasteTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > Chrome (single outer monospace wrapper) → one CodeNode"
    ))
  func chrome() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, VSCODE_CHROME_HTML)
    try editor.read { () throws -> Void in
      let nodes = try codeNodes()
      #expect(nodes.count == 1)
      #expect(try nodes.first?.getTextContent() == VSCODE_EXPECTED)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > Safari (flat sibling monospace divs / brs) → one CodeNode"
    ))
  func safari() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, VSCODE_SAFARI_HTML)
    try editor.read { () throws -> Void in
      // The run collapses into a single CodeNode rather than one per div,
      // as the legacy importDOM produces.
      let nodes = try codeNodes()
      #expect(nodes.count == 1)
      #expect(try nodes.first?.getTextContent() == VSCODE_EXPECTED)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > paste without the VS Code structural signal does not install the overlay"
    ))
  func noStructuralSignal() throws {
    // A single bare monospace+pre <div> with only inline content matches
    // neither the wrapper nor the sibling run, so the overlay is not
    // installed and DivRule makes a one-line CodeNode.
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor, #"<div style="font-family: monospace; white-space: pre;">just one line</div>"#)
    try editor.read { () throws -> Void in
      let nodes = try codeNodes()
      #expect(nodes.count == 1)
      #expect(try nodes.first?.getTextContent() == "just one line")
    }
  }
}
