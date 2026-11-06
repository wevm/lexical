import Foundation
import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalHashtag
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalOverflow
import LexicalRichText
import LexicalSelection
import Testing

/// Replays `vectors/property/*.json`, the cases
/// packages/lexical-conformance/src/property.ts draws with
/// `@lexical/fast-check`: every random document must parse, export, and
/// convert as it did in JS, and every random command sequence must leave the
/// same state after each step, byte for byte.
@Suite(.enabled(if: Vectors.available))
struct PropertyTests {
  struct Case: CustomTestStringConvertible, Sendable {
    let name: String
    let value: JSONValue
    var testDescription: String { name }
  }

  static func load(_ file: String) -> [Case] {
    ((try? Vectors.loadJSON(file)) ?? []).map {
      Case(name: $0.objectValue?["name"]?.stringValue ?? "", value: $0)
    }
  }

  static let documents = load("property/documents")
  static let sequences = load("property/sequences")

  /// The vectors are there (a decoding error would leave no cases to run).
  @Test func vectorsLoad() {
    #expect(!Self.documents.isEmpty)
    #expect(!Self.sequences.isEmpty)
  }

  @Test(arguments: documents)
  func document(_ vector: Case) throws {
    let want = try #require(vector.value.objectValue)
    let input = try #require(want["input"]?.stringValue)
    let got = try PropertyRunner.runDocument(input)
    for name in PropertyRunner.documentFields {
      let a = got[name].map { $0.stringify() }
      let b = want[name].map { $0.stringify() }
      if a != b {
        Issue.record("\(name) differs\n\(difference(a ?? "absent", b ?? "absent"))")
      }
    }
  }

  @Test(arguments: sequences)
  func sequence(_ vector: Case) throws {
    let value = try #require(vector.value.objectValue)
    let expected = value["results"]?.arrayValue ?? []
    let steps = value["steps"]?.arrayValue ?? []
    let actual = try PropertyRunner.runSequence(vector.value)
    #expect(actual.count == expected.count)
    // Only the first divergent step is reported: later steps diverge with it.
    for (i, (got, want)) in zip(actual, expected).enumerated() {
      let want = try #require(want.objectValue)
      let mismatches = PropertyRunner.stepFields.filter {
        got[$0]?.stringify() != want[$0]?.stringify()
      }
      if mismatches.isEmpty { continue }
      for name in mismatches {
        Issue.record(
          """
          step \(i) \(name) differs
            step: \(steps[i].stringify())
          \(difference(got[name]?.stringify() ?? "absent", want[name]?.stringify() ?? "absent"))
          """)
      }
      break
    }
  }
}

/// For the nightly property run (packages/lexical-conformance/src/
/// propertyNightly.test.ts): answers cases one per line over two named pipes in
/// `$LEXICAL_PROPERTY_SERVER` (`requests`, `responses`), so fast-check can
/// generate and shrink in JS with Swift's answer to each candidate.
@Suite(.enabled(if: ProcessInfo.processInfo.environment["LEXICAL_PROPERTY_SERVER"] != nil))
struct PropertyServer {
  @Test func serve() throws {
    let directory = ProcessInfo.processInfo.environment["LEXICAL_PROPERTY_SERVER"] ?? ""
    let requests = try #require(FileHandle(forReadingAtPath: directory + "/requests"))
    let responses = try #require(FileHandle(forWritingAtPath: directory + "/responses"))
    var buffer = Data()
    while true {
      guard let newline = buffer.firstIndex(of: 10) else {
        let chunk = requests.availableData
        if chunk.isEmpty { return }
        buffer.append(chunk)
        continue
      }
      let line = String(decoding: buffer[buffer.startIndex..<newline], as: UTF8.self)
      buffer.removeSubrange(buffer.startIndex...newline)
      func object(_ record: [String: JSONValue]) -> JSONValue {
        .object(JSONObject(record.sorted { $0.key < $1.key }.map { ($0.key, $0.value) }))
      }
      let response: JSONValue
      do {
        let request = try JSONValue.parse(line)
        switch request.objectValue?["kind"]?.stringValue {
        case "document":
          response = object(
            try PropertyRunner.runDocument(request.objectValue?["input"]?.stringValue ?? ""))
        default:
          response = .array(
            try PropertyRunner.runSequence(request.objectValue?["case"] ?? .null).map(object))
        }
      } catch {
        response = object(["thrown": .string("\(error)")])
      }
      responses.write(Data((try response.stringify() + "\n").utf8))
    }
  }
}

