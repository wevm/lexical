import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// `registerTableOfContents`, the logic of @lexical/react's
// TableOfContentsPlugin, which has no upstream unit tests: the entries it
// reports as headings are added, moved, edited and removed.

@MainActor
struct TableOfContentsTests {
  /// An editor holding `blocks` (a heading tag or `"p"`, and the text),
  /// and the entries its table of contents last reported, as
  /// `"tag text"`.
  private final class Fixture {
    let editor: Editor
    var reports: [[TableOfContentsEntry]] = []
    var unregister: () -> Void = {}

    init(_ blocks: [(String, String)]) throws {
      editor = try buildEditorFromExtensions(
        defineExtension(name: "[root]", dependencies: [RichTextExtension]))
      // Mutation listeners fire from the reconciler, which a root runs.
      try editor.setRootElement(TestRootElement())
      try editor.update(discrete: true) {
        try getRoot().clear()
        for (tag, text) in blocks {
          let block: ElementNode =
            tag == "p" ? try createParagraphNode() : try createHeadingNode(tag)
          try getRoot().append(block.append(createTextNode(text)))
        }
      }
      unregister = try registerTableOfContents(editor) { [unowned self] in
        self.reports.append($0)
      }
    }

    var entries: [String] { (reports.last ?? []).map { "\($0.tag) \($0.text)" } }

    /// The heading or paragraph whose text is `text`.
    func block(_ text: String) throws -> ElementNode {
      try getRoot().getChildren().compactMap { $0 as? ElementNode }
        .first { try $0.getTextContent() == text }!
    }
  }

  @Test func reportsTheHeadingsInDocumentOrderAtOnce() throws {
    let fixture = try Fixture([("h1", "One"), ("p", "Body"), ("h2", "Two"), ("h3", "Three")])
    #expect(fixture.reports.count == 1)
    #expect(fixture.entries == ["h1 One", "h2 Two", "h3 Three"])
  }

  /// A heading inserted between two others lands after the one before it,
  /// not at either end.
  @Test func placesANewHeadingAfterThePreviousOne() throws {
    let fixture = try Fixture([("h1", "One"), ("p", "Body"), ("h2", "Three")])
    try fixture.editor.update(discrete: true) {
      try fixture.block("Body").insertAfter(
        createHeadingNode("h2").append(createTextNode("Two")))
    }
    #expect(fixture.entries == ["h1 One", "h2 Two", "h2 Three"])
    try fixture.editor.update(discrete: true) {
      try fixture.block("One").insertBefore(
        createHeadingNode("h3").append(createTextNode("Zero")))
    }
    #expect(fixture.entries == ["h3 Zero", "h1 One", "h2 Two", "h2 Three"])
  }

  @Test func followsAHeadingsTextAndTag() throws {
    let fixture = try Fixture([("h1", "One"), ("h2", "Two")])
    try fixture.editor.update(discrete: true) {
      let text = try fixture.block("Two").getFirstChild() as! TextNode
      try text.setTextContent("Second")
    }
    #expect(fixture.entries == ["h1 One", "h2 Second"])
    try fixture.editor.update(discrete: true) {
      try (fixture.block("One") as! HeadingNode).setTag("h3")
    }
    #expect(fixture.entries == ["h3 One", "h2 Second"])
  }

  /// A heading moved below another moves in the list, and a removed one
  /// leaves it.
  @Test func followsMovedAndRemovedHeadings() throws {
    let fixture = try Fixture([("h1", "One"), ("h2", "Two"), ("p", "Body"), ("h2", "Three")])
    try fixture.editor.update(discrete: true) {
      try fixture.block("Body").insertAfter(fixture.block("One"))
    }
    #expect(fixture.entries == ["h2 Two", "h1 One", "h2 Three"])
    try fixture.editor.update(discrete: true) { try fixture.block("Two").remove() }
    #expect(fixture.entries == ["h1 One", "h2 Three"])
  }

  @Test func stopsReportingWhenUnregistered() throws {
    let fixture = try Fixture([("h1", "One")])
    fixture.unregister()
    let count = fixture.reports.count
    try fixture.editor.update(discrete: true) {
      try getRoot().append(createHeadingNode("h2").append(createTextNode("Two")))
    }
    #expect(fixture.reports.count == count)
  }
}
