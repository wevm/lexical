import LexicalTesting
import Testing

@testable import Lexical

// Port of the `set-valued state (arraySetValue)` suite of
// LexicalNodeState.test.ts. JS freezes the canonical array; Swift values are
// immutable, so the frozen checks have nothing to assert and only the
// canonical form is checked.

nonisolated(unsafe) private let tagsState = createState(
  "tags", schema: try! arraySetValue(stringValue()))

private func paragraphWithTags(_ tags: JSONValue) throws -> ParagraphNode {
  let p = try setState(createParagraphNode(), tagsState, tags)
  try getRoot().clear().append(p)
  return p
}

private func createSetEditor(_ nodes: [LexicalNode.Type] = []) throws -> Editor {
  try createEditor(
    CreateEditorArgs(namespace: "", nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

private final class TagsParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TagsParagraphNode.self, type: "tags-paragraph", extends: ParagraphNode.self,
      stateConfigs: [RequiredNodeStateConfig(tagsState)])
  }
}

private final class TagsFieldNode: ElementNode {
  var __tags: [String] = []
}

private final class DirtyKeys: @unchecked Sendable {
  var keys: [NodeKey] = []
}

@Suite struct LexicalNodeStateSetTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the value is a readonly array, and the schema says it is a set"
    ))
  func schemaSaysSet() throws {
    #expect(tagsState.defaultValue == json("[]"))
    let schema = try #require(tagsState.schema)
    guard case .set(let item) = schema.meta else {
      Issue.record("not a set schema")
      return
    }
    #expect(schema.meta.kind == "set")
    #expect(item.meta.kind == "string")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > parsing drops duplicates and rejected entries, sorts and freezes"
    ))
  func parsingCanonicalizes() throws {
    let parsed = tagsState.parse(json(#"["b","a","b",1,null,"c"]"#))
    #expect(parsed == json(#"["a","b","c"]"#))
    // By UTF-16 code unit, as JS sorts.
    #expect(tagsState.parse(json(#"["b","B","a","é","e"]"#)) == json(#"["B","a","b","e","é"]"#))
    for notAnArray: JSONValue? in ["a", nil, .null, 1, json(#"{"a":true}"#)] {
      #expect(tagsState.parse(notAnArray) == json("[]"))
    }
    #expect(tagsState.parse(parsed) == parsed)
    let sizeState = createState("size", schema: try arraySetValue(enumValue(["s", "m", "l"])))
    #expect(sizeState.parse(json(#"["xl","l","s","l"]"#)) == json(#"["l","s"]"#))
    #expect(sizeState.parse(json(#"["l","xl"]"#)) == json(#"["l"]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the value is frozen, so mutating it in place throws"
    ))
  func storedValueUnaffectedByCopies() throws {
    let editor = try createSetEditor()
    try editor.update(discrete: true) { () throws in
      let p = try paragraphWithTags(json(#"["b","a"]"#))
      var stored = try getState(p, tagsState)?.arrayValue ?? []
      stored.append("c")
      #expect(try getState(p, tagsState) == json(#"["a","b"]"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > $setState stores unsorted or duplicated input in the canonical form"
    ))
  func setStateCanonicalizes() throws {
    let editor = try createSetEditor()
    var key = ""
    try editor.update(discrete: true) { () throws in
      let p = try paragraphWithTags(json(#"["b","a","a"]"#))
      key = p.getKey()
      #expect(try getState(p, tagsState) == json(#"["a","b"]"#))
      try setState(p, tagsState) { prev in
        .array((prev?.arrayValue ?? []) + ["c", "a", "c"])
      }
      #expect(try getState(p, tagsState) == json(#"["a","b","c"]"#))
      #expect(try p.exportJSON()[NODE_STATE_KEY] == json(#"{"tags":["a","b","c"]}"#))
    }
    try editor.read { () throws in
      #expect(try getState(getNodeByKeyOrThrow(key), tagsState) == json(#"["a","b","c"]"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > equality is element by element on the canonical form"
    ))
  func equalityElementwise() {
    let ab = tagsState.parse(json(#"["a","b"]"#))
    #expect(tagsState.isEqual(ab, tagsState.parse(json(#"["b","a","b"]"#))))
    #expect(tagsState.isEqual(ab, json(#"["a","b"]"#)))
    #expect(tagsState.isEqual(ab, tagsState.parse(json(#"["a"]"#))) == false)
    #expect(tagsState.isEqual(ab, tagsState.parse(json(#"["a","c"]"#))) == false)
    #expect(tagsState.isEqual(ab, "a,b") == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the JSON is one sorted array, and the empty set is omitted"
    ))
  func jsonSortedEmptyOmitted() throws {
    let editor = try createSetEditor()
    try editor.update(discrete: true) { () throws in
      let p = try paragraphWithTags(json(#"["c","a","b"]"#))
      #expect(try p.exportJSON()[NODE_STATE_KEY] == json(#"{"tags":["a","b","c"]}"#))
      try setState(p, tagsState, json("[]"))
      #expect(try p.exportJSON().has(NODE_STATE_KEY) == false)
      try setState(p, tagsState, json(#"["a","a"]"#))
      #expect(try p.exportJSON()[NODE_STATE_KEY] == json(#"{"tags":["a"]}"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > setting the same members in another order does not mark the node dirty"
    ))
  func reorderNotDirty() throws {
    let editor = try createSetEditor()
    var key = ""
    try editor.update(discrete: true) { key = try paragraphWithTags(json(#"["a","b"]"#)).getKey() }
    let dirty = DirtyKeys()
    _ = editor.registerUpdateListener { payload in
      for (k, _) in payload.dirtyElements { dirty.keys.append(k) }
      for k in payload.dirtyLeaves { dirty.keys.append(k) }
    }
    try editor.update(discrete: true) {
      let p = try getRoot().getFirstChildOrThrow()
      try setState(p, tagsState) { _ in json(#"["b","a","b"]"#) }
      try setState(p, tagsState) { $0 }
    }
    #expect(dirty.keys.contains(key) == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the JSON is canonical and stable across a round trip"
    ))
  func canonicalRoundTrip() throws {
    let editor = try createSetEditor([TagsParagraphNode.self])
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        setState(create(TagsParagraphNode.self), tagsState, json(#"["b","a"]"#)))
    }
    let serialized = try editor.getEditorState().toJSON().stringify()
    #expect(serialized.contains(#""$":{"tags":["a","b"]}"#))
    let unordered = serialized.replacingFirst(#"["a","b"]"#, with: #"["b","a","b",7]"#)
    for input in [serialized, unordered] {
      let loaded = try createSetEditor([TagsParagraphNode.self])
      try loaded.setEditorState(loaded.parseEditorState(input))
      #expect(try loaded.getEditorState().toJSON().stringify() == serialized)
      try loaded.read { () throws in
        #expect(
          try getState(getRoot().getFirstChildOrThrow(), tagsState) == json(#"["a","b"]"#))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an unparse of its own still wins"
    ))
  func ownUnparse() throws {
    let csvState = createState(
      "csv", schema: try arraySetValue(stringValue()),
      unparse: { .string(($0?.arrayValue ?? []).compactMap(\.stringValue).joined(separator: ",")) })
    #expect(csvState.unparse(csvState.parse(json(#"["b","a"]"#))) == "a,b")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an arraySetValue can only be the whole value of a createState"
    ))
  func setOnlyWholeValue() throws {
    let item = stringValue()
    let builds: [() throws -> Any] = [
      { try arrayValue(arraySetValue(item)) },
      { try nullable(arraySetValue(item)) },
      { try arraySetValue(arraySetValue(item)) },
      { try objectValue([("tags", arraySetValue(item))]) },
      { try SchemaField("tags", arraySetValue(item), field: \TagsFieldNode.__tags) },
      {
        try SchemaField(
          "tags", arraySetValue(item), of: ParagraphNode.self, get: nil, set: nil)
      },
      { try optional(arraySetValue(item)) },
      { try unionValue([arraySetValue(item), stringValue()]) },
      { try aliasedValue(arraySetValue(item), [:]) },
      { try transformValue(arraySetValue(item)) { $0 } },
    ]
    for build in builds {
      #expect {
        _ = try build()
      } throws: { "\($0)".contains("arraySetValue can only be the whole value") }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > the item schema must produce strings"
    ))
  func itemMustProduceStrings() {
    #expect {
      try arraySetValue(numberValue())
    } throws: { "\($0)".contains("the item schema must produce strings") }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > an item schema that declines some strings drops them"
    ))
  func itemDeclines() throws {
    // `/^\w+$/`.
    @Sendable func isWord(_ s: String) -> Bool {
      !s.isEmpty
        && s.utf16.allSatisfy { c in
          (c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x5A) || (c >= 0x61 && c <= 0x7A)
            || c == 0x5F
        }
    }
    let word = SerializationSchema(
      meta: .string, defaultValue: "",
      accepts: { $0?.stringValue.map(isWord) ?? false },
      parse: { $0?.stringValue.map(JSONValue.string) ?? "" })
    let parse = try arraySetValue(word)
    #expect(parse(json(#"["b","a b","","a",7]"#)) == json(#"["a","b"]"#))
    #expect(parse(json(#"["a","b c"]"#)) == json(#"["a"]"#))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > a frozen value that is not canonical is still canonicalized"
    ))
  func nonCanonicalCanonicalized() throws {
    for (input, expected) in [
      (#"["a","a"]"#, #"["a"]"#), (#"["b","a"]"#, #"["a","b"]"#), (#"["a",1]"#, #"["a"]"#),
      ("[]", "[]"),
    ] {
      #expect(tagsState.parse(json(input)) == json(expected))
    }
    let editor = try createSetEditor()
    try editor.update(discrete: true) { () throws in
      let p = try paragraphWithTags(json("[]"))
      try setState(p, tagsState, json(#"["b","a","a"]"#))
      #expect(try getState(p, tagsState) == json(#"["a","b"]"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::set-valued state (arraySetValue) > a state that is not declared is canonical when it is read after loading JSON"
    ))
  func undeclaredCanonicalAfterLoad() throws {
    let editor = try createSetEditor()
    try editor.update(discrete: true) { _ = try paragraphWithTags(json(#"["a"]"#)) }
    let serialized = try editor.getEditorState().toJSON().stringify().replacingFirst(
      #"["a"]"#, with: #"["b","a","b"]"#)
    let loaded = try createSetEditor()
    try loaded.setEditorState(loaded.parseEditorState(serialized))
    try loaded.read { () throws in
      #expect(try getState(getRoot().getFirstChildOrThrow(), tagsState) == json(#"["a","b"]"#))
    }
  }
}

extension String {
  /// JS `String.prototype.replace` with a string pattern: the first
  /// occurrence only.
  func replacingFirst(_ target: String, with replacement: String) -> String {
    let chars = Array(self)
    let pattern = Array(target)
    guard pattern.count <= chars.count else { return self }
    for i in 0...(chars.count - pattern.count) where Array(chars[i..<i + pattern.count]) == pattern
    {
      return String(chars[..<i]) + replacement + String(chars[(i + pattern.count)...])
    }
    return self
  }
}
