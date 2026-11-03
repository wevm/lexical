import LexicalTesting
import Testing

@testable import Lexical

// Port of the `LexicalNode state`, `$config interleaved abstract/concrete
// classes`, `flat NodeState through a node replacement` and `importJSON
// applies state to the node it just built` suites of LexicalNodeState.test.ts.
// JSON number states are `Double`.

/// JS truthiness, for `parse: Boolean`.
func jsTruthy(_ value: JSONValue?) -> Bool {
  switch value {
  case nil, .null?: false
  case .bool(let b)?: b
  case .number(let n)?: n != 0 && !n.isNaN
  case .string(let s)?: !s.isEmpty
  default: true
  }
}

/// The entries of `object`, compared without regard to key order, as
/// `toEqual` compares.
func unordered(_ object: JSONObject) -> [String: JSONValue] {
  Dictionary(uniqueKeysWithValues: object.map { ($0.key, $0.value) })
}

nonisolated(unsafe) private let numberState = createState(
  "numberState", parse: { $0?.numberValue ?? 0 })
nonisolated(unsafe) private let boolState = createState("boolState", parse: jsTruthy)

private class StateNode: TestNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      StateNode.self, type: "state", extends: TestNode.self,
      stateConfigs: [
        RequiredNodeStateConfig(numberState, flat: true), RequiredNodeStateConfig(boolState),
      ])
  }
  func getNumber() throws -> Double { try getState(self, numberState) }
  @discardableResult
  func setNumber(_ value: Double) throws -> Self { try setState(self, numberState, value) }
  @discardableResult
  func setNumber(_ updater: (Double) throws -> Double) throws -> Self {
    try setState(self, numberState, updater)
  }
}

private func createStateNode() throws -> StateNode { try StateNode() }

private func stateEditor() throws -> Editor {
  try createTestEditor(nodes: [.klass(TestNode.self), .klass(StateNode.self)])
}

