import Lexical
import LexicalHTML
import Testing

/// Replays `vectors/html/import.json`: `generateNodesFromDOM` makes the
/// same nodes from parsed HTML as `$generateNodesFromDOM` does in jsdom.
@Suite(.enabled(if: Vectors.available))
struct HTMLImportTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let html: String
    let nodes: String
    var testDescription: String { String(html.prefix(60)) }
  }

  static let cases: [Case] = (try? Vectors.load("html/import")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/import", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  /// A node and its subtree, as `exportJSON` writes each node.
  static func nodeToJSON(_ node: LexicalNode) throws -> JSONValue {
    var json = try node.exportJSON()
    if let element = node as? ElementNode {
      json["children"] = .array(try element.getChildren().map(nodeToJSON))
    }
    return .object(json)
  }

  @Test(arguments: cases)
  func importNodes(_ vector: Case) throws {
    let editor = try createEditor(CreateEditorArgs(headless: true))
    let dom = DOMParser().parseFromString(vector.html, "text/html")
    var nodes = ""
    try editor.update(
      {
        nodes = try JSONValue.array(generateNodesFromDOM(editor, dom).map(Self.nodeToJSON))
          .stringify()
      }, EditorUpdateOptions(discrete: true))
    #expect(nodes == vector.nodes)
  }
}
