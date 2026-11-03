import Lexical

/// `await editor.update(fn)`: the update, then its commit.
func update(_ editor: Editor, _ updateFn: @escaping () throws -> Void) throws {
  try editor.update(updateFn)
  drainMicrotasks()
}
