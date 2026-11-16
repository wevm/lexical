import Foundation
import Lexical
import LexicalA11y
import LexicalClipboard
import LexicalCodeCore
import LexicalDecoratorBlockNode
import LexicalExtension
import LexicalHashtag
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMark
import LexicalMarkdown
import LexicalOverflow
import LexicalPlainText
import LexicalRichText
import LexicalSelection
import LexicalText
import LexicalUtils

// The module-private functions of the menu and auto-embed that ops.ts
// re-exports from upstream's modules (`INTERNAL_*`, vitest.config.mts).
@testable import LexicalAutoEmbed
@testable import LexicalMenu

// The Swift interpreter of the script vectors that
// packages/lexical-conformance/src/ops.ts writes. It must do exactly what
// ops.ts does, step for step, so that a difference in the recorded results is
// a difference between the Swift and JS cores.

struct ScriptError: Error, CustomStringConvertible {
  let description: String
  init(_ description: String) { self.description = description }
}

extension JSONValue {
  fileprivate var int: Int? { numberValue.map { Int($0) } }

  fileprivate func at(_ i: Int) -> JSONValue {
    guard let items = arrayValue, i < items.count else { return .null }
    return items[i]
  }

  fileprivate subscript(_ key: String) -> JSONValue? { objectValue?[key] }

  fileprivate static func array(_ keys: [String]) -> JSONValue { .array(keys.map { .string($0) }) }
}

/// The records of one script step, in the vector's form.
struct StepRecord {
  var state: String?
  var compact: String?
  var selection: JSONValue
  var text: String
  var log: [JSONValue]
  var error: Bool
}

/// `CORE_COMMANDS` in ops.ts: Lexical's own commands, which `dispatch` and
/// handlers name as JS exports them. Their payloads are typed in Swift, so
/// the script's JSON payload is converted on the way in and out.
private enum CoreCommand {
  case void(LexicalCommand<Void>)
  case bool(LexicalCommand<Bool>)
  case string(LexicalCommand<String>)
  /// `SET_TEXT_FORMAT_COMMAND`: an object of booleans, in key order.
  case formats(LexicalCommand<[(TextFormatType, Bool)]>)
  /// `SELECT_ALL_COMMAND`, whose keyboard event no handler reads; ops.ts
  /// dispatches it with `null`.
  case keyboard(LexicalCommand<KeyboardEvent>)
  /// `KEY_ENTER_COMMAND`, which takes a keyboard event or `null`; ops.ts
  /// dispatches it with `null`.
  case key(LexicalCommand<KeyboardEvent?>)
  /// `UPDATE_LIST_START_COMMAND`: `{listNodeKey, newStart}`.
  case listStart(LexicalCommand<UpdateListStartPayload>)
  /// `TOGGLE_LINK_COMMAND`: `null`, a URL, or `{url, target, rel, title}`.
  case toggleLink(LexicalCommand<ToggleLinkPayload?>)
}

private let coreCommands: [String: CoreCommand] = [
  "CAN_REDO_COMMAND": .bool(CAN_REDO_COMMAND),
  "CAN_UNDO_COMMAND": .bool(CAN_UNDO_COMMAND),
  "CLEAR_EDITOR_COMMAND": .void(CLEAR_EDITOR_COMMAND),
  "CLEAR_HISTORY_COMMAND": .void(CLEAR_HISTORY_COMMAND),
  "CONTROLLED_TEXT_INSERTION_COMMAND": .string(CONTROLLED_TEXT_INSERTION_COMMAND),
  "DELETE_CHARACTER_COMMAND": .bool(DELETE_CHARACTER_COMMAND),
  "DELETE_LINE_COMMAND": .bool(DELETE_LINE_COMMAND),
  "DELETE_WORD_COMMAND": .bool(DELETE_WORD_COMMAND),
  "FORMAT_ELEMENT_COMMAND": .string(FORMAT_ELEMENT_COMMAND),
  "FORMAT_TEXT_COMMAND": .string(FORMAT_TEXT_COMMAND),
  "INDENT_CONTENT_COMMAND": .void(INDENT_CONTENT_COMMAND),
  "INSERT_LINE_BREAK_COMMAND": .bool(INSERT_LINE_BREAK_COMMAND),
  "INSERT_PARAGRAPH_COMMAND": .void(INSERT_PARAGRAPH_COMMAND),
  "INSERT_TAB_COMMAND": .void(INSERT_TAB_COMMAND),
  "KEY_ENTER_COMMAND": .key(KEY_ENTER_COMMAND),
  "OUTDENT_CONTENT_COMMAND": .void(OUTDENT_CONTENT_COMMAND),
  "REDO_COMMAND": .void(REDO_COMMAND),
  "REMOVE_TEXT_COMMAND": .void(REMOVE_TEXT_COMMAND),
  "SELECT_ALL_COMMAND": .keyboard(SELECT_ALL_COMMAND),
  "SET_TEXT_FORMAT_COMMAND": .formats(SET_TEXT_FORMAT_COMMAND),
  "UNDO_COMMAND": .void(UNDO_COMMAND),
  // `PACKAGE_COMMANDS` in ops.ts: the list and link commands.
  "INSERT_CHECK_LIST_COMMAND": .void(INSERT_CHECK_LIST_COMMAND),
  "INSERT_ORDERED_LIST_COMMAND": .void(INSERT_ORDERED_LIST_COMMAND),
  "INSERT_UNORDERED_LIST_COMMAND": .void(INSERT_UNORDERED_LIST_COMMAND),
  "REMOVE_LIST_COMMAND": .void(REMOVE_LIST_COMMAND),
  "UPDATE_LIST_START_COMMAND": .listStart(UPDATE_LIST_START_COMMAND),
  "TOGGLE_LINK_COMMAND": .toggleLink(TOGGLE_LINK_COMMAND),
  "INSERT_EMBED_COMMAND": .string(INSERT_EMBED_COMMAND),
]

/// `TestDecoratorBlockNode` in ops.ts: a `DecoratorBlockNode` that adds
/// nothing, as an app's block embed would extend it.
final class TestDecoratorBlockNode: DecoratorBlockNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TestDecoratorBlockNode.self, type: "test-decorator-block", extends: DecoratorBlockNode.self)
  }
}

// The node classes of scripts/node-structure (ops.ts), for the structure
// rules the packages' own nodes don't declare: a block container, an inline
// element merged by a schema field and a NodeState, derived values, and
// subclasses that inherit those declarations.

/// `testTagColorState`: a NodeState that `TestTagNode`'s `mergeAdjacent`
/// names.
nonisolated(unsafe) private let testTagColorState = createState(
  "color", parse: { value -> String in value?.stringValue ?? "" })

/// `testTagCountState`: a derived NodeState of `TestTagNode`.
nonisolated(unsafe) private let testTagCountState = createState(
  "count", parse: { value -> Double in value?.numberValue ?? 0 }, derived: true)

/// `TestBoxNode`: a block that holds blocks (`children: 'block'`).
class TestBoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestBoxNode.self, type: "test-box", extends: ElementNode.self, children: .block)
  }
}

/// `TestSubBoxNode`: a `TestBoxNode` that declares nothing of its own.
final class TestSubBoxNode: TestBoxNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestSubBoxNode.self, type: "test-sub-box", extends: TestBoxNode.self)
  }
}

/// `TestTagNode`: an inline element holding inline content, which cannot
/// be empty; equal adjacent ones (same `tag`, same `color` state) merge. Its
/// `label` field and `count` state are derived.
class TestTagNode: ElementNode {
  var __tag = ""
  var __label = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TestTagNode.self, type: "test-tag", extends: ElementNode.self,
      json: [
        SchemaField("label", stringValue(), field: \TestTagNode.__label, derived: true),
        SchemaField("tag", stringValue(), field: \TestTagNode.__tag),
      ],
      stateConfigs: [
        RequiredNodeStateConfig(testTagColorState), RequiredNodeStateConfig(testTagCountState),
      ], inline: true, children: .inline,
      mergeAdjacent: .by([.field("tag"), .state(testTagColorState)]))
  }
  override func canBeEmpty() throws -> Bool { false }
}

/// `TestSubTagNode`: a `TestTagNode` that inherits its declarations, so the
/// normalization never merges two of them.
final class TestSubTagNode: TestTagNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestSubTagNode.self, type: "test-sub-tag", extends: TestTagNode.self)
  }
}

/// `STRUCTURE_JSON(klass)` in ops.ts: what `logNodeStructure` logs.
private func structureJSON(_ klass: LexicalNode.Type) -> JSONValue {
  let declared = getDeclaredNodeStructure(klass)
  var out = JSONObject()
  if let inline = declared.inline {
    out["inline"] = .bool(inline)
  }
  if let children = declared.children {
    switch children {
    case .inline: out["children"] = .string("inline")
    case .block: out["children"] = .string("block")
    case .list(let entries):
      out["children"] = .array(
        entries.map { entry -> JSONValue in
          switch entry {
          case .inline: return .string("inline")
          case .klass(let k): return .string(k.getType())
          }
        })
    }
  }
  if let merge = declared.mergeAdjacent {
    out["mergeAdjacent"] = .array(
      merge.by.map { entry -> JSONValue in
        switch entry {
        case .field(let name): return .string(name)
        case .state(let state): return .object(JSONObject([("state", .string(state.key))]))
        }
      })
  }
  out["derived"] = .array(declared.derived.sorted().map { .string($0) })
  var by = JSONObject()
  if let k = declared.declaredBy.inline { by["inline"] = .string(k.getType()) }
  if let k = declared.declaredBy.children { by["children"] = .string(k.getType()) }
  if let k = declared.declaredBy.mergeAdjacent { by["mergeAdjacent"] = .string(k.getType()) }
  out["declaredBy"] = .object(by)
  return .object(out)
}

/// The class the active editor registers under `type`.
private func registeredClass(_ type: String) throws -> LexicalNode.Type {
  guard let klass = try getEditor()._nodes.get(type)?.klass else {
    throw ScriptError("unknown node type \(type)")
  }
  return klass
}

/// The session's root element (`sessionDOM(ctx).root` in ops.ts): a root
/// that is not in its document, as ops.ts's `div` is not, whose document's
/// body hosts `AriaLiveRegionExtension`'s regions.
private final class SessionRoot: LexicalRootElement, LiveRegionHost {
  var isConnected: Bool { false }
  private(set) var regions: [LiveRegion] = []

