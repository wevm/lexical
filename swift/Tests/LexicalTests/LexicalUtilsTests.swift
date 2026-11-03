import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalUtils.test.ts. The DOM helpers
// (scheduleMicroTask, isSelectionWithinEditor, isExactShortcutMatch,
// scrollIntoViewIfNeeded, getCaretRect, $updateTextNodeFromDOMContent,
// getParentElement, $getDocument) and the JS-only isArray and emptyFunction
// are skipped in parity/skips.json.

/// `ExtendedTextNode` from the `$applyNodeReplacement` tests.
private class ExtendedTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ExtendedTextNode.self, type: "extended-text", extends: TextNode.self)
  }
  func initWithTextNode(_ node: TextNode) throws -> Self {
    try getWritable().updateFromJSON(try node.exportJSON())
  }
}

/// `ExtendedExtendedTextNode` from the `$applyNodeReplacement` tests.
private final class ExtendedExtendedTextNode: ExtendedTextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      ExtendedExtendedTextNode.self, type: "extended-extended-text",
      extends: ExtendedTextNode.self)
  }
  func initWithExtendedTextNode(_ node: ExtendedTextNode) throws -> Self {
    try initWithTextNode(node)
  }
}

private func createExtendedTextNode(_ text: String = "") throws -> ExtendedTextNode {
  try applyNodeReplacement(try ExtendedTextNode(text))
}

private func createExtendedExtendedTextNode(
  _ text: String = ""
) throws
  -> ExtendedExtendedTextNode
{
  try applyNodeReplacement(try ExtendedExtendedTextNode(text))
}

/// The replacement `{replace: TextNode, with: $createExtendedTextNode()…}`.
private func textToExtended(withKlass: LexicalNode.Type?) -> LexicalNodeConfig {
  .replacement(
    LexicalNodeReplacement(
      replace: TextNode.self,
      with: { try createExtendedTextNode().initWithTextNode($0 as! TextNode) },
      withKlass: withKlass))
}

/// The replacement `{replace: ExtendedTextNode, with: …}`.
nonisolated(unsafe) private let extendedToExtendedExtended = LexicalNodeConfig.replacement(
  LexicalNodeReplacement(
    replace: ExtendedTextNode.self,
    with: {
      try createExtendedExtendedTextNode().initWithExtendedTextNode($0 as! ExtendedTextNode)
    },
    withKlass: ExtendedExtendedTextNode.self))

/// `createEditor({nodes, onError(err) { throw err; }})`.
private func throwingEditor(_ nodes: [LexicalNodeConfig]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes, onError: { throw $0 }))
}

/// Expects the discrete update that appends a paragraph with `text` to throw
/// an error whose message contains `message`.
private func expectAppendThrows(
  _ editor: Editor, _ message: String, extended: Bool = false,
  sourceLocation: SourceLocation = #_sourceLocation
) {
  do {
    try editor.update(discrete: true) { () throws in
      let text: TextNode =
        extended ? try createExtendedTextNode("text") : try createTextNode("text")
      try getRoot().clear().append(try createParagraphNode().append(text))
    }
    Issue.record("expected the update to throw", sourceLocation: sourceLocation)
  } catch {
    #expect("\(error)".contains(message), sourceLocation: sourceLocation)
  }
}

nonisolated(unsafe) private let stringState = createState(
  "string-state", parse: { $0?.stringValue ?? "" })

/// `ExtendedParagraphNode` from the `$copyNode` tests.
private final class ExtendedParagraphNode: ParagraphNode {
  var __string = "default"
  override class func config() -> NodeConfig? {
    NodeConfig(ExtendedParagraphNode.self, type: "extended-paragraph", extends: ParagraphNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    __string = (prevNode as! ExtendedParagraphNode).__string
  }
  func setString(_ value: String) throws -> Self {
    let writable = try getWritable()
    writable.__string = value
    return writable
  }
}

private final class TextNodeA: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TextNodeA.self, type: "text-a", extends: TextNode.self)
  }
}

private final class StaticConfigNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(StaticConfigNode.self, type: "static-config-node", extends: TextNode.self)
  }
}

private final class CachedStaticConfigNode: TextNode {
  nonisolated(unsafe) static var configCalls = 0
  override class func config() -> NodeConfig? {
    configCalls += 1
    return NodeConfig(
      CachedStaticConfigNode.self, type: "cached-static-config-node", extends: TextNode.self)
  }
}

