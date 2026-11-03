import Lexical
import LexicalExtension
import LexicalRichText
import Testing

// Helpers shared by the ports of packages/lexical-extension/src/__tests__/unit/
// BlockIdExtension*.test.ts, each of which defines its own copies upstream.

/// `counter(prefix)`: a `generateId` that hands out `b1`, `b2`, ….
func counter(_ prefix: String = "b") -> () -> String {
  var n = 0
  return {
    n += 1
    return "\(prefix)\(n)"
  }
}

/// `asEdited(editor)`: the first content an empty editor gets is a load, so
/// its blocks get positional ids. Editors that have had content once give
/// the first blocks they add the ids of `generateId`.
@discardableResult
func asEdited(_ editor: Editor) throws -> Editor {
  try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) {
    try getRoot().append(createParagraphNode())
  }
  try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) { try getRoot().clear() }
  return editor
}

/// `update(editor, fn, tag)`: a discrete update.
func discrete(
  _ editor: Editor, _ tags: UpdateTag..., skipTransforms: Bool = false,
  _ fn: @escaping () throws -> Void
) throws {
  try editor.update(
    fn, EditorUpdateOptions(skipTransforms: skipTransforms, tag: tags, discrete: true))
}

/// `$paragraph(text)`.
func paragraph(_ text: String) throws -> ElementNode {
  try createParagraphNode().append(createTextNode(text))
}

/// `$deepCopy(node)`: a deep copy that keeps NodeState, as a Lexical-JSON
/// paste does.
func deepCopy<T: LexicalNode>(_ node: T) throws -> T {
  func serialize(_ n: LexicalNode) throws -> JSONObject {
    var json = try n.exportJSON()
    if let element = n as? ElementNode {
      json["children"] = .array(try element.getChildren().map { .object(try serialize($0)) })
    }
    return json
  }
  return try #require(try parseSerializedNode(serialize(node)) as? T)
}

/// `$nth(index)`: the root's child at `index`, an element.
func nth(_ index: Int) throws -> ElementNode {
  try #require(try getRoot().getChildAtIndex(index) as? ElementNode, "child \(index) is an element")
}

/// `$caret(index, offset)`: a collapsed selection at `offset` characters
/// into block `index`.
func caret(_ index: Int, _ offset: Int) throws {
  let block = try nth(index)
  if let text = try block.getFirstDescendant() as? TextNode {
    _ = try text.select(offset, offset)
  } else {
    _ = try block.select(0, 0)
  }
}

/// `$selectBlocks(from, to)`: from the start of block `from` to the end of
/// block `to`.
func selectBlocks(_ from: Int, _ to: Int) throws {
  let first = try #require(try nth(from).getFirstDescendant() as? TextNode)
  let last = try #require(try nth(to).getLastDescendant() as? TextNode)
  _ = try first.select(0, 0)
  let range = try #require(try getSelection() as? RangeSelection)
  try range.focus.set(last.getKey(), last.getTextContentSize(), .text)
}

/// `seed(editor, texts)`: the root holds one paragraph per text.
func seed(_ editor: Editor, _ texts: [String]) throws {
  try discrete(editor) {
    _ = try getRoot().clear().append(contentsOf: texts.map { try paragraph($0) })
  }
}

/// `paragraphJSON(text, id)`: a paragraph's JSON, with `"$": {"blockId": id}`
/// when `id` is given.
func paragraphJSON(_ text: String, _ id: JSONValue? = nil) -> JSONObject {
  var json = JSONObject()
  if let id {
    json["$"] = .object(JSONObject([("blockId", id)]))
  }
  json["children"] = [
    .object(
      JSONObject([
        ("detail", 0), ("format", 0), ("mode", "normal"), ("style", ""), ("text", .string(text)),
        ("type", "text"), ("version", 1),
      ]))
  ]
  json["direction"] = .null
  json["format"] = ""
  json["indent"] = 0
  json["textFormat"] = 0
  json["textStyle"] = ""
  json["type"] = "paragraph"
  json["version"] = 1
  return json
}

