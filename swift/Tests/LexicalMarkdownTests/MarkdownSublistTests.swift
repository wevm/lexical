import Lexical
import LexicalHTML
import LexicalList
import LexicalMarkdown
import Testing

// Ports of the list describe blocks of
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts from
// `List marker details` to `Ordered list start adjustment (#8677)`. Each
// block's `baseNodes` are `createTestEditor`'s default nodes.

private func html(_ editor: Editor) throws -> String {
  try editor.read(.latest) { try generateHtmlFromNodes(editor) }
}

private func importMarkdown(
  _ markdown: String, _ transformers: [any Transformer] = TRANSFORMERS
)
  throws -> Editor
{
  let editor = try createMarkdownTestEditor()
  try editor.update(discrete: true) { try convertFromMarkdownString(markdown, transformers) }
  return editor
}

/// Types `text` into the selection one character at a time, each in its own
/// discrete update.
private func typeText(_ editor: Editor, _ text: String) throws {
  for char in text {
    try editor.update(discrete: true) {
      if let selection = try getSelection() as? RangeSelection {
        try selection.insertText(String(char))
      }
    }
  }
}

/// A shortcut editor whose document `setUp` builds.
private func createShortcutEditor(_ setUp: @escaping () throws -> Void) throws -> Editor {
  let editor = try createMarkdownTestEditor()
  _ = try registerMarkdownShortcuts(editor, TRANSFORMERS)
  try editor.update(discrete: true) {
    try getRoot().clear()
    try setUp()
  }
  return editor
}

@Suite struct MarkdownListMarkerDetailsTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > reads an uppercase [X] as checked, as the /i on its regex intends"
    ))
  func uppercaseCheckedMarker() throws {
    let editor = try importMarkdown("- [X] up\n- [ ] no", [CHECK_LIST] + TRANSFORMERS)
    #expect(
      try html(editor)
        == "<ul __lexicallisttype=\"check\"><li role=\"checkbox\" tabindex=\"-1\" aria-checked=\"true\" value=\"1\"><span style=\"white-space: pre-wrap;\">up</span></li><li role=\"checkbox\" tabindex=\"-1\" aria-checked=\"false\" value=\"2\"><span style=\"white-space: pre-wrap;\">no</span></li></ul>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > expands a tab after the marker when measuring the content column"
    ))
  func tabAfterMarker() throws {
    // The tab opens the content of `a` at column four, so a two-space `- b`
    // does not reach it and stays a sibling.
    let editor = try importMarkdown("-\ta\n  - b")
    #expect(
      try html(editor)
        == "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span></li><li value=\"2\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > keeps a typed start number out of a list the item only passes through"
    ))
  func typedStartStaysOutOfPassedList() throws {
    // Typing an indented ordered marker above an existing ordered list sends
    // the item into a sublist, so the outer list's start stays what it was.
    let editor = try createShortcutEditor {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        paragraph,
        createListNode("number", start: 5).append(
          createListItemNode().append(createTextNode("y"))))
      try paragraph.selectEnd()
    }
    try typeText(editor, "    1. z")
    try editor.update(discrete: true) {}
    try editor.read(.latest) {
      let list = try #require(try getRoot().getFirstChild() as? ListNode, "expected a list")
      #expect(try list.getStart() == 5)
      #expect(try list.getLastChild()?.getTextContent() == "y")
    }
  }
}

@Suite struct MarkdownLooseSublistTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose sublists > nests a loose type-changing sublist into the item above it"
    ))
  func looseTypeChangingSublist() throws {
    let editor = try importMarkdown("- a\n\n  1. b")
    #expect(
      try html(editor)
        == "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose sublists > returns one joined list from $generateNodesFromMarkdownString"
    ))
  func generateNodesJoinsList() throws {
    // No ListNode transform runs over the returned nodes, so the loose list
    // has to come back joined rather than as two lists a later transform
    // would merge.
    let editor = try createMarkdownTestEditor()
    var shape = ""
    try editor.update(discrete: true) {
      shape = try generateNodesFromMarkdownString("- a\n\n  - b", TRANSFORMERS)
        .map { $0 is ListNode ? "list" : $0.getType() }
        .joined(separator: ",")
    }
    #expect(shape == "list")
  }
}

@Suite struct MarkdownSublistIndentBoundaryTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist indent boundaries > does not nest an indent short of a wide marker's content column"
    ))
  func indentShortOfWideMarker() throws {
    // `10. ` opens its content at column four, so three spaces is one short of
    // a sublist and the line starts a list of its own.
    let editor = try importMarkdown("10. a\n   - b")
    #expect(
      try html(editor)
        == "<ol start=\"10\"><li value=\"10\"><span style=\"white-space: pre-wrap;\">a</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist indent boundaries > keeps a retyped sublist in front of the one it was placed before"
    ))
  func retypedSublistStaysInFront() throws {
    // Typing above a list whose first child is a nested list of another type
    // puts the new item in front of that nested list, so the list built for
    // its own type belongs in front of it too.
    let editor = try createShortcutEditor {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        paragraph,
        createListNode("number").append(
          createListItemNode().append(
            createListNode("bullet").append(createListItemNode().append(createTextNode("x")))),
          createListItemNode().append(createTextNode("y"))))
      try paragraph.selectEnd()
    }
    try typeText(editor, "    1. z")
    try editor.update(discrete: true) {}
    #expect(
      try html(editor)
        == "<ol><li value=\"1\"><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">z</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">x</span></li></ul></li><li value=\"1\"><span style=\"white-space: pre-wrap;\">y</span></li></ol>"
    )
  }
}

@Suite struct MarkdownLooseListIndentRoundTripTests {
  private func convert(_ md: String) throws -> String {
    try importMarkdown(md).read(.latest) { try convertToMarkdownString(TRANSFORMERS) }
  }

  // Export writes one fixed step per level and drops the blank line, so a
  // blank line that changed how the indent was read would send the second
  // pass somewhere the first did not go.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\n  - b\" unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\n        - b\" unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"1. First\n\n   - detail\" unchanged"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\ntext\n\n  - b\" unchanged"
    ),
    arguments: [
      "- a\n\n  - b", "- a\n\n        - b", "1. First\n\n   - detail", "- a\n\ntext\n\n  - b",
    ])
  func reimportsOwnExport(_ md: String) throws {
    let once = try convert(md)
    #expect(try convert(once) == once)
  }
}

@Suite struct MarkdownSublistMarkerTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist markers > records a marker on the list the item lands in, not the one above it"
    ))
  func markerOnLandingList() throws {
    let md = "- a\n    * b\n    * c"
    let editor = try importMarkdown(md)
    #expect(try editor.read(.latest) { try convertToMarkdownString(TRANSFORMERS) } == md)
  }
}

@Suite struct MarkdownTypedSublistShortcutTests {
  private func createEditor() throws -> Editor {
    let editor = try createMarkdownTestEditor()
    _ = try registerMarkdownShortcuts(editor, TRANSFORMERS)
    return editor
  }

  private func typeLines(_ editor: Editor, _ lines: [String]) throws {
    for line in lines {
      try editor.update(discrete: true) {
        let paragraph = try createParagraphNode()
        try getRoot().append(paragraph)
        try paragraph.selectEnd()
      }
      // One character at a time: the shortcut fires on the space that closes
      // the marker, so anything after it is typed into whatever the shortcut
      // left the caret in.
      try typeText(editor, line)
    }
  }

