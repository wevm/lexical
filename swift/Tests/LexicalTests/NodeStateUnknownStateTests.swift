import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/NodeStateUnknownState.test.ts. Neither state is
// known to ParagraphNode in a fresh editor until it is first read or written,
// so a paragraph made from JSON keeps their values in `unknownState` until
// then.

nonisolated(unsafe) private let idState = createState(
  "nsuId", parse: { $0?.stringValue }, unparse: { $0.map { .string($0) } })
nonisolated(unsafe) private let countState = createState(
  "nsuCount", parse: { Int($0?.numberValue ?? 0) })

private let jsonState = #"{"nsuCount":2,"nsuId":"stored","nsuOther":true}"#

private func appendFromJSON() throws -> LexicalNode {
  let json = try JSONValue.parse(
    #"{"$":\#(jsonState),"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}"#
  )
  let node = try parseSerializedNode(json.objectValue!)
  try getRoot().clear().append(node)
  return node
}

/// The exported `$` state as a dictionary, since `toEqual` ignores key order.
private func exportState(_ editor: Editor) throws -> [String: JSONValue] {
  let state = try editor.getEditorState().read {
    try getRoot().getFirstChildOrThrow().exportJSON()[NODE_STATE_KEY]?.objectValue
  }
  return Dictionary(uniqueKeysWithValues: (state ?? JSONObject()).map { ($0.key, $0.value) })
}

private func makeEditor() throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: [.klass(ParagraphNode.self)], onError: { throw $0 }))
}

private let countAndOther: [String: JSONValue] = ["nsuCount": 2, "nsuOther": true]

@Suite(.serialized) struct NodeStateUnknownStateTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > $setState to the default removes it, in the same update and after"
    ))
  func setToDefaultRemovesIt() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { () throws in
      let node = try appendFromJSON()
      try setState(node, idState, nil as String?)
      #expect(try getState(node, idState) == nil)
    }
    try editor.read { () throws in
      #expect(try getState(try getRoot().getFirstChildOrThrow(), idState) == nil)
    }
    #expect(try exportState(editor) == countAndOther)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > $setState to the default after a read removes it"
    ))
  func setToDefaultAfterReadRemovesIt() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { () throws in
      let node = try appendFromJSON()
      #expect(try getState(node, idState) == "stored")
      try setState(node, idState) { (_: String?) in nil }
      #expect(try getState(node, idState) == nil)
    }
    try editor.read { () throws in
      #expect(try getState(try getRoot().getFirstChildOrThrow(), idState) == nil)
    }
    #expect(try exportState(editor) == countAndOther)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > updateFromUnknown with a known key removes it"
    ))
  func updateFromUnknownWithKnownKey() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { () throws in
      let node = try appendFromJSON()
      // Make the key known to the type after the node was made from JSON.
      try setState(try createParagraphNode(), idState, "other")
      try getWritableNodeState(node).updateFromUnknown("nsuId", nil)
      #expect(try getState(node, idState) == nil)
    }
    #expect(try exportState(editor) == countAndOther)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > a value that is set is the only copy of the key"
    ))
  func setValueIsOnlyCopy() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { () throws in
      let node = try appendFromJSON()
      try setState(node, idState, "new")
      try setState(node, countState, 0)
      let state = try getWritableNodeState(node)
      #expect(state.unknownState == JSONObject([("nsuOther", true)]))
      #expect(state.knownState.get(ObjectIdentifier(idState))?.value as? String == "new")
      #expect(state.knownState.get(ObjectIdentifier(countState))?.value as? Int == 0)
      #expect(try getState(node, idState) == "new")
      #expect(try getState(node, countState) == 0)
    }
    #expect(try exportState(editor) == ["nsuId": "new", "nsuOther": true])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/NodeStateUnknownState.test.ts::a state that is still in unknownState > reading it does not change the JSON"
    ))
  func readingDoesNotChangeJSON() throws {
    let editor = try makeEditor()
    try editor.update(discrete: true) { () throws in
      let node = try appendFromJSON()
      #expect(try getState(node, idState) == "stored")
      #expect(try getState(node, countState) == 2)
    }
    let state = try editor.getEditorState().read {
      try getRoot().getFirstChildOrThrow().exportJSON()[NODE_STATE_KEY]
    }
    #expect(state?.stringify() == jsonState)
  }
}
