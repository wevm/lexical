import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalNormalization.test.tsx.

private func paragraph() throws -> ParagraphNode {
  try getRoot().getFirstChild() as! ParagraphNode
}

private func texts(_ editor: Editor) throws -> [String] {
  try editor.getEditorState().read { try paragraph().getChildren().map { try $0.getTextContent() } }
}

private func lastChildKey(_ editor: Editor) throws -> NodeKey {
  try editor.getEditorState().read { try paragraph().getLastChildOrThrow().getKey() }
}

private func startComposition(_ editor: Editor) throws -> NodeKey {
  var composedKey: NodeKey = ""
  try update(editor) {
    let composed = try createTextNode("にj2z")
    try getRoot().clear().append(try createParagraphNode().append(composed))
    composedKey = composed.getKey()
    _ = try composed.select(1, 1)
    try setCompositionKey(composedKey)
  }
  return composedKey
}

/// Text from another source lands right before the composition.
private func insertBefore(_ editor: Editor, _ tag: UpdateTag? = nil) throws {
  try editor.update(
    { _ = try paragraph().getLastChildOrThrow().insertBefore(try createTextNode("j1z")) },
    EditorUpdateOptions(tag: tag.map { [$0] } ?? []))
  drainMicrotasks()
}

private func compositionStep(_ editor: Editor) throws {
  try update(editor) {
    let node = try paragraph().getLastChildOrThrow() as! TextNode
    _ = try node.setTextContent(
      try node.getTextContent().replacingOccurrences(of: "に", with: "にほ"))
  }
}

/// Sets the selection's points (swapped when `reversed`), normalizes it, and
/// returns the normalized anchor and focus as `key:offset:type` (unswapped).
private func normalized(
  _ paragraph: ParagraphNode, reversed: Bool, _ anchor: (NodeKey, Int, PointType),
  _ focus: (NodeKey, Int, PointType)
) throws -> [String] {
  let selection = try paragraph.select()
  let (a, f) = reversed ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
  try a.set(anchor.0, anchor.1, anchor.2)
  try f.set(focus.0, focus.1, focus.2)
  let result = try normalizeSelection(selection)
  let (na, nf) = reversed ? (result.focus, result.anchor) : (result.anchor, result.focus)
  return try [na, nf].map { "\(try $0.getNode().getKey()):\($0.offset):\($0.type.rawValue)" }
}

@Suite(.serialized) struct LexicalNormalizationTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > text tagged DEFER_COMPOSITION_MERGE_TAG is not merged into the composed node until it ends"
    ))
  func deferredMergeWaitsForCompositionEnd() throws {
    let editor = try createTestEditor()
    let composedKey = try startComposition(editor)
    try insertBefore(editor, DEFER_COMPOSITION_MERGE_TAG)
    #expect(try texts(editor) == ["j1z", "にj2z"])
    #expect(try lastChildKey(editor) == composedKey)
    // Later steps of the same composition do not merge either.
    try compositionStep(editor)
    #expect(try texts(editor) == ["j1z", "にほj2z"])
    #expect(try lastChildKey(editor) == composedKey)
    // The composition ends: the nodes merge.
    try update(editor) { try setCompositionKey(nil) }
    #expect(try texts(editor) == ["j1zにほj2z"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > text inserted without the tag merges at once"
    ))
  func untaggedTextMergesAtOnce() throws {
    let editor = try createTestEditor()
    _ = try startComposition(editor)
    try insertBefore(editor)
    #expect(try texts(editor) == ["j1zにj2z"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > COLLABORATION_TAG alone merges at once"
    ))
  func collaborationTagMergesAtOnce() throws {
    let editor = try createTestEditor()
    _ = try startComposition(editor)
    try insertBefore(editor, COLLABORATION_TAG)
    #expect(try texts(editor) == ["j1zにj2z"])
    try compositionStep(editor)
    #expect(try texts(editor) == ["j1zにほj2z"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > a new composition of the same node merges as usual"
    ))
  func newCompositionMergesAsUsual() throws {
    let editor = try createTestEditor()
    let composedKey = try startComposition(editor)
    try insertBefore(editor, DEFER_COMPOSITION_MERGE_TAG)
    #expect(try texts(editor) == ["j1z", "にj2z"])
    // The composition is reset without normalizing (as setEditorState
    // does), then a new one starts on the same node: nothing is held.
    try update(editor) { editor._compositionKey = nil }
    try update(editor) { try setCompositionKey(composedKey) }
    #expect(try texts(editor) == ["j1zにj2z"])
    try insertBefore(editor)
    #expect(try texts(editor) == ["j1zj1zにj2z"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text nodes"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text nodes (reversed)"
    ), arguments: [false, true])
  func paragraphToTextNodes(reversed: Bool) throws {
    let editor = try createTestEditor()
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      let text1 = try createTextNode("a")
      let text2 = try createTextNode("b")
      try getRoot().append(try paragraph.append(text1, text2))
      let p = paragraph.getKey()
      #expect(
        try normalized(paragraph, reversed: reversed, (p, 0, .element), (p, 2, .element)) == [
          "\(text1.getKey()):0:text", "\(text2.getKey()):1:text",
        ])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + element"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + element (reversed)"
    ), arguments: [false, true])
  func paragraphToTextAndElement(reversed: Bool) throws {
    let editor = try createTestEditor()
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      let text1 = try createTextNode("a")
      let elementNode = try createTestElementNode()
      try getRoot().append(try paragraph.append(text1, elementNode))
      let p = paragraph.getKey()
      #expect(
        try normalized(paragraph, reversed: reversed, (p, 0, .element), (p, 2, .element)) == [
          "\(text1.getKey()):0:text", "\(elementNode.getKey()):0:element",
        ])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + decorator"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + decorator (reversed)"
    ), arguments: [false, true])
  func paragraphToTextAndDecorator(reversed: Bool) throws {
    let editor = try createTestEditor()
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      let text1 = try createTextNode("a")
      let decoratorNode = try createTestDecoratorNode()
      try getRoot().append(try paragraph.append(text1, decoratorNode))
      let p = paragraph.getKey()
      #expect(
        try normalized(paragraph, reversed: reversed, (p, 0, .element), (p, 2, .element)) == [
          "\(text1.getKey()):0:text", "\(p):2:element",
        ])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > text + text node"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > text + text node (reversed)"
    ), arguments: [false, true])
  func textAndTextNode(reversed: Bool) throws {
    let editor = try createTestEditor()
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      let text1 = try createTextNode("a")
      let text2 = try createTextNode("b")
      try getRoot().append(try paragraph.append(text1, text2))
      #expect(
        try normalized(
          paragraph, reversed: reversed, (text1.getKey(), 0, .text), (text2.getKey(), 1, .text))
          == ["\(text1.getKey()):0:text", "\(text2.getKey()):1:text"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to test element to text + text"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to test element to text + text (reversed)"
    ), arguments: [false, true])
  func paragraphToElementToTexts(reversed: Bool) throws {
    let editor = try createTestEditor()
    try editor.update { () throws in
      let paragraph = try createParagraphNode()
      let elementNode = try createTestElementNode()
      let text1 = try createTextNode("a")
      let text2 = try createTextNode("b")
      try elementNode.append(text1, text2)
      try getRoot().append(try paragraph.append(elementNode))
      #expect(
        try normalized(
          paragraph, reversed: reversed, (text1.getKey(), 0, .text), (text2.getKey(), 1, .text))
          == ["\(text1.getKey()):0:text", "\(text2.getKey()):1:text"])
    }
  }
}
