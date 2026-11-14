import Foundation
import Lexical
import Testing

/// Replays `vectors/scripts/*.json` and the seeded `vectors/fuzz/*.json`: every step of every script must leave
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

  static let files = [
    "scripts/updates", "scripts/selection", "scripts/transforms", "scripts/commands",
    "scripts/listeners", "scripts/caret", "scripts/rich-text", "scripts/rich-text-commands",
    "scripts/plain-text", "scripts/history", "scripts/list", "scripts/link",
    "scripts/autolink", "scripts/code", "scripts/overflow", "scripts/node-state",
    "scripts/set-blocks-type", "scripts/decorator", "scripts/text", "scripts/extensions",
    "fuzz/core",
  ]

  static let cases: [Case] = files.flatMap { file in
    ((try? Vectors.loadJSON(file)) ?? []).map { script in
      Case(file: file, name: script.objectValue?["name"]?.stringValue ?? "", script: script)
    }
  }

  @Test(arguments: cases)
  func replay(_ vector: Case) throws {
    let expected = vector.script.objectValue?["results"]?.arrayValue ?? []
    let (build, actual) = try runScriptWithBuild(vector.script)
    // A script with `extensions` records its build: the log, then the
    // editor's state, selection and text, or the error.
    let wantBuild = vector.script.objectValue?["build"]
    #expect((build == nil) == (wantBuild == nil))
    if let build, let wantBuild {
      for field in ["log", "selection", "text", "state", "compact", "error"] {
        let got = build.objectValue?[field]?.stringify()
        let want = wantBuild.objectValue?[field]?.stringify()
        if got != want {
          Issue.record("build \(field) differs\n\(difference(got ?? "nil", want ?? "nil"))")
        }
      }
    }
    #expect(actual.count == expected.count)
    let steps = vector.script.objectValue?["steps"]?.arrayValue ?? []
    // Only the first divergent step is reported: later steps diverge with it.
    for (i, (record, want)) in zip(actual, expected).enumerated() {
      let want = try #require(want.objectValue)
      let fields: [(String, String?, String?)] = [
        ("error", "\(record.error)", "\(want["error"]?.boolValue ?? false)"),
        ("text", record.text, want["text"]?.stringValue),
        ("selection", record.selection.stringify(), (want["selection"] ?? .null).stringify()),
        ("log", JSONValue.array(record.log).stringify(), (want["log"] ?? .array([])).stringify()),
        ("state", record.state, want["state"]?.stringValue),
        ("compact", record.compact, want["compact"]?.stringValue),
      ]
      let mismatches = fields.filter { $0.1 != $0.2 }
      if mismatches.isEmpty { continue }
      let step = i < steps.count ? steps[i].stringify() : "?"
      for (name, got, want) in mismatches {
        Issue.record(
          "step \(i) \(name) differs\n  step: \(step)\n\(difference(got ?? "nil", want ?? "nil"))")
      }
      break
    }
  }
}

/// Where `got` and `want` first differ, with some context.
func difference(_ got: String, _ want: String) -> String {
  let a = Array(got.unicodeScalars)
  let b = Array(want.unicodeScalars)
  var i = 0
  while i < a.count, i < b.count, a[i] == b[i] { i += 1 }
  func context(_ s: [Unicode.Scalar]) -> String {
    String(String.UnicodeScalarView(s[max(0, i - 80)..<min(s.count, i + 120)]))
  }
  return "  swift: …\(context(a))\n  js:    …\(context(b))"
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
