import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/IndentationLimits.test.ts.

private func createIndentEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "IndentationLimits",
      dependencies: [
        RichTextExtension, ListExtension,
        configExtension(TabIndentationExtension) { $0.maxIndent = 7 },
      ]))
}

private func indent(_ editor: Editor, _ times: Int) throws {
  for _ in 0..<times {
    try editor.update(discrete: true) { try editor.dispatchCommand(INDENT_CONTENT_COMMAND) }
  }
}

/// `$items`: each list item that holds no element, as `indent:text`.
private func items(_ node: LexicalNode? = nil) throws -> [String] {
  let node = try node ?? getRoot()
  if let item = node as? ListItemNode, !(try item.getFirstChild() is ElementNode) {
    return ["\(try item.getIndent()):\(try item.getTextContent())"]
  }
  guard let element = node as? ElementNode else { return [] }
  return try element.getChildren().flatMap { try items($0) }
}

private func paragraphIndent() throws -> Int {
  try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode).getIndent()
}

@Suite struct IndentationLimitsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for \"\""
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for \"World\""
    ), arguments: ["", "World"])
  func capsParagraphIndentation(_ text: String) throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      if !text.isEmpty { try paragraph.append(createTextNode(text)) }
      try getRoot().clear().append(paragraph)
      _ = try paragraph.selectEnd()
    }
    for i in 1...14 {
      try indent(editor, 1)
      #expect(try editor.read(paragraphIndent) == min(i, 6))
    }
  }

  // JS indents are numbers, so upstream also outdents 0.025, 0.05 and
  // 0.075; a Swift indent is an Int (see the skips).
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0 negative"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 1 negative"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 6 negative"
    ), arguments: [0, 1, 6])
  func outdentNeverNegative(_ initial: Int) throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(createTextNode("Hello"))
      try paragraph.setIndent(initial)
      try getRoot().clear().append(paragraph)
      _ = try paragraph.selectEnd()
    }
    for i in 1...8 {
      try editor.update(discrete: true) { try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND) }
      #expect(try editor.read(paragraphIndent) == max(0, initial - i))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps list indentation for \"\""
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps list indentation for \"World\""
    ), arguments: ["", "World"])
  func capsListIndentation(_ text: String) throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let item = try createListItemNode()
      if !text.isEmpty { try item.append(createTextNode(text)) }
      try getRoot().clear().append(createListNode("bullet").append(item))
      _ = try item.selectEnd()
    }
    for i in 1...14 {
      try indent(editor, 1)
      #expect(try editor.read { try items() } == ["\(min(i, 6)):\(text)"])
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps each selected item independently in a nested list"
    ))
  func capsEachItemInANestedList() throws {
    let editor = try createIndentEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet")
      try getRoot().clear().append(list)
      var parent = list
      for text in ["Hello", "from", "the", "other", "side"] {
        try parent.append(createListItemNode().append(createTextNode(text)))
        if text != "side" {
          let nested = try createListNode("bullet")
          try parent.append(createListItemNode().append(nested))
          parent = nested
        }
      }
      try parent.append(createListItemNode())
      _ = try selectAll()
    }
    try indent(editor, 3)
    #expect(
      try editor.read { try items() } == [
        "3:Hello", "4:from", "5:the", "6:other", "6:side", "6:",
      ])
    let expected = ["Hello", "from", "the", "other", "side", ""].map { "6:\($0)" }
    try indent(editor, 3)
    #expect(try editor.read { try items() } == expected)
    try indent(editor, 3)
    #expect(try editor.read { try items() } == expected)
  }
}