private class ChainBase: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ChainBase.self, type: "chain-base", extends: ElementNode.self)
  }
}

private final class ChainSub: ChainBase {}

nonisolated(unsafe) private var transformCalls: [NodeKey] = []

private class TransformBase: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TransformBase.self, type: "chain-transform-base", extends: ElementNode.self,
      transform: { transformCalls.append($0.getKey()) })
  }
}

private final class TransformSub: TransformBase {}

/// `[...map].map(([type, subtypes]) => [type, [...subtypes].sort()])`.
private func subtypeObject(_ map: JSMap<String, JSSet<String>>) -> [String: [String]] {
  var object: [String: [String]] = [:]
  for (type, subtypes) in map {
    object[type] = subtypes.array.sorted()
  }
  return object
}

@Suite(.serialized) struct LexicalUtilsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > resetRandomKey()"))
  func resetRandomKeyRestartsKeys() {
    resetRandomKey()
    let key1 = generateRandomKey()
    resetRandomKey()
    let key2 = generateRandomKey()
    #expect(key1 != "")
    #expect(key2 != "")
    #expect(key1 == key2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > generateRandomKey()"))
  func generateRandomKeyIsUnique() {
    let key1 = generateRandomKey()
    let key2 = generateRandomKey()
    #expect(key1 != "")
    #expect(key2 != "")
    #expect(key1 != key2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > getTextDirection()"))
  func textDirection() {
    for text in ["", " ", "0"] {
      #expect(getTextDirection(text) == nil, "\(text)")
    }
    let ltr: [String] = [
      "A", "Z", "a", "z", "\u{00C0}", "\u{00D6}", "\u{00D8}", "\u{00F6}", "\u{00F8}", "\u{02B8}",
      "\u{0300}", "\u{0590}", "\u{0800}", "\u{1FFF}", "\u{200E}", "\u{2C00}", "\u{FB1C}",
      "\u{FE00}", "\u{FE6F}", "\u{FEFD}", "\u{FFFF}",
    ]
    for text in ltr {
      #expect(getTextDirection(text) == "ltr", "\(text.unicodeScalars.first!.value)")
    }
    for text in ["\u{0591}", "\u{07FF}", "\u{FB1D}", "\u{FDFD}", "\u{FE70}", "\u{FEFC}"] {
      #expect(getTextDirection(text) == "rtl", "\(text.unicodeScalars.first!.value)")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > isTokenOrSegmented()"))
  func tokenOrSegmented() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(try isTokenOrSegmented(try createTextNode("foo")) == false)
      #expect(try isTokenOrSegmented(try createTextNode().setMode("token")))
      #expect(try isTokenOrSegmented(try createTextNode("foo").setMode("segmented")))
    }
  }

  @Test(
    .upstream("lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $getNodeByKey")
  )
  func nodeByKey() throws {
    let editor = try createTestEditor()
    var paragraphNode: ParagraphNode?
    var textNode: TextNode?
    try update(editor) { () throws in
      paragraphNode = try ParagraphNode()
      textNode = try TextNode("foo")
      try paragraphNode!.append(textNode!)
      try getRoot().append(paragraphNode!)
    }
    try editor.read(.latest) { () throws in
      #expect(try getNodeByKey("1") === paragraphNode)
      #expect(try getNodeByKey("2") === textNode)
      #expect(try getNodeByKey("3") == nil)
    }
    // `$getNodeByKey()` without an active editor state throws.
    #expect(throws: (any Error).self) { try getNodeByKey("1") }
  }

  @Test(
    .upstream("lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $nodesOfType"))
  func nodesOfParagraphType() throws {
    let editor = try createTestEditor()
    var paragraphKeys: [NodeKey] = []
    let paragraphKeysNow = { try nodesOfType(ParagraphNode.self).map { $0.getKey() } }
    try update(editor) { () throws in
      let root = try getRoot()
      let paragraph1 = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      _ = try createParagraphNode()
      try root.append(paragraph1, paragraph2)
      paragraphKeys = [paragraph1.getKey(), paragraph2.getKey()]
      #expect(try paragraphKeysNow().sorted() == paragraphKeys.sorted())
    }
    try editor.read(.latest) { () throws in
      #expect(try paragraphKeysNow().sorted() == paragraphKeys.sorted())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > deferred even when there are no dirty nodes"
    ))
  func onUpdateDeferredWithoutDirtyNodes() throws {
    let editor = try createTestEditor()
    var runs: [String] = []
    try editor.update(onUpdate: { runs.append("first") }) {
      try onUpdate { runs.append("second") }
    }
    #expect(runs == [])
    try editor.update {
      try onUpdate { runs.append("third") }
    }
    #expect(runs == [])
    try editor.read {}
    #expect(runs == ["first", "second", "third"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > added fn runs after update, original onUpdate, and prior calls to $onUpdate"
    ))
  func onUpdateRunsInOrder() throws {
    let editor = try createTestEditor()
    var runs: [String] = []
    try editor.update(onUpdate: { runs.append("first") }) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("foo")))
      try onUpdate { runs.append("second") }
      try onUpdate { runs.append("third") }
    }
    try editor.read {}
    #expect(runs == ["first", "second", "third"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > $onUpdate > adding fn throws outside update"
    ))
  func onUpdateThrowsOutsideUpdate() {
    #expect(throws: (any Error).self) { try onUpdate {} }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::LexicalUtils tests > getCachedTypeToNodeMap"
    ))
  func cachedTypeToNodeMap() throws {
    let editor = try createTestEditor()
    var paragraphKeys: [NodeKey] = []
    // Upstream's testEnv mounts the editor, which commits the initial state;
    // a headless editor still has the writable empty state, so commit it.
    try editor.update(discrete: true) { try getRoot().markDirty() }
    let initialTypeToNodeMap = try getCachedTypeToNodeMap(editor.getEditorState())
    #expect(try getCachedTypeToNodeMap(editor.getEditorState()) === initialTypeToNodeMap)
    #expect(initialTypeToNodeMap.keysArray == ["root"])
    #expect(initialTypeToNodeMap.get("root")?.size == 1)
    try editor.update(discrete: true) { () throws in
      let root = try getRoot()
      let paragraph1 = try createParagraphNode().append(try createTextNode("a"))
      let paragraph2 = try createParagraphNode().append(try createTextNode("b"))
      _ = try createParagraphNode().append(try createTextNode("c"))
      try root.append(paragraph1, paragraph2)
      paragraphKeys = [paragraph1.getKey(), paragraph2.getKey()]
    }
    let typeToNodeMap = try getCachedTypeToNodeMap(editor.getEditorState())
    #expect(typeToNodeMap !== initialTypeToNodeMap)
    #expect(try getCachedTypeToNodeMap(editor.getEditorState()) === typeToNodeMap)
    #expect(typeToNodeMap.size == 3)
    #expect(typeToNodeMap.keysArray.sorted() == ["paragraph", "root", "text"])
    let paragraphMap = try #require(typeToNodeMap.get("paragraph"))
    #expect(paragraphMap.keysArray.sorted() == paragraphKeys.sorted())
    let textMap = try #require(typeToNodeMap.get("text"))
    #expect(textMap.size == 2)
    #expect(textMap.valuesArray.map { ($0 as! TextNode).__text }.sorted() == ["a", "b"])
  }

  // MARK: $applyNodeReplacement

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node configuration"
    ))
  func replaceNodeConfiguration() throws {
    let editor = try throwingEditor([textToExtended(withKlass: ExtendedExtendedTextNode.self)])
    expectAppendThrows(
      editor,
      "Attempted to create node ExtendedTextNode that was not configured to be used on the editor")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node type withKlass"
    ))
  func replaceNodeTypeWithKlass() throws {
    let editor = try throwingEditor([
      .replacement(
        LexicalNodeReplacement(
          replace: TextNode.self, with: { $0 }, withKlass: ExtendedTextNode.self))
    ])
    expectAppendThrows(
      editor,
      "$applyNodeReplacement failed. Expected replacement node to be an instance of ExtendedTextNode with type extended-text but returned TextNode with type text from original node TextNode with type text"
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node type change"
    ))
  func replaceNodeTypeChange() throws {
    var warnings: [String] = []
    let editor = try withWarningHandler({ warnings.append($0) }) {
      try throwingEditor([
        .replacement(
          LexicalNodeReplacement(
            replace: TextNode.self, with: { try TextNode(($0 as! TextNode).__text) }))
      ])
    }
    #expect(
      warnings == [
        "Override for TextNode specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version."
      ])
    expectAppendThrows(
      editor,
      "$applyNodeReplacement failed. Ensure replacement node TextNode with type text is a subclass of the original node TextNode with type text"
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node key change"
    ))
  func replaceNodeKeyChange() throws {
    let editor = try throwingEditor([
      .replacement(
        LexicalNodeReplacement(
          replace: TextNode.self,
          with: { try ExtendedTextNode(($0 as! TextNode).__text, key: $0.getKey()) },
          withKlass: ExtendedTextNode.self))
    ])
    expectAppendThrows(
      editor,
      "Lexical node with constructor ExtendedTextNode attempted to re-use key from node in active editor state with constructor TextNode. Keys must not be re-used when the type is changed."
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates replace node configuration withKlass"
    ))
  func replaceNodeConfigurationWithKlass() throws {
    let editor = try throwingEditor([textToExtended(withKlass: ExtendedTextNode.self)])
    expectAppendThrows(
      editor,
      "Attempted to create node ExtendedTextNode that was not configured to be used on the editor")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates nested replace node configuration"
    ))
  func nestedReplaceNodeConfiguration() throws {
    let editor = try throwingEditor([.klass(ExtendedTextNode.self), extendedToExtendedExtended])
    expectAppendThrows(
      editor,
      "Attempted to create node ExtendedExtendedTextNode that was not configured to be used on the editor",
      extended: true)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > validates nested replace node configuration withKlass"
    )
  )
  func nestedReplaceNodeConfigurationWithKlass() throws {
    let editor = try throwingEditor([
      .klass(ExtendedTextNode.self), textToExtended(withKlass: ExtendedTextNode.self),
      extendedToExtendedExtended,
    ])
    expectAppendThrows(
      editor,
      "Attempted to create node ExtendedExtendedTextNode that was not configured to be used on the editor"
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$applyNodeReplacement > nested replace node configuration works"
    ))
  func nestedReplaceNodeConfigurationWorks() throws {
    let editor = try throwingEditor([
      .klass(ExtendedTextNode.self), .klass(ExtendedExtendedTextNode.self),
      textToExtended(withKlass: ExtendedTextNode.self), extendedToExtendedExtended,
    ])
    try editor.update(discrete: true) { () throws in
      try getRoot().clear().append(try createParagraphNode().append(try createTextNode("text")))
    }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      #expect(textNodes.count == 1)
      #expect(type(of: textNodes[0]) == ExtendedExtendedTextNode.self)
      #expect(try textNodes[0].getTextContent() == "text")
    }
  }

  // MARK: $copyNode

  private func copyNodeEditor() throws -> Editor {
    try throwingEditor([
      .klass(ExtendedParagraphNode.self), .klass(TextNode.self), .klass(ParagraphNode.self),
    ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::$copyNode > does not mark the original as dirty"
    ))
  func copyNodeLeavesOriginalClean() throws {
    let editor = try copyNodeEditor()
    var initialParagraph: ExtendedParagraphNode?
    try editor.update(discrete: true) { () throws in
      initialParagraph = try applyNodeReplacement(try ExtendedParagraphNode())
      try getRoot().clear().append(try initialParagraph!.append(try createTextNode("text")))
    }
    try editor.update(discrete: true) { () throws in
      #expect(try getRoot().getFirstChild() === initialParagraph)
      let copiedParagraph = try copyNode(initialParagraph!)
      #expect(try getRoot().getFirstChild() === initialParagraph)
      #expect(copiedParagraph !== initialParagraph)
      #expect(type(of: copiedParagraph) == ExtendedParagraphNode.self)
    }
  }

  @Test(
    .upstream("lexical/src/__tests__/unit/LexicalUtils.test.ts::$copyNode > returns a shallow copy")
  )
  func copyNodeIsShallow() throws {
    let editor = try copyNodeEditor()
    var initialParagraph: ExtendedParagraphNode?
    var copiedParagraph: ExtendedParagraphNode?
    try editor.update(discrete: true) { () throws in
      let initial = try applyNodeReplacement(try ExtendedParagraphNode()).setString("non-default")
      _ = try setState(initial, stringState, "non-default")
      let root = try getRoot().clear()
      try root.append(try initial.append(try createTextNode("text")))
      let copied = try copyNode(initial)
      try root.append(copied)
      _ = try setState(try initial.setString("not-aliased"), stringState, "not-aliased")
      initialParagraph = initial
      copiedParagraph = copied
    }
    try editor.read { () throws in
      let initial = try initialParagraph!.getLatest()
      let copied = try copiedParagraph!.getLatest()
      #expect(
        try getRoot().getChildren().map(ObjectIdentifier.init)
          == [initial, copied].map(ObjectIdentifier.init))
      #expect(try initial.getTextContent() == "text")
      #expect(try copied.getTextContent() == "")
      #expect(try getState(initial, stringState) == "not-aliased")
      #expect(try getState(copied, stringState) == "non-default")
      #expect(initial.__string == "not-aliased")
      #expect(copied.__string == "non-default")
    }
  }

  // MARK: getRegisteredSubtypeMap

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > maps each type to itself and its registered subclass types"
    ))
  func subtypeMapOfCoreNodes() {
    let map = getRegisteredSubtypeMap([
      TextNode.self, TabNode.self, ParagraphNode.self, LineBreakNode.self,
    ])
    #expect(
      subtypeObject(map) == [
        "linebreak": ["linebreak"], "paragraph": ["paragraph"], "tab": ["tab"],
        "text": ["tab", "text"],
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > expands a $config subclass under its base type"
    ))
  func subtypeMapExpandsSubclass() {
    #expect(
      subtypeObject(getRegisteredSubtypeMap([TextNode.self, TextNodeA.self])) == [
        "text": ["text", "text-a"], "text-a": ["text-a"],
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getRegisteredSubtypeMap > omits an unregistered base type even when a subclass is registered"
    ))
  func subtypeMapOmitsUnregisteredBase() {
    let map = getRegisteredSubtypeMap([TextNodeA.self])
    #expect(map.has("text") == false)
    #expect(map.get("text-a")?.array.sorted() == ["text-a"])
  }

  // MARK: getStaticNodeConfig and iterStaticNodeConfigChain

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > getStaticNodeConfig() > derives the type and config from $config()"
    ))
  func staticNodeConfigFromConfig() {
    let config = getStaticNodeConfig(StaticConfigNode.self)
    #expect(config.ownNodeType == "static-config-node")
    #expect(config.ownNodeConfig?.type == "static-config-node")
    #expect(config.ownNodeConfig?.extends == TextNode.self)
    #expect(StaticConfigNode.getType() == "static-config-node")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > getStaticNodeConfig() > caches the result for a node class"
    )
  )
  func staticNodeConfigIsCached() {
    let first = getStaticNodeConfig(CachedStaticConfigNode.self)
    let second = getStaticNodeConfig(CachedStaticConfigNode.self)
    #expect(first.klass == second.klass)
    #expect(first.ownNodeType == second.ownNodeType)
    #expect(CachedStaticConfigNode.configCalls == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > getStaticNodeConfig() > resolves symbol-keyed config for abstract node classes"
    ))
  func staticNodeConfigOfAbstractClass() {
    let config = getStaticNodeConfig(ElementNode.self)
    #expect(config.ownNodeType == nil)
    #expect(config.ownNodeConfig?.extends == LexicalNode.self)
    #expect(config.ownNodeConfig?.transform != nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > iterStaticNodeConfigChain > a subclass that declares no $config contributes nothing, and does not hide its parent"
    ))
  func chainSkipsSubclassWithoutConfig() {
    let chain = iterStaticNodeConfigChain(ChainSub.self).map {
      ("\($0.klass)", $0.ownNodeConfig == nil)
    }
    #expect(chain.map(\.0) == ["ChainSub", "ChainBase", "ElementNode", "LexicalNode"])
    #expect(chain.map(\.1) == [true, false, false, true])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > iterStaticNodeConfigChain > an inline $transform is registered once for a subclass that declares no $config"
    ))
  func inlineTransformRegisteredOnce() throws {
    transformCalls = []
    let editor = try createTestEditor(nodes: [
      .klass(TransformBase.self), .klass(TransformSub.self),
    ])
    try editor.update(discrete: true) { () throws in
      try getRoot().clear().append(try TransformSub().append(try createTextNode("x")))
    }
    #expect(transformCalls.count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUtils.test.ts::getParentElement > iterStaticNodeConfigChain > handles a class transform"
    ))
  func paragraphChain() {
    #expect(
      iterStaticNodeConfigChain(ParagraphNode.self).map { ObjectIdentifier($0.klass) } == [
        ObjectIdentifier(ParagraphNode.self), ObjectIdentifier(ElementNode.self),
        ObjectIdentifier(LexicalNode.self),
      ])
  }
}
