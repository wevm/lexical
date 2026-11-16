import Lexical
import LexicalClipboard
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalHashtag
import LexicalLink
import LexicalList
import LexicalOverflow
import LexicalRichText
import Testing

/// Replays `vectors/html/clipboard.json`: at each case's selection,
/// `getClipboardDataFromSelection` gives the entries jsdom's copy gave, or
/// `insertDataTransferForRichText` of the case's data leaves the editor
/// state and selection JS's paste left, in the case's scenario's editor
/// (`clipboardEditor` in html.test.ts, repeated here).
@Suite(.enabled(if: Vectors.available))
struct HTMLClipboardTests {
  /// A point: the child indices from the root to its node, its offset and
  /// type.
  struct PathPoint: Decodable, Equatable, Sendable {
    let path: [Int]
    let offset: Int
    let type: String

    init(path: [Int], offset: Int, type: String) {
      (self.path, self.offset, self.type) = (path, offset, type)
    }

    init(from decoder: Decoder) throws {
      var container = try decoder.unkeyedContainer()
      path = try container.decode([Int].self)
      offset = try container.decode(Int.self)
      type = try container.decode(String.self)
    }
  }

  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let scenario: String
    let editorState: String
    let selection: [PathPoint]
    let clipboard: [[String]]?
    let data: [[String]]?
    let state: String?
    let after: [PathPoint]?
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("html/clipboard")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/clipboard", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  /// `DOCUMENT_NODES`.
  static let documentNodes: [LexicalNode.Type] = [
    HeadingNode.self, QuoteNode.self, ListNode.self, ListItemNode.self, LinkNode.self,
    AutoLinkNode.self, CodeNode.self, CodeHighlightNode.self, OverflowNode.self,
    HashtagNode.self,
  ]

  /// `clipboardEditor(scenario)`, with the disposer of an editor built from
  /// extensions.
  static func clipboardEditor(_ scenario: String) throws -> (Editor, () -> Void) {
    if scenario == "headless" {
      let editor = try createEditor(
        CreateEditorArgs(
          namespace: "conformance", nodes: documentNodes.map { .klass($0) }, headless: true))
      return (editor, {})
    }
    let dependencies: [any AnyLexicalExtensionArgument]
    switch scenario {
    case "custom-export":
      // `CUSTOM_EXPORT`.
      dependencies = [
        configExtension(GetClipboardDataExtension) {
          $0.exportMimeType = [
            "application/x-lexical-editor": [{ _, _ in nil }],
            "application/x-node-count": [
              { selection, _ in try selection.map { String(try $0.getNodes().count) } }
            ],
            "text/html": [
              { _, next in
                guard let html = try next() else { return nil }
                return html.isEmpty ? html : "<!--wrapped-->\(html)"
              }
            ],
          ]
        }
      ]
    case "dom-import":
      dependencies = [
        RichTextExtension, CheckListExtension, LinkExtension, CodeExtension,
        ClipboardDOMImportExtension,
      ]
    case "next-data":
      dependencies = [
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "text/html": [{ html, _, next, _ in try next("<p><b>first</b></p>\(html)") }]
          ]
        }
      ]
    case "priority":
      dependencies = [
        configExtension(ClipboardImportExtension) { $0.priority = ["text/plain": 5] }
      ]
    default:
      throw LexicalError("unknown scenario \(scenario)")
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]", dependencies: dependencies,
        nodes: { documentNodes.map { .klass($0) } }))
    return (editor, { editor.dispose() })
  }

  static func node(at path: [Int]) throws -> LexicalNode {
    var node: LexicalNode = try getRoot()
    for i in path {
      guard let child = try (node as? ElementNode)?.getChildAtIndex(i) else {
        throw LexicalError("no node at \(path)")
      }
      node = child
    }
    return node
  }

  static func path(of node: LexicalNode) throws -> [Int] {
    var path: [Int] = []
    var current = node
    while let parent = try current.getParent() {
      path.insert(try current.getIndexWithinParent(), at: 0)
      current = parent
    }
    return path
  }

  /// `$selectPaths([anchor, focus])`.
  static func select(_ points: [PathPoint]) throws {
    let selection = createRangeSelection()
    for (point, target) in zip(points, [selection.anchor, selection.focus]) {
      try target.set(
        try node(at: point.path).getKey(), point.offset, point.type == "text" ? .text : .element)
    }
    try setSelection(selection)
  }

  /// `$selectionPaths()`.
  static func selectionPaths() throws -> [PathPoint]? {
    guard let selection = try getSelection() as? RangeSelection else { return nil }
    return try [selection.anchor, selection.focus].map { point in
      PathPoint(
        path: try path(of: try getNodeByKeyOrThrow(point.key)), offset: point.offset,
        type: point.type.rawValue)
    }
  }

  @Test(arguments: cases)
  func clipboard(_ vector: Case) throws {
    let (editor, dispose) = try Self.clipboardEditor(vector.scenario)
    defer { dispose() }
    try editor.setEditorState(editor.parseEditorState(vector.editorState))
    guard let data = vector.data else {
      var clipboard: [[String]] = []
      try editor.update(
        {
          try Self.select(vector.selection)
          clipboard = try getClipboardDataFromSelection().map { [$0.key, $0.value] }
        }, EditorUpdateOptions(discrete: true))
      #expect(clipboard == vector.clipboard)
      return
    }
    let dataTransfer = DataTransfer()
    for entry in data {
      dataTransfer.setData(entry[0], entry[1])
    }
    try editor.update(
      {
        try Self.select(vector.selection)
        guard let selection = try getSelection() else { throw LexicalError("no selection") }
        try insertDataTransferForRichText(dataTransfer, selection, editor)
      }, EditorUpdateOptions(discrete: true))
    #expect(try editor.getEditorState().toJSON().stringify() == vector.state)
    #expect(try editor.read { try Self.selectionPaths() } == vector.after)
  }
}