@Suite struct LexicalNodeStateTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > state.$set() and state.$get() need to be inside an update"
    ))
  func needsUpdate() throws {
    let editor = try stateEditor()
    var root: RootNode!
    try editor.update(discrete: true) { root = try getRoot() }
    let stringState = createState("stringState", parse: { $0?.stringValue ?? "" })
    #expect(throws: LexicalError.self) { try setState(root, stringState, "hello") }
    try editor.update(discrete: true) {
      try setState(root, stringState, "hello")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > getState and setState"
    ))
  func getAndSet() throws {
    let editor = try stateEditor()
    let stringState = createState("stringState", parse: { $0?.stringValue ?? "" })
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      #expect(try getState(root, stringState) == "")
      try setState(root, stringState, "hello")
      #expect(try getState(root, stringState) == "hello")
      let maybeStringState = createState("maybeStringState", parse: { $0?.stringValue })
      #expect(try getState(root, maybeStringState) == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > import and export state"
    ))
  func importExport() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode()
      #expect(try paragraph.exportJSON().has(NODE_STATE_KEY) == false)
      try setState(paragraph, numberState, 1)
      let json2 = try paragraph.exportJSON()
      #expect(json2[NODE_STATE_KEY] == json(#"{"numberState":1}"#))
      let paragraph2 = try ParagraphNode.importJSON(json2)
      #expect(try paragraph2.exportJSON()[NODE_STATE_KEY] == json(#"{"numberState":1}"#))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > importJSON ignores prototype-polluting state keys"
    ))
  func prototypePollution() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try ParagraphNode.importJSON(
        jsonObject(#"{"type":"paragraph","version":1,"$":{"__proto__":{"injectedKey":"evil"}}}"#))
      let injectedState = createState("injectedKey", parse: { $0?.stringValue ?? "default" })
      #expect(try getState(paragraph, injectedState) == "default")
      #expect(try paragraph.exportJSON().has(NODE_STATE_KEY) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > states cannot be registered with the same key string"
    ))
  func keyCollision() throws {
    let editor = try stateEditor()
    let message =
      #"State key collision "foo" detected in RootNode node with type root and key root. Only one StateConfig with a given key should be used on a node."#
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let k0 = createState("foo", parse: jsTruthy)
      let k1 = createState("foo", parse: { _ in "foo" })
      #expect(try getState(root, k0) == false)
      try setState(root, k0, true)
      #expect(try getState(root, k0) == true)
      #expect { _ = try getState(root, k1) } throws: { "\($0)".contains(message) }
      #expect { try setState(root, k1, "foo") } throws: { "\($0)".contains(message) }
      #expect(try getState(root, k0) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeGetter() and nodeSetter()"
    ))
  func nodeGetterSetter() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let stateNode = try createStateNode()
      #expect(try stateNode.getNumber() == 0)
      try stateNode.setNumber(1)
      #expect(try stateNode.getNumber() == 1)
      try stateNode.setNumber { $0 + 1 }
      #expect(try stateNode.getNumber() == 2)
      try setState(stateNode, boolState, true)
      let exported = try stateNode.exportJSON()
      #expect(exported[NODE_STATE_KEY] == json(#"{"boolState":true}"#))
      #expect(exported["numberState"] == 2)
      #expect(exported["type"] == "state")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > flat config serialization round-trip"
    ))
  func flatRoundTrip() throws {
    let editor = try stateEditor()
    let flat = jsonObject(#"{"$":{"boolState":true},"numberState":2,"type":"state","version":1}"#)
    let nested = jsonObject(#"{"$":{"boolState":true,"numberState":2},"type":"state","version":1}"#)
    let both = jsonObject(
      #"{"$":{"boolState":true,"numberState":3},"numberState":2,"type":"state","version":1}"#)
    try editor.update(discrete: true) { () throws in
      for doc in [flat, nested, both] {
        let node = try StateNode.importJSON(doc)
        #expect(unordered(try node.exportJSON()) == unordered(flat))
        #expect(try getState(node, boolState) == true)
        #expect(try getState(node, numberState) == 2)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > an absent flat key leaves the nested value alone"
    ))
  func absentFlatKey() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let node = try StateNode.importJSON(
        jsonObject(#"{"$":{"numberState":7},"type":"state","version":1}"#))
      #expect(try getState(node, numberState) == 7)
      let both = try StateNode.importJSON(
        jsonObject(#"{"$":{"numberState":7},"numberState":9,"type":"state","version":1}"#))
      #expect(try getState(both, numberState) == 9)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > default value should not be exported"
    ))
  func defaultNotExported() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let indentState = createState("indent", parse: { $0?.numberValue ?? 0 })
      let paragraph = try createParagraphNode()
      #expect(try getState(paragraph, indentState) == 0)
      #expect(try paragraph.exportJSON().has(NODE_STATE_KEY) == false)
      try setState(paragraph, indentState, 1)
      #expect(try paragraph.exportJSON()[NODE_STATE_KEY] == json(#"{"indent":1}"#))
      try setState(paragraph, indentState, 0)
      #expect(try paragraph.exportJSON().has(NODE_STATE_KEY) == false)
      let foo = createState("foo", parse: { $0?.stringValue ?? "" })
      try setState(paragraph, foo, "fooValue")
      #expect(try paragraph.exportJSON()[NODE_STATE_KEY] == json(#"{"foo":"fooValue"}"#))
    }
  }

  // The JS half of this test is type-level; the runtime half sets object
  // values, which Swift holds as JSON.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > getState returns immutable values, setState require an Object literal"
    ))
  func objectValues() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode()
      let objectState = createState(
        "object", parse: { $0 ?? json(#"{"bar":0,"foo":""}"#) })
      let paragraphObject = try getState(paragraph, objectState)
      #expect(paragraphObject == json(#"{"bar":0,"foo":""}"#))
      try setState(paragraph, objectState, json(#"{"bar":1,"baz":"baz","foo":"foo"}"#))
      try setState(paragraph, objectState, paragraphObject)
      #expect(try getState(paragraph, objectState) == paragraphObject)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > setting state shouldn’t affect previous reconciled versions of the node"
    ))
  func previousVersionsUnaffected() throws {
    let editor = try stateEditor()
    let vk = createState("vk", parse: { $0?.numberValue })
    var initialRoot: RootNode!
    var v0: RootNode!
    var v1: RootNode!
    try editor.update(discrete: true) { () throws in
      initialRoot = try getRoot()
      v0 = try setState(initialRoot, vk, 0)
      #expect(try getState(v0, vk) == 0)
      let change = try #require(try getStateChange(v0, initialRoot, vk))
      #expect(change.value == 0 && change.prevValue == nil)
    }
    let state0 = editor.getEditorState()
    try editor.update(discrete: true) { () throws in
      v1 = try setState(v0, vk, 1)
      #expect(v1 !== v0)
      #expect(v1.is(v0))
      #expect(try getState(v0, vk) == 1)
      #expect(try getState(v0, vk, .direct) == 0)
      #expect(try getState(v1, vk) == 1)
      let change = try #require(try getStateChange(v1, v0, vk))
      #expect(change.value == 1 && change.prevValue == 0)
    }
    let state1 = editor.getEditorState()
    #expect(try state0.read { try getState(v0, vk) } == 0)
    #expect(try state1.read { try getState(v1, vk) } == 1)
    #expect(try state0.read { try getState(v1, vk) } == 0)
    #expect(try state1.read { try getState(v0, vk) } == 1)
    #expect(try getState(initialRoot, vk, .direct) == nil)
    #expect(try getState(v0, vk, .direct) == 0)
    #expect(try getState(v1, vk, .direct) == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > undefined states are equivalent"
    ))
  func undefinedEquivalent() {
    #expect(nodeStatesAreEquivalent(nil, nil))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > merges text nodes with different number of default state values"
    ))
  func mergesWithDefaultStates() throws {
    let editor = try stateEditor()
    try editor.update(discrete: true) {
      let first = try createTextNode("hello")
      let second = try createTextNode("world")
      try setState(first, numberState, 1)
      try setState(first, boolState, true)
      try setState(second, boolState, true)
      try getRoot().clear().append(createParagraphNode().append(first, second))
    }
    #expect(try editor.read { try getRoot().getAllTextNodes().count } == 2)
    try editor.update(discrete: true) {
      let paragraph = try getRoot().getFirstChild() as! ParagraphNode
      try setState(paragraph.getChildren()[0], numberState, 0)
    }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 1)
      #expect(try getState(textNodes[0], numberState) == 0)
      #expect(try getState(textNodes[0], boolState) == true)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > TextNode merging only with equivalent state"
    ))
  func mergesOnlyEquivalent() throws {
    let editor = try stateEditor()
    let classNameState = createState("className", parse: { $0?.stringValue ?? "" })
    func textWithClassName(_ className: String) throws -> TextNode {
      try setState(createTextNode(className), classNameState, className)
    }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(
          textWithClassName("red"), createTextNode("none!"), textWithClassName("red"),
          textWithClassName("blue"), textWithClassName("blue"), textWithClassName("red"),
          textWithClassName("blue")))
    }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 6)
      #expect(
        try textNodes.map { try getState($0, classNameState) } == [
          "red", "", "red", "blue", "red", "blue",
        ])
      #expect(
        try textNodes.map { try $0.getTextContent() } == [
          "red", "none!", "red", "blueblue", "red", "blue",
        ])
    }
    try editor.update(discrete: true) {
      let paragraph = try getRoot().getFirstChildOrThrow() as! ParagraphNode
      try setState(paragraph.getChildAtIndex(1)!, classNameState, "red")
    }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 4)
      #expect(
        try textNodes.map { try getState($0, classNameState) } == ["red", "blue", "red", "blue"])
      #expect(
        try textNodes.map { try $0.getTextContent() } == ["rednone!red", "blueblue", "red", "blue"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > nodeStatesAreEquivalent > different versions of the same state are not equivalent"
    ))
  func versionsNotEquivalent() throws {
    let editor = try stateEditor()
    let vk = createState("vk", parse: { $0?.numberValue })
    var roots: [RootNode] = []
    try editor.update(discrete: true) {
      let initialRoot = try getRoot()
      roots = [initialRoot, try setState(initialRoot, vk, 0)]
    }
    try editor.update(discrete: true) { roots.append(try setState(roots[1], vk, 1)) }
    try editor.update(discrete: true) { roots.append(try setState(roots[2], vk, 0)) }
    try editor.update(discrete: true) { roots.append(try setState(roots[3], vk, nil)) }
    try editor.update(discrete: true) {
      var exported = try roots[4].exportJSON()
      exported[NODE_STATE_KEY] = json(#"{"vk":1}"#)
      roots.append(try roots[4].updateFromJSON(exported))
    }
    try editor.update(discrete: true) {
      var exported = try roots[5].exportJSON()
      exported[NODE_STATE_KEY] = json(#"{"unknown":true,"vk":1}"#)
      roots.append(try roots[5].updateFromJSON(exported))
    }
    let (initialRoot, v0, v1, v2, v3, v4, v5) = (
      roots[0], roots[1], roots[2], roots[3], roots[4], roots[5], roots[6]
    )
    let equivalences: [[NodeState?]] = [
      [initialRoot.__state, v3.__state, nil],
      [v0.__state, v2.__state],
      [v1.__state, v4.__state],
      [v5.__state],
    ]
    for i in equivalences.indices {
      for j in (i + 1)..<equivalences.count {
        for a in equivalences[i] {
          for b in equivalences[j] {
            #expect(nodeStatesAreEquivalent(a, b) == false)
            #expect(nodeStatesAreEquivalent(b, a) == false)
          }
        }
      }
      let aSet = equivalences[i]
      for j in aSet.indices {
        for k in (j + 1)..<aSet.count {
          #expect(nodeStatesAreEquivalent(aSet[j], aSet[k]))
          #expect(nodeStatesAreEquivalent(aSet[k], aSet[j]))
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state with resetOnCopyNode: true is reset when using $copyNode"
    ))
  func resetOnCopyTrue() throws {
    let editor = try stateEditor()
    let resetState = createState(
      "resetState", parse: { $0?.numberValue ?? 0 }, resetOnCopyNode: true)
    try editor.update(discrete: true) { () throws in
      let node = try createStateNode()
      try setState(node, resetState, 42)
      #expect(try getState(node, resetState) == 42)
      let copy = try copyNode(node)
      #expect(try getState(copy, resetState) == 0)
      #expect(try getState(node, resetState) == 42)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state with resetOnCopyNode: false is preserved when using $copyNode"
    ))
  func resetOnCopyFalse() throws {
    let editor = try stateEditor()
    let persistState = createState(
      "persistState", parse: { $0?.numberValue ?? 0 }, resetOnCopyNode: false)
    try editor.update(discrete: true) { () throws in
      let node = try createStateNode()
      try setState(node, persistState, 42)
      let copy = try copyNode(node)
      #expect(try getState(copy, persistState) == 42)
      #expect(try getState(node, persistState) == 42)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > state without resetOnCopyNode option is preserved when using $copyNode"
    ))
  func resetOnCopyDefault() throws {
    let editor = try stateEditor()
    let defaultState = createState("defaultState", parse: { $0?.numberValue ?? 0 })
    try editor.update(discrete: true) { () throws in
      let node = try createStateNode()
      try setState(node, defaultState, 42)
      let copy = try copyNode(node)
      #expect(try getState(copy, defaultState) == 42)
      #expect(try getState(node, defaultState) == 42)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::LexicalNode state > resetOnCopyNode > multiple states with different resetOnCopyNode configurations"
    ))
  func resetOnCopyMixed() throws {
    let editor = try stateEditor()
    let resetState = createState(
      "resetState", parse: { $0?.numberValue ?? 0 }, resetOnCopyNode: true)
    let persistState = createState(
      "persistState", parse: { $0?.stringValue ?? "" }, resetOnCopyNode: false)
    let defaultState = createState("defaultState", parse: { $0?.boolValue ?? false })
    try editor.update(discrete: true) { () throws in
      let node = try createStateNode()
      try setState(node, resetState, 100)
      try setState(node, persistState, "hello")
      try setState(node, defaultState, true)
      let copy = try copyNode(node)
      #expect(try getState(copy, resetState) == 0)
      #expect(try getState(copy, persistState) == "hello")
      #expect(try getState(copy, defaultState) == true)
      #expect(try getState(node, resetState) == 100)
      let fullCopy = try copyNode(node, skipReset: true)
      #expect(try getState(node, resetState) == 100)
      #expect(try getState(fullCopy, resetState) == 100)
      #expect(try getState(fullCopy, persistState) == "hello")
      #expect(try getState(fullCopy, defaultState) == true)
    }
  }
}

nonisolated(unsafe) private let ipState = createState("ipState", parse: { $0?.numberValue ?? 0 })
nonisolated(unsafe) private let iaState = createState("iaState", parse: { $0?.numberValue ?? 0 })
nonisolated(unsafe) private let icState = createState("icState", parse: { $0?.numberValue ?? 0 })

private final class InterleaveTransforms: @unchecked Sendable {
  var types: [String] = []
}
private let interleaveTransforms = InterleaveTransforms()

private class InterleaveParagraph: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InterleaveParagraph.self, type: "interleave-paragraph", extends: ElementNode.self,
      stateConfigs: [RequiredNodeStateConfig(ipState, flat: true)])
  }
}

