import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalPlainText
import LexicalRichText
import LexicalTextKit
import Testing

// Replays the Tier 2 scenarios (packages/lexical-conformance/src/scenarios.ts,
// recorded in WebKit into vectors/scenarios/*.json) through
// TextKitInputController, as an NSTextView sends them: each key goes to
// `keyDown` first, and a key no handler prevented reaches the Cocoa
// standard key bindings, which insert its text, send the selector the
// views map to a beforeinput type, or move the caret over the rendered
// text. After every step the editor state and selection must match WebKit.

private enum ScenarioVectors {
  static let directory: URL? = {
    let url =
      ProcessInfo.processInfo.environment["LEXICAL_CONFORMANCE_VECTORS"].map {
        URL(fileURLWithPath: $0, isDirectory: true)
      }
      ?? URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent()  // LexicalTextKitTests
      .deletingLastPathComponent()  // Tests
      .deletingLastPathComponent()  // swift
      .deletingLastPathComponent()
      .appendingPathComponent("packages/lexical-conformance/vectors", isDirectory: true)
    return FileManager.default.fileExists(atPath: url.path) ? url : nil
  }()

  static let files = ["text-entry", "history"]

  /// `swift/parity/deviations.json`: the deviation listing each scenario
  /// (`<name> (<mode>)`) that may differ from WebKit.
  static let deviations: [String: String] = {
    let url = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent()  // LexicalTextKitTests
      .deletingLastPathComponent()  // Tests
      .deletingLastPathComponent()  // swift
      .appendingPathComponent("parity/deviations.json")
    guard let text = try? String(contentsOf: url, encoding: .utf8),
      let file = try? JSONValue.parse(text).objectValue
    else { return [:] }
    var byScenario: [String: String] = [:]
    for (id, entry) in file {
      for scenario in entry.objectValue?["scenarios"]?.arrayValue ?? [] {
        if let scenario = scenario.stringValue { byScenario[scenario] = id }
      }
    }
    return byScenario
  }()

  static func cases(_ name: String) throws -> [JSONValue] {
    guard let directory else { return [] }
    let text = try String(
      contentsOf: directory.appendingPathComponent("scenarios/\(name).json"), encoding: .utf8)
    let file = try JSONValue.parse(text)
    #expect(file.objectValue?["formatVersion"]?.numberValue == 1)
    return file.objectValue?["cases"]?.arrayValue ?? []
  }
}

/// A key chord in Playwright syntax (`Alt+Shift+ArrowLeft`).
private struct Chord {
  var key: String
  var alt = false
  var ctrl = false
  var meta = false
  var shift = false

  init(_ text: String) {
    var parts = text.split(separator: "+", omittingEmptySubsequences: false).map(String.init)
    // `Meta++` presses the plus key.
    if parts.last == "", parts.count > 1 {
      parts.removeLast(2)
      parts.append("+")
    }
    key = parts.removeLast()
    for modifier in parts {
      switch modifier {
      case "Alt": alt = true
      case "Control": ctrl = true
      case "Meta": meta = true
      case "Shift": shift = true
      default: Issue.record("unknown modifier \(modifier)")
      }
    }
  }

  var event: KeyboardEvent {
    KeyboardEvent(key: key, altKey: alt, ctrlKey: ctrl, metaKey: meta, shiftKey: shift)
  }
}

private final class ScenarioRunner {
  let editor: Editor
  let controller: TextKitInputController
  /// The view's own selection, which keeps its anchor while a shift-arrow
  /// extends it, as NSTextView does; the model's range is what the view was
  /// told last.
  private var viewSelection: (anchor: Int, focus: Int)?

