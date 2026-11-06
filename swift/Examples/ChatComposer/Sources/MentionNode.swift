import Lexical

/// A person mentioned with "@": one segmented text entity, so the caret
/// steps over it and Backspace removes it whole. The theme draws it under
/// its type, `"mention"`.
final class MentionNode: TextNode {
  var __mention: String = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      MentionNode.self, type: "mention", extends: TextNode.self,
      json: [SchemaField("mentionName", stringValue(), field: \MentionNode.__mention)])
  }

  override func isTextEntity() -> Bool { true }

  override func canInsertTextBefore() throws -> Bool { false }

  override func canInsertTextAfter() throws -> Bool { false }
}

func createMentionNode(_ name: String) throws -> MentionNode {
  let node = try MentionNode("@" + name)
  node.__mention = name
  try node.setMode("segmented").toggleDirectionless()
  return try applyNodeReplacement(node)
}

/// Brings `MentionNode` to an editor.
let MentionExtension = defineExtension(
  name: "chat-composer/Mention", nodes: { [.klass(MentionNode.self)] })
