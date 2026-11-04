import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of
// packages/lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts.
//
// Shift+Tab (OUTDENT_CONTENT_COMMAND) with a *collapsed* caret at column 0 of
// an indented code line silently did nothing: $getCodeLines drops a trailing
// line when the selection ends exactly at its start, which for a collapsed
// caret is the only line, so the outdent loop had nothing to work on.

private func buildOutdentEditor(
  name: String, tabSize: Int? = nil, _ initialEditorState: @escaping () throws -> Void
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name,
      dependencies: [
        tabSize == nil
          ? CodeIndentExtension : configExtension(CodeIndentExtension) { $0.tabSize = tabSize },
        RichTextExtension,
      ],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in try initialEditorState() }
      )))
}

/// The upstream `buildEditor(tabSize)`: a code block with a tab and `hello`.
private func buildEditor(tabSize: Int? = nil) throws -> Editor {
  try buildOutdentEditor(name: "[root-outdent]", tabSize: tabSize) {
    let code = try createCodeNode("javascript")
    try code.append(createTabNode(), createCodeHighlightNode("hello"))
    try getRoot().append(code)
  }
}

private func codeText() throws -> String {
  let code = try getRoot().getFirstChildOrThrow()
  try #require(isCodeNode(code), "expected a CodeNode")
  return try code.getTextContent()
}

@Suite struct CodeOutdentCollapsedAtLineStartTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > outdents when the caret is collapsed at column 0 (on the TabNode)"
    ))
  func outdentsAtColumnZeroOnTheTabNode() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let tab = try #require(try getRoot().getFirstDescendant() as? TabNode, "expected a TabNode")
      try tab.select(0, 0)
    }
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())

    #expect(try editor.read(codeText) == "hello")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > outdents when the caret is collapsed at column 0 of the code text"
    ))
  func outdentsAtColumnZeroOfTheCodeText() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let text = try #require(try getRoot().getLastDescendant(), "expected a text node")
      try text.selectStart()
    }
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())

    #expect(try editor.read(codeText) == "hello")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > still outdents from a non-zero column (unchanged behaviour)"
    ))
  func stillOutdentsFromANonZeroColumn() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let text = try #require(
        try getRoot().getLastDescendant() as? TextNode, "expected a text node")
      try text.select(1, 1)
    }
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())

    #expect(try editor.read(codeText) == "hello")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > strips a space indent from column 0 when tabSize is configured"
    ))
  func stripsASpaceIndentWhenTabSizeIsConfigured() throws {
    let editor = try buildOutdentEditor(name: "[root-outdent-spaces]", tabSize: 2) {
      let code = try createCodeNode("javascript")
      try code.append(createCodeHighlightNode("  hello"))
      try getRoot().append(code)
    }
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let text = try #require(try getRoot().getLastDescendant(), "expected a text node")
      try text.selectStart()
    }
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())

    #expect(try editor.read(codeText) == "hello")
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeOutdentCollapsedAtLineStart.test.ts::OUTDENT_CONTENT_COMMAND at the start of a code line > is still a no-op on an unindented line"
    ))
  func isANoOpOnAnUnindentedLine() throws {
    let editor = try buildOutdentEditor(name: "[root-outdent-flat]") {
      let code = try createCodeNode("javascript")
      try code.append(createCodeHighlightNode("hello"))
      try getRoot().append(code)
    }
    defer { editor.dispose() }

    try editor.update(discrete: true) {
      let text = try #require(try getRoot().getLastDescendant(), "expected a text node")
      try text.selectStart()
    }
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND, ())

    #expect(try editor.read(codeText) == "hello")
  }
}
