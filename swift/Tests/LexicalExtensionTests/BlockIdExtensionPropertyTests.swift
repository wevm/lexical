import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalList
import LexicalRichText
import LexicalSelection
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts.
//
// Property test of BlockIdExtension over random local edit sequences: typing,
// Enter, Backspace and forward-delete joins, range deletes, block type
// changes, moves, duplicates, pastes of several copies, indent and outdent,
// list conversion, moves into table cells, replace, undo and redo, and reloads
// of the state with an injected duplicate id. After every step each
// identified block must have exactly one id, the ids must be unique,
// getBlockById must find every identified block, blocks that existed before
// the step must keep their ids (where the step does not restructure them),
// and undo and redo must return to a state seen before.
//
// Upstream draws every choice from the seeded mulberry32 with
// `Math.floor(rnd() * n)`, in the same order as here, so each seed replays
// the same edits. Upstream's BLOCK_ID_PROPERTY_SEEDS and
// BLOCK_ID_PROPERTY_FIRST_SEED overrides are not ported: CI's 20 seeds run.

private let STEPS = 40

private func build() throws -> Editor {
  var clock = 0.0
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-property]",
      dependencies: [
        RichTextExtension, ListExtension, TableExtension,
        configExtension(HistoryExtension) {
          $0.disabled = false
          $0.now = {
            clock += 10000
            return clock
          }
        },
        configExtension(BlockIdExtension) { $0.generateId = counter() },
      ],
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  // Deleting characters reads the DOM selection.
  try editor.setRootElement(TestRootElement())
  // Have had content once, so that new blocks are edits, not a load.
  return try asEdited(editor)
}

/// Every node below the root in document order (slots before children).
private func all() throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  func walk(_ n: LexicalNode) throws {
    out.append(n)
    for name in try getSlotNames(n) {
      if let slot = try getSlot(n, name) {
        try walk(slot)
      }
    }
    if let element = n as? ElementNode {
      try element.getChildren().forEach(walk)
    }
  }
  try getRoot().getChildren().forEach(walk)
  return out
}

private struct Row {
  let key: NodeKey
  let type: String
  let text: String
  let id: String?
  let identified: Bool
}

private func rows(_ editor: Editor) throws -> [Row] {
  let isIdentified = try blockIdOutput(editor).isIdentifiedBlock
  return try editor.read {
    try all().filter { !($0 is TextNode) }.map { n in
      Row(
        key: n.getKey(), type: n.getType(), text: try n.getTextContent(), id: try getBlockId(n),
        identified: try isIdentified(n))
    }
  }
}

/// `type:text:id` of the identified blocks in document order.
private func outline(_ editor: Editor) throws -> [String] {
  try rows(editor).filter(\.identified).map { "\($0.type):\($0.text):\($0.id ?? "null")" }
}

/// Missing ids, repeated ids, and blocks the index does not find.
private func invariantProblems(_ editor: Editor) throws -> [String] {
  var problems: [String] = []
  let identified = try rows(editor).filter(\.identified)
  var seen: [String: String] = [:]
  for r in identified {
    guard let id = r.id else {
      problems.append("no id: \(r.type):\(r.text)")
      continue
    }
    if let previous = seen[id] {
      problems.append("dup \(id): \(previous) / \(r.type):\(r.text)")
    }
    seen[id] = "\(r.type):\(r.text)"
  }
  try editor.read {
    for r in identified {
      if let id = r.id {
        let found = try getBlockById(id)
        if found == nil || found!.getKey() != r.key {
          problems.append("index \(id) -> \(found?.getKey() ?? "null") != \(r.key)")
        }
      }
    }
  }
  return problems
}

private func textBlocks() throws -> [ElementNode] {
  try all().compactMap { n in
    n is ParagraphNode || n is HeadingNode || n is QuoteNode || n is ListItemNode
      ? n as? ElementNode : nil
  }
}

private func caretIn(_ block: ElementNode, _ offset: Int) throws {
  var rest = offset
  for text in try block.getAllTextNodes() {
    if try text.getParent() != nil, rest <= (try text.getTextContentSize()) {
      _ = try text.select(rest, rest)
      return
    }
    rest -= try text.getTextContentSize()
  }
  _ = try block.selectStart()
}

private enum Op: CaseIterable {
  case type, enter, join, fwdjoin, rangeDelete, setType, move, dup, pasteMany, indent, outdent
  case toList, intoCell, replace, undo, redo, setStateDup
}

