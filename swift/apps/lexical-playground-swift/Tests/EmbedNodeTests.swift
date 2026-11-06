import Lexical
import LexicalExtension
import LexicalHTML
import LexicalHistory
import LexicalRichText
import Testing

@testable import LexicalPlayground

/// The embed nodes' JSON, HTML and text, compared byte for byte with what
/// the web playground's YouTubeNode, TweetNode and FigmaNode write for the
/// same document.
@MainActor
struct EmbedNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EmbedNodeTests",
        dependencies: [RichTextExtension, EmbedsExtension, PlaygroundDOMRenderExtension]))
  }

  private let webJSON =
    #"{"root":{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"videoID":"jNQXAC9IVRw","format":"center","type":"youtube","version":1},{"id":"20","format":"","type":"tweet","version":1},{"documentID":"LKQ4FJ4bTnCSjedbRpk931","format":"","type":"figma","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
  private let webHTML =
    #"<p><br></p><iframe data-lexical-youtube="jNQXAC9IVRw" width="560" height="315" src="https://www.youtube-nocookie.com/embed/jNQXAC9IVRw" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen="true" title="YouTube video"></iframe><div data-lexical-tweet-id="20">https://x.com/i/web/status/20</div><div></div>"#

  private func insertEmbeds(_ editor: Editor) throws {
    try editor.update(discrete: true) {
      let youTube = try createYouTubeNode("jNQXAC9IVRw").setFormat("center")
      try getRoot().clear().append(
        createParagraphNode(), youTube, createTweetNode("20"),
        createFigmaNode("LKQ4FJ4bTnCSjedbRpk931"))
    }
  }

  @Test func serializesAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertEmbeds(editor)
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
    #expect(
      try editor.read { try getRoot().getChildren().map { try $0.getTextContent() } } == [
        "", "https://www.youtube.com/watch?v=jNQXAC9IVRw", "https://x.com/i/web/status/20",
        "https://www.figma.com/file/LKQ4FJ4bTnCSjedbRpk931",
      ])
  }

  @Test func exportsHTMLAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertEmbeds(editor)
    #expect(try editor.read { try generateHtmlFromNodes(editor) } == webHTML)
  }

  /// The web's JSON parses back to the same document, and its HTML
  /// imports the YouTube video and the tweet (Figma exports a bare
  /// `<div>`), through the `DOMImportExtension` rules and the legacy
  /// `importDOM` alike.
  @Test(arguments: [true, false])
  func parsesTheWebsJSONAndImportsItsHTML(viaExtension: Bool) throws {
    let editor = try makeEditor()
    try editor.setEditorState(editor.parseEditorState(webJSON))
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
    let imported = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/EmbedNodeTests/Import",
        dependencies: [RichTextExtension, CoreImportExtension, EmbedsExtension]))
    try imported.update {
      let dom = DOMParser().parseFromString(webHTML)
      let nodes =
        try viaExtension
        ? generateNodesFromDOMViaExtension(dom) : generateNodesFromDOM(imported, dom)
      try getRoot().clear().append(createParagraphNode())
      try getRoot().selectEnd().insertNodes(nodes)
    }
    let ids = try imported.read {
      try getRoot().getChildren().compactMap { node -> String? in
        if let video = node as? YouTubeNode { return "youtube:\(try video.getId())" }
        if let tweet = node as? TweetNode { return "tweet:\(try tweet.getId())" }
        return nil
      }
    }
    #expect(ids == ["youtube:jNQXAC9IVRw", "tweet:20"])
  }
}
