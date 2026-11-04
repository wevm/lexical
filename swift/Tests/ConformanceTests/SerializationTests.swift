import Lexical
import LexicalList
import Testing

/// Replays `vectors/serialization/*.json`: each document parses from both
/// forms and re-exports, byte for byte, to the JSON text JS writes.
@Suite(.enabled(if: Vectors.available))
struct SerializationTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let legacy: String
    let compact: String
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("serialization/core")) ?? []
  static let listCases: [Case] = (try? Vectors.load("serialization/list")) ?? []

  @Test(arguments: cases)
  func roundTrip(_ vector: Case) throws {
    try check(vector, createEditor())
  }

  @Test(arguments: listCases)
  func listRoundTrip(_ vector: Case) throws {
    try check(
      vector,
      createEditor(CreateEditorArgs(nodes: [.klass(ListNode.self), .klass(ListItemNode.self)])))
  }

  func check(_ vector: Case, _ editor: Editor) throws {
    for input in [vector.legacy, vector.compact] {
      let editorState = try editor.parseEditorState(input)
      #expect(try editorState.toJSON().stringify() == vector.legacy)
      #expect(try editorState.toJSON(true).stringify() == vector.compact)
    }
  }
}
