import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import LexicalUtils
import Testing

@testable import LexicalPlayground

/// The poll and sticky nodes' JSON and HTML, compared byte for byte with
/// what the web playground's PollNode and StickyNode write for the same
/// documents, and the poll's option edits.
@MainActor
struct PollStickyNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/PollStickyNodeTests",
        dependencies: [RichTextExtension, PollExtension, PlaygroundDOMRenderExtension],
        nodes: { [.klass(StickyNode.self)] }))
  }

  private let webJSON =
    #"{"root":{"children":[{"children":[{"type":"poll","version":1,"question":"Q \"1\" <b>","options":[{"text":"A & B","uid":"abcde","votes":["Cat"]},{"text":"","uid":"fghij","votes":[]}]}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"caption":{"editorState":{"root":{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}}},"color":"pink","xOffset":10.5,"yOffset":20,"type":"sticky","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
  /// The web playground's `PlaygroundDOMRenderExtension` writes a paragraph
  /// holding a block as `<div role="paragraph">`.
  private let webHTML =
    #"<p><span data-lexical-poll-question="Q &quot;1&quot; <b>" data-lexical-poll-options="[{&quot;text&quot;:&quot;A &amp; B&quot;,&quot;uid&quot;:&quot;abcde&quot;,&quot;votes&quot;:[&quot;Cat&quot;]},{&quot;text&quot;:&quot;&quot;,&quot;uid&quot;:&quot;fghij&quot;,&quot;votes&quot;:[]}]"></span></p><div role="paragraph"><div style="display: contents;"></div></div>"#
  /// A sticky whose caption has text.
  private let webCaptionJSON =
    #"{"root":{"children":[{"children":[{"caption":{"editorState":{"root":{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hi","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}},"color":"yellow","xOffset":5,"yOffset":6,"type":"sticky","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#

  private func insertPollAndSticky(_ editor: Editor) throws {
    try editor.update(discrete: true) {
      let poll = try createPollNode(
        #"Q "1" <b>"#,
        [
          PollOption(text: "A & B", uid: "abcde", votes: ["Cat"]),
          PollOption(text: "", uid: "fghij", votes: []),
        ])
      let sticky = try createStickyNode(10.5, 20).toggleColor()
      try getRoot().clear().append(createParagraphNode().append(poll), sticky)
    }
  }

  @Test func serializesAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertPollAndSticky(editor)
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
  }

  @Test func exportsHTMLAsTheWebDoes() throws {
    let editor = try makeEditor()
    try insertPollAndSticky(editor)
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html == webHTML)
  }

  @Test func serializesAStickysCaptionAsTheWebDoes() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) {
      let sticky = try createStickyNode(5, 6)
      try sticky.caption.update(discrete: true) {
        try getRoot().append(createParagraphNode().append(createTextNode("Hi")))
      }
      try getRoot().clear().append(sticky)
    }
    #expect(try editor.getEditorState().toJSON().stringify() == webCaptionJSON)
    // And the caption parses back into the nested editor.
    let parsed = try makeEditor()
    try parsed.setEditorState(parsed.parseEditorState(webCaptionJSON))
    #expect(try parsed.getEditorState().toJSON().stringify() == webCaptionJSON)
  }

  /// The web's JSON parses back to the same document, and its HTML imports
  /// the poll with its question and options, through the
  /// `DOMImportExtension` rule and the legacy `importDOM` alike.
  @Test(arguments: [true, false])
  func parsesTheWebsJSONAndImportsItsHTML(viaExtension: Bool) throws {
    let editor = try makeEditor()
    try editor.setEditorState(editor.parseEditorState(webJSON))
    #expect(try editor.getEditorState().toJSON().stringify() == webJSON)
    let imported = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/PollStickyNodeTests/Import",
        dependencies: [RichTextExtension, CoreImportExtension, PollExtension]))
    try imported.update {
      let dom = DOMParser().parseFromString(webHTML)
      let nodes =
        try viaExtension
        ? generateNodesFromDOMViaExtension(dom) : generateNodesFromDOM(imported, dom)
      try getRoot().clear().append(createParagraphNode())
      try getRoot().selectEnd().insertNodes(nodes)
    }
    let polls = try imported.read {
      try dfs(getRoot()).compactMap { $0.node as? PollNode }.map {
        (try $0.getQuestion(), try $0.getOptions().map { "\($0.uid):\($0.text):\($0.votes)" })
      }
    }
    #expect(polls.count == 1)
    #expect(polls.first?.0 == #"Q "1" <b>"#)
    #expect(polls.first?.1 == [#"abcde:A & B:["Cat"]"#, "fghij::[]"])
  }

  /// Votes toggle per user, and edits keep each option's uid; an option
  /// that is no longer in the poll changes nothing.
  @Test func editsOptions() throws {
    let editor = try makeEditor()
    try insertPollAndSticky(editor)
    func options() throws -> [String] {
      try editor.read {
        try dfs(getRoot()).compactMap { $0.node as? PollNode }.first!.getOptions().map {
          "\($0.uid):\($0.text):\($0.votes.joined(separator: ","))"
        }
      }
    }
    func withPoll(_ body: @escaping (PollNode, [PollOption]) throws -> Void) throws {
      try editor.update(discrete: true) {
        let poll = try dfs(getRoot()).compactMap { $0.node as? PollNode }.first!
        try body(poll, try poll.getOptions())
      }
    }
    try withPoll { poll, options in
      try poll.toggleVote(options[0], "Dog")
      try poll.toggleVote(try poll.getOptions()[0], "Cat")
    }
    #expect(try options() == ["abcde:A & B:Dog", "fghij::"])
    var stale: PollOption?
    try withPoll { poll, options in
      stale = options[1]
      try poll.setOptionText(options[1], "C")
      try poll.addOption(PollOption(text: "D", uid: "klmno", votes: []))
    }
    #expect(try options() == ["abcde:A & B:Dog", "fghij:C:", "klmno:D:"])
    try withPoll { poll, options in
      try poll.deleteOption(stale!)
      try poll.deleteOption(options[0])
    }
    #expect(try options() == ["fghij:C:", "klmno:D:"])
  }
}
