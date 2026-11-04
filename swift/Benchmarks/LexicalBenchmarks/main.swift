// Gating (DESIGN.md, Benchmarks; Q32): the editor the native
// views build (rich text and history) with a TextKit controller rendering
// into a plain UTF-16 storage, so the numbers are Lexical's share of a
// keystroke or a load; a real NSTextStorage adds its own layout cost.
//
//   swift run -c release LexicalBenchmarks [--markdown <path>]
//
// `--markdown` appends the results as a table (CI's job summary).
//
// Targets: keystroke to reconcile under 8 ms p95 and load under 300 ms, both
// on 5,000 paragraphs. A missed keystroke target exits with status 1, after
// the report; load is reported against its target but does not gate yet.

import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalTextKit

/// The storage a view applies the reconciler's edits to, without layout.
final class Storage: TextKitStorage {
  var units: [UInt16] = []
  func beginEditing() {}
  func endEditing() {}
  func replaceCharacters(location: Int, length: Int, with string: String, runs: [TextKitRun]) {
    units.replaceSubrange(location..<(location + length), with: string.utf16)
  }
}

let paragraphCount = 5_000
/// `BENCHMARK=<prefix>` runs only the benchmarks whose name starts with it
/// (for profiling one of them).
let only = ProcessInfo.processInfo.environment["BENCHMARK"]
func selected(_ name: String) -> Bool { only.map { name.hasPrefix($0) } ?? true }
let clock = ContinuousClock()

func milliseconds(_ duration: Duration) -> Double {
  Double(duration.components.seconds) * 1e3 + Double(duration.components.attoseconds) / 1e15
}

func percentile(_ values: [Duration], _ p: Int) -> Double {
  milliseconds(values.sorted()[min(values.count - 1, values.count * p / 100)])
}

func makeEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[benchmark]",
      dependencies: [RichTextExtension, configExtension(HistoryExtension) { $0.now = { 0 } }]))
}

/// A view's editor: the controller renders every commit into `storage`.
func mount(_ editor: Editor) -> (TextKitInputController, Storage) {
  let controller = TextKitInputController(editor: editor)
  let storage = Storage()
  storage.apply([
    TextKitEdit(
      location: 0, length: 0, string: controller.document.string,
      runs: controller.document.runs)
  ])
  controller.onRender = { storage.apply($0.edits.map(\.edit)) }
  return (controller, storage)
}

/// 5,000 blocks of prose: paragraphs with bold and italic runs, and every
/// tenth block a heading, every seventh a quote.
func fixture() throws -> String {
  let editor = try makeEditor()
  try editor.update(discrete: true) {
    let root = try getRoot().clear()
    for i in 0..<paragraphCount {
      let block: ElementNode =
        i % 10 == 0
        ? try createHeadingNode("h2")
        : i % 7 == 0 ? try createQuoteNode() : try createParagraphNode()
      try block.append(
        createTextNode("Paragraph \(i) has some plain text, "),
        createTextNode("a bold part").setFormat("bold"),
        createTextNode(" and "),
        createTextNode("an italic one").setFormat("italic"),
        createTextNode(" before it ends."))
      try root.append(block)
    }
  }
  return JSONValue.object(try editor.getEditorState().toJSON()).stringify()
}

struct Result {
  var name: String
  var p50: Double
  var p95: Double
  var target: Double?
  /// Whether missing `target` fails the run.
  var gating: Bool
}

var results: [Result] = []

@MainActor func report(
  _ name: String, _ samples: [Duration], target: Double? = nil, gating: Bool = true
) {
  let result = Result(
    name: name, p50: percentile(samples, 50), p95: percentile(samples, 95), target: target,
    gating: gating)
  results.append(result)
  let goal =
    target.map {
      " (target \($0) ms p95\(result.p95 <= $0 ? ", met" : ", NOT met")\(gating ? "" : ", not gating"))"
    } ?? ""
  print(
    String(format: "%-44@ p50 %8.3f ms  p95 %8.3f ms", name as NSString, result.p50, result.p95)
      + goal)
}

let json = try fixture()
print("Fixture: \(paragraphCount) blocks, \(json.utf8.count / 1024) KB of JSON")

