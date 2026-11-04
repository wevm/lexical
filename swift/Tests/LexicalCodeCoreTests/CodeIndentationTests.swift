import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts.

/// The upstream `[root-middle]` editor: CodeIndentExtension with
/// `escapeWithArrows`, and rich text.
private func buildEscapeEditor(
  _ initialEditorState: @escaping () throws -> Void
) throws
  -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: "[root-middle]",
      dependencies: [
        configExtension(CodeIndentExtension) { $0.escapeWithArrows = true },
        RichTextExtension,
      ],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in try initialEditorState() }
      )))
}

/// A code block with one `hello world` CodeHighlightNode, `select`ed.
private func buildHelloWorldEditor(
  paragraphBefore: Bool = false, paragraphAfter: Bool = false,
  _ select: @escaping (CodeHighlightNode) throws -> Void
) throws -> Editor {
  try buildEscapeEditor {
    let codeNode = try createCodeNode("javascript")
    let text = try createCodeHighlightNode("hello world")
    try codeNode.append(text)
    if paragraphBefore {
      try getRoot().append(createParagraphNode(), codeNode)
    } else if paragraphAfter {
      try getRoot().append(codeNode, createParagraphNode())
    } else {
      try getRoot().append(codeNode)
    }
    try select(text)
  }
}

/// An empty code block with the selection at its start.
private func buildEmptyCodeEditor() throws -> Editor {
  try buildEscapeEditor {
    let codeNode = try createCodeNode("javascript")
    try getRoot().append(codeNode)
    try codeNode.selectStart()
  }
}

private func dispatch(
  _ editor: Editor, _ command: LexicalCommand<KeyboardEvent>, _ key: String,
  altKey: Bool = false
) throws {
  _ = try editor.dispatchCommand(command, KeyboardEvent("keydown", key: key, altKey: altKey))
}

@Suite struct CodeIndentExtensionEscapeWithArrowsTests {
  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowDown when cursor is at the end"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowRight when cursor is at the end"
    ),
    arguments: [("ArrowDown", KEY_ARROW_DOWN_COMMAND), ("ArrowRight", KEY_ARROW_RIGHT_COMMAND)])
  func escapesByArrowAtTheEnd(key: String, command: LexicalCommand<KeyboardEvent>) throws {
    let editor = try buildHelloWorldEditor { try $0.selectEnd() }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let paragraph = try root.getLastChildOrThrow()
      #expect(isParagraphNode(paragraph))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowDown when cursor is not at the end"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowRight when cursor is not at the end"
    ),
    arguments: [("ArrowDown", KEY_ARROW_DOWN_COMMAND), ("ArrowRight", KEY_ARROW_RIGHT_COMMAND)])
  func doesNotEscapeByArrowWhenNotAtTheEnd(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildHelloWorldEditor { text in
      let length = try text.getTextContentSize()
      try text.select(length - 1, length - 1)
    }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(isCodeNode(try root.getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowDown when cursor is at the end and code block has next sibling"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowRight when cursor is at the end and code block has next sibling"
    ),
    arguments: [("ArrowDown", KEY_ARROW_DOWN_COMMAND), ("ArrowRight", KEY_ARROW_RIGHT_COMMAND)])
  func doesNotEscapeByArrowAtTheEndWithNextSibling(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildHelloWorldEditor(paragraphAfter: true) { try $0.selectEnd() }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      #expect(isCodeNode(try root.getFirstChildOrThrow()))
      #expect(isParagraphNode(try root.getLastChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowUp when cursor is at the beginning"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowLeft when cursor is at the beginning"
    ),
    arguments: [("ArrowUp", KEY_ARROW_UP_COMMAND), ("ArrowLeft", KEY_ARROW_LEFT_COMMAND)])
  func escapesByArrowAtTheBeginning(key: String, command: LexicalCommand<KeyboardEvent>) throws {
    let editor = try buildHelloWorldEditor { try $0.selectStart() }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let paragraph = try root.getFirstChildOrThrow()
      #expect(isParagraphNode(paragraph))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowUp when cursor is not at the beginning"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowLeft when cursor is not at the beginning"
    ),
    arguments: [("ArrowUp", KEY_ARROW_UP_COMMAND), ("ArrowLeft", KEY_ARROW_LEFT_COMMAND)])
  func doesNotEscapeByArrowWhenNotAtTheBeginning(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildHelloWorldEditor { try $0.select(1, 1) }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(isCodeNode(try root.getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowUp when cursor is at the beginning and code block has previous sibling"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by ArrowLeft when cursor is at the beginning and code block has previous sibling"
    ),
    arguments: [("ArrowUp", KEY_ARROW_UP_COMMAND), ("ArrowLeft", KEY_ARROW_LEFT_COMMAND)])
  func doesNotEscapeByArrowAtTheBeginningWithPreviousSibling(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildHelloWorldEditor(paragraphBefore: true) { try $0.selectStart() }
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      #expect(isParagraphNode(try root.getFirstChildOrThrow()))
      #expect(isCodeNode(try root.getLastChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowUp when cursor is inside an empty block"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowLeft when cursor is inside an empty block"
    ),
    arguments: [("ArrowUp", KEY_ARROW_UP_COMMAND), ("ArrowLeft", KEY_ARROW_LEFT_COMMAND)])
  func escapesUpOrLeftFromAnEmptyBlock(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildEmptyCodeEditor()
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let paragraph = try root.getFirstChildOrThrow()
      #expect(isParagraphNode(paragraph))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowDown when cursor is inside an empty block"
    ),
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should escape code block to a new paragraph by ArrowRight when cursor is inside an empty block"
    ),
    arguments: [("ArrowDown", KEY_ARROW_DOWN_COMMAND), ("ArrowRight", KEY_ARROW_RIGHT_COMMAND)])
  func escapesDownOrRightFromAnEmptyBlock(
    key: String, command: LexicalCommand<KeyboardEvent>
  ) throws {
    let editor = try buildEmptyCodeEditor()
    defer { editor.dispose() }

    try dispatch(editor, command, key)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 2)
      let paragraph = try root.getLastChildOrThrow()
      #expect(isParagraphNode(paragraph))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by Alt/Option + ArrowDown when cursor is at the end"
    ))
  func doesNotEscapeByAltArrowDownAtTheEnd() throws {
    let editor = try buildHelloWorldEditor { try $0.selectEnd() }
    defer { editor.dispose() }

    try dispatch(editor, KEY_ARROW_DOWN_COMMAND, "ArrowDown", altKey: true)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(isCodeNode(try root.getFirstChildOrThrow()))
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > escapeWithArrows > should not escape code block to a new paragraph by Alt/Option + ArrowUp when cursor is at the beginning"
    ))
  func doesNotEscapeByAltArrowUpAtTheBeginning() throws {
    let editor = try buildHelloWorldEditor { try $0.selectStart() }
    defer { editor.dispose() }

    try dispatch(editor, KEY_ARROW_UP_COMMAND, "ArrowUp", altKey: true)

    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try root.getChildrenSize() == 1)
      #expect(isCodeNode(try root.getFirstChildOrThrow()))
    }
  }
}

