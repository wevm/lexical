import Lexical
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalMark
import LexicalRichText
import Testing

/// Replays `vectors/html/mark-state.json`: in an editor built from
/// `RichTextExtension`, `LinkExtension` and the scenario's mark flavors,
/// `$generateHtmlFromNodes` writes the ids attribute of each flavor with
/// `exportIDsToHTML` as it does in jsdom, and `DOMImportExtension`'s import of
/// HTML carrying ids makes the nodes it made.
@Suite(.enabled(if: Vectors.available))
struct HTMLMarkStateTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let scenario: String
    let editorState: String?
    let html: String?
    let importHtml: String?
    let nodes: String?
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("html/mark-state")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/mark-state", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  /// `MARK_STATE_SCENARIOS` in html.test.ts.
  static func markStateEditor(_ scenario: String) throws -> Editor {
    let exportIDs = Partial<MarkStateConfig> { $0.set(\.exportIDsToHTML, true) }
    let flavors: [any AnyLexicalExtensionArgument]
    switch scenario {
    case "default": flavors = [MarkStateExtension]
    case "export": flavors = [configExtension(MarkStateExtension, exportIDs)]
    case "flavors":
      guard let comments = markFlavor("CommentMarks"),
        let suggestions = markFlavor("SuggestionMarks")
      else { throw LexicalError("no mark flavors") }
      flavors = [
        configExtension(MarkStateExtension, exportIDs), configExtension(comments, exportIDs),
        suggestions,
      ]
    default: throw LexicalError("unknown scenario \(scenario)")
    }
    return try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]",
        dependencies: [RichTextExtension, LinkExtension] + flavors))
  }

  @Test(arguments: cases)
  func markState(_ vector: Case) throws {
    let editor = try Self.markStateEditor(vector.scenario)
    defer { editor.dispose() }
    guard let importHtml = vector.importHtml else {
      try editor.setEditorState(editor.parseEditorState(vector.editorState ?? ""))
      #expect(try editor.read { try generateHtmlFromNodes(editor) } == vector.html)
      return
    }
    let dom = DOMParser().parseFromString(importHtml, "text/html")
    var nodes = ""
    try editor.update(discrete: true) {
      nodes = try JSONValue.array(
        generateNodesFromDOMViaExtension(dom).map(HTMLImportTests.nodeToJSON)
      ).stringify()
    }
    #expect(nodes == vector.nodes)
  }
}