// Load: a mounted view gets a serialized document, from the JSON string to
// the storage holding its text.
var parses: [Duration] = []
var loads: [Duration] = []
for _ in 0..<(selected("load") ? 10 : 0) {
  let editor = try makeEditor()
  let (controller, storage) = mount(editor)
  let start = clock.now
  let state = try editor.parseEditorState(json)
  parses.append(clock.now - start)
  try editor.setEditorState(state)
  loads.append(clock.now - start)
  precondition(storage.units.count == controller.document.length)
  editor.dispose()
}
if selected("load") {
  report("load: parseEditorState", parses)
  // Not gating yet: CI runners measured 253 and 306 ms p95 on one commit.
  report("load: JSON to rendered storage", loads, target: 300, gating: false)
}

// Keystrokes in the middle of the document, as the views send them.
let editor = try makeEditor()
let (controller, storage) = mount(editor)
try editor.setEditorState(editor.parseEditorState(json))
let middle = try editor.read { try getRoot().getChildAtIndex(paragraphCount / 2)!.getKey() }
@MainActor func caretAtEnd(of key: NodeKey) throws {
  let span = controller.document.span(key)!
  try controller.setSelectedRange(location: span.end, length: 0)
}

/// Times `body` `count` times and reports it, when `name` is selected.
@MainActor func bench(
  _ name: String, _ count: Int, target: Double? = nil, _ body: () throws -> Void
) rethrows {
  guard selected(name) else { return }
  var samples: [Duration] = []
  for _ in 0..<count {
    let start = clock.now
    try body()
    samples.append(clock.now - start)
  }
  report(name, samples, target: target)
}

try caretAtEnd(of: middle)
try bench("keystroke: insert a character", 200, target: 8) { try controller.insertText("a") }
// The deletes remove what the inserts typed, so they stay in one paragraph.
if !selected("keystroke: insert a character") {
  for _ in 0..<200 { try controller.insertText("a") }
}
try caretAtEnd(of: middle)
try bench("keystroke: delete a character", 200, target: 8) {
  try controller.beforeInput("deleteContentBackward")
}
try caretAtEnd(of: middle)
try bench("keystroke: split a paragraph (Enter)", 100, target: 8) {
  try controller.beforeInput("insertParagraph")
}
try bench("keystroke: join paragraphs (Backspace at start)", 100, target: 8) {
  try controller.beforeInput("deleteContentBackward")
}
precondition(storage.units.count == controller.document.length)

// Whole-document commands.
try bench("select all and toggle bold", 10) {
  try editor.update(discrete: true) {
    _ = try selectAll()
    _ = try editor.dispatchCommand(FORMAT_TEXT_COMMAND, "bold")
  }
}
try bench("undo and redo the last change", 10) {
  _ = try editor.dispatchCommand(UNDO_COMMAND, ())
  _ = try editor.dispatchCommand(REDO_COMMAND, ())
}
precondition(storage.units.count == controller.document.length)

if let index = CommandLine.arguments.firstIndex(of: "--markdown"),
  index + 1 < CommandLine.arguments.count
{
  var table = """
    ### Benchmarks (\(paragraphCount) blocks)

    | Benchmark | p50 (ms) | p95 (ms) | Target p95 (ms) |
    | --- | ---: | ---: | ---: |

    """
  for result in results {
    let target =
      result.target.map {
        "\($0 == $0.rounded() ? String(Int($0)) : String($0))\(result.p95 <= $0 ? " ✅" : " ❌")"
          + (result.gating ? "" : " (not gating)")
      } ?? ""
    table += String(
      format: "| %@ | %.3f | %.3f | %@ |\n", result.name, result.p50, result.p95, target)
  }
  let url = URL(fileURLWithPath: CommandLine.arguments[index + 1])
  if let handle = try? FileHandle(forWritingTo: url) {
    handle.seekToEndOfFile()
    handle.write(Data(table.utf8))
    try handle.close()
  } else {
    try table.write(to: url, atomically: true, encoding: .utf8)
  }
}

let missed = results.filter { result in
  result.gating && result.target.map { result.p95 > $0 } ?? false
}
if !missed.isEmpty {
  print("Missed the p95 target: \(missed.map(\.name).joined(separator: ", "))")
  exit(1)
}