private class InterleaveAbstract: InterleaveParagraph {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InterleaveAbstract.self, extends: InterleaveParagraph.self,
      transform: { interleaveTransforms.types.append($0.getType()) },
      stateConfigs: [RequiredNodeStateConfig(iaState, flat: true)])
  }
}

private final class InterleaveConcrete: InterleaveAbstract {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InterleaveConcrete.self, type: "interleave-concrete", extends: InterleaveAbstract.self,
      stateConfigs: [RequiredNodeStateConfig(icState, flat: true)])
  }
}

nonisolated(unsafe) private let replacedFlatState = createState(
  "replacedFlatKey", parse: { $0?.stringValue ?? "" })

private final class FlatParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FlatParagraph.self, type: "flat-paragraph", extends: ParagraphNode.self,
      stateConfigs: [RequiredNodeStateConfig(replacedFlatState, flat: true)])
  }
}

private final class SameTypeParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      SameTypeParagraph.self, type: "paragraph", extends: ParagraphNode.self,
      stateConfigs: [RequiredNodeStateConfig(replacedFlatState, flat: true)])
  }
}

nonisolated(unsafe) private let flatKey = createState(
  "importFlatKey", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let directKey = createState(
  "importDirectKey", parse: { $0?.numberValue ?? 0 })

private final class StatefulParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      StatefulParagraph.self, type: "stateful-paragraph", extends: ParagraphNode.self,
      stateConfigs: [RequiredNodeStateConfig(flatKey, flat: true)])
  }
}