  func mountLiveRegion(_ region: LiveRegion) { regions.append(region) }
  func unmountLiveRegion(_ region: LiveRegion) { regions.removeAll { $0 === region } }
}

/// `useBasicTypeaheadTriggerMatch(trigger, options)` of a `TypeaheadSpec`,
/// the options left out taking its defaults.
private func typeaheadTrigger(_ spec: JSONValue) throws -> TriggerFn {
  basicTypeaheadTriggerMatch(
    try spec["trigger"]?.stringValue.orThrow() ?? "", minLength: spec["minLength"]?.int ?? 1,
    maxLength: spec["maxLength"]?.int ?? 75,
    punctuation: spec["punctuation"]?.stringValue ?? PUNCTUATION,
    allowWhitespace: spec["allowWhitespace"]?.boolValue ?? false)
}

/// `$typeaheadMatch(spec, editor)` in ops.ts: the text before the caret and
/// its match (`nil`s past a `nil`).
private func typeaheadMatch(
  _ spec: JSONValue, _ editor: Editor
) throws -> (
  String?, MenuTextMatch?
) {
  let text = try (getSelection() as? RangeSelection).flatMap { try getTextUpToAnchor($0) }
  return (text, try text.flatMap { try typeaheadTrigger(spec)($0, editor) })
}

private func menuTextMatchJSON(_ match: MenuTextMatch) -> JSONValue {
  .object(
    JSONObject([
      ("leadOffset", .int(match.leadOffset)), ("matchingString", .string(match.matchingString)),
      ("replaceableString", .string(match.replaceableString)),
    ]))
}

/// `TOGGLE_LINK_COMMAND`'s payload as the script writes it: `null`, a URL, or
/// an object with a URL and the attributes it has (`null` or a string each).
private func toggleLinkPayload(_ payload: JSONValue) throws -> ToggleLinkPayload? {
  if payload == .null { return nil }
  if let url = payload.stringValue { return .url(url) }
  let object = try payload.objectValue.orThrow()
  func attribute(_ key: String) throws -> String?? {
    switch object[key] {
    case nil: return .none
    case .null?: return .some(nil)
    case let value?: return .some(try value.stringValue.orThrow())
    }
  }
  return .attributes(
    url: try object["url"]?.stringValue.orThrow() ?? "",
    LinkAttributes(
      rel: try attribute("rel"), target: try attribute("target"), title: try attribute("title")))
}

/// `TOGGLE_LINK_COMMAND`'s payload as JSON: `url`, then the attributes it has.
private func toggleLinkJSON(_ payload: ToggleLinkPayload?) -> JSONValue {
  switch payload {
  case nil: return .null
  case .url(let url)?: return .string(url)
  case .attributes(let url, let attributes)?:
    var entries: [(String, JSONValue)] = [("url", .string(url))]
    for (key, value) in [
      ("target", attributes.target), ("rel", attributes.rel), ("title", attributes.title),
    ] {
      if let value { entries.append((key, value.map { .string($0) } ?? .null)) }
    }
    return .object(JSONObject(entries))
  }
}

/// `UPDATE_LIST_START_COMMAND`'s payload as the script writes it.
private func listStart(_ payload: JSONValue) throws -> UpdateListStartPayload {
  try UpdateListStartPayload(
    listNodeKey: payload["listNodeKey"]?.stringValue.orThrow() ?? "",
    newStart: payload["newStart"]?.int.orThrow() ?? 1)
}

/// `SET_TEXT_FORMAT_COMMAND`'s payload as the script writes it.
private func formats(_ payload: JSONValue) throws -> [(TextFormatType, Bool)] {
  let object = try payload.objectValue.orThrow()
  return try object.keys.map { ($0, try object[$0]?.boolValue.orThrow() ?? false) }
}

/// A NodeState config of the script context, with its value type erased:
/// `setState`, `logState` and `logStateChange` on JSON values.
private struct ScriptState {
  let set: (LexicalNode, JSONValue) throws -> Void
  let get: (LexicalNode) throws -> JSONValue
  let change: (LexicalNode, LexicalNode) throws -> JSONValue

  init<V>(
    _ config: StateConfig<V>, _ toJSON: @escaping (V) -> JSONValue,
    _ fromJSON: @escaping (JSONValue) -> V?
  ) {
    set = { node, json in
      guard let value = fromJSON(json) else {
        throw ScriptError("\(json.stringify()) is no value of \(config.key)")
      }
      try setState(node, config, value)
    }
    get = { try toJSON(getState($0, config)) }
    change = { node, prevNode in
      guard let (value, prevValue) = try getStateChange(node, prevNode, config) else {
        return .null
      }
      return .array([toJSON(value), toJSON(prevValue)])
    }
  }
}

/// `STRLEN[name]` in ops.ts: UTF-16 code units, or with `utf8` UTF-8 bytes.
private func scriptStrlen(_ name: String) throws -> (String) -> Int {
  switch name {
  case "utf16": return { $0.utf16.count }
  case "utf8": return { $0.utf8.count }
  default: throw ScriptError("unknown strlen \(name)")
  }
}

/// `mountStep(step, ctx)` in ops.ts: `useCharacterLimit` or
/// `TableOfContentsPlugin`, as their registrations.
private func mountStep(_ step: JSONValue, _ ctx: ScriptContext) throws {
  let editor: Editor = ctx.editor
  if let spec = step["characterLimit"] {
    guard let maxCharacters = spec["maxCharacters"]?.int else {
      throw ScriptError("characterLimit needs maxCharacters")
    }
    let remainingCharacters: (Int) -> Void = { characters in
      ctx.log.append(.array([.string("remainingCharacters"), .number(Double(characters))]))
    }
    if let name = spec["strlen"]?.stringValue {
      ctx.mounted["characterLimit"] = try registerCharacterLimit(
        editor, maxCharacters: maxCharacters, strlen: scriptStrlen(name),
        remainingCharacters: remainingCharacters)
    } else {
      ctx.mounted["characterLimit"] = try registerCharacterLimit(
        editor, maxCharacters: maxCharacters, remainingCharacters: remainingCharacters)
    }
  } else {
    ctx.mounted["tableOfContents"] = try registerTableOfContents(editor) { entries in
      ctx.log.append(
        .array([
          .string("tableOfContents"),
          .array(entries.map { .array([.string($0.key), .string($0.text), .string($0.tag)]) }),
        ]))
    }
  }
}

private final class ScriptContext {
  /// Set once the editor is made: a script with `extensions` makes it after
  /// its extensions have started logging.
  var editor: Editor!
  var log: [JSONValue] = []
  var commands: [String: LexicalCommand<JSONValue>] = [:]
  /// The session's clock (ms): what the history listener reads as `Date.now()`.
  var clock: Double = 0
  /// The `DataTransfer` of the last `copy`.
  var clipboard: DataTransfer?
  /// The editor states `saveEditorState` kept, by name.
  var saved: [String: EditorState] = [:]
  /// The block handles `logBlockHandles` kept, by name.
  var handles: [String: JSMap<String, String>] = [:]
  /// What `characterLimit` and `tableOfContents` mounted, to unmount.
  var mounted: [String: () -> Void] = [:]
  /// The disposers `registerNodeStructure` returned, by name.
  var structures: [String: () -> Void] = [:]
  /// The script's `parent` editor.
  var parent: Editor?
  /// The session's root element, made on first use (`setRootElement`).
  private var root: SessionRoot?

  /// `sessionDOM(ctx).root`.
  fileprivate func sessionRoot() -> SessionRoot {
    if let root { return root }
    let root = SessionRoot()
    self.root = root
    return root
  }

  /// `liveRegionEntry(ctx)`: `['liveRegion', [[aria-live, textContent]...]]`.
  func liveRegionEntry() -> JSONValue {
    .array([
      .string("liveRegion"),
      .array(
        (root?.regions ?? []).map {
          .array([.string($0.politeness.rawValue), .string($0.textContent)])
        }),
    ])
  }

  init(_ editor: Editor? = nil) { self.editor = editor }

  // ops.ts's `STATES`, one set per session (JS's are shared by every
  // session, which no session can tell: a node type learns a config in one
  // editor only).
  /// A number, 1 by default.
  let count = createState("count", parse: { (v: JSONValue?) -> Double in v?.numberValue ?? 1 })
  /// A flag that a copy of its node (`copyNode`) does not keep.
  let fresh = createState(
    "fresh", parse: { (v: JSONValue?) -> Bool in v?.boolValue == true }, resetOnCopyNode: true)
  /// A string, "" by default.
  let label = createState("label", parse: { (v: JSONValue?) -> String in v?.stringValue ?? "" })
  /// Another config with the key of `label`, which collides with it on a
  /// node type that uses `label`.
  let otherLabel = createState(
    "label", parse: { (v: JSONValue?) -> String in v?.stringValue ?? "" })
  /// An array of words, written as one string and compared by its words.
  let words = createState(
    "words",
    parse: { (v: JSONValue?) -> [String] in
      guard let s = v?.stringValue, !s.isEmpty else { return [] }
      return s.split(separator: " ", omittingEmptySubsequences: false).map(String.init)
    }, unparse: { .string($0.joined(separator: " ")) }, isEqual: { $0 == $1 })

  /// The state `name` names, with its values written as and read from JSON.
  func state(_ name: String) throws -> ScriptState {
    switch name {
    case "count": ScriptState(count, { .number($0) }, \.numberValue)
    case "fresh": ScriptState(fresh, { .bool($0) }, \.boolValue)
    case "label": ScriptState(label, { .string($0) }, \.stringValue)
    case "otherLabel": ScriptState(otherLabel, { .string($0) }, \.stringValue)
    case "words":
      ScriptState(words, { .array($0.map { .string($0) }) }) { v in
        v.arrayValue.flatMap { items in
          let words = items.compactMap(\.stringValue)
          return words.count == items.count ? words : nil
        }
      }
    default: throw ScriptError("unknown state \(name)")
    }
  }

