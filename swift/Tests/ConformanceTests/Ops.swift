import Foundation
import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalOverflow
import LexicalPlainText
import LexicalRichText

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
]

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

private final class ScriptContext {
  let editor: Editor
  var log: [JSONValue] = []
  var commands: [String: LexicalCommand<JSONValue>] = [:]
  /// The session's clock (ms): what the history listener reads as `Date.now()`.
  var clock: Double = 0

  init(_ editor: Editor) { self.editor = editor }

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
    case .listStart(let command):
      return try editor.dispatchCommand(command, try listStart(payload))
    case .toggleLink(let command):
      return try editor.dispatchCommand(command, try toggleLinkPayload(payload))
    case nil:
      return try editor.dispatchCommand(self.command(name), payload)
    }
  }

  /// `editor.registerCommand(command(ctx, name), listener, priority)`.
  func register(
    _ name: String, _ priority: Int, _ listener: @escaping (JSONValue) throws -> Bool
  ) {
    switch coreCommands[name] {
    case .void(let command):
      _ = editor.registerCommand(command, { _, _ in try listener(.null) }, priority)
    case .bool(let command):
      _ = editor.registerCommand(command, { payload, _ in try listener(.bool(payload)) }, priority)
    case .string(let command):
      _ = editor.registerCommand(
        command, { payload, _ in try listener(.string(payload)) }, priority)
    case .formats(let command):
      _ = editor.registerCommand(
        command,
        { payload, _ in try listener(.object(JSONObject(payload.map { ($0, .bool($1)) }))) },
        priority)
    case .keyboard(let command):
      _ = editor.registerCommand(command, { _, _ in try listener(.null) }, priority)
    case .listStart(let command):
      _ = editor.registerCommand(
        command,
        { payload, _ in
          try listener(
            .object(
              JSONObject([
                ("listNodeKey", .string(payload.listNodeKey)), ("newStart", .int(payload.newStart)),
              ])))
        }, priority)
    case .toggleLink(let command):
      _ = editor.registerCommand(
        command, { payload, _ in try listener(toggleLinkJSON(payload)) }, priority)
    case nil:
      _ = editor.registerCommand(
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
  case "append": try element(n).append(contentsOf: parse(a0))
  case "clear": try element(n).clear()
  case "insertAfter": _ = try n.insertAfter(one(), restoreSelection: a1.boolValue ?? true)
  case "insertBefore": _ = try n.insertBefore(one(), restoreSelection: a1.boolValue ?? true)
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
    } else {
      try element(n).setFormat(a0.stringValue.orThrow())
    }
  case "setIndent": try element(n).setIndent(a0.int.orThrow())
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
  case "selectAll": _ = try selectAll()
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
  case "dispatch":
    let name = try action["command"]?.stringValue.orThrow() ?? ""
    let handled = try ctx.dispatch(name, action["payload"] ?? .null)
    ctx.log.append(.array([.string("dispatched"), .string(name), .bool(handled)]))
  default:
    throw ScriptError("unknown action \(String(describing: kind))")
  }
}

private func registerTransform(_ name: String, _ ctx: ScriptContext) throws {
  let editor = ctx.editor
  func log(_ key: NodeKey) {
    ctx.log.append(.array([.string("transform"), .string(name), .string(key)]))
  }
  switch name {
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
  resetRandomKey()
  // `richText: true` registers @lexical/rich-text's nodes and
  // `registerRichText`, `list: true` @lexical/list's nodes,
  // `registerList` and `registerCheckList`, and `link: true` @lexical/link's
  // nodes and `registerLink` with its default config, and `code: true`
  // @lexical/code-core's nodes, `CodeExtension`'s TabNode transform and
  // `registerCodeIndentation`, and `overflow: true` @lexical/overflow's
  // `OverflowNode`, as ops.ts does.
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
  let editor = try createEditor(
    CreateEditorArgs(namespace: "", nodes: nodes, onError: { throw $0 }))
  let ctx = ScriptContext(editor)
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
    // takes the builder's state, so its transform is registered here. Its
    // KEY_ENTER_COMMAND handler is left out: no script dispatches a key.
    _ = try editor.registerNodeTransform(TabNode.self) { node in
      if try node.getFormat() != 0, try node.getParent() is CodeNode {
        try node.setFormat(0)
      }
    }
    _ = registerCodeIndentation(editor)
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
  let steps = script["steps"]?.arrayValue ?? []
  let finalOnly = script["snapshot"]?.stringValue == "final"
  return try steps.enumerated().map { i, step in
    if let now = step["now"]?.numberValue {
      ctx.clock = now
    }
    ctx.log = []
    var failed = false
    do {
      if let actions = step["update"]?.arrayValue {
        let tags = (step["tag"]?.arrayValue ?? []).compactMap(\.stringValue)
        try editor.update(
          {
            for action in actions {
              try runAction(action, ctx)
            }
          }, EditorUpdateOptions(tag: tags, discrete: true))
      } else {
        let name = try step["dispatch"]?.stringValue.orThrow() ?? ""
        let handled = try ctx.dispatch(name, step["payload"] ?? .null)
        ctx.log.append(.array([.string("dispatched"), .string(name), .bool(handled)]))
      }
    } catch {
      failed = true
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
