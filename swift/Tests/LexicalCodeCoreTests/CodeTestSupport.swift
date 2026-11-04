import Lexical
import LexicalCodeCore
import LexicalTesting

// Shared helpers for the ports of packages/lexical-code-core/src/__tests__/unit.

/// `initializeUnitTest`'s editor, as `createMountedTestEditor` builds it,
/// with the code nodes added to the default test nodes.
func createCodeTestEditor(
  nodes: [LexicalNodeConfig] = [], theme: EditorThemeClasses = [:]
) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  let editor = try createEditor(
    CreateEditorArgs(
      namespace: "test",
      nodes: defaultTestNodes + [.klass(CodeNode.self), .klass(CodeHighlightNode.self)] + nodes,
      onError: { throw $0 }, theme: theme))
  try editor.setRootElement(TestRootElement())
  return editor
}