  /// `$setState(node, STATES[state], UPDATERS[updater])`.
  func updateState(_ node: LexicalNode, _ name: String, _ updater: String) throws {
    switch (name, updater) {
    case ("count", "count:increment"): try setState(node, count) { $0 + 1 }
    case ("count", "count:same"): try setState(node, count) { $0 }
    case ("label", "label:exclaim"): try setState(node, label) { $0 + "!" }
    case ("words", "words:copy"): try setState(node, words) { Array($0) }
    case ("words", "words:push-x"): try setState(node, words) { $0 + ["x"] }
    default: throw ScriptError("unknown updater \(updater) of \(name)")
    }
  }

  func command(_ name: String) -> LexicalCommand<JSONValue> {
    if let command = commands[name] { return command }
    let command = LexicalCommand<JSONValue>(name)
    commands[name] = command
    return command
  }

  /// `editor.dispatchCommand(command(ctx, name), payload)`.
  func dispatch(_ name: String, _ payload: JSONValue) throws -> Bool {
    switch coreCommands[name] {
    case .void(let command):
      guard payload == .null else { throw ScriptError("\(name) takes no payload") }
      return try editor.dispatchCommand(command, ())
    case .bool(let command):
      return try editor.dispatchCommand(command, try payload.boolValue.orThrow())
    case .string(let command):
      return try editor.dispatchCommand(command, try payload.stringValue.orThrow())
    case .formats(let command):
      return try editor.dispatchCommand(command, try formats(payload))
    case .keyboard(let command):
      guard payload == .null else { throw ScriptError("\(name) takes no payload") }
      return try editor.dispatchCommand(command, KeyboardEvent())
    case .key(let command):
      guard payload == .null else { throw ScriptError("\(name) takes no payload") }
      return try editor.dispatchCommand(command, nil)
    case .listStart(let command):
      return try editor.dispatchCommand(command, try listStart(payload))
    case .toggleLink(let command):
      return try editor.dispatchCommand(command, try toggleLinkPayload(payload))
    case nil:
      return try editor.dispatchCommand(self.command(name), payload)
    }
  }

  /// `editor.registerCommand(command(ctx, name), listener, priority)`.
  @discardableResult
  func register(
    _ name: String, _ priority: Int, _ listener: @escaping (JSONValue) throws -> Bool
  ) -> () -> Void {
    switch coreCommands[name] {
    case .void(let command):
      return editor.registerCommand(command, { _, _ in try listener(.null) }, priority)
    case .bool(let command):
      return editor.registerCommand(
        command, { payload, _ in try listener(.bool(payload)) }, priority)
    case .string(let command):
      return editor.registerCommand(
        command, { payload, _ in try listener(.string(payload)) }, priority)
    case .formats(let command):
      return editor.registerCommand(
        command,
        { payload, _ in try listener(.object(JSONObject(payload.map { ($0, .bool($1)) }))) },
        priority)
    case .keyboard(let command):
      return editor.registerCommand(command, { _, _ in try listener(.null) }, priority)
    case .key(let command):
      return editor.registerCommand(command, { _, _ in try listener(.null) }, priority)
    case .listStart(let command):
      return editor.registerCommand(
        command,
        { payload, _ in
          try listener(
            .object(
              JSONObject([
                ("listNodeKey", .string(payload.listNodeKey)), ("newStart", .int(payload.newStart)),
              ])))
        }, priority)
    case .toggleLink(let command):
      return editor.registerCommand(
        command, { payload, _ in try listener(toggleLinkJSON(payload)) }, priority)
    case nil:
      return editor.registerCommand(
        self.command(name), { payload, _ in try listener(payload) }, priority)
    }
  }
}

private func node(_ key: String) throws -> LexicalNode {
  guard let node = try getNodeByKey(key) else { throw ScriptError("no node \(key)") }
  return node
}

private func text(_ node: LexicalNode) throws -> TextNode {
  guard let text = node as? TextNode else {
    throw ScriptError("\(node.getKey()) is not a text node")
  }
  return text
}

private func element(_ node: LexicalNode) throws -> ElementNode {
  guard let element = node as? ElementNode else {
    throw ScriptError("\(node.getKey()) is not an element")
  }
  return element
}

private func codeNode(_ node: LexicalNode) throws -> CodeNode {
  guard let code = node as? CodeNode else {
    throw ScriptError("\(node.getKey()) is not a code node")
  }
  return code
}

private func markNode(_ node: LexicalNode) throws -> MarkNode {
  guard let mark = node as? MarkNode else {
    throw ScriptError("\(node.getKey()) is not a mark node")
  }
  return mark
}

private func strings(_ values: [String]) -> JSONValue { .array(values.map { .string($0) }) }

private func parse(_ nodes: JSONValue) throws -> [LexicalNode] {
  try (nodes.arrayValue ?? []).map { value in
    guard let object = value.objectValue else { throw ScriptError("not a serialized node") }
    return try parseSerializedNode(object)
  }
}

private func rangeSelection() throws -> RangeSelection {
  guard let selection = try getSelection() as? RangeSelection else {
    throw ScriptError("not a range selection")
  }
  return selection
}

private func anySelection() throws -> any BaseSelection {
  guard let selection = try getSelection() else { throw ScriptError("no selection") }
  return selection
}

private func point(_ spec: JSONValue) throws -> (NodeKey, Int, PointType) {
  guard let key = spec.at(0).stringValue, let offset = spec.at(1).int,
    let type = spec.at(2).stringValue.flatMap(PointType.init(rawValue:))
  else { throw ScriptError("bad point") }
  return (key, offset, type)
}

/// `selectionSnapshot` in ops.ts.
func selectionSnapshot() throws -> JSONValue {
  guard let selection = try getSelection() else { return .null }
  if let range = selection as? RangeSelection {
    let points = [range.anchor, range.focus].map { point in
      JSONValue.array([.string(point.key), .int(point.offset), .string(point.type.rawValue)])
    }
    return .array([.string("range")] + points + [.int(range.format), .string(range.style)])
  }
  if let nodes = selection as? NodeSelection {
    return .array([.string("node"), .array(Array(nodes._nodes))])
  }
  return .array([.string("other")])
}

private func caretSnapshot(_ caret: PointCaret?) -> JSONValue {
  guard let caret else { return .null }
  let key = JSONValue.string(caret.origin.getKey())
  let direction = JSONValue.string(caret.direction.rawValue)
  switch caret.kind {
  case .text(let offset): return .array([.string("text"), key, direction, .int(offset)])
  case .sibling: return .array([.string("sibling"), key, direction])
  case .child: return .array([.string("child"), key, direction])
  }
}

private func caret(_ spec: JSONValue) throws -> PointCaret {
  guard let key = spec.at(0).stringValue,
    let direction = spec.at(1).stringValue.flatMap(CaretDirection.init(rawValue:)),
    let kind = spec.at(2).stringValue
  else { throw ScriptError("bad caret") }
  let origin = try node(key)
  switch kind {
  case "text":
    let offsetSpec = spec.at(3)
    let offset: TextOffset =
      if let n = offsetSpec.int { .offset(n) } else {
        .direction(try offsetSpec.stringValue.flatMap(CaretDirection.init(rawValue:)).orThrow())
      }
    return try getTextPointCaret(text(origin), direction, offset)
  case "child":
    return getChildCaret(try element(origin), direction)
  default:
    return getSiblingCaret(origin, direction)
  }
}

extension Optional {
  fileprivate func orThrow() throws -> Wrapped {
    guard let self else { throw ScriptError("missing value") }
    return self
  }
}

private func runSelectionMethod(_ method: String, _ args: JSONValue) throws {
  let a0 = args.at(0)
  switch method {
  case "deleteCharacter": try rangeSelection().deleteCharacter(a0.boolValue ?? false)
  case "deleteLine": try rangeSelection().deleteLine(a0.boolValue ?? false)
  case "deleteWord": try rangeSelection().deleteWord(a0.boolValue ?? false)
  case "formatText":
    try rangeSelection().formatText(a0.stringValue.orThrow(), args.at(1).int)
  case "insertLineBreak": try rangeSelection().insertLineBreak(a0.boolValue ?? false)
  case "insertNodes": try anySelection().insertNodes(parse(a0))
  case "insertParagraph": _ = try rangeSelection().insertParagraph()
  case "insertRawText": try anySelection().insertRawText(a0.stringValue.orThrow())
  case "insertText": try anySelection().insertText(a0.stringValue.orThrow())
  case "modify":
    // A headless editor has no `Selection.modify`: only the moves the model
    // resolves (around decorators and blocks) succeed.
    guard let alter = SelectionAlter(rawValue: try a0.stringValue.orThrow()),
      let granularity = SelectionGranularity(rawValue: try args.at(2).stringValue.orThrow())
    else { throw ScriptError("bad modify arguments \(args.stringify())") }
    try rangeSelection().modify(alter, args.at(1).boolValue ?? false, granularity)
  case "removeText": try rangeSelection().removeText()
  case "setFormat": try rangeSelection().setFormat(a0.int.orThrow())
  case "setStyle": try rangeSelection().setStyle(a0.stringValue.orThrow())
  case "toggleFormat": try rangeSelection().toggleFormat(a0.stringValue.orThrow())
  default: throw ScriptError("unknown selection method \(method)")
  }
}

