import Foundation
import Lexical
import LexicalMarkdown
import LexicalTesting
import LexicalText
import Testing

// Port of
// packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts.
//
// registerLexicalTextEntity records what its transforms do with the
// characters they touch, around the calls it made before. This compares it
// with a copy of the helper as it was before that change, in two editors
// with a root element driven by the same random edits next to two kinds of
// entities: after every edit, the same JSON, the same selection, the same
// mutation-listener events and the same node keys. The Swift port does not
// record text edits (`_recordTextEdits`, for collaboration), so only the
// halves without recording are ported.

/// registerLexicalTextEntity as it was before it recorded its edits.
private func registerLexicalTextEntityBefore<T: TextNode>(
  _ editor: Editor, _ getMatch: @escaping (String) -> EntityMatch?, _ targetNode: T.Type,
  _ createNode: @escaping (TextNode) throws -> T
) throws -> [() -> Void] {
  func isTargetNode(_ node: LexicalNode?) -> Bool {
    node is T
  }

  func replaceWithSimpleText(_ node: TextNode) throws {
    let textNode = try createTextNode(node.getTextContent())
      .setFormat(node.getFormat())
      .setStyle(node.getStyle())
      .setDetail(node.getDetail())
    _ = try node.replace(textNode)
  }

  func getMode(_ node: TextNode) throws -> Int {
    try node.getLatest().__mode
  }

  func textNodeTransform(_ node: TextNode) throws {
    if try !node.isSimpleText() {
      return
    }

    var prevSibling = try node.getPreviousSibling()
    var text = try node.getTextContent()
    var currentNode: TextNode? = node

    if let prev = prevSibling as? TextNode {
      let previousText = try prev.getTextContent()
      let previousLength = previousText.utf16.count
      let combinedText = previousText + text
      let prevMatch = getMatch(combinedText)

      if isTargetNode(prev) {
        guard let prevMatch, try getMode(prev) == 0 else {
          try replaceWithSimpleText(prev)
          return
        }
        let diff = prevMatch.end - previousLength
        if diff > 0 {
          let concatText = text.jsSlice(0, diff)
          let newTextContent = previousText + concatText
          _ = try prev.select()
          _ = try prev.setTextContent(newTextContent)
          if diff == text.utf16.count {
            try node.remove()
          } else {
            let remainingText = text.jsSlice(diff)
            _ = try node.setTextContent(remainingText)
          }
          return
        }
      } else if prevMatch == nil || prevMatch!.start < previousLength {
        return
      }
    }

    var prevMatchLengthToSkip = 0

    while true {
      let remainingText = text
      let match = getMatch(remainingText)
      let nextText = match.map { remainingText.jsSlice($0.end) } ?? ""
      text = nextText

      if nextText == "" {
        if let nextSibling = try currentNode!.getNextSibling() as? TextNode {
          let combinedText = remainingText + (try nextSibling.getTextContent())
          let nextMatch = getMatch(combinedText)
          if nextMatch == nil {
            if isTargetNode(nextSibling) {
              try replaceWithSimpleText(nextSibling)
            } else {
              try nextSibling.markDirty()
            }
            return
          } else if match == nil || nextMatch!.start != match!.start {
            return
          }
        }
      }

      guard let match else {
        return
      }

      if match.start == 0, let prev = prevSibling as? TextNode, prev.isTextEntity() {
        prevMatchLengthToSkip += match.end
        continue
      }

      var nodeToReplace: TextNode?
      if match.start == 0 {
        let parts = try currentNode!.splitText(match.end)
        nodeToReplace = parts.first
        currentNode = parts.count > 1 ? parts[1] : nil
      } else {
        let parts = try currentNode!.splitText(
          match.start + prevMatchLengthToSkip, match.end + prevMatchLengthToSkip)
        nodeToReplace = parts.count > 1 ? parts[1] : nil
        currentNode = parts.count > 2 ? parts[2] : nil
      }

      guard let nodeToReplace else {
        throw LexicalError("nodeToReplace should not be undefined")
      }

      let replacementNode = try createNode(nodeToReplace)
      _ =
        try replacementNode
        .setFormat(nodeToReplace.getFormat())
        .setStyle(nodeToReplace.getStyle())
        .setDetail(nodeToReplace.getDetail())
      _ = try nodeToReplace.replace(replacementNode)

      if currentNode == nil {
        return
      }
      prevMatchLengthToSkip = 0
      prevSibling = replacementNode
    }
  }

  func reverseNodeTransform(_ node: T) throws {
    let text = try node.getTextContent()
    guard let match = getMatch(text), match.start == 0 else {
      try replaceWithSimpleText(node)
      return
    }

    if text.utf16.count > match.end {
      _ = try node.splitText(match.end)
      return
    }

    if let prevSibling = try node.getPreviousSibling() as? TextNode, prevSibling.isTextEntity() {
      try replaceWithSimpleText(prevSibling)
      try replaceWithSimpleText(node)
    }

    if let nextSibling = try node.getNextSibling() as? TextNode, nextSibling.isTextEntity() {
      try replaceWithSimpleText(nextSibling)
      if isTargetNode(node) {
        try replaceWithSimpleText(node)
      }
    }
  }

  let removePlainTextTransform = try editor.registerNodeTransform(
    TextNode.self, textNodeTransform)
  let removeReverseNodeTransform = try editor.registerNodeTransform(
    targetNode, reverseNodeTransform)
  return [removePlainTextTransform, removeReverseNodeTransform]
}

