import Lexical
import LexicalExtension
import LexicalHTML
import LexicalList
import LexicalRichText
import LexicalTable
import Testing

/// Replays `vectors/html/table.json`: each table document's HTML export, in
/// an editor built from `RichTextExtension` and `TableExtension` (with and
/// without `hasHorizontalScroll`, with and without a table theme), matches
/// what `$generateHtmlFromNodes` writes in jsdom. And
/// `vectors/html/import-table.json`: table HTML imports, in an editor built
/// from `RichTextExtension`, `CheckListExtension` and `TableExtension`, to
/// the nodes JS made, through `DOMImportExtension` (`nodes`) and through
/// the `importDOM` path (`legacy`).
@Suite(.enabled(if: Vectors.available))
struct HTMLTableTests {
  struct ExportCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let theme: [String: HTMLExportTests.ThemeEntry]?
    let horizontalScroll: Bool
    let editorState: String
    let html: String
    var testDescription: String {
      "\(name) (scroll: \(horizontalScroll), theme: \(theme != nil))"
    }
  }

  struct ImportCase: Decodable, CustomTestStringConvertible, Sendable {
    let html: String
    let nodes: String
    let legacy: String
    var testDescription: String { String(html.prefix(60)) }
  }

  static let exportCases: [ExportCase] = (try? Vectors.load("html/table")) ?? []
  static let importCases: [ImportCase] = (try? Vectors.load("html/import-table")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/table", as: ExportCase.self).count == Self.exportCases.count)
    #expect(!Self.exportCases.isEmpty)
    #expect(
      try Vectors.load("html/import-table", as: ImportCase.self).count == Self.importCases.count)
    #expect(!Self.importCases.isEmpty)
  }

  @Test(arguments: exportCases)
  func export(_ vector: ExportCase) throws {
    let horizontalScroll = vector.horizontalScroll
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]",
        dependencies: [
          RichTextExtension,
          configExtension(TableExtension) { $0.hasHorizontalScroll = horizontalScroll },
        ],
        editorConfig: InitialEditorConfig(theme: vector.theme?.mapValues(\.json) ?? [:])))
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(vector.editorState))
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html == vector.html)
  }

  @Test(arguments: importCases)
  func importTable(_ vector: ImportCase) throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]",
        dependencies: [RichTextExtension, CheckListExtension, TableExtension]))
    defer { editor.dispose() }
    let dom = DOMParser().parseFromString(vector.html, "text/html")
    var nodes = ""
    var legacy = ""
    try editor.update(
      {
        nodes = try JSONValue.array(
          generateNodesFromDOMViaExtension(dom).map(HTMLImportTests.nodeToJSON)
        ).stringify()
        legacy = try JSONValue.array(
          generateNodesFromDOM(editor, dom).map(HTMLImportTests.nodeToJSON)
        ).stringify()
      }, EditorUpdateOptions(discrete: true))
    #expect(nodes == vector.nodes)
    #expect(legacy == vector.legacy)
  }
}
