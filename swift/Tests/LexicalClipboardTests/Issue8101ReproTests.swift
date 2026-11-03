import Lexical
import LexicalClipboard
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8101Repro.test.ts.

/// Builds `<p style="text-align: {format}">Hello</p><p>World</p>` and selects
/// the whole of the first paragraph's text, i.e. steps 1-3 of the report.
private func setUpAndSelectFirstParagraph(_ format: ElementFormatType) throws {
  let root = try getRoot()
  _ = try root.clear()
  let first = try createParagraphNode()
  _ = try first.setFormat(format)
  let text = try createTextNode("Hello")
  _ = try first.append(text)
  let second = try createParagraphNode().append(createTextNode("World"))
  _ = try root.append(first, second)
  try selectWholeText(text)
}

private func selectWholeText(_ text: TextNode) throws {
  let selection = createRangeSelection()
  try selection.anchor.set(text.getKey(), 0, .text)
  try selection.focus.set(text.getKey(), text.getTextContentSize(), .text)
  try setSelection(selection)
}

private func rangeSelection() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection, "Expected RangeSelection")
}

/// `JSON.stringify($generateJSONFromSelectedNodes(editor, selection))`.
private func copyPayload(_ editor: Editor) throws -> String {
  try editor.read {
    try generateJSONFromSelectedNodes(editor, rangeSelection()).toJSON().stringify()
  }
}

/// Inserts the nodes of `payload` at the current selection.
private func paste(_ editor: Editor, _ payload: String) throws {
  try editor.update(discrete: true) { () throws in
    let nodes = try #require(try JSONValue.parse(payload).objectValue?["nodes"]?.arrayValue)
    try insertGeneratedNodes(
      editor, generateNodesFromSerializedNodes(nodes.compactMap(\.objectValue)), rangeSelection())
  }
}

@Suite struct Issue8101ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying a whole aligned paragraph keeps the alignment in the Lexical clipboard payload"
    ))
  func lexicalPayloadKeepsAlignment() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try setUpAndSelectFirstParagraph("center") }
    try editor.read { () throws in
      let nodes = try generateJSONFromSelectedNodes(editor, rangeSelection()).nodes
      // The payload must describe the paragraph, not just its text, or the
      // alignment has nowhere to live.
      try #require(nodes.count == 1)
      #expect(nodes[0]["type"] == "paragraph")
      #expect(nodes[0]["format"] == "center")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying a whole aligned paragraph keeps the alignment in the HTML clipboard payload"
    ))
  func htmlPayloadKeepsAlignment() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { try setUpAndSelectFirstParagraph("right") }
    try editor.read { () throws in
      #expect(
        try getHtmlContent(editor, rangeSelection())
          == #"<p style="text-align: right;"><span style="white-space: pre-wrap;">Hello</span></p>"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > a paragraph with no block-level state of its own still copies as inline content"
    ))
  func unalignedCopiesInline() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    // '' is the default (unset) format: nothing block-level to preserve.
    try editor.update(discrete: true) { try setUpAndSelectFirstParagraph("") }
    try editor.read { () throws in
      #expect(
        try getHtmlContent(editor, rangeSelection())
          == #"<span style="white-space: pre-wrap;">Hello</span>"#)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copy a centered paragraph and paste it into an empty paragraph"
    ))
  func pasteIntoEmptyParagraph() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let first = try createParagraphNode().setFormat("center")
      let text = try createTextNode("Hello")
      _ = try first.append(text)
      // The paste target: a second, default (left) aligned paragraph.
      _ = try getRoot().clear().append(first, createParagraphNode())
      try selectWholeText(text)
    }
    let payload = try copyPayload(editor)
    try editor.update(discrete: true) { () throws in
      let target = try #require(try getRoot().getLastChild() as? ElementNode)
      _ = try target.select()
    }
    try paste(editor, payload)
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let pasted = try #require(children[1] as? ElementNode)
      #expect(try pasted.getTextContent() == "Hello")
      #expect(try pasted.getFormatType() == "center")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > pasting a whole-line copy into the middle of another line still merges inline"
    ))
  func pasteIntoMiddleMergesInline() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello")
      _ = try getRoot().clear().append(
        createParagraphNode().append(text), createParagraphNode().append(createTextNode("abcdef")))
      try selectWholeText(text)
    }
    let payload = try copyPayload(editor)
    try editor.update(discrete: true) { () throws in
      let second = try #require(try getRoot().getLastChild() as? ElementNode)
      let text = try #require(try second.getFirstChild())
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 3, .text)
      try selection.focus.set(text.getKey(), 3, .text)
      try setSelection(selection)
    }
    try paste(editor, payload)
    try editor.read { () throws in
      let children = try getRoot().getChildren()
      // The copied block merges into the target line rather than splitting
      // it into a separate paragraph.
      try #require(children.count == 2)
      #expect(try children[1].getTextContent() == "abcHellodef")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8101Repro.test.ts::Text alignment is lost on copy-paste (#8101) > copying part of a paragraph still yields inline content only"
    ))
  func partialCopyIsInline() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try createTextNode("Hello")
      _ = try getRoot().clear().append(createParagraphNode().setFormat("center").append(text))
      // Only "ell" is selected: a fragment of a line, not a block.
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 1, .text)
      try selection.focus.set(text.getKey(), 4, .text)
      try setSelection(selection)
    }
    try editor.read { () throws in
      let selection = try rangeSelection()
      let nodes = try generateJSONFromSelectedNodes(editor, selection).nodes
      try #require(nodes.count == 1)
      #expect(nodes[0]["type"] == "text")
      #expect(
        try getHtmlContent(editor, selection) == #"<span style="white-space: pre-wrap;">ell</span>"#
      )
    }
  }
}
