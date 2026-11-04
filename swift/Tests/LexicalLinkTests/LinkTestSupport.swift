import Lexical
import LexicalLink
import LexicalTesting
import Testing

// Shared helpers for the ports of packages/lexical-link/src/__tests__/unit.

/// The upstream test utils' default nodes, with the real LinkNode and
/// AutoLinkNode in place of TestLinkNode (all three are type "link" or
/// derived from it, and upstream's defaults are the real ones).
var linkTestNodes: [LexicalNodeConfig] {
  defaultTestNodes.filter {
    if case .klass(let klass) = $0 { return klass != TestLinkNode.self }
    return true
  } + [.klass(LinkNode.self), .klass(AutoLinkNode.self)]
}

/// `initializeUnitTest`'s editor, as `createMountedTestEditor` builds it,
/// with `linkTestNodes`.
func createLinkTestEditor(
  nodes: [LexicalNodeConfig] = [], theme: EditorThemeClasses = [:]
) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  let editor = try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: linkTestNodes + nodes, onError: { throw $0 }, theme: theme))
  try editor.setRootElement(TestRootElement())
  return editor
}
