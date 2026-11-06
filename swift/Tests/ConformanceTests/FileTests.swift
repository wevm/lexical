import Lexical
import LexicalFile
import LexicalList
import Testing

/// Replays `vectors/file/*.json`: the exact text of a `.lexical` document
/// that `@lexical/file` writes for an editor state and a config, and the
/// editor state it reads back from a document.
@Suite(.enabled(if: Vectors.available))
struct FileTests {
  struct Config: Decodable, Sendable {
    let source: String?
    let lastSaved: Double?
  }

  struct ExportCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let editorState: String
    let config: Config
    let document: String
    var testDescription: String { name }
  }

  struct ImportCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let document: String
    let editorState: String
    var testDescription: String { name }
  }

  static let exportCases: [ExportCase] = (try? Vectors.load("file/export")) ?? []
  static let importCases: [ImportCase] = (try? Vectors.load("file/import")) ?? []

  static func newEditor() throws -> Editor {
    try createEditor(
      CreateEditorArgs(nodes: [.klass(ListNode.self), .klass(ListItemNode.self)]))
  }

  @Test(arguments: exportCases)
  func serializedDocument(_ vector: ExportCase) throws {
    let editorState = try Self.newEditor().parseEditorState(vector.editorState)
    let document = try serializedDocumentFromEditorState(
      editorState, source: vector.config.source, lastSaved: vector.config.lastSaved)
    #expect(document.stringify() == vector.document)
  }

  @Test(arguments: exportCases)
  func documentRoundTrip(_ vector: ExportCase) throws {
    let editorState = try editorStateFromSerializedDocument(Self.newEditor(), vector.document)
    #expect(try editorState.toJSON().stringify() == vector.editorState)
  }

  @Test(arguments: importCases)
  func editorStateFromDocument(_ vector: ImportCase) throws {
    let editorState = try editorStateFromSerializedDocument(Self.newEditor(), vector.document)
    #expect(try editorState.toJSON().stringify() == vector.editorState)
  }
}
