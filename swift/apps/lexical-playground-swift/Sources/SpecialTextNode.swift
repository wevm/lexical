import Lexical
import LexicalExtension

// Ports of packages/lexical-playground/src/nodes/SpecialTextNode.tsx and
// plugins/SpecialTextExtension: with "Use Brackets for Highlighting" on,
// text typed in square brackets becomes a highlighted SpecialTextNode
// without them.

/// `SpecialTextNode`.
final class SpecialTextNode: TextNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(SpecialTextNode.self, type: "specialText", extends: TextNode.self)
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let dom = getDocument().createElement("span")
    addClassNamesToElement(dom, config.theme["specialText"]?.stringValue)
    dom.textContent = try getTextContent()
    return dom
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    // Write the text the same way createDOM does: returning false promises
    // the reconciler that the DOM already has every difference. (`__text`
    // is not visible outside the package, so it is written every time.)
    dom.textContent = try getTextContent()
    addClassNamesToElement(dom, config.theme["specialText"]?.stringValue)
    return false
  }

  override func isTextEntity() -> Bool { true }

  /// Prevents appending text to this node.
  override func canInsertTextAfter() throws -> Bool { false }
}

/// `$createSpecialTextNode(text)`.
func createSpecialTextNode(_ text: String = "") throws -> SpecialTextNode {
  try applyNodeReplacement(SpecialTextNode(text))
}

func isSpecialTextNode(_ node: LexicalNode?) -> Bool { node is SpecialTextNode }

/// `.PlaygroundEditorTheme__specialText`, which the views apply under the
/// node's type.
let specialTextThemeStyle = "background-color: yellow; font-weight: bold"

/// `BRACKETED_TEXT_REGEX`: `/\[([^[\]]+)\]/`.
nonisolated(unsafe) private let BRACKETED_TEXT_REGEX = /\[([^\[\]]+)\]/

/// `$findAndTransformText`: the first bracketed text in `node` becomes a
/// SpecialTextNode; returns the node after it to look at next, if any.
private func findAndTransformText(_ node: TextNode) throws -> TextNode? {
  let text = try node.getTextContent()
  guard let match = text.firstMatch(of: BRACKETED_TEXT_REGEX) else { return nil }
  let matchedText = String(match.1)
  // `match.index` and the match's length, in UTF-16 code units.
  let startIndex = text.utf16.distance(from: text.startIndex, to: match.range.lowerBound)
  let length = text.utf16.distance(from: match.range.lowerBound, to: match.range.upperBound)
  let targetNode: TextNode
  if startIndex == 0 {
    targetNode = try node.splitText(startIndex + length)[0]
  } else {
    targetNode = try node.splitText(startIndex, startIndex + length)[1]
  }
  let specialTextNode = try createSpecialTextNode(matchedText)
  try targetNode.replace(specialTextNode)
  return specialTextNode
}

/// `$textNodeTransform`.
private func textNodeTransform(_ node: TextNode) throws {
  var targetNode: TextNode? = node
  while let current = targetNode {
    if try !current.isSimpleText() { return }
    targetNode = try findAndTransformText(current)
  }
}

struct SpecialTextConfig {
  var disabled = true
}

/// `SpecialTextExtension`.
let SpecialTextExtension = defineExtension(
  name: "@lexical/playground/SpecialText", config: SpecialTextConfig(),
  nodes: { [.klass(SpecialTextNode.self)] },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if output.disabled.value { return nil }
      return try editor.registerNodeTransform(TextNode.self) { try textNodeTransform($0) }
    }
  })