/// A hashtag-like entity whose setter refuses to clear it.
private final class TagNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TagNode.self, type: "equivalence-tag", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
  @discardableResult
  override func setTextContent(_ text: String) throws -> Self {
    if try text == "" && getTextContent() != "" {
      throw LexicalError("a tag is never cleared")
    }
    return try super.setTextContent(text)
  }
}

/// A plain entity (the review's repros).
private final class EntityNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(EntityNode.self, type: "equivalence-entity", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

/// A keyword entity that text is not typed into from either end.
private final class KeywordNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(KeywordNode.self, type: "equivalence-keyword", extends: TextNode.self)
  }
  override func canInsertTextBefore() throws -> Bool { false }
  override func canInsertTextAfter() throws -> Bool { false }
  override func isTextEntity() -> Bool { true }
}

private let tagMatch = regexMatch("#[a-z0-9]+")
private let keywordRegex = JSRegExp(#"(^|\s)(la)(?=\s|$)"#)
private func keywordMatch(_ text: String) -> EntityMatch? {
  guard let m = keywordRegex.exec(text) else {
    return nil
  }
  let start = m.index + (m[1]?.utf16.count ?? 0)
  return EntityMatch(start: start, end: start + 2)
}

/// The helper a session registers: the one before, or the current one.
private enum Register {
  case before
  case current

  func callAsFunction<T: TextNode>(
    _ editor: Editor, _ getMatch: @escaping (String) -> EntityMatch?, _ targetNode: T.Type,
    _ createNode: @escaping (TextNode) throws -> T
  ) throws -> [() -> Void] {
    switch self {
    case .before:
      try registerLexicalTextEntityBefore(editor, getMatch, targetNode, createNode)
    case .current:
      try registerLexicalTextEntity(editor, getMatch, targetNode, createNode)
    }
  }
}

private final class Events {
  var items: [String] = []
}

private func makeEditor(_ register: Register) throws -> (Editor, Events) {
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(TagNode.self), .klass(KeywordNode.self)], onError: { throw $0 }))
  _ = try register(editor, tagMatch, TagNode.self) {
    try create(TagNode.self).setTextContent($0.getTextContent())
  }
  _ = try register(editor, keywordMatch, KeywordNode.self) {
    try create(KeywordNode.self).setTextContent($0.getTextContent())
  }
  let events = Events()
  for klass: TextNode.Type in [TextNode.self, TagNode.self, KeywordNode.self] {
    try editor.registerMutationListener(
      klass,
      { mutations, _ in
        let entries = mutations.map { "\($0.0),\($0.1)" }.sorted()
        events.items.append("\(klass.getType()) \(entries)")
      }, skipInitialization: true)
  }
  try editor.setRootElement(TestRootElement())
  return (editor, events)
}

/// The xorshift32 generator of the upstream test.
private func rng(_ seed: UInt32) -> () -> Double {
  var s = seed == 0 ? 1 : seed
  return {
    s ^= s << 13
    s ^= s >> 17
    s ^= s << 5
    return Double(s) / 4_294_967_296
  }
}

private func texts() throws -> [TextNode] {
  let line = try getRoot().getFirstChildOrThrow()
  guard let line = line as? ElementNode else {
    return []
  }
  return try line.getChildren().compactMap { $0 as? TextNode }
}

