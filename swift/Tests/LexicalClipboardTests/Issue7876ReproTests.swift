import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue7876Repro.test.ts:
// setEditorState of a parsed state dirties every node, so transforms run,
// and consumes the `_parsed` flag.

private let PARSED_TEXT = "hello {{name}}"
private let TRANSFORMED_TEXT = "VARIABLE"

private func buildEditor(_ onTransform: ((TextNode) throws -> Void)? = nil) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "issue-7876-repro", dependencies: [RichTextExtension],
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
  if let onTransform {
    _ = try editor.registerNodeTransform(TextNode.self, onTransform)
  }
  return editor
}

/// `JSON.stringify(editorState)` of an editor holding `fill`'s content.
private func makeParsedJSON(_ fill: @escaping () throws -> Void) throws -> String {
  let src = try buildEditor()
  defer { src.dispose() }
  try src.update(discrete: true) {
    _ = try getRoot().clear()
    try fill()
  }
  return try src.getEditorState().toJSON().stringify()
}

private func makeParsedJSON(_ text: String) throws -> String {
  try makeParsedJSON {
    _ = try getRoot().append(createParagraphNode().append(createTextNode(text)))
  }
}

private func replaceVariable(_ node: TextNode) throws {
  let text = try node.getTextContent()
  if text.contains("{{") && text != TRANSFORMED_TEXT {
    _ = try node.setTextContent(TRANSFORMED_TEXT)
  }
}

private final class Counter {
  var value = 0
}

@Suite struct Issue7876ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > text-node transform fires for a JSON state passed to setEditorState"
    ))
  func transformFiresForJSONState() throws {
    let editor = try buildEditor(replaceVariable)
    defer { editor.dispose() }
    let parsed = try editor.parseEditorState(makeParsedJSON(PARSED_TEXT))
    #expect(parsed._parsed)
    try editor.setEditorState(parsed)
    #expect(try editor.read { try getRoot().getTextContent() } == TRANSFORMED_TEXT)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > text-node transform fires for a cloned parsed state (the documented no-focus form)"
    ))
  func transformFiresForClonedState() throws {
    let editor = try buildEditor(replaceVariable)
    defer { editor.dispose() }
    let parsed = try editor.parseEditorState(makeParsedJSON(PARSED_TEXT))
    #expect(parsed._parsed)
    // `editorState.clone(null)`: applying a state without focusing the
    // editor (docs/concepts/editor-state.md).
    let cloned = parsed.clone(.some(nil))
    #expect(cloned._parsed)
    try editor.setEditorState(cloned)
    #expect(try editor.read { try getRoot().getTextContent() } == TRANSFORMED_TEXT)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > setEditorState consumes the `_parsed` flag so re-applying the resulting state does not re-trigger transforms"
    ))
  func consumesParsedFlag() throws {
    let transformCalls = Counter()
    let editor = try buildEditor { node in
      transformCalls.value += 1
      try replaceVariable(node)
    }
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(makeParsedJSON(PARSED_TEXT)))
    #expect(transformCalls.value >= 1)
    let currentState = editor.getEditorState()
    #expect(!currentState._parsed)
    let callsBeforeReapply = transformCalls.value
    try editor.setEditorState(currentState)
    #expect(transformCalls.value - callsBeforeReapply == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > a non-idempotent transform stabilises in two passes on a parsed state"
    ))
  func nonIdempotentTransformStabilises() throws {
    let SENTINEL = "#done:"
    let transformCalls = Counter()
    let editor = try buildEditor { node in
      transformCalls.value += 1
      let text = try node.getTextContent()
      if !text.hasPrefix(SENTINEL) {
        _ = try node.setTextContent(SENTINEL + text)
      }
    }
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(makeParsedJSON(PARSED_TEXT)))
    #expect(try editor.read { try getRoot().getTextContent() } == SENTINEL + PARSED_TEXT)
    #expect(transformCalls.value <= 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > setEditorState completes within bound on a 1000-paragraph parsed state"
    ))
  func boundedOnThousandParagraphs() throws {
    let transformCalls = Counter()
    let editor = try buildEditor { _ in transformCalls.value += 1 }
    defer { editor.dispose() }
    let NUM_PARAS = 1000
    let json = try makeParsedJSON {
      let root = try getRoot()
      for i in 0..<NUM_PARAS {
        _ = try root.append(createParagraphNode().append(createTextNode("line \(i)")))
      }
    }
    let parsed = try editor.parseEditorState(json)
    let clock = ContinuousClock()
    let elapsed = try clock.measure { try editor.setEditorState(parsed) }
    #expect(elapsed < .milliseconds(500))
    #expect(transformCalls.value >= NUM_PARAS)
  }
}
