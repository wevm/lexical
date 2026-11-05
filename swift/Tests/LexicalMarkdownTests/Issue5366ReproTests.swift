import Lexical
import LexicalExtension
import LexicalHashtag
import LexicalMarkdown
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts.

// The hashtag entity turns the leading "#" of "##Welcome" into its own
// node, which moves the caret to a different leaf while the shortcut is
// being typed. HashtagExtension is a dependency rather than a sibling so
// that its transform is registered ahead of the shortcut listener, which is
// the order #5366 reproduces in.
private let Issue5366TestExtension = defineExtension(
  name: "Issue5366Test", dependencies: [MarkdownTestExtension, HashtagExtension],
  register: { editor, _, _ in try registerMarkdownShortcuts(editor) })

/// Types each character in its own discrete update, firing the shortcut
/// listener.
private func type(_ editor: Editor, _ text: String) throws {
  for character in text {
    try editor.update(discrete: true) {
      if let selection = try getSelection() as? RangeSelection {
        try selection.insertText(String(character))
      }
    }
  }
}

// https://github.com/meta/lexical/issues/5366
@Suite struct Issue5366ReproTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"# \" shortcut typed before existing text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"## \" shortcut typed before existing text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"### \" shortcut typed before existing text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"#### \" shortcut typed before existing text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"##### \" shortcut typed before existing text"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/Issue5366Repro.test.ts::Issue #5366: hashtags block the heading shortcut > transforms with the \"###### \" shortcut typed before existing text"
    ),
    arguments: [
      ("# ", "h1"),
      ("## ", "h2"),
      ("### ", "h3"),
      ("#### ", "h4"),
      ("##### ", "h5"),
      ("###### ", "h6"),
    ])
  func headingShortcutBeforeText(_ shortcut: String, _ tag: String) throws {
    let editor = try buildEditorFromExtensions(Issue5366TestExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot()
      try root.clear()
      let paragraph = try createParagraphNode()
      let text = try createTextNode("Welcome to the playground")
      try paragraph.append(text)
      try root.append(paragraph)
      _ = try text.select(0, 0)
    }
    try type(editor, shortcut)
    try editor.read {
      let heading = try getRoot().getFirstChild()
      let headingNode = try #require(heading as? HeadingNode)
      #expect(try headingNode.getTag() == tag)
      #expect(try heading?.getTextContent() == "Welcome to the playground")
    }
  }
}
