import Lexical
import LexicalHashtag
import LexicalText
import Testing

// Port of one test of
// packages/lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts,
// the one that uses @lexical/hashtag's own matcher.

private let B = 1  // IS_BOLD

/// A run of text: `*` bold, `_` italic, `` ` `` code, then the text.
private func createRun(_ run: String) throws -> TextNode {
  var formats: [TextFormatType] = []
  var rest = Substring(run)
  while let c = rest.first {
    if c == "*" {
      formats.append("bold")
    } else if c == "_" {
      formats.append("italic")
    } else if c == "`" {
      formats.append("code")
    } else {
      break
    }
    rest = rest.dropFirst()
  }
  let node = try createTextNode(String(rest))
  for format in formats {
    _ = try node.toggleFormat(format)
  }
  return node
}

private func paragraph() throws -> ElementNode {
  try getRoot().getFirstChildOrThrow() as! ElementNode
}

private struct Out: Equatable {
  var type: String
  var text: String
  var format: Int
}

private func read(_ editor: Editor) throws -> [Out] {
  try editor.getEditorState().read {
    try paragraph().getChildren().compactMap { $0 as? TextNode }.map {
      Out(type: $0.getType(), text: try $0.getTextContent(), format: try $0.getFormat())
    }
  }
}

private func setUp(_ editor: Editor, _ runs: [String]) throws {
  try editor.update(discrete: true) {
    try getRoot().clear().append(
      createParagraphNode().append(contentsOf: runs.map { try createRun($0) }))
  }
}

private func select(_ node: TextNode, _ offset: Int) throws -> RangeSelection {
  let sel = createRangeSelection()
  try sel.anchor.set(node.getKey(), offset, .text)
  try sel.focus.set(node.getKey(), offset, .text)
  try setSelection(sel)
  return sel
}

private final class Dirty {
  var sizes: [Int] = []
}

@Suite struct RegisterLexicalTextEntitySettleHashtagTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntitySettle.test.ts::registerLexicalTextEntity between text nodes of other formats (#9637) > typing in text with a match before a formatted hashtag marks only that text dirty"
    ))
  func typingBeforeFormattedHashtagMarksOnlyThatTextDirty() throws {
    // With @lexical/hashtag's own matcher.
    let editor = try createEditor(
      CreateEditorArgs(nodes: [.klass(HashtagNode.self)], onError: { throw $0 }))
    let config = HashtagConfig()
    _ = try registerLexicalTextEntity(editor, config.getHashtagMatch, HashtagNode.self) {
      try createHashtagNode($0.getTextContent())
    }
    try setUp(editor, ["*#x", "#y!ab#", "_#z"])
    #expect(try read(editor).first == Out(type: "hashtag", text: "#x", format: B))
    let dirty = Dirty()
    _ = editor.registerUpdateListener { payload in
      dirty.sizes.append(payload.dirtyLeaves.size)
    }
    for c in "cde" {
      try editor.update(discrete: true) {
        let node = try paragraph().getChildAtIndex(1) as! TextNode
        try select(node, node.getTextContentSize() - 2).insertText(String(c))
      }
    }
    #expect(dirty.sizes == [1, 1, 1])
  }
}
