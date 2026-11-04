import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue6477Repro.test.ts, which
// pastes with $insertDataTransferForRichText from @lexical/clipboard, so it
// is here rather than in LexicalTests.

private let ext = defineExtension(
  name: "[6477]", dependencies: [RichTextExtension],
  nodes: {
    [.klass(LinkNode.self), .klass(TestDecoratorNode.self), .klass(TestInlineElementNode.self)]
  })

/// A compact `type("text")` outline of the node tree.
private func describeTree(_ node: LexicalNode) throws -> String {
  guard let element = node as? ElementNode else {
    return "\(node.getType())(\"\(try node.getTextContent())\")"
  }
  return
    "\(element.getType())(\(try element.getChildren().map(describeTree).joined(separator: " ")))"
}

private func pasteHTML(_ editor: Editor, _ html: String) throws {
  let selection = try #require(try getSelection() as? RangeSelection, "Expected a RangeSelection")
  let dataTransfer = DataTransfer()
  dataTransfer.setData("text/html", html)
  try insertDataTransferForRichText(dataTransfer, selection, editor)
}

// A custom inline ElementNode does not implement insertNewAfter(), so it can
// not be split. Content inserted with the caret inside it used to land after
// the whole node instead of at the caret.
@Suite struct Issue6477ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > pasted text is inserted at the caret inside the node"
    ))
  func pastedTextAtCaretInsideNode() throws {
    let editor = try buildEditorFromExtensions(ext)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let inner = try createTextNode("abcd")
      try getRoot().clear().append(
        try createParagraphNode().append(
          try createTextNode("before"), try createTestInlineElementNode().append(inner),
          try createTextNode("after")))
      _ = try inner.select(2, 2)
    }
    try editor.update(discrete: true) { try pasteHTML(editor, "<b>XYZ</b>") }
    try editor.read { () throws -> Void in
      #expect(
        try describeTree(getRoot())
          == #"root(paragraph(text("before") test_inline_block(text("ab") text("XYZ") text("cd")) text("after")))"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > an inline node inserted at the caret goes inside the node"
    ))
  func inlineNodeAtCaretInsideNode() throws {
    let editor = try buildEditorFromExtensions(ext)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let inner = try createTextNode("abcd")
      try getRoot().clear().append(
        try createParagraphNode().append(try createTestInlineElementNode().append(inner)))
      _ = try inner.select(2, 2)
    }
    try editor.update(discrete: true) {
      let selection = try #require(
        try getSelection() as? RangeSelection, "Expected a RangeSelection")
      try selection.insertNodes([try createTestDecoratorNode()])
    }
    try editor.read { () throws -> Void in
      #expect(
        try describeTree(getRoot())
          == #"root(paragraph(test_inline_block(text("ab") test_decorator("Hello world") text("cd"))))"#
      )
    }
  }

  // Control: a LinkNode implements insertNewAfter(), so it is split around the
  // pasted content. This assertion holds both with and without the fix.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > control: a splittable inline ElementNode is still split"
    ))
  func splittableInlineElementStillSplit() throws {
    let editor = try buildEditorFromExtensions(ext)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let inner = try createTextNode("abcd")
      try getRoot().clear().append(
        try createParagraphNode().append(try createLinkNode("https://lexical.dev").append(inner)))
      _ = try inner.select(2, 2)
    }
    try editor.update(discrete: true) { try pasteHTML(editor, "<b>XYZ</b>") }
    try editor.read { () throws -> Void in
      #expect(
        try describeTree(getRoot())
          == #"root(paragraph(link(text("ab")) text("XYZ") link(text("cd"))))"#)
    }
  }
}
