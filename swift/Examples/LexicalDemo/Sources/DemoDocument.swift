import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import SwiftUI

// A small document with a heading, a quote, formatted text and two
// decorators (one inline, one block), for the view tests and the
// screenshots.

/// An inline decorator: a SwiftUI capsule with its label.
final class BadgeNode: DecoratorNode {
  var __label = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      BadgeNode.self, type: "demo-badge", extends: DecoratorNode.self,
      json: [SchemaField("label", stringValue(), field: \BadgeNode.__label)])
  }
  override func getTextContent() throws -> String { try getLatest().__label }
  override func getTextContentSize() throws -> Int { try getTextContent().utf16.count }
  override func decorate(_ editor: Editor) throws -> Any? {
    let label = try getLatest().__label
    return Text(label)
      .font(.system(size: 12, weight: .semibold))
      .foregroundStyle(.white)
      .padding(.horizontal, 8)
      .padding(.vertical, 2)
      .background(Capsule().fill(Color.blue))
  }
}

/// A block decorator: a SwiftUI card.
final class CardNode: DecoratorNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(CardNode.self, type: "demo-card", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? {
    HStack(spacing: 10) {
      Image(systemName: "swift")
        .font(.system(size: 22))
        .foregroundStyle(.orange)
      VStack(alignment: .leading, spacing: 2) {
        Text("A block decorator").font(.system(size: 14, weight: .semibold))
        Text("DecoratorNode.decorate(_:) returns this SwiftUI view.")
          .font(.system(size: 12))
          .foregroundStyle(.secondary)
      }
      Spacer()
    }
    .padding(12)
    .background(RoundedRectangle(cornerRadius: 10).fill(Color.orange.opacity(0.12)))
  }
}

@MainActor
func makeDemoEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "demo",
      dependencies: [RichTextExtension, HistoryExtension, NormalizeTripleClickSelectionExtension],
      nodes: { [.klass(BadgeNode.self), .klass(CardNode.self)] }))
}

/// Fills the editor with the demo document.
func loadDemoDocument(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let root = try getRoot().clear()
    try root.append(createHeadingNode("h1").append(createTextNode("Lexical for Swift")))
    let badge = try create(BadgeNode.self)
    badge.__label = "SwiftUI"
    try root.append(
      createParagraphNode().append(
        createTextNode("Native editing with "),
        createTextNode("bold").setFormat("bold"),
        createTextNode(", "),
        createTextNode("italic").setFormat("italic"),
        createTextNode(", "),
        createTextNode("inline code").setFormat("code"),
        createTextNode(" and an inline decorator "),
        badge,
        createTextNode(".")))
    try root.append(
      createQuoteNode().append(
        createTextNode("A quote, with its bar drawn behind the text by the view.")))
    try root.append(createHeadingNode("h2").append(createTextNode("Decorators")))
    try root.append(create(CardNode.self))
    try root.append(
      createParagraphNode().append(
        createTextNode("Typed through the model, rendered by TextKit 2.")))
    _ = try (root.getLastDescendant() as? TextNode)?.select()
  }
}