private func runNodeMethod(
  _ n: LexicalNode, _ method: String, _ args: JSONValue, _ ctx: ScriptContext
) throws {
  let a0 = args.at(0)
  let a1 = args.at(1)
  func one() throws -> LexicalNode {
    try parse(.array([a0])).first.orThrow()
  }
  switch method {
  case "addID": try markNode(n).addID(a0.stringValue.orThrow())
  case "append": try element(n).append(contentsOf: parse(a0))
  case "clear": try element(n).clear()
  case "deleteID": try markNode(n).deleteID(a0.stringValue.orThrow())
  case "excludeFromCopy":
    let destination = try a0.stringValue.orThrow()
    ctx.log.append(
      .array([
        .string("excludeFromCopy"), .string(n.getKey()), .string(destination),
        .bool(try element(n).excludeFromCopy(destination)),
      ]))
  case "extractWithChild":
    let child = try a0.stringValue.orThrow()
    let destination = try a1.stringValue.orThrow()
    let extracted = try element(n).extractWithChild(node(child), anySelection(), destination)
    ctx.log.append(
      .array([
        .string("extractWithChild"), .string(n.getKey()), .string(child), .string(destination),
        .bool(extracted),
      ]))
  case "getIDs":
    ctx.log.append(
      .array([.string("getIDs"), .string(n.getKey()), strings(try markNode(n).getIDs())]))
  case "hasID":
    let id = try a0.stringValue.orThrow()
    ctx.log.append(
      .array([.string("hasID"), .string(n.getKey()), .string(id), .bool(try markNode(n).hasID(id))])
    )
  // A string names an existing node, which moves.
  case "insertAfter":
    _ = try n.insertAfter(
      a0.stringValue.map(node) ?? one(), restoreSelection: a1.boolValue ?? true)
  case "insertBefore":
    _ = try n.insertBefore(
      a0.stringValue.map(node) ?? one(), restoreSelection: a1.boolValue ?? true)
  case "markDirty": try n.markDirty()
  case "mergeWithSibling":
    _ = try text(n).mergeWithSibling(text(node(a0.stringValue.orThrow())))
  case "remove": try n.remove(preserveEmptyParent: a0.boolValue ?? false)
  case "replace": _ = try n.replace(one(), includeChildren: a1.boolValue ?? false)
  case "select":
    if let t = n as? TextNode {
      try t.select(a0.int, a1.int)
    } else {
      try element(n).select(a0.int, a1.int)
    }
  case "selectEnd": _ = try n.selectEnd()
  case "selectNext": _ = try n.selectNext(a0.int, a1.int)
  case "selectPrevious": _ = try n.selectPrevious(a0.int, a1.int)
  case "selectStart": _ = try n.selectStart()
  case "setDetail": try text(n).setDetail(a0.int.orThrow())
  case "setDirection": try element(n).setDirection(a0.stringValue)
  case "setFormat":
    if let t = n as? TextNode {
      if let format = a0.int {
        try t.setFormat(format)
      } else {
        try t.setFormat(a0.stringValue.orThrow() as TextFormatType)
      }
    } else if let block = n as? DecoratorBlockNode {
      try block.setFormat(a0.stringValue.orThrow())
    } else {
      try element(n).setFormat(a0.stringValue.orThrow())
    }
  case "setIDs":
    try markNode(n).setIDs((a0.arrayValue.orThrow()).map { try $0.stringValue.orThrow() })
  case "setIndent": try element(n).setIndent(a0.int.orThrow())
  case "setIsSyntaxHighlightSupported":
    try codeNode(n).setIsSyntaxHighlightSupported(a0.boolValue.orThrow())
  // `null` is `undefined`.
  case "setLanguage": try codeNode(n).setLanguage(a0.stringValue)
  case "setTag":
    guard let heading = n as? HeadingNode else {
      throw ScriptError("\(n.getKey()) is not a heading")
    }
    _ = try heading.setTag(a0.stringValue.orThrow())
  case "setMode": try text(n).setMode(a0.stringValue.orThrow())
  case "setStyle":
    if let t = n as? TextNode {
      try t.setStyle(a0.stringValue.orThrow())
    } else {
      try element(n).setStyle(a0.stringValue.orThrow())
    }
  case "setTextContent": try text(n).setTextContent(a0.stringValue.orThrow())
  case "setTextFormat": try element(n).setTextFormat(a0.int.orThrow())
  case "setTextStyle": try element(n).setTextStyle(a0.stringValue.orThrow())
  case "splice":
    try element(n).splice(a0.int.orThrow(), a1.int.orThrow(), parse(args.at(2)))
  case "spliceText":
    _ = try text(n).spliceText(
      a0.int.orThrow(), a1.int.orThrow(), args.at(2).stringValue.orThrow(),
      moveSelection: args.at(3).boolValue ?? false)
  case "splitText":
    let parts = try text(n).splitText(offsets: (args.arrayValue ?? []).compactMap(\.int))
    ctx.log.append(.array([.string("splitText"), .array(parts.map { $0.getKey() })]))
  case "toggleChecked":
    guard let item = n as? ListItemNode else {
      throw ScriptError("\(n.getKey()) is not a list item")
    }
    _ = try item.toggleChecked()
  case "toggleDirectionless": try text(n).toggleDirectionless()
  case "toggleFormat": try text(n).toggleFormat(a0.stringValue.orThrow())
  case "toggleUnmergeable": try text(n).toggleUnmergeable()
  default: throw ScriptError("unknown node method \(method)")
  }
}

