import Lexical
import LexicalExtension
import LexicalMarkdown
import LexicalText

/// A `getMatch` that runs the JS regular expression `source` (with JS
/// semantics, through `JSRegExp`) and returns its first match.
func regexMatch(_ source: String, _ flags: String = "") -> @Sendable (String) -> EntityMatch? {
  let regex = JSRegExp(source, flags)
  return { text in
    guard let match = regex.exec(text) else { return nil }
    return EntityMatch(start: match.index, end: match.index + (match[0]?.utf16.count ?? 0))
  }
}

/// An editor with `nodes` and the transforms of each registration in
/// `register`, built with `buildEditorFromExtensions`.
func buildEntityEditor(
  _ name: String = "test", nodes: [LexicalNodeConfig],
  register: @escaping (Editor) throws -> [() -> Void]
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name, nodes: { nodes },
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error }),
      register: { editor, _, _ in mergeRegister(try register(editor)) }))
}