private func pointAt(_ at: Int, _ left: Bool) throws -> (TextNode, Int)? {
  var p = 0
  var found: (TextNode, Int)?
  for t in try texts() {
    let n = try t.getTextContentSize()
    if at >= p && at <= p + n {
      found = (t, at - p)
      if left || at < p + n {
        return found
      }
    }
    p += n
  }
  return found
}

private func selectionSummary() throws -> [String]? {
  let s = try getSelection()
  if let s = s as? RangeSelection {
    return [
      "range", s.anchor.key, "\(s.anchor.offset)", s.anchor.type.rawValue, s.focus.key,
      "\(s.focus.offset)", s.focus.type.rawValue, "\(s.format)", s.style,
    ]
  }
  if let s = s as? NodeSelection {
    return try ["node"] + s.getNodes().map { $0.getKey() }
  }
  return nil
}

private let PIECES = ["#ab", " ", "la", "la ", "cd", "#", "x y", "ab", " #c", "a "]
private let TYPED = ["a", " ", "#", "la", "b ", "#ab", "x", "l", "a ", "1", "cd"]
private let KINDS = ["type", "type", "backspace", "delete", "over", "set", "remove"]

private struct Op: CustomStringConvertible {
  var at: Int
  /// At the end of text node `node` instead of at `at`.
  var edge: Bool
  var kind: String
  var left: Bool
  var len: Int
  var node: Int
  var s: String

  var description: String {
    "{at: \(at), edge: \(edge), kind: \(kind), left: \(left), len: \(len), node: \(node), s: \(s)}"
  }
}

/// One edit, as a caret edit (or app code setting or removing a node).
private func apply(_ op: Op) throws {
  let textLen = try getRoot().getTextContent().utf16.count
  if op.kind == "set" || op.kind == "remove" {
    let texts = try texts()
    let index = op.node % max(1, texts.count)
    guard index < texts.count else {
      return
    }
    let node = texts[index]
    if op.kind == "remove" {
      try node.remove()
    } else {
      let t = try node.getTextContent()
      let at = op.at % (t.utf16.count + 1)
      let next = t.jsSlice(0, at) + op.s + t.jsSlice(at + op.len)
      if next != "" {
        _ = try node.setTextContent(next)
      }
    }
    return
  }
  var at = op.at
  if op.edge {
    let texts = try texts()
    at = 0
    // `texts.slice(0, NaN)` is empty when there are no text nodes
    if !texts.isEmpty {
      for t in texts.prefix((op.node % texts.count) + 1) {
        at += try t.getTextContentSize()
      }
    }
  }
  var from = at
  var to = at
  if op.kind == "backspace" {
    from = at - 1
  } else if op.kind == "delete" {
    to = at + 1
  } else if op.kind == "over" {
    to = min(textLen, at + op.len)
  }
  if from < 0 || to > textLen {
    return
  }
  let a = try pointAt(from, from == to ? op.left : true)
  let b = from == to ? a : try pointAt(to, false)
  guard let a, let b else {
    return
  }
  let sel = createRangeSelection()
  try sel.anchor.set(a.0.getKey(), a.1, .text)
  try sel.focus.set(b.0.getKey(), b.1, .text)
  try setSelection(sel)
  if op.kind == "type" || op.kind == "over" {
    try sel.insertText(op.s)
  } else if from != to {
    try sel.removeText()
  }
}

private struct Step: Equatable {
  var events: [String]
  var json: JSONObject
  var result: String
  var selection: [String]?
}

/// A session: its initial pieces, then its ops, each step after an op.
private func session(_ register: Register, _ pieces: [String], _ ops: [Op]) throws -> [Step] {
  resetRandomKey()
  let (editor, events) = try makeEditor(register)
  try editor.update(discrete: true) {
    try getRoot().append(
      createParagraphNode().append(contentsOf: pieces.map { try createTextNode($0) }))
  }
  var steps: [Step] = []
  for op in ops {
    events.items = []
    var result = "ok"
    do {
      try editor.update(discrete: true) { try apply(op) }
    } catch {
      result = String(describing: error)
    }
    steps.append(
      Step(
        events: events.items, json: try editor.getEditorState().toJSON(), result: result,
        selection: try editor.read { try selectionSummary() }))
  }
  try editor.setRootElement(nil)
  return steps
}

