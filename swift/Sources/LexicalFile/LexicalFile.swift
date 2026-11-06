import Foundation
import Lexical

// Port of packages/lexical-file/src/fileImportExport.ts: a `.lexical`
// document is an editor state's JSON with the time it was saved, its source
// and the Lexical version, written as `JSON.stringify` writes it. Upstream
// reads and writes the file through a DOM `<input type="file">` and a
// download link; here the app supplies both, as `ReadTextFileFromSystem`
// and `ExportBlob`, since a native app picks files with its own panels.

/// `SerializedDocument`.
public struct SerializedDocument: Equatable, Sendable {
  /// The serialized editor state produced by `editorState.toJSON()`.
  public var editorState: JSONObject
  /// The time this document was created in epoch milliseconds
  /// (`Date.now()`).
  public var lastSaved: Double
  /// The source of the document, `"Lexical"` by default.
  public var source: String
  /// The version of Lexical that produced this document.
  public var version: String

  public init(editorState: JSONObject, lastSaved: Double, source: String, version: String) {
    self.editorState = editorState
    self.lastSaved = lastSaved
    self.source = source
    self.version = version
  }

  /// The document as JSON, with JS's key order.
  public func toJSON() -> JSONObject {
    JSONObject([
      ("editorState", .object(editorState)), ("lastSaved", .number(lastSaved)),
      ("source", .string(source)), ("version", .string(version)),
    ])
  }

  /// `JSON.stringify(document)`: the text of a `.lexical` file.
  public func stringify() -> String {
    toJSON().stringify()
  }
}

/// The version `SerializedDocument.version` records: `@lexical/file`'s,
/// which is the upstream release this package conforms to.
public let lexicalFileVersion = lexicalUpstreamVersion

/// `Date.now()`: the current time in whole epoch milliseconds.
private func dateNow() -> Double {
  (Date().timeIntervalSince1970 * 1000).rounded(.down)
}

/// `serializedDocumentFromEditorState(editorState, config)`. As in JS, an
/// empty `source` is `"Lexical"` and a `lastSaved` of 0 is now.
public func serializedDocumentFromEditorState(
  _ editorState: EditorState, source: String? = nil, lastSaved: Double? = nil
) throws -> SerializedDocument {
  SerializedDocument(
    editorState: try editorState.toJSON(),
    lastSaved: lastSaved.flatMap { $0 == 0 || $0.isNaN ? nil : $0 } ?? dateNow(),
    source: source.flatMap { $0.isEmpty ? nil : $0 } ?? "Lexical",
    version: lexicalFileVersion)
}

/// `editorStateFromSerializedDocument(editor, document)`.
public func editorStateFromSerializedDocument(
  _ editor: Editor, _ document: SerializedDocument
) throws -> EditorState {
  try editor.parseEditorState(document.editorState)
}

/// `editorStateFromSerializedDocument(editor, string)`: the editor state of
/// a `.lexical` file's text.
public func editorStateFromSerializedDocument(
  _ editor: Editor, _ text: String
) throws
  -> EditorState
{
  guard case .object(let json) = try JSONValue.parse(text),
    case .object(let editorState)? = json["editorState"]
  else {
    throw LexicalError("editorStateFromSerializedDocument: expected a document with an editorState")
  }
  return try editor.parseEditorState(editorState)
}

/// `readTextFileFromSystem`: asks the user for a file whose names end in
/// `accept` (`".lexical"`), reads it as UTF-8 and calls `callback` with its
/// text. The app decides how to report an error `callback` throws, such as
/// a file that is not a Lexical document.
public typealias ReadTextFileFromSystem = (
  _ accept: String, _ callback: @escaping (String) throws -> Void
) -> Void

/// `exportBlob`: saves `text`, the JSON of a document, as the file
/// `fileName` (which ends in `.lexical`), as upstream's download link does.
public typealias ExportBlob = (_ text: String, _ fileName: String) -> Void

/// `importFile(editor)`: lets the user pick a `.lexical` file, sets the
/// editor's state to it and clears its history.
public func importFile(_ editor: Editor, readTextFileFromSystem: ReadTextFileFromSystem) {
  readTextFileFromSystem(".lexical") { text in
    try editor.setEditorState(editorStateFromSerializedDocument(editor, text))
    _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
  }
}

/// `exportFile(editor, config)`: saves the editor's state as
/// `<fileName>.lexical`, named by the current time (`toISOString()`) when
/// `fileName` is empty.
public func exportFile(
  _ editor: Editor, fileName: String? = nil, source: String? = nil, exportBlob: ExportBlob
) throws {
  let now = dateNow()
  let serializedDocument = try serializedDocumentFromEditorState(
    editor.getEditorState(), source: source, lastSaved: now)
  let name = fileName.flatMap { $0.isEmpty ? nil : $0 } ?? toISOString(now)
  exportBlob(serializedDocument.stringify(), "\(name).lexical")
}

/// `new Date(time).toISOString()`: `YYYY-MM-DDTHH:mm:ss.sssZ` in UTC.
func toISOString(_ time: Double) -> String {
  var calendar = Calendar(identifier: .gregorian)
  calendar.timeZone = TimeZone(identifier: "UTC")!
  let date = Date(timeIntervalSince1970: time / 1000)
  let c = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
  let ms = Int(time.truncatingRemainder(dividingBy: 1000))
  func pad(_ value: Int, _ width: Int) -> String {
    let digits = String(value)
    return String(repeating: "0", count: max(0, width - digits.count)) + digits
  }
  return
    "\(pad(c.year!, 4))-\(pad(c.month!, 2))-\(pad(c.day!, 2))T\(pad(c.hour!, 2)):\(pad(c.minute!, 2)):\(pad(c.second!, 2)).\(pad(ms, 3))Z"
}