  init(mode: String) throws {
    editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[scenario]",
        dependencies: [
          mode == "rich" ? RichTextExtension : PlainTextExtension,
          configExtension(HistoryExtension) { $0.now = { 0 } },
        ]))
    controller = TextKitInputController(editor: editor)
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      try getRoot().clear().append(paragraph)
      try paragraph.select()
    }
  }

  func run(_ step: JSONObject) throws {
    if let text = step["type"]?.stringValue {
      for character in text {
        try press(Chord(String(character)))
      }
    } else if step["advanceHistoryClock"] != nil {
      // The e2e helper: history's clock jumps past its merge delay.
      let history = try getExtensionDependencyFromEditor(editor, HistoryExtension).output
      let frozen = history.now.peek()() + history.delay.peek() + 50
      history.now.value = { frozen }
    } else if let press = step["press"]?.stringValue {
      let times = step["times"]?.numberValue.map { Int($0) } ?? 1
      for _ in 0..<times {
        try self.press(Chord(press))
      }
    }
  }

  private func press(_ chord: Chord) throws {
    if try controller.keyDown(chord.event) { return }
    try keyBinding(chord)
  }

  /// What the Cocoa standard key bindings do with a key no handler took.
  private func keyBinding(_ chord: Chord) throws {
    let c = controller
    switch (chord.key, chord.alt, chord.ctrl, chord.meta) {
    case ("Backspace", false, false, false), ("h", false, true, false):
      try c.beforeInput("deleteContentBackward")
    case ("Backspace", true, false, false): try c.beforeInput("deleteWordBackward")
    case ("Backspace", false, false, true): try c.beforeInput("deleteSoftLineBackward")
    case ("Delete", false, false, false), ("d", false, true, false):
      try c.beforeInput("deleteContentForward")
    case ("Delete", true, false, false): try c.beforeInput("deleteWordForward")
    case ("Enter", _, false, false):
      try c.beforeInput(chord.shift ? "insertLineBreak" : "insertParagraph")
    case ("ArrowLeft", _, false, _), ("ArrowRight", _, false, _):
      try moveHorizontally(chord)
    case ("ArrowUp", false, false, _), ("ArrowDown", false, false, _):
      try moveVertically(chord)
    case ("Tab", _, _, _), ("Escape", _, _, _):
      break
    case (let key, false, false, false) where key.count == 1:
      try c.insertText(key)
    default:
      Issue.record("no key binding for \(chord)")
    }
  }

  /// `moveLeft:`, `moveWordLeft:`, `moveToLeftEndOfLine:` and their
  /// `AndModifySelection:` forms (and the right ones).
  private func moveHorizontally(_ chord: Chord) throws {
    let isBackward = chord.key == "ArrowLeft"
    let granularity: SelectionGranularity =
      chord.meta ? .lineboundary : chord.alt ? .word : .character
    let document = controller.document
    let (anchor, focus) = try currentSelection()
    if chord.shift {
      let target = document.move(focus, isBackward: isBackward, granularity: granularity)
      try select(anchor: anchor, focus: target)
    } else if anchor != focus && granularity == .character {
      let edge = isBackward ? min(anchor, focus) : max(anchor, focus)
      try select(anchor: edge, focus: edge)
    } else {
      let from = isBackward ? min(anchor, focus) : max(anchor, focus)
      let target = document.move(from, isBackward: isBackward, granularity: granularity)
      try select(anchor: target, focus: target)
    }
  }

  /// `moveUp:` and `moveDown:` (to the start or end of the text from the
  /// first or last line), `moveToBeginningOfDocument:` and
  /// `moveToEndOfDocument:` with Command, and their `AndModifySelection:`
  /// forms.
  private func moveVertically(_ chord: Chord) throws {
    let isBackward = chord.key == "ArrowUp"
    let document = controller.document
    var (anchor, focus) = try currentSelection()
    let target: Int
    if chord.meta {
      target = isBackward ? 0 : document.length
    } else {
      let from = chord.shift ? focus : isBackward ? min(anchor, focus) : max(anchor, focus)
      let moved = document.move(from, isBackward: isBackward, granularity: .line)
      target = moved != from ? moved : isBackward ? 0 : document.length
    }
    if !chord.shift { anchor = target }
    focus = target
    try select(anchor: anchor, focus: focus)
  }

  /// The view's selection: the model's, with the view's own anchor while
  /// the range is the one the view set last.
  private func currentSelection() throws -> (Int, Int) {
    let document = controller.document
    let model = try editor.read(.latest) { () -> (Int, Int) in
      guard let selection = try getSelection() as? RangeSelection,
        let anchor = document.location(for: selection.anchor),
        let focus = document.location(for: selection.focus)
      else { return (0, 0) }
      return (anchor, focus)
    }
    if let view = viewSelection, min(view.anchor, view.focus) == min(model.0, model.1),
      max(view.anchor, view.focus) == max(model.0, model.1)
    {
      return view
    }
    return model
  }

  private func select(anchor: Int, focus: Int) throws {
    viewSelection = (anchor, focus)
    try controller.setSelectedRange(location: min(anchor, focus), length: abs(focus - anchor))
  }

  func snapshot() throws -> JSONValue {
    try editor.read(.latest) {
      let state = JSONValue.object(try editor.getEditorState().toJSON())
      let selection = try getSelection()
      var selectionValue = JSONValue.null
      if let range = selection as? RangeSelection {
        selectionValue = .object(
          JSONObject([
            ("anchor", try point(range.anchor)), ("focus", try point(range.focus)),
            ("format", .int(range.format)), ("style", .string(range.style)),
            ("type", .string("range")),
          ]))
      } else if let nodes = selection as? NodeSelection {
        selectionValue = .object(
          JSONObject([
            ("nodes", .array(try nodes.getNodes().map { .array(try path($0).map(JSONValue.int)) })),
            ("type", .string("node")),
          ]))
      }
      return .object(JSONObject([("selection", selectionValue), ("state", state)]))
    }
  }

  private func point(_ point: Point) throws -> JSONValue {
    .object(
      JSONObject([
        ("offset", .int(point.offset)),
        ("path", .array(try path(point.getNode()).map(JSONValue.int))),
        ("type", .string(point.type.rawValue)),
      ]))
  }

  private func path(_ node: LexicalNode) throws -> [Int] {
    var path: [Int] = []
    var current = node
    while let parent = try current.getParent() {
      path.insert(try current.getIndexWithinParent(), at: 0)
      current = parent
    }
    return path
  }
}

