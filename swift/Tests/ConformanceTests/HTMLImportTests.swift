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
/// And `vectors/html/import-extension.json`: `DOMImportExtension`'s import,
/// in an editor built from `CoreImportExtension` and `HorizontalRuleNode`,
/// makes the nodes it made in a jsdom document. And
/// `vectors/html/import-packages.json`: the same with the packages' import
/// rules, in an editor built from their extensions. And
/// `vectors/html/import-style.json`: formatting from the CSSOM (the `font`
/// shorthand, and style sheet rules with pseudo-classes) through
/// `DOMImportExtension`, as `import-extension` is replayed.
@Suite(.enabled(if: Vectors.available))
struct HTMLImportTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let html: String
    let nodes: String
    let shadowRootQuote: Bool?
    let nodeStructure: Bool?
    var testDescription: String { String(html.prefix(60)) }
  }

  static let cases: [Case] = (try? Vectors.load("html/import")) ?? []
  static let nodesCases: [Case] = (try? Vectors.load("html/import-nodes")) ?? []
  static let extensionCases: [Case] = (try? Vectors.load("html/import-extension")) ?? []
  static let packageCases: [Case] = (try? Vectors.load("html/import-packages")) ?? []
  static let styleCases: [Case] = (try? Vectors.load("html/import-style")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/import", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
    #expect(try Vectors.load("html/import-nodes", as: Case.self).count == Self.nodesCases.count)
    #expect(!Self.nodesCases.isEmpty)
    #expect(
      try Vectors.load("html/import-extension", as: Case.self).count == Self.extensionCases.count)
    #expect(!Self.extensionCases.isEmpty)
    #expect(
      try Vectors.load("html/import-packages", as: Case.self).count == Self.packageCases.count)
    #expect(!Self.packageCases.isEmpty)
    #expect(try Vectors.load("html/import-style", as: Case.self).count == Self.styleCases.count)
    #expect(!Self.styleCases.isEmpty)
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

  @Test(arguments: extensionCases + styleCases)
  func importViaExtension(_ vector: Case) throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]", dependencies: [CoreImportExtension],
        nodes: { [.klass(HorizontalRuleNode.self)] }))
    defer { editor.dispose() }
    let dom = DOMParser().parseFromString(vector.html, "text/html")
    var nodes = ""
    try editor.update(
      {
        nodes = try JSONValue.array(generateNodesFromDOMViaExtension(dom).map(Self.nodeToJSON))
          .stringify()
      }, EditorUpdateOptions(discrete: true))
    #expect(nodes == vector.nodes)
  }

  @Test(arguments: packageCases)
  func importViaPackageExtensions(_ vector: Case) throws {
    var dependencies: [any AnyLexicalExtensionArgument] = [
      RichTextExtension, CheckListExtension, LinkExtension, CodeExtension,
      WordListImportExtension, HorizontalRuleExtension,
    ]
    if vector.shadowRootQuote == true {
      dependencies.append(
        configExtension(DOMImportExtension) { $0.rules = [ShadowRootQuoteRule.entry] })
    }
    if vector.nodeStructure == true {
      dependencies.append(NodeStructureExtension)
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "[conformance]", dependencies: dependencies))
    defer { editor.dispose() }
    let dom = DOMParser().parseFromString(vector.html, "text/html")
    var nodes = ""
    try editor.update(
      {
        nodes = try JSONValue.array(generateNodesFromDOMViaExtension(dom).map(Self.nodeToJSON))
          .stringify()
      }, EditorUpdateOptions(discrete: true))
    #expect(nodes == vector.nodes)
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