private let OPS: [Op] = [
  .type, .enter, .enter, .join, .fwdjoin, .rangeDelete, .setType, .move, .dup, .pasteMany,
  .indent, .outdent, .toList, .intoCell, .replace, .undo, .redo, .setStateDup,
]

/// Which pre-existing blocks a step must leave on their ids.
private enum Kept { case all, none, enter }

/// `Math.floor(rnd() * n)`.
private func index(_ rnd: () -> Double, _ n: Int) -> Int {
  Int((rnd() * Double(n)).rounded(.down))
}

private func edit(
  _ editor: Editor, _ op: Op, _ rnd: @escaping () -> Double, _ step: Int
) throws
  -> Kept
{
  func pick<T>(_ xs: [T]) -> T { xs[index(rnd, xs.count)] }
  let blocks = try textBlocks()
  if blocks.isEmpty {
    try getRoot().append(paragraph("x"))
    return .all
  }
  let b = pick(blocks)
  let off = index(rnd, try b.getTextContentSize() + 1)
  switch op {
  case .type:
    try caretIn(b, off)
    let selection = try #require(try getSelection() as? RangeSelection)
    try selection.insertText("z")
    return .all
  case .enter:
    try caretIn(b, off)
    _ = try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
    return .enter
  case .join:
    _ = try b.selectStart()
    _ = try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
    return .none
  case .fwdjoin:
    _ = try b.selectEnd()
    _ = try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, false)
    return .none
  case .rangeDelete:
    let c = pick(blocks)
    let (x, y) = try b.isBefore(c) ? (b, c) : (c, b)
    let xt = try x.getAllTextNodes()
    let yt = try y.getAllTextNodes()
    if !xt.isEmpty && !yt.isEmpty && !x.is(y) {
      let start = min(1, try xt[0].getTextContentSize())
      _ = try xt[0].select(start, start)
      let selection = try #require(try getSelection() as? RangeSelection)
      let last = yt[yt.count - 1]
      try selection.focus.set(last.getKey(), min(1, try last.getTextContentSize()), .text)
      try selection.removeText()
    }
    return .none
  case .setType:
    try caretIn(b, off)
    let makers: [() throws -> ElementNode] = [
      { try createHeadingNode("h2") }, { try createQuoteNode() }, { try createParagraphNode() },
    ]
    try setBlocksType(getSelection(), { try pick(makers)() })
    return .none
  case .move:
    try setSelection(nil)
    let kids = try getRoot().getChildren()
    let moved = pick(kids)
    let target = pick(kids)
    if !moved.is(target) {
      if rnd() < 0.5 {
        _ = try target.insertBefore(moved)
      } else {
        _ = try target.insertAfter(moved)
      }
    }
    return .all
  case .dup:
    try setSelection(nil)
    let copy = try deepCopy(b)
    if rnd() < 0.5 {
      _ = try b.insertBefore(copy)
    } else {
      _ = try b.insertAfter(copy)
    }
    return .all
  case .pasteMany:
    try setSelection(nil)
    let src = pick(try getRoot().getChildren())
    let n = 1 + index(rnd, 3)
    for _ in 0..<n {
      _ = try pick(try getRoot().getChildren()).insertAfter(try deepCopy(src))
    }
    return .all
  case .indent:
    try caretIn(b, off)
    _ = try editor.dispatchCommand(INDENT_CONTENT_COMMAND)
    return .none
  case .outdent:
    try caretIn(b, off)
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND)
    return .none
  case .toList:
    try caretIn(b, off)
    _ = try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND)
    return .none
  case .intoCell:
    try setSelection(nil)
    let cells = try all().compactMap { $0 as? TableCellNode }
    if !cells.isEmpty && !(b is ListItemNode) {
      let cell = pick(cells)
      if try !b.isParentOf(cell) {
        try cell.append(b)
      }
    }
    return .all
  case .replace:
    try setSelection(nil)
    if !(b is ListItemNode) {
      _ = try b.replace(paragraph("r\(step)"))
    }
    return .none
  case .undo, .redo, .setStateDup:
    throw LexicalError("not an edit: \(op)")
  }
}

