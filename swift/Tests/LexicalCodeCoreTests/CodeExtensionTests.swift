import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-code-core/src/__tests__/unit/CodeExtension.test.ts.

private func buildEditor(
  _ name: String, _ children: @escaping () throws -> [LexicalNode]
)
  throws -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: name, dependencies: [CodeExtension, RichTextExtension],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in
          try getRoot().append(createCodeNode("javascript").append(contentsOf: children()))
        })))
}

private func firstCodeNode() throws -> CodeNode {
  try #require(try getRoot().getFirstChild() as? CodeNode)
}

private func pressEnter(_ editor: Editor) throws {
  _ = try editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter"))
}

@Suite struct CodeExtensionTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block when content has consecutive blank lines (paste scenario)"
    ))
  func consecutiveBlankLinesStay() throws {
    let editor = try buildEditor("[root]") {
      [
        try createCodeHighlightNode("line1"), try createLineBreakNode(),
        try createLineBreakNode(), try createLineBreakNode(),
        try createCodeHighlightNode("line2"),
      ]
    }
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getFirstChildOrThrow() is CodeNode)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should escape code block on Enter when cursor is after two trailing blank lines"
    ))
  func escapesAfterTwoBlankLines() throws {
    let editor = try buildEditor("[root-escape]") {
      [try createCodeHighlightNode("hello"), try createLineBreakNode(), try createLineBreakNode()]
    }
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let codeNode = try firstCodeNode()
      _ = try codeNode.select(codeNode.getChildrenSize(), codeNode.getChildrenSize())
    }
    try pressEnter(editor)
    try editor.update(discrete: true) {
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      #expect(try root.getFirstChildOrThrow() is CodeNode)
      let secondChild = try root.getLastChildOrThrow()
      #expect(secondChild is ElementNode)
      #expect(secondChild.getType() == "paragraph")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block on Enter with only one trailing blank line"
    ))
  func oneBlankLineStays() throws {
    let editor = try buildEditor("[root-one-blank]") {
      [try createCodeHighlightNode("hello"), try createLineBreakNode()]
    }
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let codeNode = try firstCodeNode()
      _ = try codeNode.select(codeNode.getChildrenSize(), codeNode.getChildrenSize())
    }
    try pressEnter(editor)
    try editor.update(discrete: true) {
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getFirstChildOrThrow() is CodeNode)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should strip format from TabNode inside CodeNode"
    ))
  func stripsTabFormat() throws {
    let editor = try buildEditor("[tab-format]") {
      [try createCodeHighlightNode("x"), try createTabNode()]
    }
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let tab = try #require(try firstCodeNode().getLastChild() as? TabNode)
      try tab.setFormat(1)
    }
    try editor.read { () throws -> Void in
      let tab = try #require(try firstCodeNode().getLastChild() as? TabNode)
      #expect(try tab.getFormat() == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeExtension.test.ts::CodeExtension > should not escape code block on Enter when cursor is not at the end"
    ))
  func middleCaretStays() throws {
    let editor = try buildEditor("[root-middle]") {
      [
        try createCodeHighlightNode("hello"), try createLineBreakNode(),
        try createLineBreakNode(), try createCodeHighlightNode("world"),
      ]
    }
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try firstCodeNode().select(1, 1)
    }
    try pressEnter(editor)
    try editor.update(discrete: true) {
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(try root.getFirstChildOrThrow() is CodeNode)
    }
  }
}
