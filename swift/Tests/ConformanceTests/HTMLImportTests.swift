import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalHashtag
import LexicalLink
import LexicalList
import LexicalOverflow
import LexicalRichText
import Testing

/// Replays `vectors/html/import.json` and `vectors/html/import-nodes.json`:
/// `generateNodesFromDOM` makes the same nodes from parsed HTML as
/// `$generateNodesFromDOM` does in jsdom, with the core nodes and with the
/// node packages' (those of `property/documents` and `HorizontalRuleNode`).
@Suite(.enabled(if: Vectors.available))
struct HTMLImportTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let html: String
    let nodes: String
    var testDescription: String { String(html.prefix(60)) }
  }

  static let cases: [Case] = (try? Vectors.load("html/import")) ?? []
  static let nodesCases: [Case] = (try? Vectors.load("html/import-nodes")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/import", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
    #expect(try Vectors.load("html/import-nodes", as: Case.self).count == Self.nodesCases.count)
    #expect(!Self.nodesCases.isEmpty)
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
    try Self.check(vector, try createEditor(CreateEditorArgs(headless: true)))
  }

  @Test(arguments: nodesCases)
  func importPackageNodes(_ vector: Case) throws {
    let nodes: [LexicalNode.Type] = [
      HeadingNode.self, QuoteNode.self, ListNode.self, ListItemNode.self, LinkNode.self,
      AutoLinkNode.self, CodeNode.self, CodeHighlightNode.self, OverflowNode.self,
      HashtagNode.self, HorizontalRuleNode.self,
    ]
    try Self.check(
      vector,
      try createEditor(
        CreateEditorArgs(namespace: "", nodes: nodes.map { .klass($0) }, headless: true)))
  }

  static func check(_ vector: Case, _ editor: Editor) throws {
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
