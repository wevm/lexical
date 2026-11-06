import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts.
//
// MarkNode.excludeFromCopy() is `destination != "clone"`: keep it in the
// internal clone payload, drop it from HTML.

private let markClipboardExtension = defineExtension(
  name: "[mark-clipboard]", dependencies: [RichTextExtension],
  nodes: { [.klass(MarkNode.self)] },
  editorConfig: InitialEditorConfig(
    initialEditorState: .update { _ in
      try getRoot().clear().append(
        createParagraphNode().append(
          createMarkNode(["comment-1"]).append(createTextNode("marked"))))
    }))

/// Runs `check` in the update that rebuilds the nodes of the whole
/// document's clone payload.
private func roundTrip(_ check: @escaping ([LexicalNode]) throws -> Void) throws {
  let editor = try buildEditorFromExtensions(markClipboardExtension)
  defer { editor.dispose() }
  try editor.update(discrete: true) {
    _ = try selectAll()
    let nodes = try generateJSONFromSelectedNodes(editor, getSelection()).nodes
    try check(try generateNodesFromSerializedNodes(nodes))
  }
}

/// The first MarkNode in `nodes`, depth first.
private func firstMark(_ nodes: [LexicalNode]) throws -> MarkNode? {
  var stack = nodes
  while !stack.isEmpty {
    let node = stack.removeFirst()
    if let mark = node as? MarkNode { return mark }
    if let element = node as? ElementNode {
      stack.insert(contentsOf: try element.getChildren(), at: 0)
    }
  }
  return nil
}

@Suite struct MarkClipboardJSONTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > keeps the MarkNode in the clone payload"
    ))
  func keepsTheMarkNode() throws {
    try roundTrip { rebuilt throws in #expect(try firstMark(rebuilt) != nil) }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > keeps the mark ids on the round trip"
    ))
  func keepsTheMarkIDs() throws {
    try roundTrip { rebuilt throws in
      let mark = try #require(try firstMark(rebuilt))
      #expect(try mark.getIDs() == ["comment-1"])
    }
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/MarkClipboardJSON.test.ts::$generateJSONFromSelectedNodes with a MarkNode > still carries the marked text"
    ))
  func carriesTheMarkedText() throws {
    try roundTrip { rebuilt throws in
      #expect(try rebuilt.map { try $0.getTextContent() }.joined().contains("marked"))
    }
  }
}
