import Lexical
import LexicalTesting
import LexicalText
import Testing

// Ports of packages/lexical-react/src/__tests__/unit/useLexicalIsTextContentEmpty.test.tsx
// and useLexicalIsTextContentEmptyResync.test.tsx. The hook reads
// `$isRootTextContentEmptyCurry` from the editor state when it mounts and
// after every update; these assert the values it reports.

/// The value `useLexicalIsTextContentEmpty(editor, trim)` reports.
@MainActor
private func isTextContentEmpty(_ editor: Editor, trim: Bool = true) throws -> Bool {
  try editor.read(.latest, isRootTextContentEmptyCurry(editor.isComposing(), trim: trim))
}

@MainActor
private func makeEditor(_ text: String) throws -> Editor {
  let editor = try createMountedTestEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    if !text.isEmpty { try paragraph.append(createTextNode(text)) }
    _ = try getRoot().clear().append(paragraph)
  }
  return editor
}

@MainActor
@Suite struct IsTextContentEmptyTests {
  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/useLexicalIsTextContentEmpty.test.tsx::useLexicalIsTextContentEmpty > hook works"
    ))
  func hookWorks() throws {
    let editor = try createMountedTestEditor()
    var reported = [try isTextContentEmpty(editor)]
    let unregister = editor.registerUpdateListener { _ in
      reported.append((try? isTextContentEmpty(editor)) ?? true)
    }
    defer { unregister() }
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(createTextNode("foo"))
    }
    #expect(reported == [true, false])
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/useLexicalIsTextContentEmptyResync.test.tsx::useLexicalIsTextContentEmpty re-derives its value > follows a change of the trim argument"
    ))
  func followsTheTrimArgument() throws {
    // Whitespace-only content is empty when trimmed and not empty otherwise.
    let editor = try makeEditor("   ")
    #expect(try isTextContentEmpty(editor, trim: false) == false)
    #expect(try isTextContentEmpty(editor, trim: true) == true)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/useLexicalIsTextContentEmptyResync.test.tsx::useLexicalIsTextContentEmpty re-derives its value > follows a change of the editor"
    ))
  func followsTheEditor() throws {
    #expect(try isTextContentEmpty(makeEditor("hello")) == false)
    #expect(try isTextContentEmpty(makeEditor("")) == true)
  }
}
