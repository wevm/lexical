import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import Testing

@testable import LexicalPlayground

/// `EmojisExtension` and `EmojiNode`, compared byte for byte with what the
/// web playground makes of the same text.
@MainActor
struct EmojiNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EmojiNodeTests",
        dependencies: [RichTextExtension, EmojisExtension]))
  }

  private let typed = "Hi :) and :D, :( <3!"
  private let webJSON =
    #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hi ","type":"text","version":1},{"className":"emoji happysmile","detail":0,"format":0,"mode":"token","style":"","text":"🙂","type":"emoji","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":" and ","type":"text","version":1},{"className":"emoji veryhappysmile","detail":0,"format":0,"mode":"token","style":"","text":"😀","type":"emoji","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":", ","type":"text","version":1},{"className":"emoji unhappysmile","detail":0,"format":0,"mode":"token","style":"","text":"🙁","type":"emoji","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":" ","type":"text","version":1},{"className":"emoji heart","detail":0,"format":0,"mode":"token","style":"","text":"❤","type":"emoji","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"!","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
  private let webHTML =
    #"<p><span style="white-space: pre-wrap;">Hi </span><span class="emoji happysmile" style="white-space: pre-wrap;"><span class="emoji-inner">🙂</span></span><span style="white-space: pre-wrap;"> and </span><span class="emoji veryhappysmile" style="white-space: pre-wrap;"><span class="emoji-inner">😀</span></span><span style="white-space: pre-wrap;">, </span><span class="emoji unhappysmile" style="white-space: pre-wrap;"><span class="emoji-inner">🙁</span></span><span style="white-space: pre-wrap;"> </span><span class="emoji heart" style="white-space: pre-wrap;"><span class="emoji-inner">❤</span></span><span style="white-space: pre-wrap;">!</span></p>"#

  private func type(_ text: String, in editor: Editor) throws {
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
    }
  }

  /// Each emoticon becomes an emoji node, as the web's transform makes it.
  @Test func turnsEmoticonsIntoEmojiAsTheWebDoes() throws {
    let editor = try makeEditor()
    try type(typed, in: editor)
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
    #expect(try editor.read { try generateHtmlFromNodes(editor) } == webHTML)
  }

  /// The web's JSON parses back to the same document.
  @Test func parsesTheWebsJSON() throws {
    let editor = try makeEditor()
    try editor.setEditorState(editor.parseEditorState(webJSON))
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
  }

  /// An image's caption turns emoticons into emoji too, as its nested
  /// editor has `EmojisExtension`, and takes the mentions its menu inserts.
  @Test func captionTakesEmojiAndMentions() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EmojiNodeTests/caption",
        dependencies: [RichTextExtension, ImagesExtension]))
    var caption: Editor!
    try editor.update(discrete: true) {
      let image = try createImageNode(ImagePayload(altText: "a", showCaption: true, src: "x.png"))
      caption = image.caption
      try getRoot().clear().append(createParagraphNode().append(image))
    }
    try type("ok <3", in: caption)
    let emoji = try caption.read {
      try getRoot().getAllTextNodes().compactMap { $0 as? EmojiNode }.map {
        [try $0.getClassName(), try $0.getTextContent()]
      }
    }
    #expect(emoji == [["emoji heart", "❤"]])
    try caption.update(discrete: true) {
      let paragraph = try getRoot().getFirstChildOrThrow() as! ElementNode
      try paragraph.append(createMentionNode("Luke Skywalker"))
    }
    #expect(try caption.read { try getRoot().getTextContent() } == "ok ❤Luke Skywalker")
  }
}
