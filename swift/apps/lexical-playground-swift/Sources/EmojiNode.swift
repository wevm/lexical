import Lexical
import LexicalExtension

// Port of packages/lexical-playground/src/nodes/EmojiNode.tsx and
// plugins/EmojisExtension: `:)`, `:D`, `:(` and `<3` typed in text become
// an emoji, a token-mode text node of the emoji's character. The web draws
// the character transparent over a 16px image (`.emoji.happysmile` and the
// others in index.css); the views draw the character itself.

/// `EmojiNode`: an emoji, with the class names that pick its image.
final class EmojiNode: TextNode {
  var __className: String = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      EmojiNode.self, type: "emoji", extends: TextNode.self,
      json: [SchemaField("className", stringValue(), field: \EmojiNode.__className)])
  }

  /// `new EmojiNode(className, text, key)`.
  convenience init(_ className: String, text: String = "", key: NodeKey? = nil) throws {
    try self.init(text, key: key)
    __className = className
  }

  func getClassName() throws -> String { try getLatest().__className }

  @discardableResult
  func setClassName(_ className: String) throws -> Self {
    let writable = try getWritable()
    writable.__className = className
    return writable
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let dom = getDocument().createElement("span")
    let inner = try super.createDOM(config, editor)
    dom.className = __className
    // Add to the class names TextNode.createDOM applied for the text formats
    // rather than replacing them, otherwise a formatted emoji renders unstyled.
    addClassNamesToElement(inner, "emoji-inner")
    dom.appendChild(inner)
    return dom
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    guard let inner = dom.firstChild as? DOMElement else { return true }
    // The outer span carries the class, which setClassName and the
    // schema-driven updateFromJSON can change on an existing node.
    if (prevNode as! EmojiNode).__className != __className {
      dom.className = __className
    }
    // TextNode.updateDOM returns true when the format change needs a
    // different tag, in which case it has not touched the DOM at all.
    return try super.updateDOM(prevNode, inner, config)
  }
}

func isEmojiNode(_ node: LexicalNode?) -> Bool { node is EmojiNode }

/// `$createEmojiNode(className, emojiText)`.
func createEmojiNode(_ className: String, _ emojiText: String) throws -> EmojiNode {
  try applyNodeReplacement(EmojiNode(className, text: emojiText).setMode("token"))
}

/// The emoticons and what each becomes: its class names and character.
private let emojis: [String: (String, String)] = [
  ":)": ("emoji happysmile", "🙂"),
  ":D": ("emoji veryhappysmile", "😀"),
  ":(": ("emoji unhappysmile", "🙁"),
  "<3": ("emoji heart", "❤"),
]

/// `$findAndTransformEmoji`: replaces the first emoticon in `node` with its
/// emoji, which it returns.
private func findAndTransformEmoji(_ node: TextNode) throws -> TextNode? {
  let text = Array(try node.getTextContent().utf16)
  for i in text.indices {
    let one = String(decoding: text[i..<(i + 1)], as: UTF16.self)
    let two = String(decoding: text[i..<min(i + 2, text.count)], as: UTF16.self)
    guard let (emojiStyle, emojiText) = emojis[one] ?? emojis[two] else { continue }
    let targetNode =
      i == 0 ? try node.splitText(i + 2)[0] : try node.splitText(i, i + 2)[1]
    let emojiNode = try createEmojiNode(emojiStyle, emojiText)
    try targetNode.replace(emojiNode)
    return emojiNode
  }
  return nil
}

/// `$textNodeTransform`.
private func textNodeTransform(_ node: TextNode) throws {
  var targetNode: TextNode? = node
  while let current = targetNode {
    if try !current.isSimpleText() { return }
    targetNode = try findAndTransformEmoji(current)
  }
}

/// `EmojisExtension`.
let EmojisExtension = defineExtension(
  name: "@lexical/playground/Emojis", nodes: { [.klass(EmojiNode.self)] },
  register: { editor, _, _ in
    try editor.registerNodeTransform(TextNode.self) { try textNodeTransform($0) }
  })
