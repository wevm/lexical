import Foundation
import Lexical
import LexicalDevtoolsCore
import LexicalRichText
import Testing

/// Replays `vectors/devtools/tree.json`: the playground's tree view
/// (`generateContent(editor, [], false)` in `@lexical/devtools-core`) of each
/// document and selection matches JS, with JS's node keys mapped to the
/// port's (both listed in tree order) and the port's version.
@Suite(.enabled(if: Vectors.available))
struct TreeViewTests {
  struct PointPath: Decodable, Sendable {
    let path: [Int]
    let offset: Int
    let type: String
  }

  struct SelectionPaths: Decodable, Sendable {
    let type: String
    let anchor: PointPath?
    let focus: PointPath?
    let format: Int?
    let style: String?
    let paths: [[Int]]?
  }

  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let editorState: String
    let selection: SelectionPaths?
    let editable: Bool
    let keys: [String]
    let version: String
    let tree: String
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("devtools/tree")) ?? []

  /// The vectors decode (a decoding error would leave no cases to run).
  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("devtools/tree", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  @Test(arguments: cases)
  func tree(_ vector: Case) throws {
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "playground", nodes: [.klass(HeadingNode.self), .klass(QuoteNode.self)],
        onError: { throw $0 }, headless: true))
    try editor.setEditorState(editor.parseEditorState(vector.editorState))
    try editor.update(discrete: true) { try apply(vector.selection) }
    try editor.setEditable(vector.editable)
    var keys: [NodeKey] = []
    try editor.read {
      func visit(_ node: LexicalNode) throws {
        for child in try (node as? ElementNode)?.getChildren() ?? [] {
          keys.append(child.getKey())
          try visit(child)
        }
      }
      try visit(getRoot())
    }
    try #require(keys.count == vector.keys.count)
    let swiftKey = Dictionary(uniqueKeysWithValues: zip(vector.keys, keys))
    let tree = try generateContent(editor, [], false)
    let want = try expected(vector, swiftKey)
    #expect(tree == want)
  }

  /// The JS tree with its keys replaced by the port's, and its version line
  /// by the port's.
  private func expected(_ vector: Case, _ swiftKey: [String: NodeKey]) throws -> String {
    // A `^` line is padded by its node's printed key, whose width differs
    // when the keys do.
    var lines: [String] = []
    var widthChange = 0
    for line in vector.tree.split(separator: "\n", omittingEmptySubsequences: false) {
      if let key = line.firstMatch(of: #/^[> ] [ |├└]* \((\d+)\) /#)?.output.1 {
        widthChange = String(key).count - (swiftKey[String(key)]?.count ?? key.count)
        lines.append(String(line))
      } else if line.hasPrefix(">"), let run = line.firstMatch(of: #/ +\^/#) {
        let start = run.range.lowerBound
        let kept = line.index(start, offsetBy: max(0, widthChange))
        lines.append(
          String(line[..<start]) + String(repeating: " ", count: max(0, -widthChange))
            + line[kept...])
      } else {
        lines.append(String(line))
      }
    }
    var tree = lines.joined(separator: "\n").replacing(
      "(v\(vector.version))", with: "(v\(Editor.version))")
    for pattern in [#/\((\d+)\)/#, #/key: (\d+)/#] {
      tree = tree.replacing(pattern) { match in
        match.output.0.replacing(match.output.1, with: swiftKey[String(match.output.1)] ?? "?")
      }
    }
    // A node selection's `[k1, k2]`.
    return tree.replacing(#/└ \[([\d, ]*)\]/#) { match in
      let mapped = match.output.1.split(separator: ", ").map { swiftKey[String($0)] ?? "?" }
      return "└ [\(mapped.joined(separator: ", "))]"
    }
  }

  private func node(at path: [Int]) throws -> LexicalNode? {
    var node: LexicalNode? = try getRoot()
    for index in path {
      node = try (node as? ElementNode)?.getChildAtIndex(index)
    }
    return node
  }

  private func apply(_ selection: SelectionPaths?) throws {
    guard let selection else { return try setSelection(nil) }
    if selection.type == "node" {
      let nodes = createNodeSelection()
      for path in selection.paths ?? [] { try nodes.add(#require(try node(at: path)).getKey()) }
      return try setSelection(nodes)
    }
    let anchor = try #require(selection.anchor)
    let focus = try #require(selection.focus)
    let anchorNode = try #require(try node(at: anchor.path))
    let focusNode = try #require(try node(at: focus.path))
    let range =
      try (anchorNode as? TextNode)?.select(0, 0)
      ?? #require(anchorNode as? ElementNode).select(0, 0)
    try range.anchor.set(
      anchorNode.getKey(), anchor.offset, PointType(rawValue: anchor.type)!)
    try range.focus.set(focusNode.getKey(), focus.offset, PointType(rawValue: focus.type)!)
    if let format = selection.format { range.format = format }
    if let style = selection.style { range.style = style }
  }
}