private let SESSIONS =
  ProcessInfo.processInfo.environment["TEXT_ENTITY_EQUIVALENCE_SESSIONS"].flatMap { Int($0) }
  ?? 300

enum EquivalenceEdgeCase: String, CaseIterable {
  case range = "range"
  case nodeSelection = "node selection"
  case strictSetter = "strict setter"
  case shortened = "shortened"
}

private final class Errors {
  var messages: [String] = []
}

private struct EdgeResult: Equatable {
  var errors: [String]
  var json: JSONObject
  var selection: [String]?
}

private func run(_ register: Register, _ kind: EquivalenceEdgeCase) throws -> EdgeResult {
  resetRandomKey()
  let errors = Errors()
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(TagNode.self), .klass(EntityNode.self)],
      onError: { errors.messages.append(String(describing: $0)) }))
  func entity(_ text: String) throws -> TextNode {
    kind == .strictSetter
      ? try create(TagNode.self).setTextContent(text)
      : try create(EntityNode.self).setTextContent(text)
  }
  if kind == .strictSetter {
    _ = try register(editor, tagMatch, TagNode.self) {
      try create(TagNode.self).setTextContent($0.getTextContent())
    }
  } else {
    _ = try register(editor, tagMatch, EntityNode.self) {
      try create(EntityNode.self).setTextContent($0.getTextContent())
    }
  }
  try editor.setRootElement(TestRootElement())
  if kind == .shortened {
    try editor.update(discrete: true) {
      let node = try entity("#abc")
      try getRoot().append(createParagraphNode().append(node))
      _ = try node.select(4, 4)
    }
    try editor.update(discrete: true) {
      _ = try (getRoot().getFirstDescendant() as! TextNode).setTextContent("xx")
    }
  } else {
    try editor.update(discrete: true, skipTransforms: true) {
      let text = try createTextNode("#abc")
      try getRoot().append(createParagraphNode().append(text))
      if kind == .nodeSelection {
        let sel = createNodeSelection()
        sel.add(text.getKey())
        try setSelection(sel)
      } else {
        let sel = try text.select(2, 3)
        sel.setFormat(1)
        sel.setStyle("color: red")
      }
    }
    try editor.update(discrete: true) {
      try (getRoot().getFirstDescendant() as! TextNode).markDirty()
    }
  }
  let result = EdgeResult(
    errors: errors.messages, json: try editor.getEditorState().toJSON(),
    selection: try editor.read { try selectionSummary() })
  try editor.setRootElement(nil)
  return result
}

@Suite struct RegisterLexicalTextEntityEquivalenceTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::registerLexicalTextEntity against the helper before it recorded its edits > 300 random sessions of 8 edits next to entities, not recording text edits: the same JSON, selection, mutations and keys"
    ))
  func randomSessions() throws {
    for seed in 1...SESSIONS {
      let r = rng(UInt32(seed * 7919 + 13))
      func pick(_ xs: [String]) -> String {
        xs[Int((r() * Double(xs.count)).rounded(.down))]
      }
      let pieceCount = 2 + Int((r() * 4).rounded(.down))
      let pieces = (0..<pieceCount).map { _ in pick(PIECES) }
      let ops = (0..<8).map { _ in
        let at = Int((r() * 20).rounded(.down))
        let edge = r() < 0.5
        let kind = pick(KINDS)
        let left = r() < 0.5
        let len = Int((r() * 3).rounded(.down))
        let node = Int((r() * 6).rounded(.down))
        let s = pick(TYPED)
        return Op(at: at, edge: edge, kind: kind, left: left, len: len, node: node, s: s)
      }
      let before = try session(.before, pieces, ops)
      let after = try session(.current, pieces, ops)
      for k in 0..<ops.count {
        #expect(after[k] == before[k], "seed \(seed), edit \(k) \(ops[k])")
      }
    }
  }

  // Merging into a fresh node instead of replace() would leave a stale caret
  // inside the entity unclamped, run a setter that refuses empty text, and
  // turn a NodeSelection into a range: the helper makes the same calls.
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > range, not recording"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > node selection, not recording"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > strict setter, not recording"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > shortened, not recording"
    ),
    arguments: EquivalenceEdgeCase.allCases)
  func edgeCase(_ kind: EquivalenceEdgeCase) throws {
    let before = try run(.before, kind)
    #expect(try run(.current, kind) == before)
    #expect(before.errors == [])
  }
}
