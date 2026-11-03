import Lexical
import LexicalTesting

/// `createTestEditor()` with `_headless = true`, as the upstream utils tests
/// make it.
func createHeadlessTestEditor(nodes: [LexicalNodeConfig] = []) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: nodes, onError: { throw $0 }, theme: [:], headless: true))
}

func createTextAndParagraphWithDepth(_ depth: Int) throws -> [LexicalNode] {
  if depth <= 0 { return [try createTextNode("<\(depth) />")] }
  return [
    try createTextNode("<\(depth)>"),
    try createParagraphNode().append(contentsOf: createTextAndParagraphWithDepth(depth - 1)),
    try createTextNode("</\(depth)>"),
  ]
}

func textContentForDepth(_ i: Int) -> String {
  i > 0 ? "<\(i)>\(textContentForDepth(i - 1))</\(i)>" : "<\(i) />"
}

func fiveTextNodes() throws -> ParagraphNode {
  try createParagraphNode().splice(0, 0, (0..<5).map { try createTextNode("\($0)") })
}

func texts(_ parent: ElementNode) throws -> [String] {
  try parent.getAllTextNodes().map { try $0.getTextContent() }
}