private func editorWith(_ nodes: [LexicalNodeConfig]) throws -> Editor {
  try createEditor(CreateEditorArgs(namespace: "", nodes: nodes, onError: { throw $0 }))
}

@Suite struct LexicalNodeStateConfigTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::$config interleaved abstract/concrete classes > serializes, transforms, and types every interleaved level’s state"
    ))
  func interleaved() throws {
    interleaveTransforms.types = []
    let editor = try editorWith([.klass(InterleaveConcrete.self)])
    try editor.update(discrete: true) { () throws in
      let node = try setState(
        setState(setState(create(InterleaveConcrete.self), ipState, 1), iaState, 2), icState, 3)
      try getRoot().append(node)
      let exported = try node.exportJSON()
      #expect(exported["iaState"] == 2)
      #expect(exported["icState"] == 3)
      #expect(exported["ipState"] == 1)
      #expect(exported["type"] == "interleave-concrete")
    }
    #expect(interleaveTransforms.types.contains("interleave-concrete"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > the subclass writes and reads its own flat state top-level"
    ))
  func replacementFlatState() throws {
    let editor = try editorWith([
      .klass(FlatParagraph.self),
      .replacement(
        LexicalNodeReplacement(
          replace: ParagraphNode.self, with: { _ in try FlatParagraph() },
          withKlass: FlatParagraph.self)),
    ])
    try editor.update(discrete: true) { () throws in
      let p = try createParagraphNode()
      #expect(p is FlatParagraph)
      #expect(p.getType() == "flat-paragraph")
      try getRoot().clear().append(p.append(createTextNode("x")))
      try setState(p, replacedFlatState, "hello")
    }
    let exportedState = try editor.getEditorState().toJSON()
    let exported = try #require(
      exportedState["root"]?.objectValue?["children"]?.arrayValue?.first?.objectValue)
    #expect(exported["replacedFlatKey"] == "hello")
    #expect(exported[NODE_STATE_KEY] == nil)
    let reparsed = try editor.parseEditorState(exportedState.stringify())
    #expect(
      try reparsed.read { try getState(getRoot().getFirstChildOrThrow(), replacedFlatState) }
        == "hello")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > withKlass must itself be registered"
    ))
  func withKlassRegistered() throws {
    let editor = try editorWith([
      .replacement(
        LexicalNodeReplacement(
          replace: ParagraphNode.self, with: { _ in try FlatParagraph() },
          withKlass: FlatParagraph.self))
    ])
    #expect {
      try editor.update(discrete: true) { _ = try createParagraphNode() }
    } throws: { "\($0)".contains("not configured to be used on the editor") }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::flat NodeState through a node replacement > a replacement class cannot reuse the replaced type"
    ))
  func replacementSameType() throws {
    let editor = try editorWith([
      .replacement(
        LexicalNodeReplacement(
          replace: ParagraphNode.self, with: { _ in try SameTypeParagraph() },
          withKlass: SameTypeParagraph.self))
    ])
    #expect {
      try editor.update(discrete: true) { _ = try createParagraphNode() }
    } throws: { "\($0)".contains("does not match registered node ParagraphNode") }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > both flat and nested state survive a round trip through importJSON"
    ))
  func importJSONRoundTrip() throws {
    let editor = try editorWith([.klass(StatefulParagraph.self)])
    try editor.update(discrete: true) {
      let p = try create(StatefulParagraph.self)
      try getRoot().clear().append(p.append(createTextNode("x")))
      try setState(p, flatKey, "flat-value")
      try setState(p, directKey, 42)
    }
    let reparsed = try editor.parseEditorState(editor.getEditorState().toJSON().stringify())
    try reparsed.read { () throws in
      let p = try getRoot().getFirstChildOrThrow()
      #expect(try getState(p, flatKey) == "flat-value")
      #expect(try getState(p, directKey) == 42)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > a node whose JSON carries no state still imports its own properties"
    ))
  func importJSONNoState() throws {
    let editor = try editorWith([.klass(StatefulParagraph.self)])
    try editor.update(discrete: true) {
      let p = try create(StatefulParagraph.self)
      try getRoot().clear().append(p.append(createTextNode("hello")))
      try p.setIndent(2)
    }
    let serialized = try editor.getEditorState().toJSON().stringify()
    #expect(serialized.contains(NODE_STATE_KEY) == false)
    try editor.parseEditorState(serialized).read { () throws in
      let p = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      #expect(try p.getIndent() == 2)
      #expect(try p.getTextContent() == "hello")
      #expect(try getState(p, flatKey) == "")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNodeState.test.ts::importJSON applies state to the node it just built > the node importJSON returns is writable, so its properties stick"
    ))
  func importJSONWritable() throws {
    let editor = try editorWith([.klass(StatefulParagraph.self)])
    try editor.update(discrete: true) {
      let node = try #require(
        try ParagraphNode.importJSON(
          jsonObject(
            #"{"children":[],"direction":null,"format":"","indent":3,"type":"paragraph","version":1}"#
          )) as? ParagraphNode)
      try getRoot().clear().append(node.append(createTextNode("y")))
    }
    try editor.read { () throws in
      let p = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
      #expect(try p.getIndent() == 3)
      #expect(try p.getTextContent() == "y")
    }
  }
}
