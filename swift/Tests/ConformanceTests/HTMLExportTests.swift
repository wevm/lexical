import Lexical
import LexicalHTML
import Testing

/// Replays `vectors/html/export.json`: each document's HTML export matches
/// what `$generateHtmlFromNodes` writes in jsdom, byte for byte.
@Suite(.enabled(if: Vectors.available))
struct HTMLExportTests {
  /// A theme entry: class names, or (for `text`) class names by format.
  enum ThemeEntry: Decodable, Sendable {
    case names(String)
    case nested([String: String])

    init(from decoder: Decoder) throws {
      let container = try decoder.singleValueContainer()
      if let names = try? container.decode(String.self) {
        self = .names(names)
      } else {
        self = .nested(try container.decode([String: String].self))
      }
    }

    var json: JSONValue {
      switch self {
      case .names(let names): .string(names)
      case .nested(let entries):
        .object(JSONObject(entries.sorted { $0.key < $1.key }.map { ($0.key, .string($0.value)) }))
      }
    }
  }

  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let theme: [String: ThemeEntry]?
    let editorState: String
    let html: String
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("html/export")) ?? []

  /// The vectors decode (a decoding error would leave no cases to run).
  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/export", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  @Test(arguments: cases)
  func export(_ vector: Case) throws {
    let editor = try createEditor(
      CreateEditorArgs(theme: vector.theme?.mapValues(\.json), headless: true))
    try editor.setEditorState(editor.parseEditorState(vector.editorState))
    let html = try editor.read { try generateHtmlFromNodes(editor) }
    #expect(html == vector.html)
  }
}
