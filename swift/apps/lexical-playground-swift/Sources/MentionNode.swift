import Lexical

// Port of packages/lexical-playground/src/nodes/MentionNode.ts.

/// The element `TextNode.exportDOM` wraps its output with, one per text
/// format, innermost first.
private let FORMAT_WRAPPER_TAGS: [(TextFormatType, String)] = [
  ("bold", "b"), ("italic", "i"), ("strikethrough", "s"), ("underline", "u"),
]

/// The mention's background; the TextKit theme draws it under the
/// `"mention"` key (see `mentionThemeStyle`).
let mentionBackgroundColor = "rgba(24, 119, 232, 0.2)"

/// `MentionNode`: a person's name, as one segmented text entity.
final class MentionNode: TextNode {
  var __mention: String = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      MentionNode.self, type: "mention", extends: TextNode.self,
      json: [SchemaField("mentionName", stringValue(), field: \MentionNode.__mention)])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// the `DOMImportExtension` rule below does not reach, so the node also
  /// converts the span itself, as upstream's `importDOM` did before the
  /// rule replaced it.
  override class func importDOM() -> DOMConversionMap? {
    [
      "span": { node in
        guard (node as? DOMElement)?.hasAttribute("data-lexical-mention") == true else {
          return nil
        }
        return DOMConversion(priority: 1) { node in
          DOMConversionOutput(node: [try importMention(node as! DOMElement)])
        }
      }
    ]
  }

  /// `new MentionNode(mentionName, text, key)`.
  convenience init(_ mentionName: String, text: String? = nil, key: NodeKey? = nil) throws {
    try self.init(text ?? mentionName, key: key)
    __mention = mentionName
  }

  func getMentionName() throws -> String { try getLatest().__mention }

  @discardableResult
  func setMentionName(_ mentionName: String) throws -> Self {
    let writable = try getWritable()
    writable.__mention = mentionName
    return writable
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let dom = try super.createDOM(config, editor)
    dom.style.setProperty("background-color", mentionBackgroundColor)
    // Add to the class names TextNode.createDOM applied for the text
    // formats rather than replacing them.
    addClassNamesToElement(dom, "mention")
    dom.setAttribute("spellcheck", "false")
    return dom
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let document = getDocument()
    let text = try getTextContent()
    let style = try getStyle()
    let element = document.createElement("span")
    element.setAttribute("data-lexical-mention", "true")
    if text != __mention {
      element.setAttribute("data-lexical-mention-name", __mention)
    }
    element.textContent = text
    // Carry the inline style and the text formats onto the exported markup
    // the way TextNode.exportDOM does. The mention itself stays a <span> so
    // the `span[data-lexical-mention]` import rule keeps matching it.
    if !style.isEmpty {
      element.style.cssText = style
    }
    var wrapped: DOMElement = element
    for (format, tag) in FORMAT_WRAPPER_TAGS where try hasFormat(format) {
      let wrapper = document.createElement(tag)
      wrapper.appendChild(wrapped)
      wrapped = wrapper
    }
    return DOMExportOutput(element: wrapped)
  }

  override func isTextEntity() -> Bool { true }

  override func canInsertTextBefore() throws -> Bool { false }

  override func canInsertTextAfter() throws -> Bool { false }
}

/// `$createMentionNode(mentionName, textContent)`.
func createMentionNode(_ mentionName: String, _ textContent: String? = nil) throws -> MentionNode {
  let mentionNode = try MentionNode(mentionName, text: textContent)
  try mentionNode.setMode("segmented").toggleDirectionless()
  return try applyNodeReplacement(mentionNode)
}

func isMentionNode(_ node: LexicalNode?) -> Bool { node is MentionNode }

/// The import of a `span[data-lexical-mention]`.
func importMention(_ el: DOMElement) throws -> MentionNode {
  let textContent = el.textContent
  let mentionName = el.getAttribute("data-lexical-mention-name") ?? textContent
  return try createMentionNode(mentionName, textContent)
}