/// `docJSON(children)`: a serialized editor state holding `children`.
func docJSON(_ children: [JSONObject]) -> String {
  JSONObject([
    (
      "root",
      .object(
        JSONObject([
          ("children", .array(children.map { .object($0) })), ("direction", .null),
          ("format", ""), ("indent", 0), ("type", "root"), ("version", 1),
        ]))
    )
  ]).stringify()
}

/// `P(editor, place)`: the positional id of `place` in the editor's current
/// content.
func positionalId(_ editor: Editor, _ place: String) throws -> String {
  derivePositionalId(try editor.read { try documentKey() }, place)
}

/// `aliases(editor)`: the positional ids of places in the editor's current
/// content as `@place`.
func aliases(_ editor: Editor) throws -> [String: String] {
  let key = try editor.read { try documentKey() }
  var map: [String: String] = [:]
  for i in 0..<32 {
    for place in [String(i)] + (0..<8).map({ "\(i).\($0)" }) {
      map[derivePositionalId(key, place)] = "@\(place)"
      map[derivePositionalId(key, "\(place)#1")] = "@\(place)#1"
    }
  }
  return map
}

/// `blocks(editor)`: `type:text:id` for each child of the root (positional
/// ids aliased; a missing id shows as `null`).
func blocks(_ editor: Editor) throws -> [String] {
  let shown = try aliases(editor)
  return try editor.read {
    try getRoot().getChildren().map { node in
      let id = try getBlockId(node).map { shown[$0] ?? $0 } ?? "null"
      return "\(node.getType()):\(try node.getTextContent()):\(id)"
    }
  }
}

/// `ids(editor)`: the ids of the children of the root (positional ids
/// aliased).
func ids(_ editor: Editor) throws -> [String?] {
  let shown = try aliases(editor)
  return try editor.read {
    try getRoot().getChildren().map { try getBlockId($0).map { shown[$0] ?? $0 } }
  }
}

/// The ids of the children of the root, unaliased.
func rawIds(_ editor: Editor) throws -> [String?] {
  try editor.read { try getRoot().getChildren().map { try getBlockId($0) } }
}

/// The output of the editor's `BlockIdExtension`.
func blockIdOutput(_ editor: Editor) throws -> BlockIdOutput {
  try getExtensionDependencyFromEditor(editor, BlockIdExtension).output
}

/// `expectAllIndexed(editor)`: every identified block is found by
/// `getBlockById` under its own id.
func expectAllIndexed(
  _ editor: Editor, sourceLocation: SourceLocation = #_sourceLocation
) throws {
  let isIdentifiedBlock = try blockIdOutput(editor).isIdentifiedBlock
  try editor.read {
    func walk(_ node: LexicalNode) throws {
      if try isIdentifiedBlock(node) {
        let id = try getBlockId(node)
        #expect(id != nil, "\(node.getType()) has an id", sourceLocation: sourceLocation)
        if let id {
          #expect(
            try getBlockById(id) === node.getLatest(), "getBlockById(\(id))",
            sourceLocation: sourceLocation)
        }
      }
      if let element = node as? ElementNode {
        try element.getChildren().forEach(walk)
      }
    }
    try walk(getRoot())
  }
}

/// `expectUnique(editor)`: every child of the root has an id of its own.
func expectUnique(_ editor: Editor, sourceLocation: SourceLocation = #_sourceLocation) throws {
  let all = try ids(editor)
  #expect(all.allSatisfy { $0 != nil }, sourceLocation: sourceLocation)
  #expect(Set(all.compactMap { $0 }).count == all.count, sourceLocation: sourceLocation)
}

/// Lowercase hex of `bytes`.
func hex(_ bytes: [UInt8]) -> String {
  bytes.map {
    String($0, radix: 16).count == 1 ? "0" + String($0, radix: 16) : String($0, radix: 16)
  }
  .joined()
}

/// `mulberry32(seed)`, the tests' seeded generator.
func mulberry32(_ seed: UInt32) -> () -> Double {
  var a = seed
  return {
    a = a &+ 0x6d2b_79f5
    var t = a
    t = (t ^ (t >> 15)) &* (t | 1)
    t ^= t &+ ((t ^ (t >> 7)) &* (t | 61))
    return Double(t ^ (t >> 14)) / 4_294_967_296
  }
}