@Suite struct CodeIndentExtensionShiftLinesTests {
  /// "A", "" and "B" — a blank line between two lines of code.
  private func buildBlankLineEditor(caretOnLastLine: Bool) throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "[root-shift-lines]", dependencies: [CodeIndentExtension, RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let codeNode = try createCodeNode("javascript")
            let first = try createCodeHighlightNode("A")
            let last = try createCodeHighlightNode("B")
            try codeNode.append(first, createLineBreakNode(), createLineBreakNode(), last)
            try getRoot().append(codeNode)
            try (caretOnLastLine ? last : first).select(0, 0)
          })))
  }

  private func shift(_ editor: Editor, _ command: LexicalCommand<KeyboardEvent>) throws {
    let key = command === KEY_ARROW_UP_COMMAND ? "ArrowUp" : "ArrowDown"
    try dispatch(editor, command, key, altKey: true)
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > shiftLines > moves a line up past a blank line without merging it into the line above"
    ))
  func movesALineUpPastABlankLine() throws {
    let editor = try buildBlankLineEditor(caretOnLastLine: true)
    defer { editor.dispose() }

    try shift(editor, KEY_ARROW_UP_COMMAND)

    try editor.read {
      let codeNode = try getRoot().getFirstChildOrThrow()
      #expect(isCodeNode(codeNode))
      #expect(try codeNode.getTextContent() == "A\nB\n")
    }
  }

  @Test(
    .upstream(
      "lexical-code-core/src/__tests__/unit/CodeIndentation.test.ts::CodeIndentExtension > shiftLines > moves a line down past a blank line without merging it into the line below"
    ))
  func movesALineDownPastABlankLine() throws {
    let editor = try buildBlankLineEditor(caretOnLastLine: false)
    defer { editor.dispose() }

    try shift(editor, KEY_ARROW_DOWN_COMMAND)

    try editor.read {
      let codeNode = try getRoot().getFirstChildOrThrow()
      #expect(isCodeNode(codeNode))
      #expect(try codeNode.getTextContent() == "\nA\nB")
    }
  }
}
