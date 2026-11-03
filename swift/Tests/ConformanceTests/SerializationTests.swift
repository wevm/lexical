import Lexical
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

  @Test(arguments: cases)
  func roundTrip(_ vector: Case) throws {
    let editor = try createEditor()
    for input in [vector.legacy, vector.compact] {
      let editorState = try editor.parseEditorState(input)
      #expect(try editorState.toJSON().stringify() == vector.legacy)
      #expect(try editorState.toJSON(true).stringify() == vector.compact)
    }
  }
}