private func runAction(_ action: JSONValue, _ ctx: ScriptContext) throws {
  let kind = try action["do"]?.stringValue.orThrow()
  switch kind {
  case "append":
    try element(node(action["key"]?.stringValue.orThrow() ?? "")).append(
      contentsOf: parse(action["nodes"] ?? .null))
  case "select":
    let selection = createRangeSelection()
    let anchor = try point(action["anchor"] ?? .null)
    let focus = try point(action["focus"] ?? action["anchor"] ?? .null)
    try selection.anchor.set(anchor.0, anchor.1, anchor.2)
    try selection.focus.set(focus.0, focus.1, focus.2)
    try setSelection(selection)
  case "selectNodes":
    let selection = createNodeSelection()
    for key in action["keys"]?.arrayValue ?? [] {
      selection.add(try key.stringValue.orThrow())
    }
    try setSelection(selection)
  case "clearSelection": try setSelection(nil)
  case "selectAll":
    if action["scoped"]?.boolValue == true {
      _ = try selectAll(rangeSelection())
    } else {
      _ = try selectAll()
    }
  case "insertNodes": try insertNodes(parse(action["nodes"] ?? .null))
  case "selection":
    try runSelectionMethod(action["method"]?.stringValue.orThrow() ?? "", action["args"] ?? .null)
  case "node":
    try runNodeMethod(
      node(action["key"]?.stringValue.orThrow() ?? ""),
      action["method"]?.stringValue.orThrow() ?? "", action["args"] ?? .null, ctx)
  case "logCaretRange":
    let range: CaretRange
    if let anchor = action["anchor"], let focus = action["focus"] {
      range = try getCaretRange(caret(anchor), caret(focus))
    } else {
      range = try caretRangeFromSelection(rangeSelection())
    }
    let (anchorSlice, focusSlice) = try range.getTextSlices()
    func slice(_ s: TextPointCaretSlice?) -> JSONValue {
      guard let s else { return .null }
      return .array([.string(s.caret.origin.getKey()), .int(s.caret.offset), .int(s.distance)])
    }
    ctx.log.append(
      .array([
        .string("caretRange"), caretSnapshot(range.anchor), caretSnapshot(range.focus),
        .array(try range.iterNodeCarets().toArray().map(caretSnapshot)),
        .array([slice(anchorSlice), slice(focusSlice)]),
      ]))
  case "removeTextFromCaretRange":
    let range = try removeTextFromCaretRange(caretRangeFromSelection(rangeSelection()))
    ctx.log.append(
      .array([
        .string("removeTextFromCaretRange"), caretSnapshot(range.anchor),
        caretSnapshot(range.focus),
      ]))
    _ = try setSelectionFromCaretRange(range)
  case "logNormalizeCaret":
    ctx.log.append(
      .array([
        .string("normalizeCaret"),
        caretSnapshot(try normalizeCaret(caret(action["caret"] ?? .null))),
      ]))
  case "logSelectionText":
    guard let selection = try getSelection() else { throw ScriptError("no selection") }
    ctx.log.append(.array([.string("selectionText"), .string(try selection.getTextContent())]))
  case "setSlot":
    let host = try node(action["key"]?.stringValue.orThrow() ?? "")
    guard isSlotHost(host) else { throw ScriptError("not a slot host") }
    let value: LexicalNode
    if let valueKey = action["valueKey"]?.stringValue {
      value = try node(valueKey)
    } else {
      value = try parse(.array([action["node"] ?? .null]))[0]
    }
    try setSlot(host, action["name"]?.stringValue.orThrow() ?? "", value)
  case "removeSlot":
    let host = try node(action["key"]?.stringValue.orThrow() ?? "")
    guard isSlotHost(host) else { throw ScriptError("not a slot host") }
    try removeSlot(host, action["name"]?.stringValue.orThrow() ?? "")
  case "logSlots":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    let slotted = try node(key)
    let keyOf: (LexicalNode?) -> JSONValue = { $0.map { .string($0.getKey()) } ?? .null }
    ctx.log.append(
      .array([
        .string("slots"), .string(key), strings(try getSlotNames(slotted)),
        keyOf(try getSlotHost(slotted)),
        try getSlotNameWithinHost(slotted).map { .string($0) } ?? .null,
        keyOf(try getSlotFrame(slotted)),
      ]))
  case "logSelectionSlotFrame":
    let frame = try getSelectionSlotFrame(getSelection())
    ctx.log.append(
      .array([.string("selectionSlotFrame"), frame.map { .string($0.getKey()) } ?? .null]))
  case "logDfsWithSlots":
    let start = try action["start"]?.stringValue.map { try node($0) }
    let end = try action["end"]?.stringValue.map { try node($0) }
    let reverse = action["reverse"]?.boolValue ?? false
    let nodes = try reverse ? reverseDfsWithSlots(start, end) : dfsWithSlots(start, end)
    ctx.log.append(
      .array([
        .string("dfsWithSlots"), .bool(reverse),
        .array(nodes.map { .array([.string($0.node.getKey()), .int($0.depth)]) }),
      ]))
  case "logNodesExist":
    let keys = action["keys"]?.arrayValue ?? []
    ctx.log.append(
      .array([
        .string("nodesExist"),
        .array(
          try keys.map { key in
            let key = key.stringValue ?? ""
            return .array([.string(key), .bool(try getNodeByKey(key) != nil)])
          }),
      ]))
  case "setBlockId":
    try setState(
      node(action["key"]?.stringValue ?? ""), blockIdState, action["id"]?.stringValue)
  case "logNodeStructure":
    for type in action["types"]?.arrayValue ?? [] {
      let type = type.stringValue ?? ""
      ctx.log.append(
        .array([.string("nodeStructure"), .string(type), structureJSON(try registeredClass(type))]))
    }
  case "logBlockContainer":
    for key in action["keys"]?.arrayValue ?? [] {
      let key = key.stringValue ?? ""
      ctx.log.append(
        .array([.string("blockContainer"), .string(key), .bool(try isBlockContainer(node(key)))]))
    }
  case "logStructureEnforced":
    ctx.log.append(
      .array([.string("structureEnforced"), .bool(INTERNAL_isNodeStructureEnforced(ctx.editor))]))
  case "normalizeNodeStructure":
    try INTERNAL_normalizeNodeStructure(node(action["key"]?.stringValue ?? ""))
  case "appendNew":
    let klass = try registeredClass(action["type"]?.stringValue ?? "")
    try element(node(action["key"]?.stringValue ?? "")).append(try create(klass))
  case "logBlockIds":
    let keys = action["keys"]?.arrayValue ?? []
    ctx.log.append(
      .array([
        .string("blockIds"),
        .array(
          try keys.map { key in
            let key = key.stringValue ?? ""
            return .array([.string(key), try getBlockId(node(key)).map { .string($0) } ?? .null])
          }),
      ]))
  case "logBlockById":
    let id = action["id"]?.stringValue ?? ""
    ctx.log.append(
      .array([
        .string("blockById"), .string(id),
        try getBlockById(id).map { .string($0.getKey()) } ?? .null,
      ]))
  case "logIdentified":
    let key = action["key"]?.stringValue ?? ""
    let target = try node(key)
    ctx.log.append(
      .array([
        .string("identified"), .string(key),
        .bool(try getExtensionOutput(BlockIdExtension).isIdentifiedBlock(target)),
        .bool(try defaultIsIdentifiedBlock(target)), .bool(try isRootChildBlock(target)),
      ]))
  case "logBlockHandles":
    let handles = try getBlockHandles()
    if let name = action["name"]?.stringValue {
      ctx.handles[name] = handles
    }
    ctx.log.append(
      .array([
        .string("blockHandles"),
        .array(handles.map { id, handle in .array([.string(id), .string(handle)]) }),
      ]))
  case "logBlockHandle":
    let key = action["key"]?.stringValue ?? ""
    ctx.log.append(
      .array([
        .string("blockHandle"), .string(key),
        try getBlockHandle(node(key)).map { .string($0) } ?? .null,
      ]))
  case "resolveBlockHandle":
    let handle = action["handle"]?.stringValue ?? ""
    var options: BlockHandleOptions?
    if let name = action["read"]?.stringValue {
      guard let read = ctx.handles[name] else { throw ScriptError("no handles kept as \(name)") }
      options = BlockHandleOptions(read: read)
    }
    let result = try resolveBlockHandle(handle, options)
    let detail: JSONValue =
      switch result {
      case .found(let found): .string(found.getKey())
      case .ambiguous(_, let candidates): .array(candidates.map { .string($0) })
      case .invalid(_, let reason): .string(reason.rawValue)
      case .notFound: .null
      }
    ctx.log.append(.array([.string("resolved"), .string(handle), .string(result.status), detail]))
  case "setState":
    try ctx.state(action["state"]?.stringValue.orThrow() ?? "").set(
      node(action["key"]?.stringValue.orThrow() ?? ""), action["value"] ?? .null)
  case "updateState":
    try ctx.updateState(
      node(action["key"]?.stringValue.orThrow() ?? ""),
      action["state"]?.stringValue.orThrow() ?? "", action["updater"]?.stringValue.orThrow() ?? "")
  case "logState":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    let name = try action["state"]?.stringValue.orThrow() ?? ""
    let value = try ctx.state(name).get(node(key))
    ctx.log.append(.array([.string("state"), .string(key), .string(name), value]))
  case "logStateChange":
    // `$getStateChange` of the node's latest version against its version in
    // the committed editor state, or "new" when that state has no node with
    // the key.
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    let name = try action["state"]?.stringValue.orThrow() ?? ""
    let change: JSONValue
    if let prevNode = ctx.editor.getEditorState()._nodeMap.get(key) {
      change = try ctx.state(name).change(node(key), prevNode)
    } else {
      change = .string("new")
    }
    ctx.log.append(.array([.string("stateChange"), .string(key), .string(name), change]))
  case "setBlocksType":
    switch try action["block"]?.stringValue.orThrow() {
    case "paragraph": try setBlocksType(getSelection()) { try createParagraphNode() }
    case "quote": try setBlocksType(getSelection()) { try createQuoteNode() }
    case let tag? where ["h1", "h2", "h3", "h4", "h5", "h6"].contains(tag):
      try setBlocksType(getSelection()) { try createHeadingNode(tag) }
    case let block: throw ScriptError("unknown block \(String(describing: block))")
    }
  case "logRootTextContent":
    ctx.log.append(.array([.string("rootTextContent"), .string(try rootTextContent())]))
  case "logRootTextContentEmpty":
    // `trim` absent is `undefined` in JS, so the default (`true`) applies.
    let isComposing = try action["isComposing"]?.boolValue.orThrow() ?? false
    let trim = action["trim"]?.boolValue
    ctx.log.append(
      .array([
        .string("rootTextContentEmpty"), .bool(isComposing), trim.map(JSONValue.bool) ?? .null,
        .bool(try isRootTextContentEmpty(isComposing, trim: trim ?? true)),
        .bool(try isRootTextContentEmptyCurry(isComposing, trim: trim ?? true)()),
      ]))
  case "logCanShowPlaceholder":
    let isComposing = try action["isComposing"]?.boolValue.orThrow() ?? false
    ctx.log.append(
      .array([
        .string("canShowPlaceholder"), .bool(isComposing),
        .bool(try canShowPlaceholder(isComposing)),
        .bool(try canShowPlaceholderCurry(isComposing)()),
      ]))
  case "logTextIntersection":
    let characters = try action["characters"]?.int.orThrow() ?? 0
    let found = try findTextIntersectionFromCharacters(getRoot(), characters)
    ctx.log.append(
      .array([
        .string("textIntersection"), .number(Double(characters)),
        found.map { .array([.string($0.node.getKey()), .number(Double($0.offset))]) } ?? .null,
      ]))
  case "wrapSelectionInMarkNode":
    try wrapSelectionInMarkNode(
      rangeSelection(), action["isBackward"]?.boolValue ?? false,
      action["id"]?.stringValue.orThrow() ?? "")
  case "unwrapMarkNode":
    try unwrapMarkNode(markNode(node(action["key"]?.stringValue.orThrow() ?? "")))
  case "logMarkIDs":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    let offset = try action["offset"]?.int.orThrow() ?? 0
    let ids = try getMarkIDs(text(node(key)), offset)
    ctx.log.append(
      .array([.string("markIDs"), .string(key), .int(offset), ids.map(strings) ?? .null]))
  case "addMarkID":
    let id = try action["id"]?.stringValue.orThrow() ?? ""
    let nodes = try addMarkID(id, anySelection())
    ctx.log.append(
      .array([.string("addMarkID"), .string(id), .array(nodes.map { .string($0.getKey()) })]))
  case "removeMarkID":
    let id = try action["id"]?.stringValue.orThrow() ?? ""
    if action["inSelection"]?.boolValue == true {
      try removeMarkID(id, anySelection())
    } else {
      try removeMarkID(id)
    }
  case "logNodeMarkIDs":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    ctx.log.append(
      .array([.string("nodeMarkIDs"), .string(key), strings(try getNodeMarkIDs(node(key)))]))
  case "logMarkIDsAtSelection":
    let mode = action["mode"]?.stringValue
    let selection: (any BaseSelection)? = try getSelection()
    let ids = try getMarkIDsAtSelection(selection, mode == "all" ? .all : .any)
    ctx.log.append(
      .array([.string("markIDsAtSelection"), mode.map { .string($0) } ?? .null, strings(ids)]))
  case "migrateMarkNodesToMarkState":
    let target = try action["key"]?.stringValue.map { try node($0) }
    let count = try migrateMarkNodesToMarkState(target)
    ctx.log.append(.array([.string("migrateMarkNodesToMarkState"), .int(count)]))
  case "copy":
    // `$clipboardDataWithoutHtml` in ops.ts: `$getClipboardDataFromSelection`
    // with its default MIME types but `text/html`, which needs a DOM.
    let selection = try getSelection()
    var data: LexicalClipboardData = ["text/plain": ""]
    if let selection {
      if let lexical = try getLexicalContent(ctx.editor, selection) {
        data["application/x-lexical-editor"] = lexical
      }
      data["text/plain"] = try selection.getTextContent()
    }
    let dataTransfer = DataTransfer()
    setLexicalClipboardDataTransfer(dataTransfer, data)
    ctx.clipboard = dataTransfer
    ctx.log.append(
      .array([
        .string("copy"),
        .array(
          dataTransfer.types.map { .array([.string($0), .string(dataTransfer.getData($0))]) }),
      ]))
  case "pasteClipboard":
    guard let dataTransfer = ctx.clipboard else { throw ScriptError("nothing copied") }
    try insertDataTransfer(dataTransfer, plain: action["plain"]?.boolValue == true, ctx)
  case "pasteLexical":
    let payload = JSONObject([
      ("namespace", .string(ctx.editor._config.namespace)), ("nodes", action["nodes"] ?? .null),
    ])
    let dataTransfer = DataTransfer()
    dataTransfer.setData("application/x-lexical-editor", payload.stringify())
    try insertDataTransfer(dataTransfer, plain: false, ctx)
  case "pastePlainText":
    let dataTransfer = DataTransfer()
    dataTransfer.setData("text/plain", try action["text"]?.stringValue.orThrow() ?? "")
    try insertDataTransfer(dataTransfer, plain: true, ctx)
  case "pasteData":
    let dataTransfer = DataTransfer()
    for (type, value) in try (action["data"]?.objectValue).orThrow() {
      dataTransfer.setData(type, try value.stringValue.orThrow())
    }
    try insertDataTransfer(dataTransfer, plain: action["plain"]?.boolValue == true, ctx)
  case "generateJSON":
    let selection = action["whole"]?.boolValue == true ? nil : try getSelection()
    // The log is JSON, which leaves out what `JSON.stringify` does.
    let json = try JSONValue.parse(
      generateJSONFromSelectedNodes(ctx.editor, selection).toJSON().stringify())
    ctx.log.append(.array([.string("generateJSON"), json]))
  case "insertGeneratedNodes":
    let serialized = try (action["nodes"]?.arrayValue ?? []).map { value in
      guard let object = value.objectValue else { throw ScriptError("not a serialized node") }
      return object
    }
    try insertGeneratedNodes(
      ctx.editor, generateNodesFromSerializedNodes(serialized), anySelection())
  case "selectTypeaheadOption":
    let (_, match) = try typeaheadMatch(action["typeahead"] ?? .null, ctx.editor)
    let node = try match.flatMap { try splitNodeContainingQuery($0) }
    ctx.log.append(
      .array([
        .string("splitNodeContainingQuery"), node.map { .string($0.getKey()) } ?? .null,
        try node.map { .string(try $0.getTextContent()) } ?? .null,
      ]))
    if let node {
      let replacement = try createTextNode(action["replacement"]?.stringValue.orThrow() ?? "")
      _ = try node.replace(replacement)
      _ = try replacement.select()
    }
  case "logFullMatchOffset":
    let documentText = try action["documentText"]?.stringValue.orThrow() ?? ""
    let entryText = try action["entryText"]?.stringValue.orThrow() ?? ""
    let offset = try action["offset"]?.int.orThrow() ?? 0
    ctx.log.append(
      .array([
        .string("fullMatchOffset"), .string(documentText), .string(entryText), .int(offset),
        .int(getFullMatchOffset(documentText, entryText, offset)),
      ]))
  case "logDecoratorBlock":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    let n = try node(key)
    let block = n as? DecoratorBlockNode
    ctx.log.append(
      .array([
        .string("decoratorBlock"), .string(key), .bool(isDecoratorBlockNode(n)),
        .bool(try n.isInline()), try block.map { .string(try $0.getFormat()) } ?? .null,
        block.map { .bool($0.canIndent()) } ?? .null,
      ]))
  case "logUrlMatcher":
    let text = try action["text"]?.stringValue.orThrow() ?? ""
    let match = try URL_MATCHER.firstMatch(in: text)
    ctx.log.append(
      .array([
        .string("urlMatcher"), .string(text),
        match.map { match in
          .array([
            .int(text.utf16.distance(from: text.startIndex, to: match.range.lowerBound)),
            .string(String(text[match.range])),
          ])
        } ?? .null,
      ]))
  case "logSingleTokenPaste":
    let text = try action["text"]?.stringValue.orThrow() ?? ""
    ctx.log.append(
      .array([.string("singleTokenPaste"), .string(text), .bool(isSingleToken(text))]))
  case "findOffset":
    let text = try action["text"]?.stringValue.orThrow() ?? ""
    let name = action["strlen"]?.stringValue ?? "utf16"
    let strlen = try scriptStrlen(name)
    let offsets = (action["maxCharacters"]?.arrayValue ?? []).map { max -> JSONValue in
      .number(Double(findOffset(text, max.int ?? 0, strlen)))
    }
    ctx.log.append(
      .array([.string("findOffset"), .string(text), .string(name), .array(offsets)]))
  case "wrapOverflowedNodes":
    try wrapOverflowedNodes(action["offset"]?.int ?? 0)
  case "mergePrevious":
    let key = try action["key"]?.stringValue.orThrow() ?? ""
    guard let overflowNode = try node(key) as? OverflowNode else {
      throw ScriptError("\(key) is not an overflow node")
    }
    try mergePrevious(overflowNode)
  case "dispatch":
    let name = try action["command"]?.stringValue.orThrow() ?? ""
    let handled = try ctx.dispatch(name, action["payload"] ?? .null)
    ctx.log.append(.array([.string("dispatched"), .string(name), .bool(handled)]))
  default:
    guard try runSelectionUtilsAction(kind ?? "", action, ctx) else {
      throw ScriptError("unknown action \(String(describing: kind))")
    }
  }
}

