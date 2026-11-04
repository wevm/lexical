import Lexical
import LexicalHTML
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/registerListStrictIndentTransform.test.ts.

private let output = """
  <ul>
    <li value="1">
      <span style="white-space: pre-wrap;">0</span>
      <ul>
        <li value="1">
          <span style="white-space: pre-wrap;">1</span>
          <ul>
            <li value="1">
              <span style="white-space: pre-wrap;">2</span>
              <ul>
                <li value="1">
                  <span style="white-space: pre-wrap;">3</span>
                </li>
              </ul>
            </li>
          </ul>
        </li>
      </ul>
    </li>
  </ul>
  """

/// `<ul><li>…</li></ul>` nested `depth` levels around `content`.
private func nest(_ depth: Int, _ content: String) -> String {
  depth == 0 ? content : "<ul><li>\(nest(depth - 1, content))</li></ul>"
}

private func span(_ text: String) -> String {
  #"<span style="white-space: pre-wrap;">\#(text)</span>"#
}

// The three inputs upstream writes out, with the indent each item has.
private let cases: [String] = [
  "<ul><li>\(span("0"))</li><li><ul>"
    + "<li>\(nest(2, "<!-- Indent: 3 -->" + span("1")))</li>"
    + "<li><ul><li><!-- Indent: 2 -->\(span("2"))</li>"
    + "<li>\(nest(1, "<!-- Indent: 3 -->" + span("3")))</li></ul></li>"
    + "</ul></li></ul>",
  "<ul><li>\(span("0"))</li><li><ul>"
    + "<li>\(nest(1, "<!-- Indent: 2 -->" + span("1")))</li>"
    + "<li><ul><li>\(nest(2, "<!-- Indent: 4 -->" + span("2")))</li>"
    + "<li>\(nest(1, "<!-- Indent: 3 -->" + span("3")))</li></ul></li>"
    + "</ul></li></ul>",
  "<ul><li>\(span("0"))</li><li><ul>"
    + "<li>\(nest(1, "<!-- Indent: 2 -->" + span("1")))</li>"
    + "<li><ul><li>\(nest(2, "<!-- Indent: 4 -->" + span("2")))</li>"
    + "<li>\(nest(3, "<!-- Indent: 5 -->" + span("3")))</li></ul></li>"
    + "</ul></li></ul>",
]

@Suite struct RegisterListStrictIndentTransformTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/registerListStrictIndentTransform.test.ts::Lexical List StrictIndentTransform tests > applyStrictListIndentation"
    ))
  func applyStrictListIndentation() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    let unregister = try registerListStrictIndentTransform(editor)
    defer { unregister() }
    for input in cases {
      try update(editor) {
        let root = try getRoot()
        try root.clear()
        let dom = DOMParser().parseFromString(input, "text/html")
        try root.append(contentsOf: generateNodesFromDOM(editor, dom))
      }
      try editor.read { () throws -> Void in
        #expect(prettifyHTML(try generateHtmlFromNodes(editor)) == prettifyHTML(output))
      }
    }
  }
}
