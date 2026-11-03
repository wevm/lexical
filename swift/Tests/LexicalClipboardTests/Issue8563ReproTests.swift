import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/Issue8563Repro.test.ts: a
// full reconcile (undo through setEditorState) where a child is replaced by
// a different-key child of equal count must not crash the children fast
// path.

@Suite struct Issue8563ReproTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/Issue8563Repro.test.ts::Issue #8563: full reconcile with same-size child key swap > undo (setEditorState) does not crash when a child is replaced by a different-key child of equal count"
    ))
  func sameSizeChildKeySwap() throws {
    final class Errors { var values: [Error] = [] }
    let errors = Errors()
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "issue-8563-repro", dependencies: [RichTextExtension],
        editorConfig: InitialEditorConfig(onError: { error, _ in errors.values.append(error) })))
    defer { editor.dispose() }
    // A root element makes reconciliation run.
    try editor.setRootElement(TestRootElement())

    // State A: enough children to engage the fast path (>= 4).
    try editor.update(discrete: true) {
      let root = try getRoot().clear()
      for i in 0..<5 {
        _ = try root.append(createParagraphNode().append(createTextNode("line \(i)")))
      }
    }
    let stateA = editor.getEditorState()

    // State B: replace only the last child, keeping the count at 5.
    try editor.update(discrete: true) {
      _ = try getRoot().getLastChildOrThrow().replace(
        createParagraphNode().append(createTextNode("replaced")))
    }

    // Undo: restore state A, a full reconcile from B.
    try editor.setEditorState(stateA)

    #expect(errors.values.isEmpty)
    #expect(
      try editor.read { try getRoot().getTextContent() }
        == "line 0\n\nline 1\n\nline 2\n\nline 3\n\nline 4")
    // Upstream also reads the root element's DOM textContent; the views
    // render it here.
  }
}