/// `$insertDataTransfer(dataTransfer, plain)` in ops.ts:
/// `insertDataTransferForRichText` (or, with `plain`,
/// `insertDataTransferForPlainText`) at the selection.
private func insertDataTransfer(
  _ dataTransfer: DataTransfer, plain: Bool, _ ctx: ScriptContext
) throws {
  if plain {
    try insertDataTransferForPlainText(dataTransfer, anySelection())
  } else {
    try insertDataTransferForRichText(dataTransfer, anySelection(), ctx.editor)
  }
}

/// `$createBlock(block)` in ops.ts: a new paragraph, quote or heading.
private func createBlock(_ block: JSONValue?) throws -> ElementNode {
  switch try block?.stringValue.orThrow() {
  case "paragraph": try createParagraphNode()
  case "quote": try createQuoteNode()
  case let tag? where ["h1", "h2", "h3", "h4", "h5", "h6"].contains(tag):
    try createHeadingNode(tag)
  case let block: throw ScriptError("unknown block \(String(describing: block))")
  }
}

/// The type names of `types`, a JSON array of strings.
private func types(_ types: JSONValue?) -> [String] {
  (types?.arrayValue ?? []).compactMap(\.stringValue)
}

private func keys(_ nodes: [LexicalNode]) -> JSONValue { .array(nodes.map { $0.getKey() }) }

/// A `patchStyleText` patch: `null` removes a property, `{fn: 'describe'}`
/// is `(prev, target) => prev@kind`, and a string is the value.
private func stylePatchValue(_ value: JSONValue) -> StylePatchValue {
  if value == .null { return .remove }
  if let value = value.stringValue { return .value(value) }
  return .compute { prev, target in
    let kind =
      switch target {
      case .selection: "selection"
      case .text: "text"
      case .element: "element"
      }
    return "\(prev ?? "null")@\(kind)"
  }
}

/// The @lexical/selection and @lexical/utils actions of ops.ts; false when
/// `kind` is none of them.
private func runSelectionUtilsAction(
  _ kind: String, _ action: JSONValue, _ ctx: ScriptContext
) throws -> Bool {
  func key() throws -> String { try action["key"]?.stringValue.orThrow() ?? "" }
  func keyed() throws -> LexicalNode { try node(key()) }
  func flag(_ name: String) -> Bool { action[name]?.boolValue ?? false }
  func one() throws -> LexicalNode { try parse(.array([action["node"] ?? .null])).first.orThrow() }
  func log(_ items: JSONValue...) { ctx.log.append(.array(items)) }
  switch kind {
  case "patchStyleText":
    let patch = try action["patch"]?.objectValue.orThrow() ?? JSONObject()
    let entries = patch.keys.map { ($0, stylePatchValue(patch[$0] ?? .null)) }
    // A `StylePatch` is only written as a literal.
    switch entries.count {
    case 1: try patchStyleText(anySelection(), [entries[0].0: entries[0].1])
    case 2:
      try patchStyleText(
        anySelection(), [entries[0].0: entries[0].1, entries[1].0: entries[1].1])
    default: throw ScriptError("a patch of \(entries.count) properties")
    }
  case "logSelectionStyleValue":
    let property = try action["property"]?.stringValue.orThrow() ?? ""
    let value = try getSelectionStyleValueForProperty(
      anySelection(), property, action["defaultValue"]?.stringValue ?? "")
    log(.string("styleValue"), .string(property), .string(value))
  case "logStyleObject":
    let css = try action["css"]?.stringValue.orThrow() ?? ""
    let object = getStyleObjectFromCSS(css)
    log(
      .string("styleObject"), .string(css),
      .array(object.map { .array([.string($0.key), .string($0.value)]) }),
      .string(getCSSFromStyleObject(object)))
  case "logAtNodeEnd":
    let selection = try rangeSelection()
    log(
      .string("atNodeEnd"), .bool(try isAtNodeEnd(selection.anchor)),
      .bool(try isAtNodeEnd(selection.focus)))
  case "trimTextContentFromAnchor":
    try trimTextContentFromAnchor(
      ctx.editor, rangeSelection().anchor, action["count"]?.int.orThrow() ?? 0)
  case "forEachSelectedTextNode":
    var visited: [JSONValue] = []
    try forEachSelectedTextNode { textNode in
      visited.append(.array([.string(textNode.getKey()), .string(try textNode.getTextContent())]))
    }
    log(.string("forEachSelectedTextNode"), .array(visited))
  case "ensureForwardRangeSelection": try ensureForwardRangeSelection(rangeSelection())
  case "wrapNodes":
    let wrapping = try action["wrapping"].map { try createBlock($0) }
    try wrapNodes(anySelection(), { try createBlock(action["block"]) }, wrapping)
  case "logShouldOverride":
    let selection = try rangeSelection()
    let isBackward = flag("isBackward")
    log(
      .string("shouldOverride"), .bool(isBackward),
      .bool(try shouldOverrideDefaultCharacterSelection(selection, isBackward)),
      .bool(try isParentElementRTL(selection)))
  case "moveCharacter":
    try moveCharacter(rangeSelection(), flag("shift"), flag("isBackward"))
  case "logSlice":
    let sliced = try sliceSelectedTextNodeContent(anySelection(), text(keyed()), .clone)
    log(.string("slice"), .string(try key()), .string(try sliced.getTextContent()))
  case "copyBlockFormatIndent":
    try copyBlockFormatIndent(
      element(node(action["from"]?.stringValue.orThrow() ?? "")),
      element(node(action["to"]?.stringValue.orThrow() ?? "")))
  case "logCloneWithProperties":
    let n = try keyed()
    if let t = n as? TextNode {
      addNodeStyle(t)
    }
    log(.string("clone"), .string(try key()), .object(try cloneWithProperties(n).exportJSON()))
  case "logDfs":
    let start = try action["start"]?.stringValue.map { try node($0) }
    let end = try action["end"]?.stringValue.map { try node($0) }
    let reverse = flag("reverse")
    let nodes = try reverse ? reverseDfs(start, end) : dfs(start, end)
    log(
      .string("dfs"), .bool(reverse),
      .array(nodes.map { .array([.string($0.node.getKey()), .int($0.depth)]) }))
  case "logDepth":
    log(.string("depth"), .string(try key()), .int(try getDepth(keyed())))
  case "logNextSiblingOrParentSibling":
    let found = try getNextSiblingOrParentSibling(keyed())
    log(
      .string("nextSiblingOrParentSibling"), .string(try key()),
      found.map { .array([.string($0.0.getKey()), .int($0.1)]) } ?? .null)
  case "logNextRightPreorderNode":
    let found = try getNextRightPreorderNode(keyed())
    log(
      .string("nextRightPreorderNode"), .string(try key()),
      found.map { .string($0.getKey()) } ?? .null)
  case "logNearestBlockElementAncestor":
    let found = try getNearestBlockElementAncestorOrThrow(keyed())
    log(.string("nearestBlockElementAncestor"), .string(try key()), .string(found.getKey()))
  case "logChildren":
    let parent = try element(keyed())
    let reverse = flag("reverse")
    let children =
      try (reverse ? lastToFirstIterator(parent) : firstToLastIterator(parent)).toArray()
    log(.string("children"), .string(try key()), .bool(reverse), keys(children))
  case "unwrapAndFilterDescendants":
    let keep = types(action["keep"])
    let didMutate = try unwrapAndFilterDescendants(element(keyed())) {
      keep.contains($0.getType())
    }
    log(.string("unwrapAndFilterDescendants"), .string(try key()), .bool(didMutate))
  case "logDescendantsMatching":
    let types = types(action["types"])
    let found = try descendantsMatching(element(keyed()).getChildren()) {
      types.contains($0.getType())
    }
    log(.string("descendantsMatching"), .string(try key()), keys(found))
  case "insertFirst": try insertFirst(element(keyed()), one())
  case "insertNodeToNearestRoot":
    let inserted = try insertNodeToNearestRoot(one())
    log(.string("insertNodeToNearestRoot"), .string(inserted.getKey()))
  case "insertNodeIntoLeaf": try insertNodeIntoLeaf(one())
  case "splitNode":
    let (left, right) = try splitNode(element(keyed()), action["offset"]?.int.orThrow() ?? 0)
    log(.string("splitNode"), left.map { .string($0.getKey()) } ?? .null, .string(right.getKey()))
  case "wrapNodeInElement":
    let wrapped = try wrapNodeInElement(keyed()) { try createBlock(action["block"]) }
    log(.string("wrapNodeInElement"), .string(wrapped.getKey()))
  case "unwrapNode": try unwrapNode(element(keyed()))
  case "saveEditorState":
    ctx.saved[try action["name"]?.stringValue.orThrow() ?? ""] = ctx.editor.getEditorState()
  case "restoreEditorState":
    let name = try action["name"]?.stringValue.orThrow() ?? ""
    guard let state = ctx.saved[name] else { throw ScriptError("no saved state \(name)") }
    try restoreEditorState(ctx.editor, state)
  case "escape":
    let types = types(action["types"])
    let isContainer = { (n: LexicalNode) in n is ElementNode && types.contains(n.getType()) }
    let direction = try action["direction"]?.stringValue.orThrow() ?? ""
    let result = try direction == "up" ? onEscapeUp(isContainer) : onEscapeDown(isContainer)
    log(.string("escape"), .string(direction), .bool(result))
  case "logBlockFullySelected":
    let result = try isBlockFullySelected(element(keyed()), rangeSelection())
    log(.string("blockFullySelected"), .string(try key()), .bool(result))
  default: return false
  }
  return true
}