/// The Swift side of packages/lexical-conformance/src/property.ts, which must
/// do exactly what `runDocument` and `runSequence` do there.
enum PropertyRunner {
  static let documentFields = [
    "error", "legacy", "compact", "reparsed", "text", "markdown", "html",
  ]
  static let stepFields = ["error", "handled", "text", "selection", "state", "compact"]

  /// `runDocument` in property.ts.
  static func runDocument(_ input: String) throws -> [String: JSONValue] {
    let nodes: [LexicalNode.Type] = [
      HeadingNode.self, QuoteNode.self, ListNode.self, ListItemNode.self, LinkNode.self,
      AutoLinkNode.self, CodeNode.self, CodeHighlightNode.self, OverflowNode.self,
      HashtagNode.self,
    ]
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "", nodes: nodes.map { .klass($0) }, onError: { throw $0 }, headless: true))
    let state: EditorState
    do {
      state = try editor.parseEditorState(input)
    } catch {
      return ["error": .bool(true)]
    }
    let legacy = try state.toJSON().stringify()
    let compact = try state.toJSON(true).stringify()
    var record: [String: JSONValue] = ["legacy": .string(legacy), "compact": .string(compact)]
    let reparsed = try [legacy, compact].map { json in
      let again = try editor.parseEditorState(json)
      return (try again.toJSON().stringify(), try again.toJSON(true).stringify())
    }
    if reparsed.contains(where: { $0.0 != legacy || $0.1 != compact }) {
      record["reparsed"] = .array(reparsed.map { .array([.string($0.0), .string($0.1)]) })
    }
    try editor.setEditorState(state)
    record["text"] = .string(try editor.read { try getRoot().getTextContent() })
    record["markdown"] =
      (try? editor.read { try convertToMarkdownString(TRANSFORMERS) }).map(JSONValue.string)
      ?? .null
    record["html"] =
      (try? editor.read { try generateHtmlFromNodes(editor) }).map(JSONValue.string) ?? .null
    return record
  }

  /// `sequenceEditor` in property.ts.
  static func sequenceEditor() throws -> Editor {
    resetRandomKey()
    return try buildEditorFromExtensions(
      defineExtension(
        name: "[root]",
        dependencies: [
          RichTextExtension, ListExtension, CheckListExtension, LinkExtension, HashtagExtension,
          CodeExtension, configExtension(HistoryExtension) { $0.delay = 0 },
        ],
        nodes: { [.klass(AutoLinkNode.self), .klass(OverflowNode.self)] },
        editorConfig: InitialEditorConfig(namespace: "", onError: { error, _ in throw error })))
  }

  /// `runSequence` in property.ts, from the concrete steps the vector
  /// recorded.
  static func runSequence(_ vector: JSONValue) throws -> [[String: JSONValue]] {
    let editor = try sequenceEditor()
    let document = vector.objectValue?["document"]?.stringValue ?? ""
    try editor.setEditorState(editor.parseEditorState(document), tag: "history-merge")
    return try (vector.objectValue?["steps"]?.arrayValue ?? []).map { try runStep(editor, $0) }
  }

  private static func point(_ spec: JSONValue?) throws -> (NodeKey, Int, PointType) {
    guard let items = spec?.arrayValue, items.count == 3, let key = items[0].stringValue,
      let offset = items[1].numberValue,
      let type = items[2].stringValue.flatMap(PointType.init(rawValue:))
    else { throw ScriptError("bad point") }
    return (key, Int(offset), type)
  }

  private static func setBlocks(_ blocks: String) throws {
    switch blocks {
    case "quote": try setBlocksType(getSelection()) { try createQuoteNode() }
    case "h1", "h2": try setBlocksType(getSelection()) { try createHeadingNode(blocks) }
    default: try setBlocksType(getSelection()) { try createParagraphNode() }
    }
  }

  private static func dispatch(
    _ editor: Editor, _ name: String, _ payload: JSONValue?
  ) throws
    -> Bool
  {
    let bool = payload?.boolValue ?? false
    let string = payload?.stringValue ?? ""
    switch name {
    case "CONTROLLED_TEXT_INSERTION_COMMAND":
      return try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, string)
    case "DELETE_CHARACTER_COMMAND":
      return try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, bool)
    case "DELETE_LINE_COMMAND": return try editor.dispatchCommand(DELETE_LINE_COMMAND, bool)
    case "DELETE_WORD_COMMAND": return try editor.dispatchCommand(DELETE_WORD_COMMAND, bool)
    case "FORMAT_ELEMENT_COMMAND": return try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, string)
    case "FORMAT_TEXT_COMMAND": return try editor.dispatchCommand(FORMAT_TEXT_COMMAND, string)
    case "INDENT_CONTENT_COMMAND": return try editor.dispatchCommand(INDENT_CONTENT_COMMAND)
    case "INSERT_CHECK_LIST_COMMAND": return try editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND)
    case "INSERT_LINE_BREAK_COMMAND":
      return try editor.dispatchCommand(INSERT_LINE_BREAK_COMMAND, bool)
    case "INSERT_ORDERED_LIST_COMMAND":
      return try editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND)
    case "INSERT_PARAGRAPH_COMMAND": return try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    case "INSERT_UNORDERED_LIST_COMMAND":
      return try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
    case "OUTDENT_CONTENT_COMMAND": return try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND)
    case "REDO_COMMAND": return try editor.dispatchCommand(REDO_COMMAND)
    case "REMOVE_LIST_COMMAND": return try editor.dispatchCommand(REMOVE_LIST_COMMAND)
    case "REMOVE_TEXT_COMMAND": return try editor.dispatchCommand(REMOVE_TEXT_COMMAND)
    case "TOGGLE_LINK_COMMAND":
      let link: ToggleLinkPayload?
      if let url = payload?.stringValue {
        link = .url(url)
      } else if let object = payload?.objectValue {
        link = .attributes(
          url: object["url"]?.stringValue ?? "",
          LinkAttributes(
            rel: object["rel"].map { .some($0.stringValue) } ?? .none,
            target: object["target"].map { .some($0.stringValue) } ?? .none,
            title: object["title"].map { .some($0.stringValue) } ?? .none))
      } else {
        link = nil
      }
      return try editor.dispatchCommand(TOGGLE_LINK_COMMAND, link)
    case "UNDO_COMMAND": return try editor.dispatchCommand(UNDO_COMMAND)
    default: throw ScriptError("unknown command \(name)")
    }
  }

  /// `runStep` in property.ts.
  static func runStep(_ editor: Editor, _ step: JSONValue) throws -> [String: JSONValue] {
    let object = step.objectValue
    var failed = false
    var handled: Bool?
    do {
      let anchor = try point(object?["anchor"])
      let focus = try point(object?["focus"])
      try editor.update(
        {
          let selection = createRangeSelection()
          try selection.anchor.set(anchor.0, anchor.1, anchor.2)
          try selection.focus.set(focus.0, focus.1, focus.2)
          try setSelection(selection)
          if let blocks = object?["blocks"]?.stringValue {
            try setBlocks(blocks)
          }
        }, EditorUpdateOptions(discrete: true))
      if let name = object?["command"]?.stringValue {
        handled = try dispatch(editor, name, object?["payload"])
      }
    } catch {
      failed = true
    }
    let (selection, content) = try editor.read {
      (try selectionSnapshot(), try getRoot().getTextContent())
    }
    let state = editor.getEditorState()
    var record: [String: JSONValue] = [
      "compact": .string(try state.toJSON(true).stringify()),
      "selection": selection,
      "state": .string(try state.toJSON().stringify()),
      "text": .string(content),
    ]
    if let handled { record["handled"] = .bool(handled) }
    if failed { record["error"] = .bool(true) }
    return record
  }
}
