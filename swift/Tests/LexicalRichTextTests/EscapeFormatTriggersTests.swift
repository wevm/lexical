import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts.

/// What `$updateSelectionFormatStyleFromTextNode` does on a DOM selection
/// change: the selection takes its text node's format.
private func updateSelectionFormat(_ selection: RangeSelection, _ expectFormat: Int) throws {
  try #require(
    selection.anchor.type == .text && selection.focus.type == .text
      && selection.anchor.key == selection.focus.key,
    "Selection must have anchor and focus on the same TextNode")
  let format = try #require(try selection.anchor.getNode() as? TextNode).getFormat()
  #expect(format == expectFormat)
  selection.setFormat(format)
}

private func buildEditor(
  _ richText: any AnyLexicalExtensionArgument, _ initialState: @escaping () throws -> Void
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [richText],
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() })))
}

private func codeEditor(_ initialState: @escaping () throws -> Void) throws -> Editor {
  try buildEditor(
    configExtension(RichTextExtension) {
      $0.escapeFormatTriggers = [
        "code": TriggerConfig(enter: true, click: true, arrow: true, onlyAtBoundary: true)
      ]
    }, initialState)
}

private func firstTextNode() throws -> TextNode {
  try #require(try getRoot().getFirstDescendant() as? TextNode)
}

private func setUpCodeTextNode() throws {
  try getRoot().append(
    createParagraphNode().append(createTextNode("code text").toggleFormat("code")))
}

private func selectionFormat(_ editor: Editor) throws -> (Int, String) {
  try editor.read {
    let selection = try #require(try getSelection() as? RangeSelection)
    return (selection.format, selection.style)
  }
}

@Suite struct EscapeFormatTriggersTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > clears format when clicking at the end of a code node with no next sibling"
    ))
  func clickAtEnd() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectEnd(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click"))
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > clears format when clicking at the start of a code node with no previous sibling"
    ))
  func clickAtStart() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectStart(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click"))
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > does not clear format when clicking in the middle of a code node"
    ))
  func clickInMiddle() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(firstTextNode().select(3, 3), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click"))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > does not clear format at end of code node when it has a next sibling"
    ))
  func clickAtEndWithNextSibling() throws {
    let editor = try codeEditor {
      let codeNode = try createTextNode("code").toggleFormat("code")
      try getRoot().append(createParagraphNode().append(codeNode, createTextNode(" plain")))
      try updateSelectionFormat(codeNode.select(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(CLICK_COMMAND, MouseEvent("click"))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > clears format when pressing Enter at the end of a code node"
    ))
  func enterAtEnd() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectEnd(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > clears format when pressing Enter at the start of a code node"
    ))
  func enterAtStart() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectStart(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > preserves code format when pressing Enter in the middle of a code node"
    ))
  func enterInMiddle() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(firstTextNode().select(3, 3), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    #expect(try selectionFormat(editor).0 != 0)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > clears format when arrowing right at the end of a code node"
    ))
  func arrowRightAtEnd() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectEnd(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, KeyboardEvent("keydown"))
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > does not clear format when shift is held (extending selection)"
    ))
  func arrowRightWithShift() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectEnd(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(
      KEY_ARROW_RIGHT_COMMAND, KeyboardEvent("keydown", key: "ArrowRight", shiftKey: true))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > does not clear format in the middle of a code node"
    ))
  func arrowRightInMiddle() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(firstTextNode().select(3, 3), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ARROW_RIGHT_COMMAND, KeyboardEvent("keydown", key: "ArrowRight"))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > clears format when arrowing left at the start of a code node"
    ))
  func arrowLeftAtStart() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectStart(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, KeyboardEvent("keydown", key: "ArrowLeft"))
    let (format, style) = try selectionFormat(editor)
    #expect(format == 0)
    #expect(style == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > does not clear format when shift is held (extending selection)"
    ))
  func arrowLeftWithShift() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(getRoot().selectStart(), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(
      KEY_ARROW_LEFT_COMMAND, KeyboardEvent("keydown", key: "ArrowLeft", shiftKey: true))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > does not clear format in the middle of a code node"
    ))
  func arrowLeftInMiddle() throws {
    let editor = try codeEditor {
      try setUpCodeTextNode()
      try updateSelectionFormat(firstTextNode().select(3, 3), IS_CODE)
    }
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ARROW_LEFT_COMMAND, KeyboardEvent("keydown", key: "ArrowLeft"))
    #expect(try selectionFormat(editor).0 == IS_CODE)
  }

  private enum Key: String, CaseIterable { case enter, space, tab }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_ENTER_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_SPACE_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_TAB_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_ENTER_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_SPACE_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_TAB_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_ENTER_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_SPACE_COMMAND"
    ),
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_TAB_COMMAND"
    ),
    arguments: ["capitalize", "uppercase", "lowercase"], Key.allCases)
  private func defaultCapitalizationReset(_ format: TextFormatType, _ key: Key) throws {
    let editor = try buildEditor(RichTextExtension) {
      try updateSelectionFormat(
        getRoot().append(createParagraphNode().append(createTextNode("hello").toggleFormat(format)))
          .selectEnd(), TEXT_TYPE_TO_FORMAT[format]!)
    }
    defer { editor.dispose() }
    let event = KeyboardEvent("keydown")
    switch key {
    case .enter: try editor.dispatchCommand(KEY_ENTER_COMMAND, event)
    case .space: try editor.dispatchCommand(KEY_SPACE_COMMAND, event)
    case .tab: try editor.dispatchCommand(KEY_TAB_COMMAND, event)
    }
    try editor.read { () throws in
      #expect(try #require(try getSelection() as? RangeSelection).hasFormat(format) == false)
    }
  }

  private func uppercaseEditor(_ escapeFormatTriggers: EscapeFormatTriggerConfig) throws -> Editor {
    try buildEditor(
      configExtension(RichTextExtension) { $0.escapeFormatTriggers = escapeFormatTriggers }
    ) {
      try updateSelectionFormat(
        getRoot().append(
          createParagraphNode().append(createTextNode("hello").toggleFormat("uppercase"))
        ).selectEnd(), TEXT_TYPE_TO_FORMAT["uppercase"]!)
    }
  }

  private func hasUppercase(_ editor: Editor) throws -> Bool {
    try editor.read { try #require(try getSelection() as? RangeSelection).hasFormat("uppercase") }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > overriding one format preserves default capitalization resets"
    ))
  func overridingOneFormatPreservesDefaults() throws {
    let editor = try uppercaseEditor(["code": TriggerConfig(enter: true, onlyAtBoundary: true)])
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    #expect(try hasUppercase(editor) == false)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > null disables a default format"
    ))
  func nullDisablesDefault() throws {
    let editor = try uppercaseEditor(["uppercase": nil])
    defer { editor.dispose() }
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    #expect(try hasUppercase(editor) == true)
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > escapeFormatTriggers can be modified as a signal"
    ))
  func modifiedAsSignal() throws {
    let editor = try uppercaseEditor(["uppercase": nil])
    defer { editor.dispose() }
    // Re-enable the behavior here.
    let dep = try getExtensionDependencyFromEditor(editor, RichTextExtension)
    let triggers = dep.output.escapeFormatTriggers
    #expect(triggers.peek()["uppercase"] == .some(nil))
    var value = triggers.value
    value.updateValue(TriggerConfig(enter: true), forKey: "uppercase")
    triggers.value = value
    try editor.dispatchCommand(KEY_ENTER_COMMAND, nil)
    #expect(try hasUppercase(editor) == false)
  }
}