@Suite(.enabled(if: ScenarioVectors.directory != nil, "needs packages/lexical-conformance"))
struct ScenarioTests {
  static let cases: [(String, JSONValue)] = ScenarioVectors.files.flatMap { file in
    ((try? ScenarioVectors.cases(file)) ?? []).map { (file, $0) }
  }

  @Test(arguments: cases.indices)
  func replaysInWebKitsSteps(_ index: Int) throws {
    let (_, scenario) = Self.cases[index]
    let object = try #require(scenario.objectValue)
    let name = object["name"]?.stringValue ?? ""
    let mode = object["mode"]?.stringValue ?? ""
    let steps = object["steps"]?.arrayValue ?? []
    let after = object["after"]?.arrayValue ?? []
    let replay = {
      try withIsApple(true) {
        let runner = try ScenarioRunner(mode: mode)
        for (i, step) in steps.enumerated() {
          try runner.run(try #require(step.objectValue))
          let actual = try runner.snapshot()
          guard actual.stringify() != after[i].stringify() else { continue }
          Issue.record(
            """
            \(name) (\(mode)), after step \(i + 1) \(step.stringify()):
              swift: \(actual.stringify())
              js:    \(after[i].stringify())
            """)
          return
        }
      }
    }
    if let deviation = ScenarioVectors.deviations["\(name) (\(mode))"] {
      withKnownIssue("parity/deviations.json: \(deviation)") { try replay() }
    } else {
      try replay()
    }
  }

  @Test func deviationsNameRecordedScenarios() {
    let recorded = Set(
      Self.cases.compactMap { _, scenario -> String? in
        let object = scenario.objectValue
        guard let name = object?["name"]?.stringValue, let mode = object?["mode"]?.stringValue
        else { return nil }
        return "\(name) (\(mode))"
      })
    for scenario in ScenarioVectors.deviations.keys {
      #expect(recorded.contains(scenario), "deviations.json lists an unknown scenario: \(scenario)")
    }
  }
}
