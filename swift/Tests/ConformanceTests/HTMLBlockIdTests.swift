import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalList
import LexicalRichText
import Testing

/// Replays `vectors/html/block-id.json`: `BlockIdHTMLExtension`, in an editor
/// built from the package extensions and `BlockIdExtension` (generated ids
/// id1, id2...), writes the HTML `$generateHtmlFromNodes` writes in jsdom
/// (positional ids, or handles, or none), and its import of HTML carrying ids
/// leaves the editor state it left.
@Suite(.enabled(if: Vectors.available))
struct HTMLBlockIdTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let scenario: String
    let editorState: String?
    let html: String?
    let importHtml: String?
    let state: String?
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("html/block-id")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/block-id", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  /// `blockIdEditor(scenario)`.
  static func blockIdEditor(_ scenario: String) throws -> Editor {
    var config = BlockIdHTMLConfig()
    switch scenario {
    case "default": break
    case "export-off": config.export = false
    case "handles": config.handles = true
    case "import-off": config.import = false
    default: throw LexicalError("unknown scenario \(scenario)")
    }
    var n = 0
    return try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]",
        dependencies: [
          RichTextExtension, CheckListExtension, LinkExtension, CodeExtension,
          configExtension(BlockIdExtension) {
            $0.generateId = {
              n += 1
              return "id\(n)"
            }
          },
          configExtension(BlockIdHTMLExtension) {
            $0.export = config.export
            $0.handles = config.handles
            $0.import = config.import
          },
        ],
        nodes: { HTMLClipboardTests.documentNodes.map { .klass($0) } }))
  }

  @Test(arguments: cases)
  func blockId(_ vector: Case) throws {
    let editor = try Self.blockIdEditor(vector.scenario)
    defer { editor.dispose() }
    if let editorState = vector.editorState {
      try editor.setEditorState(editor.parseEditorState(editorState))
    } else {
      // Edited first, so that the import is not a load.
      try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) {
        _ = try getRoot().append(createParagraphNode())
      }
      try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) { try getRoot().clear() }
    }
    guard let importHtml = vector.importHtml else {
      #expect(try editor.read { try generateHtmlFromNodes(editor) } == vector.html)
      return
    }
    let dom = DOMParser().parseFromString(importHtml, "text/html")
    try editor.update(discrete: true) {
      let nodes = try generateNodesFromDOMViaExtension(dom).map { node -> LexicalNode in
        try node.isInline() ? try createParagraphNode().append(node) : node
      }
      _ = try getRoot().append(contentsOf: nodes)
    }
    #expect(try editor.getEditorState().toJSON().stringify() == vector.state)
  }
}
