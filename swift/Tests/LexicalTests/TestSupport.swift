import Lexical
import Testing

/// `createTestEditor` from packages/lexical/src/__tests__/utils: keys start
/// at 1, and errors are rethrown rather than reported.
func createTestEditor(nodes: [LexicalNodeConfig] = []) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(namespace: "test", nodes: nodes, onError: { throw $0 }, theme: [:]))
}

/// `await editor.update(fn)`: the update, then its commit.
func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}