private func registerTransform(_ name: String, _ ctx: ScriptContext) throws {
  let editor: Editor = ctx.editor
  func log(_ key: NodeKey) {
    ctx.log.append(.array([.string("transform"), .string(name), .string(key)]))
  }
  switch name {
  case "mark:nested":
    // `registerNestedElementResolver` (@lexical/utils) for MarkNode: a mark
    // directly in another moves out of it, after logging
    // `['nestedOverlap', parent, node]` and giving the parent's ids to the
    // node; the parent's children after it go into a new mark with its ids.
    _ = try registerNestedElementResolver(
      editor, MarkNode.self, { from in try createMarkNode(from.getIDs()) },
      { from, to in
        ctx.log.append(
          .array([.string("nestedOverlap"), .string(from.getKey()), .string(to.getKey())]))
        for id in try from.getIDs() {
          try to.addID(id)
        }
      })
  case "text:upper-a":
    try editor.registerNodeTransform(TextNode.self) { node in
      log(node.getKey())
      let content = try node.getTextContent()
      if content.contains("a") {
        try node.setTextContent(content.replacingOccurrences(of: "a", with: "A"))
      }
    }
  case "text:bold-hash":
    try editor.registerNodeTransform(TextNode.self) { node in
      log(node.getKey())
      if try node.isSimpleText(), try node.getTextContent().hasPrefix("#"),
        try node.hasFormat("bold") == false
      {
        try node.toggleFormat("bold")
      }
    }
  case "text:split-hash":
    try editor.registerNodeTransform(TextNode.self) { node in
      log(node.getKey())
      let utf16 = try node.getTextContent().utf16
      let index = utf16.firstIndex(of: 35).map { utf16.distance(from: utf16.startIndex, to: $0) }
      if try node.isSimpleText(), let index, index > 0 {
        try node.splitText(index)[1].toggleUnmergeable()
      }
    }
  case "paragraph:log":
    try editor.registerNodeTransform(ParagraphNode.self) { log($0.getKey()) }
  case "root:log":
    try editor.registerNodeTransform(RootNode.self) { log($0.getKey()) }
  case "paragraph:remove-empty":
    try editor.registerNodeTransform(ParagraphNode.self) { node in
      log(node.getKey())
      if try node.isEmpty(), try node.getNextSibling() != nil {
        try node.remove()
      }
    }
  default:
    throw ScriptError("unknown transform \(name)")
  }
}

/// `runScript` in ops.ts: runs `script` on a new editor and records every
/// step.
func runScript(_ script: JSONValue) throws -> [StepRecord] {
  try runScriptWithBuild(script).steps
}

/// `runScript` with what building the editor from the script's `extensions`
/// recorded (`build`: the log, then the editor's state, selection and text,
/// or the message it threw, after which there are no steps).
func runScriptWithBuild(_ script: JSONValue) throws -> (build: JSONValue?, steps: [StepRecord]) {
  resetRandomKey()
  let ctx = ScriptContext()
  var built: TestExtensions?
  var build: JSONValue?
  if let extensions = script["extensions"]?.arrayValue {
    for flag in [
      "richText", "plainText", "list", "link", "code", "overflow", "hashtag", "extension", "mark",
      "decoratorBlock",
    ]
    where script[flag]?.boolValue == true {
      throw ScriptError("extensions and \(flag) don't mix")
    }
    let defs = TestExtensions(script["extensionDefs"]?.objectValue ?? JSONObject())
    defs.log = { [unowned ctx] in ctx.log.append($0) }
    defs.command = { [unowned ctx] in ctx.command($0) }
    defs.clock = { [unowned ctx] in ctx.clock }
    built = defs
    do {
      if let parentExtensions = script["parent"]?.arrayValue {
        let parent = try buildEditorFromExtensions(try parentExtensions.map { try defs.arg($0) })
        ctx.parent = parent
        defs.parent = parent
        if script["parentRead"]?.boolValue != false {
          ctx.editor = try parent.read {
            try buildEditorFromExtensions(try extensions.map { try defs.arg($0) })
          }
        } else {
          ctx.editor = try buildEditorFromExtensions(try extensions.map { try defs.arg($0) })
        }
      } else {
        ctx.editor = try buildEditorFromExtensions(try extensions.map { try defs.arg($0) })
      }
    } catch {
      let message = (error as? LexicalError)?.message ?? "\(error)"
      return (.object(JSONObject([("error", .string(message)), ("log", .array(ctx.log))])), [])
    }
    if script["liveRegion"]?.boolValue == true {
      ctx.log.append(ctx.liveRegionEntry())
    }
    if let parent = ctx.parent {
      ctx.log.append(try nestedEntry(ctx.editor, parent))
    }
    let (selection, content) = try ctx.editor.read {
      (try selectionSnapshot(), try getRoot().getTextContent())
    }
    let state = ctx.editor.getEditorState()
    build = .object(
      JSONObject([
        ("log", .array(ctx.log)), ("selection", selection), ("text", .string(content)),
        ("state", .string(try state.toJSON().stringify())),
        ("compact", .string(try state.toJSON(true).stringify())),
      ]))
  } else {
    ctx.editor = try makeEditor(script)
  }
  return (build, try runSteps(script, ctx, built))
}

/// `createEditor` with the nodes of the script's package flags.
private func makeEditor(_ script: JSONValue) throws -> Editor {
  // `richText: true` registers @lexical/rich-text's nodes and
  // `registerRichText`, `list: true` @lexical/list's nodes,
  // `registerList` and `registerCheckList`, and `link: true` @lexical/link's
  // nodes and `registerLink` with its default config, and `code: true`
  // @lexical/code-core's nodes, `CodeExtension`'s TabNode transform and
  // `registerCodeIndentation`, `overflow: true` @lexical/overflow's
  // `OverflowNode`, `hashtag: true` @lexical/hashtag's `HashtagNode` and
  // `registerLexicalHashtag`, `extension: true` @lexical/extension's
  // `HorizontalRuleNode` and `DecoratorTextNode`, and `mark: true`
  // @lexical/mark's `MarkNode`, and `markdown: true`
  // `registerMarkdownShortcuts` with `TRANSFORMERS`, as ops.ts does.
  var nodes: [LexicalNodeConfig] =
    script["richText"]?.boolValue == true
    ? [.klass(HeadingNode.self), .klass(QuoteNode.self)] : []
  if script["list"]?.boolValue == true {
    nodes += [.klass(ListNode.self), .klass(ListItemNode.self)]
  }
  if script["link"]?.boolValue == true {
    nodes += [.klass(LinkNode.self), .klass(AutoLinkNode.self)]
  }
  if script["code"]?.boolValue == true {
    nodes += [.klass(CodeNode.self), .klass(CodeHighlightNode.self)]
  }
  if script["overflow"]?.boolValue == true {
    nodes += [.klass(OverflowNode.self)]
  }
  if script["hashtag"]?.boolValue == true {
    nodes += [.klass(HashtagNode.self)]
  }
  if script["extension"]?.boolValue == true {
    nodes += [.klass(HorizontalRuleNode.self), .klass(DecoratorTextNode.self)]
  }
  if script["mark"]?.boolValue == true {
    nodes += [.klass(MarkNode.self)]
  }
  if script["decoratorBlock"]?.boolValue == true {
    nodes += [.klass(TestDecoratorBlockNode.self)]
  }
  return try createEditor(
    // ops.ts's `NAMESPACE`, which a clipboard payload names.
    CreateEditorArgs(namespace: "conformance", nodes: nodes, onError: { throw $0 }))
}

