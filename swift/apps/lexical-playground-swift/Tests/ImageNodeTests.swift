import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import Testing

@testable import LexicalPlayground

/// `ImageNode`'s JSON and HTML, compared byte for byte with what the web
/// playground's node writes for the same images.
@MainActor
struct ImageNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/ImageNodeTests",
        dependencies: [RichTextExtension, ImagesExtension, PlaygroundDOMRenderExtension]))
  }

  /// Puts `image` alone in a paragraph as the document.
  private func insert(_ editor: Editor, _ image: @escaping () throws -> ImageNode) throws {
    try editor.update(tag: HISTORY_MERGE_TAG) {
      try getRoot().clear().append(createParagraphNode().append(image()))
    }
  }

  @Test func serializesAnUnsizedImageAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insert(editor) { try createImageNode(ImagePayload(altText: "alt", src: "x.png")) }
    let json = try editor.read {
      try (getRoot().getFirstDescendant() as! ImageNode).exportJSON().stringify()
    }
    #expect(
      json
        == #"{"altText":"alt","caption":{"editorState":{"root":{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}}},"height":0,"maxWidth":500,"showCaption":false,"src":"x.png","width":0,"type":"image","version":1}"#
    )
  }

  /// A 0 or absent size parses back as unsized, and an absent `maxWidth`
  /// keeps the default.
  @Test func parsesSizesAndDefaults() throws {
    let editor = try makeEditor()
    let state = try editor.parseEditorState(
      #"{"root":{"children":[{"children":[{"altText":"a","height":0,"src":"s.png","width":120,"showCaption":false,"type":"image","version":1}],"direction":null,"format":"","indent":0,"type":"paragraph","version":1,"textFormat":0,"textStyle":""}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
    )
    try editor.setEditorState(state)
    let image = try editor.read { try getRoot().getFirstDescendant() as! ImageNode }
    #expect(image.__width == .points(120))
    #expect(image.__height == .inherit)
    #expect(image.__maxWidth == 500)
  }

  @Test func exportsAnUnsizedImageAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insert(editor) { try createImageNode(ImagePayload(altText: "alt", src: "x.png")) }
    #expect(
      try editor.read { try generateHtmlFromNodes(editor) }
        == #"<p><img src="x.png" alt="alt" width="inherit" height="inherit"></p>"#)
  }

  /// A captioned image exports as a `<figure>`, so its paragraph becomes a
  /// `<div role="paragraph">`; the caption round-trips through the
  /// `DOMImportExtension` rules, as upstream imports it.
  @Test func exportsAndImportsACaptionedImage() throws {
    let editor = try makeEditor()
    try insert(editor) {
      let image = try createImageNode(
        ImagePayload(altText: "a<b", height: 20, showCaption: true, src: "/i.jpg", width: 10))
      try image.caption.update {
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("caption text")))
      }
      return image
    }
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(
      html
        == #"<div role="paragraph"><figure><img src="/i.jpg" alt="a<b" width="10" height="20"><figcaption><span style="white-space: pre-wrap;">caption text</span></figcaption></figure></div>"#
    )

    let imported = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/ImageNodeTests/Import",
        dependencies: [RichTextExtension, CoreImportExtension, ImagesExtension]))
    try imported.update {
      let nodes = try generateNodesFromDOMViaExtension(DOMParser().parseFromString(html))
      try getRoot().clear().append(createParagraphNode())
      try getRoot().selectEnd().insertNodes(nodes)
    }
    let image = try imported.read { try getRoot().getFirstDescendant() as? ImageNode }
    #expect(try imported.read { try image?.getSrc() } == "/i.jpg")
    #expect(image?.__showCaption == true)
    #expect(try image?.caption.read { try getRoot().getTextContent() } == "caption text")
  }
}