  private func caretText(_ editor: Editor) throws -> String {
    try editor.read(.latest) {
      let selection = try #require(
        try getSelection() as? RangeSelection, "expected a range selection")
      return "\(try selection.anchor.getNode().getTextContent())@\(selection.anchor.offset)"
    }
  }

  // The indent of a line with no list above it has no content column to be
  // measured against, so it falls back to a fixed LIST_INDENT_SIZE per level.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"1. \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"    1. \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"- \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"    - \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"      * \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"        * \" with no list above it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"\t- \" with no list above it"
    ),
    arguments: [
      ("1. ", "<ol><li value=\"1\"></li></ol>"),
      ("    1. ", "<ol><li value=\"1\"><ol><li value=\"1\"></li></ol></li></ol>"),
      ("- ", "<ul><li value=\"1\"></li></ul>"),
      ("    - ", "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>"),
      ("      * ", "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>"),
      (
        "        * ",
        "<ul><li value=\"1\"><ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul></li></ul>"
      ),
      ("\t- ", "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>"),
    ])
  func indentsWithNoListAbove(_ text: String, _ expected: String) throws {
    let editor = try createEditor()
    try typeLines(editor, [text])
    // The shortcut runs from an update listener, so it lands on the next
    // update rather than the one that typed the closing space.
    try editor.update(discrete: true) {}
    #expect(try html(editor) == expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist of another type and keeps the caret in it"
    ))
  func nestsSublistOfAnotherType() throws {
    let editor = try createEditor()
    try typeLines(editor, ["- a", "    1. b"])
    #expect(
      try html(editor)
        == "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    )
    #expect(try caretText(editor) == "b@1")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist of the same type and keeps the caret in it"
    ))
  func nestsSublistOfSameType() throws {
    let editor = try createEditor()
    try typeLines(editor, ["- a", "    - b"])
    #expect(
      try html(editor)
        == "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul></li></ul>"
    )
    #expect(try caretText(editor) == "b@1")
  }

  // A typed line stands on its own: nothing records the column the list above
  // it was written at, so its indent is read as a fixed LIST_INDENT_SIZE per
  // level.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > reads a typed indent in fixed steps, not by content column"
    ))
  func typedIndentInFixedSteps() throws {
    let editor = try createEditor()
    try typeLines(editor, ["1. a", "   - b"])
    #expect(
      try html(editor)
        == "<ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    )
    #expect(try caretText(editor) == "b@1")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist under a list a shortcut did not create"
    ))
  func nestsUnderListNotFromShortcut() throws {
    let editor = try createEditor()
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createListNode("bullet").append(createListItemNode().append(createTextNode("a"))))
    }
    try typeLines(editor, ["    1. b"])
    #expect(
      try html(editor)
        == "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    )
    #expect(try caretText(editor) == "b@1")
  }
}

@Suite struct MarkdownOrderedListStartAdjustmentTests {
  /// Builds the document with `setUp`, which types the marker's digits into
  /// a paragraph, then types the closing space.
  private func typeSpace(after setUp: @escaping () throws -> Void) throws -> String {
    let editor = try createShortcutEditor(setUp)
    try typeText(editor, " ")
    return try editor.read { try generateHtmlFromNodes(editor) }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > updates list start when typed marker precedes an existing ordered list"
    ))
  func updatesStartBeforeOrderedList() throws {
    let result = try typeSpace {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        paragraph,
        createListNode("number", start: 2).append(
          createListItemNode().append(createTextNode("A")),
          createListItemNode().append(createTextNode("B"))))
      try paragraph.selectEnd().insertText("1.")
    }
    #expect(
      result
        == "<ol><li value=\"1\"></li><li value=\"2\"><span style=\"white-space: pre-wrap;\">A</span></li><li value=\"3\"><span style=\"white-space: pre-wrap;\">B</span></li></ol>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > respects an arbitrary typed start number"
    ))
  func arbitraryTypedStart() throws {
    let result = try typeSpace {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        paragraph,
        createListNode("number", start: 2).append(
          createListItemNode().append(createTextNode("A"))))
      try paragraph.selectEnd().insertText("7.")
    }
    #expect(
      result
        == "<ol start=\"7\"><li value=\"7\"></li><li value=\"8\"><span style=\"white-space: pre-wrap;\">A</span></li></ol>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > does not change start when typed marker follows an existing ordered list"
    ))
  func keepsStartAfterOrderedList() throws {
    let result = try typeSpace {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        createListNode("number", start: 2).append(
          createListItemNode().append(createTextNode("A"))),
        paragraph)
      try paragraph.selectEnd().insertText("9.")
    }
    // Branch (2) of listReplace: the paragraph after the list is appended as
    // the trailing item and the list start stays at 2.
    #expect(
      result
        == "<ol start=\"2\"><li value=\"2\"><span style=\"white-space: pre-wrap;\">A</span></li><li value=\"3\"></li></ol>"
    )
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > creates a fresh ordered list when the next sibling is a different list type"
    ))
  func freshListBeforeOtherType() throws {
    let result = try typeSpace {
      let paragraph = try createParagraphNode()
      try getRoot().append(
        paragraph,
        createListNode("bullet").append(createListItemNode().append(createTextNode("A"))))
      try paragraph.selectEnd().insertText("5.")
    }
    #expect(
      result
        == "<ol start=\"5\"><li value=\"5\"></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">A</span></li></ul>"
    )
  }
}