/// Registers the script's packages, transforms, handlers, listeners and
/// history on `ctx.editor`, then runs and records the steps.
private func runSteps(
  _ script: JSONValue, _ ctx: ScriptContext, _ extensions: TestExtensions?
) throws -> [StepRecord] {
  let editor: Editor = ctx.editor
  var unregisterCodeHighlighting: (() -> Void)?
  if script["richText"]?.boolValue == true {
    _ = registerRichText(editor)
  }
  if script["plainText"]?.boolValue == true {
    _ = registerPlainText(editor)
  }
  if script["list"]?.boolValue == true {
    _ = try registerList(editor)
    _ = try registerCheckList(editor)
  }
  if script["link"]?.boolValue == true {
    _ = try registerLink(editor, namedSignals(LinkConfig()))
    // `autoLink: true`: `registerAutoLink` with the URL and email matchers
    // and a change handler that logs `['autoLink', url, prevUrl]`.
    if script["autoLink"]?.boolValue == true {
      let changeHandler: ChangeHandler = { url, prevUrl in
        ctx.log.append(
          .array([
            .string("autoLink"), url.map { .string($0) } ?? .null,
            prevUrl.map { .string($0) } ?? .null,
          ]))
      }
      _ = try registerAutoLink(
        editor,
        AutoLinkConfig(
          changeHandlers: [changeHandler], matchers: [autoLinkUrlMatcher, autoLinkEmailMatcher]))
    }
  }
  if script["code"]?.boolValue == true {
    // `CodeExtension`'s register, which ops.ts calls on its own; Swift's
    // takes the builder's state, so its handler and transform are
    // registered here.
    _ = editor.registerCommand(
      KEY_ENTER_COMMAND,
      { event, _ in
        guard let selection = try getSelection() as? RangeSelection,
          try exitCodeNodeOnEnter(selection) != nil
        else { return false }
        event?.preventDefault()
        return true
      }, COMMAND_PRIORITY_LOW)
    _ = try editor.registerNodeTransform(TabNode.self) { node in
      if try node.getFormat() != 0, try node.getParent() is CodeNode {
        try node.setFormat(0)
      }
    }
    // `codeHighlight`: `registerCodeHighlighting` in place of
    // `registerCodeIndentation`, over ops.ts's tokenizer
    // (CodeHighlightOps.swift); `unregisterCodeHighlighting` unregisters it.
    if let spec = script["codeHighlight"] {
      unregisterCodeHighlighting = try registerCodeHighlighting(
        editor, try scriptCodeTokenizer(spec))
    } else {
      _ = registerCodeIndentation(editor)
    }
  }
  if script["hashtag"]?.boolValue == true {
    _ = try registerLexicalHashtag(editor)
  }
  if script["markdown"]?.boolValue == true {
    _ = try registerMarkdownShortcuts(editor, TRANSFORMERS)
  }
  for name in script["transforms"]?.arrayValue ?? [] {
    try registerTransform(name.stringValue.orThrow(), ctx)
  }
  for handler in script["handlers"]?.arrayValue ?? [] {
    let id = try handler["id"]?.stringValue.orThrow() ?? ""
    let actions = handler["actions"]?.arrayValue ?? []
    let result = handler["result"]?.boolValue ?? false
    ctx.register(
      try handler["command"]?.stringValue.orThrow() ?? "",
      handler["priority"]?.int ?? COMMAND_PRIORITY_EDITOR
    ) { payload in
      ctx.log.append(.array([.string("handler"), .string(id), payload]))
      for action in actions {
        try runAction(action, ctx)
      }
      return result
    }
  }
  // `mutations`: a listener per node type, logging the mutations and the
  // update's tags, sorted.
  for type in try (script["mutations"]?.arrayValue ?? []).map({ try $0.stringValue.orThrow() }) {
    guard let klass = editor._nodes.get(type)?.klass else {
      throw ScriptError("unknown node type \(type)")
    }
    try editor.registerMutationListener(
      klass,
      { mutations, payload in
        ctx.log.append(
          .array([
            .string("mutation"), .string(type),
            .array(mutations.map { .array([.string($0.0), .string($0.1)]) }),
            .array(Array(payload.updateTags).sorted().map { .string($0) }),
          ]))
      }, skipInitialization: true)
  }
  if script["listen"]?.boolValue == true {
    editor.registerUpdateListener { payload in
      ctx.log.append(
        .array([
          .string("update"),
          .array(payload.dirtyElements.map { .array([.string($0.0), .bool($0.1)]) }),
          .array(Array(payload.dirtyLeaves)),
          .array(Array(payload.tags).sorted()),
        ]))
    }
    for klass: LexicalNode.Type in [TextNode.self, ParagraphNode.self] {
      let type = klass.getType()
      try editor.registerMutationListener(
        klass,
        { mutations, _ in
          ctx.log.append(
            .array([
              .string("mutation"), .string(type),
              .array(mutations.map { .array([.string($0.0), .string($0.1)]) }),
            ]))
        }, skipInitialization: true)
    }
    editor.registerTextContentListener { content in
      ctx.log.append(.array([.string("textContent"), .string(content)]))
    }
  }
  if let delay = script["history"]?["delay"]?.numberValue {
    // ops.ts reads `Date.now()` at every update; here that is the session clock.
    _ = try registerHistory(editor, createEmptyHistoryState(), delay) { [unowned ctx] in ctx.clock }
  }

  /// Runs `step` on `editor`: the session's, or (`inParent`) the parent.
  func runStep(_ step: JSONValue, on editor: Editor) throws {
    do {
      if let actions = step["update"]?.arrayValue {
        let tags = (step["tag"]?.arrayValue ?? []).compactMap(\.stringValue)
        try editor.update(
          {
            for action in actions {
              try runAction(action, ctx)
            }
          }, EditorUpdateOptions(tag: tags, discrete: true))
      } else if let name = step["dispatch"]?.stringValue {
        let handled = try ctx.dispatch(name, step["payload"] ?? .null)
        ctx.log.append(.array([.string("dispatched"), .string(name), .bool(handled)]))
      } else if let on = step["setRootElement"]?.boolValue {
        try editor.setRootElement(on ? ctx.sessionRoot() : nil)
      } else if let editable = step["setEditable"]?.boolValue {
        try editor.setEditable(editable)
      } else if let text = step["announce"]?.stringValue {
        try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output.announce(text)
      } else if step["unregisterCodeHighlighting"] != nil {
        let unregister = try unregisterCodeHighlighting.orThrow()
        unregisterCodeHighlighting = nil
        unregister()
      } else if let spec = step["typeahead"] {
        try editor.getEditorState().read {
          let (text, match) = try typeaheadMatch(spec, editor)
          ctx.log.append(
            .array([
              .string("typeahead"), text.map { .string($0) } ?? .null,
              match.map(menuTextMatchJSON) ?? .null,
              try match.map { .bool(try isSelectionOnEntityBoundary($0.leadOffset)) } ?? .null,
            ]))
        }
      } else if step["characterLimit"] != nil || step["tableOfContents"] != nil {
        try mountStep(step, ctx)
      } else if let name = step["registerNodeStructure"]?.stringValue {
        ctx.structures[name] = try registerNodeStructure(editor)
      } else if let name = step["disposeNodeStructure"]?.stringValue {
        guard let dispose = ctx.structures.removeValue(forKey: name) else {
          throw ScriptError("no registration \(name)")
        }
        dispose()
      } else if let name = step["unmount"]?.stringValue {
        guard let unmount = ctx.mounted.removeValue(forKey: name) else {
          throw ScriptError("nothing mounted as \(name)")
        }
        unmount()
      } else {
        try extensions.orThrow().runStep(step, editor)
      }
    }
  }

  let steps = script["steps"]?.arrayValue ?? []
  let finalOnly = script["snapshot"]?.stringValue == "final"
  return try steps.enumerated().map { i, step in
    if let now = step["now"]?.numberValue {
      ctx.clock = now
    }
    ctx.log = []
    var failed = false
    do {
      if step["inParent"]?.boolValue == true {
        guard let parent = ctx.parent else { throw ScriptError("the script has no parent editor") }
        ctx.editor = parent
        defer { ctx.editor = editor }
        try runStep(step, on: parent)
      } else {
        try runStep(step, on: editor)
      }
    } catch {
      failed = true
    }
    if script["liveRegion"]?.boolValue == true {
      ctx.log.append(ctx.liveRegionEntry())
    }
    if script["codeHighlight"] != nil {
      ctx.log.append(try codeBlocksEntry(editor))
    }
    if let parent = ctx.parent {
      ctx.log.append(try nestedEntry(editor, parent))
    }
    let (selection, content) = try editor.read {
      (try selectionSnapshot(), try getRoot().getTextContent())
    }
    var record = StepRecord(
      selection: selection, text: content, log: ctx.log, error: failed)
    if !finalOnly || i == steps.count - 1 {
      let state = editor.getEditorState()
      record.state = try state.toJSON().stringify()
      record.compact = try state.toJSON(true).stringify()
    }
    return record
  }
}

/// `nestedEntry(editor, parent)` in ops.ts: `['nested', isNested,
/// parentIsParent, sameNamespace, theme, editable, parentEditable,
/// parentText, parentSelection]`, the theme's entries sorted by key in
/// UTF-16 order, as JS's `sort()`.
private func nestedEntry(_ editor: Editor, _ parent: Editor) throws -> JSONValue {
  let (parentText, parentSelection) = try parent.read {
    (try getRoot().getTextContent(), try selectionSnapshot())
  }
  let theme = editor._config.theme.sorted { $0.key.utf16.lexicographicallyPrecedes($1.key.utf16) }
  return .array([
    "nested", .bool(isEditorIsNestedEditor(editor)), .bool(editor._parentEditor === parent),
    .bool(editor._config.namespace == parent._config.namespace),
    .array(theme.map { .array([.string($0.key), $0.value]) }),
    .bool(editor.isEditable()), .bool(parent.isEditable()), .string(parentText), parentSelection,
  ])
}
