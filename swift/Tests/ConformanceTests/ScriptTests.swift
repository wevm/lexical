import Foundation
import Lexical
import Testing

/// Replays `vectors/scripts/*.json`: every step of every script must leave
/// the same state (both JSON forms, byte for byte), selection, text and log
/// as it did in JS.
@Suite(.enabled(if: Vectors.available))
struct ScriptTests {
  struct Case: CustomTestStringConvertible, Sendable {
    let file: String
    let name: String
    let script: JSONValue
    var testDescription: String { "\(file): \(name)" }
  }

  static let files = ["updates", "selection", "transforms", "commands", "listeners", "caret"]

  static let cases: [Case] = files.flatMap { file in
    ((try? Vectors.loadJSON("scripts/\(file)")) ?? []).map { script in
      Case(file: file, name: script.objectValue?["name"]?.stringValue ?? "", script: script)
    }
  }

  @Test(arguments: cases)
  func replay(_ vector: Case) throws {
    let expected = vector.script.objectValue?["results"]?.arrayValue ?? []
    let actual = try runScript(vector.script)
    #expect(actual.count == expected.count)
    for (i, (record, want)) in zip(actual, expected).enumerated() {
      let want = try #require(want.objectValue)
      let step = "step \(i)"
      #expect(record.error == (want["error"]?.boolValue ?? false), "\(step) error")
      #expect(record.text == want["text"]?.stringValue, "\(step) text")
      #expect(
        record.selection.stringify() == (want["selection"] ?? .null).stringify(),
        "\(step) selection")
      #expect(
        JSONValue.array(record.log).stringify() == (want["log"] ?? .array([])).stringify(),
        "\(step) log")
      #expect(record.state == want["state"]?.stringValue, "\(step) state")
      #expect(record.compact == want["compact"]?.stringValue, "\(step) compact")
    }
  }
}

extension Vectors {
  /// The cases of `vectors/<name>.json` as JSON values, for vectors whose
  /// cases have no fixed shape.
  static func loadJSON(_ name: String) throws -> [JSONValue] {
    guard let directory else { throw VectorError.missingDirectory }
    let data = try String(
      contentsOf: directory.appendingPathComponent("\(name).json"), encoding: .utf8)
    let file = try JSONValue.parse(data)
    let version = file.objectValue?["formatVersion"]?.numberValue.map { Int($0) } ?? -1
    guard version == supportedFormatVersion else {
      throw VectorError.unsupportedFormat(name: name, version: version)
    }
    return file.objectValue?["cases"]?.arrayValue ?? []
  }
}