/// Reloads the state with one root child's id copied onto another.
private func reloadWithDuplicate(_ editor: Editor, _ rnd: () -> Double) throws {
  var json = try editor.getEditorState().toJSON()
  var root = try #require(json["root"]?.objectValue)
  var kids = try #require(root["children"]?.arrayValue).map { try #require($0.objectValue) }
  if kids.count < 2 {
    return
  }
  let i = index(rnd, kids.count)
  var j = index(rnd, kids.count)
  if j == i {
    j = (j + 1) % kids.count
  }
  guard let src = kids[i]["$"]?.objectValue?["blockId"]?.stringValue, !src.isEmpty else {
    return
  }
  var state = kids[j]["$"]?.objectValue ?? JSONObject()
  state["blockId"] = .string(src)
  kids[j]["$"] = .object(state)
  root["children"] = .array(kids.map { .object($0) })
  json["root"] = .object(root)
  try editor.setEditorState(editor.parseEditorState(json))
  let rootIds = try editor.read { try getRoot().getChildren().map { try getBlockId($0) } }
  // The earlier holder keeps the id.
  #expect(rootIds[min(i, j)] == src)
}

/// Pre-existing identified blocks keep their ids, as `kept` requires.
private func expectKept(_ before: [Row], _ afterRows: [Row], _ kept: Kept, _ label: String) {
  if kept == .none {
    return
  }
  var after: [NodeKey: Row] = [:]
  for r in afterRows { after[r.key] = r }
  for r in before {
    guard let a = after[r.key], r.identified, a.identified, let id = r.id else {
      continue
    }
    if kept == .enter && a.id != id {
      // Enter at the start moves the id with the text to the new block.
      if let moved = afterRows.first(where: { $0.id == id }),
        !before.contains(where: { $0.key == moved.key }), moved.text == r.text, a.text == ""
      {
        continue
      }
    }
    #expect(a.id == id, "\(label): \(r.type):\(r.text) keeps its id")
  }
  if kept == .all {
    // No id moves onto a different pre-existing block.
    var keyOfId: [String: NodeKey] = [:]
    for r in before where r.identified { if let id = r.id { keyOfId[id] = r.key } }
    for a in afterRows {
      if a.identified, let id = a.id, let k = keyOfId[id], after[k]?.identified == true {
        #expect(a.key == k, "\(label): \(id) stays on \(k)")
      }
    }
  }
}

@Suite struct BlockIdExtensionPropertyTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 1"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 2"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 3"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 4"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 5"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 6"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 7"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 8"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 9"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 10"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 11"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 12"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 13"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 14"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 15"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 16"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 17"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 18"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 19"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.property.test.ts::BlockIdExtension: property test over random edits > seed 20"
    ),
    arguments: Array(UInt32(1)...20))
  func randomEdits(seed: UInt32) throws {
    let rnd = mulberry32(seed)
    let editor = try build()
    defer { editor.dispose() }
    try discrete(editor) {
      try getRoot().append(
        paragraph("alpha"),
        createHeadingNode("h1").append(createTextNode("beta")),
        createListNode("bullet").append(
          createListItemNode().append(createTextNode("li1")),
          createListItemNode().append(createTextNode("li2"))),
        createTableNodeWithDimensions(2, 2, false),
        paragraph("gamma"))
    }
    // Outlines on the undo and redo stacks, and every outline seen.
    var undoStack: [[String]] = [try outline(editor)]
    var redoStack: [[String]] = []
    var seen: Set<String> = [try outline(editor).joined(separator: "|")]
    for step in 0..<STEPS {
      let before = try rows(editor)
      let op = OPS[index(rnd, OPS.count)]
      var kept = Kept.none
      switch op {
      case .undo:
        if undoStack.count > 1 {
          _ = try editor.dispatchCommand(UNDO_COMMAND)
          redoStack.append(undoStack.removeLast())
          #expect(seen.contains(try outline(editor).joined(separator: "|")), "undo at \(step)")
        }
      case .redo:
        if !redoStack.isEmpty {
          _ = try editor.dispatchCommand(REDO_COMMAND)
          undoStack.append(redoStack.removeLast())
          #expect(seen.contains(try outline(editor).joined(separator: "|")), "redo at \(step)")
        }
      case .setStateDup:
        try reloadWithDuplicate(editor, rnd)
        undoStack = [try outline(editor)]
        redoStack = []
        seen.insert(try outline(editor).joined(separator: "|"))
      default:
        let beforeJSON = JSONValue.object(try editor.getEditorState().toJSON()).stringify()
        try discrete(editor, HISTORY_PUSH_TAG) { kept = try edit(editor, op, rnd, step) }
        if JSONValue.object(try editor.getEditorState().toJSON()).stringify() != beforeJSON {
          undoStack.append(try outline(editor))
          redoStack = []
        }
        seen.insert(try outline(editor).joined(separator: "|"))
      }
      let label = "seed \(seed) step \(step) \(op)"
      #expect(try invariantProblems(editor) == [], "\(label)")
      expectKept(before, try rows(editor), kept, label)
    }
  }
}
