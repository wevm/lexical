import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalEditor.test.tsx, continued from
// LexicalEditorTests.swift: parsing, commands, listeners, tags, discrete
// updates and node replacement. Expected HTML is asserted as `shape()`.

/// `TestTextNode` from the upstream test utils.
final class TestTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestTextNode.self, type: "test_text", extends: TextNode.self)
  }
}

private final class CustomParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CustomParagraphNode.self, type: "custom-paragraph", extends: ParagraphNode.self)
  }
}

private final class ResolvedParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ResolvedParagraph.self, type: "resolved-paragraph", extends: ParagraphNode.self)
  }
}

private final class LegacyParagraph: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(LegacyParagraph.self, type: "legacy-paragraph", extends: ParagraphNode.self)
  }
}

private func field(_ value: Any?) -> String {
  value.map { "\($0)" } ?? "null"
}

/// The own fields of an element, as upstream's `toEqual` lists them.
private func elementFields(_ node: ElementNode) -> [String: String] {
  [
    "__dir": field(node.__dir), "__first": field(node.__first), "__format": field(node.__format),
    "__indent": field(node.__indent), "__key": node.__key, "__last": field(node.__last),
    "__next": field(node.__next), "__parent": field(node.__parent), "__prev": field(node.__prev),
    "__size": field(node.__size), "__slotHost": field(node.__slotHost),
    "__slots": field(node.__slots), "__style": node.__style,
    "__textFormat": field(node.__textFormat), "__textStyle": node.__textStyle,
    "__type": node.__type,
  ]
}

private func textFields(_ node: TextNode) -> [String: String] {
  [
    "__detail": field(node.__detail), "__format": field(node.__format), "__key": node.__key,
    "__mode": field(node.__mode), "__next": field(node.__next), "__parent": field(node.__parent),
    "__prev": field(node.__prev), "__style": node.__style, "__text": node.__text,
    "__type": node.__type,
  ]
}

private func expectedElement(
  key: NodeKey, child: NodeKey, parent: NodeKey?, type: String
) -> [String: String] {
  [
    "__dir": "null", "__first": child, "__format": "0", "__indent": "0", "__key": key,
    "__last": child, "__next": "null", "__parent": parent ?? "null", "__prev": "null",
    "__size": "1", "__slotHost": "null", "__slots": "null", "__style": "", "__textFormat": "0",
    "__textStyle": "", "__type": type,
  ]
}

private func roundTrip(_ editor: Editor) throws -> EditorState {
  try editor.parseEditorState(try editor.getEditorState().toJSON().stringify())
}

/// The parsed-nodes assertions shared by both `parseEditorState()` suites.
private func expectParsedNodes(_ parsedEditorState: EditorState) throws {
  try parsedEditorState.read { () throws in
    let parsedRoot = try getRoot()
    let parsedParagraph = try #require(try parsedRoot.getFirstChild() as? ParagraphNode)
    let parsedText = try #require(try parsedParagraph.getFirstChild() as? TextNode)
    let paragraphKey = parsedParagraph.getKey()
    let textKey = parsedText.getKey()
    var rootExpected = expectedElement(
      key: "root", child: paragraphKey, parent: nil, type: "root")
    rootExpected["__cachedText"] = "null"
    var rootActual = elementFields(parsedRoot)
    rootActual["__cachedText"] = field(parsedRoot.__cachedText)
    #expect(rootActual == rootExpected)
    #expect(
      elementFields(parsedParagraph)
        == expectedElement(key: paragraphKey, child: textKey, parent: "root", type: "paragraph"))
    #expect(
      textFields(parsedText) == [
        "__detail": "0", "__format": "0", "__key": textKey, "__mode": "0", "__next": "null",
        "__parent": paragraphKey, "__prev": "null", "__style": "", "__text": "Hello world",
        "__type": "text",
      ])
  }
}

private func expectParsedTextContent(_ parsedEditorState: EditorState) throws {
  #expect(try parsedEditorState.read { try getRoot().__cachedText } == nil)
  #expect(try parsedEditorState.read { try getRoot().getTextContent() } == "Hello world")
}

private func rangeSelectionState() throws -> EditorState {
  let editor = try initEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    let originalText = try createTextNode("Hello world")
    _ = try originalText.select(6, 11)
    try paragraph.append(originalText)
    try getRoot().append(paragraph)
  }
  return try roundTrip(editor)
}

private func nodeSelectionState() throws -> EditorState {
  let editor = try initEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    let originalText = try createTextNode("Hello world")
    let selection = createNodeSelection()
    selection.add(originalText.getKey())
    try setSelection(selection)
    try paragraph.append(originalText)
    try getRoot().append(paragraph)
  }
  return try roundTrip(editor)
}

/// `reset()` from the `Node children` suite: an editor with one paragraph.
private func nodeChildrenEditor() throws -> Editor {
  let editor = try initEditor()
  try update(editor) { try getRoot().append(try createParagraphNode()) }
  return editor
}

private func elementWithText(_ text: String) throws -> TestElementNode {
  try createTestElementNode().append(try createTextNode(text))
}

private func element(_ key: NodeKey) throws -> ElementNode {
  try #require(try getNodeByKey(key) as? ElementNode)
}

/// A replacement of `TextNode` by `TestTextNode`.
private func testTextReplacement(
  withKlass: Bool = true, reuseKey: Bool = false
)
  -> [LexicalNodeConfig]
{
  [
    .klass(TestTextNode.self),
    .replacement(
      LexicalNodeReplacement(
        replace: TextNode.self,
        with: { node in
          try TestTextNode(
            try (node as! TextNode).getTextContent(), key: reuseKey ? node.getKey() : nil)
        },
        withKlass: withKlass ? TestTextNode.self : nil)),
  ]
}

private func replaceWithoutWithKlass(_ name: String) -> String {
  "Override for \(name) specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version."
}

private func paragraphJSON() -> String {
  #"{"root":{"children":[{"children":[],"type":"paragraph"}],"type":"root"}}"#
}

private func createReplacementEditor(
  _ nodes: [LexicalNodeConfig], _ onError: Calls
) throws
  -> Editor
{
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "test", nodes: defaultTestNodes + nodes, onError: { onError.record($0) },
      theme: [:]))
}

@Suite(.serialized) struct LexicalEditorModelTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > exportJSON API - parses parsed JSON"
    ))
  func parsesParsedJSON() throws {
    let editor = try initEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      _ = try paragraph.setDirection("rtl")
      let originalText = try createTextNode("Hello world")
      _ = try originalText.select(6, 11)
      try paragraph.append(originalText)
      try getRoot().clear().append(paragraph)
    }
    let stringified = try editor.getEditorState().toJSON().stringify()
    guard case .object(let object) = try JSONValue.parse(stringified) else {
      Issue.record("not an object")
      return
    }
    try editor.parseEditorState(object).read { () throws in
      let root = try getRoot()
      let paragraph = try #require(try root.getFirstChild() as? ParagraphNode)
      #expect(try paragraph.getDirection() == "rtl")
      #expect(try root.getTextContent().contains("Hello world"))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > range selection > Parses the nodes of a stringified editor state"
    ))
  func parsesRangeSelectionNodes() throws {
    try expectParsedNodes(try rangeSelectionState())
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > range selection > Parses the text content of the editor state"
    ))
  func parsesRangeSelectionText() throws {
    try expectParsedTextContent(try rangeSelectionState())
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > node selection > Parses the nodes of a stringified editor state"
    ))
  func parsesNodeSelectionNodes() throws {
    try expectParsedNodes(try nodeSelectionState())
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > node selection > Parses the text content of the editor state"
    ))
  func parsesNodeSelectionText() throws {
    try expectParsedTextContent(try nodeSelectionState())
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > $parseSerializedNode() > parses serialized nodes"
    ))
  func parsesSerializedNodes() throws {
    let editor = try initEditor()
    try update(editor) {
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("Hello world")))
    }
    let rootJSON = try editor.getEditorState().toJSON()["root"]?.objectValue
    let childrenJSON = try #require(rootJSON?["children"]?.arrayValue)
    var actualTextContent = ""
    try update(editor) {
      let children = try childrenJSON.map { try parseSerializedNode($0.objectValue!) }
      let root = try getRoot()
      try root.append(contentsOf: children)
      actualTextContent = try root.getTextContent()
    }
    #expect(actualTextContent == "Hello world\n\nHello world")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches"
    ))
  func movesNodeToDifferentBranches() throws {
    let editor = try nodeChildrenEditor()
    var keys: [NodeKey] = []
    var element1Key: NodeKey = ""
    var element2Key: NodeKey = ""
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let element1 = try elementWithText("A")
      let element2 = try elementWithText("B")
      element1Key = element1.getKey()
      element2Key = element2.getKey()
      keys = [
        paragraph.getKey(), element1Key, try element1.getFirstChildOrThrow().getKey(),
        element2Key, try element2.getFirstChildOrThrow().getKey(),
      ]
      try paragraph.append(element1, element2)
    }
    try update(editor) {
      try element(element1Key).append(try element(element2Key))
    }
    for key in keys {
      #expect(editor._editorState._nodeMap.has(key))
    }
    #expect(editor._editorState._nodeMap.size == keys.count + 1)  // + root
    #expect(
      try shape(editor)
        == "<paragraph><test_block>[A]<test_block>[B]</test_block></test_block></paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches (inverse)"
    ))
  func movesNodeToDifferentBranchesInverse() throws {
    let editor = try nodeChildrenEditor()
    var element1Key: NodeKey = ""
    var element2Key: NodeKey = ""
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let element1 = try elementWithText("A")
      let element2 = try elementWithText("B")
      element1Key = element1.getKey()
      element2Key = element2.getKey()
      try paragraph.append(element1, element2)
    }
    try update(editor) {
      try element(element2Key).append(try element(element1Key))
    }
    #expect(
      try shape(editor)
        == "<paragraph><test_block>[B]<test_block>[A]</test_block></test_block></paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches (node appended twice in two different branches)"
    ))
  func movesNodeAppendedTwice() throws {
    let editor = try nodeChildrenEditor()
    var elementKeys: [NodeKey] = []
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let elements = try ["A", "B", "C"].map(elementWithText)
      elementKeys = elements.map { $0.getKey() }
      try paragraph.append(contentsOf: elements)
    }
    try update(editor) {
      let element3 = try element(elementKeys[2])
      try element(elementKeys[1]).append(element3)
      try element(elementKeys[0]).append(element3)
    }
    #expect(
      try shape(editor)
        == "<paragraph><test_block>[A]<test_block>[C]</test_block></test_block><test_block>[B]</test_block></paragraph>"
    )
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can subscribe and unsubscribe from commands and the callback is fired"
    ))
  func subscribeAndUnsubscribeCommands() throws {
    let editor = try initEditor()
    var calls: [(String, Editor)] = []
    let command = createCommand("TEST_COMMAND", payload: String.self)
    let payload = "testPayload"
    let removeCommandListener = editor.registerCommand(
      command,
      { payload, editor in
        calls.append((payload, editor))
        return false
      }, COMMAND_PRIORITY_EDITOR)
    for _ in 0..<3 { try editor.dispatchCommand(command, payload) }
    #expect(calls.count == 3)
    #expect(calls.allSatisfy { $0.0 == payload && $0.1 === editor })
    removeCommandListener()
    for _ in 0..<3 { try editor.dispatchCommand(command, payload) }
    #expect(calls.count == 3)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > removes the command from the command map when no listener are attached"
    ))
  func removesCommandFromMap() throws {
    let editor = try initEditor()
    var calls: [String] = []
    let command = createCommand("TEST_COMMAND", payload: Void.self)
    let removeCommandListener = editor.registerCommand(
      command,
      { _, _ in
        calls.append("one")
        return false
      }, COMMAND_PRIORITY_EDITOR)
    let removeCommandListenerTwo = editor.registerCommand(
      command,
      { _, _ in
        calls.append("two")
        return false
      }, COMMAND_PRIORITY_EDITOR)
    let id = ObjectIdentifier(command)
    #expect(editor._commands[id] != nil)
    #expect(editor._commands[id]?.map(\.size) == [2, 0, 0, 0, 0])
    try editor.dispatchCommand(command)
    #expect(calls == ["one", "two"])
    removeCommandListener()
    #expect(editor._commands[id]?.map(\.size) == [1, 0, 0, 0, 0])
    calls = []
    try editor.dispatchCommand(command)
    #expect(calls == ["two"])
    removeCommandListenerTwo()
    #expect(editor._commands[id] == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can register transforms before updates"
    ))
  func registersTransformsBeforeUpdates() throws {
    let editor = try initEditor(onError: { throw $0 })
    let removeTextTransform = try editor.registerNodeTransform(TextNode.self) { _ in }
    let removeParagraphTransform = try editor.registerNodeTransform(ParagraphNode.self) { _ in }
    try update(editor) { try getRoot().append(try createParagraphNode()) }
    removeTextTransform()
    removeParagraphTransform()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > textcontent listener"
    ))
  func textContentListener() throws {
    let editor = try initEditor(onError: { throw $0 })
    var texts: [String] = []
    try editor.update {
      try getRoot().append(try createParagraphNode().append(try createTextNode("foo")))
    }
    editor.registerTextContentListener { texts.append($0) }
    try update(editor) {
      _ = try getRoot().getLastDescendant()!.insertAfter(try createTextNode("bar"))
    }
    #expect(texts == ["foobar"])
    try update(editor) {
      _ = try getRoot().getLastDescendant()!.insertAfter(try createLineBreakNode())
    }
    #expect(texts == ["foobar", "foobar\n"])
    try update(editor) {
      let root = try getRoot()
      try root.clear()
      let paragraph = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      try root.append(paragraph)
      try paragraph.append(try createTextNode("bar"))
      try paragraph2.append(try createTextNode("yar"))
      _ = try paragraph.insertAfter(paragraph2)
    }
    #expect(texts.count == 3)
    #expect(texts.last == "bar\n\nyar")
    try update(editor) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      _ = try root.getLastChild()!.insertAfter(paragraph)
      try paragraph.append(try createTextNode("bar2"))
      try paragraph2.append(try createTextNode("yar2"))
      _ = try paragraph.insertAfter(paragraph2)
    }
    #expect(texts.count == 4)
    #expect(texts.last == "bar\n\nyar\n\nbar2\n\nyar2")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > multiple update tags"
    ))
  func multipleUpdateTags() throws {
    let editor = try initEditor()
    try editor.update({ _ = try createTextNode() }, EditorUpdateOptions(tag: ["a", "b"]))
    #expect(editor._updateTags.sorted() == ["a", "b"])
    try editor.update(
      {
        try editor.update({}, EditorUpdateOptions(tag: ["e", "f"]))
      }, EditorUpdateOptions(tag: ["c", "d"]))
    #expect(editor._updateTags.sorted() == ["a", "b", "c", "d", "e", "f"])
    drainMicrotasks()
    #expect(editor._updateTags.size == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not leak a no-op update tag into the next update"
    ))
  func noOpTagDoesNotLeak() throws {
    let editor = try initEditor()
    var observedTags: [[String]] = []
    let unregister = editor.registerUpdateListener { observedTags.append(Array($0.tags)) }
    try editor.update(
      {}, EditorUpdateOptions(onUpdate: {}, tag: ["collaboration"], discrete: true))
    #expect(editor._updateTags.size == 0)
    try editor.update(discrete: true) { try getRoot().append(try createParagraphNode()) }
    unregister()
    let lastObservedTags = try #require(observedTags.last)
    #expect(lastObservedTags.contains("collaboration") == false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > setEditorState keeps its tag when committed from inside an update"
    ))
  func setEditorStateKeepsTag() throws {
    let editor = try initEditor()
    let historicState = try historicAndCurrent(editor)
    var observed: [(content: String, tags: [String])] = []
    let unregister = editor.registerUpdateListener { payload in
      observed.append(
        (
          try payload.editorState.read { try getRoot().getTextContent() },
          Array(payload.tags)
        ))
    }
    try editor.update(discrete: true) {
      try editor.setEditorState(historicState, tag: "historic-tag")
    }
    unregister()
    let historicCommit = try #require(observed.first { $0.content == "historic" })
    #expect(historicCommit.tags.contains("historic-tag"))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > defers onUpdate callbacks when setEditorState commits inside an update"
    ))
  func defersOnUpdateInSetEditorState() throws {
    let editor = try initEditor()
    let historicState = try historicAndCurrent(editor)
    var events: [String] = []
    try editor.update(
      {
        events.append("update start")
        try editor.setEditorState(historicState)
        events.append("update end")
      }, EditorUpdateOptions(onUpdate: { events.append("onUpdate") }))
    events.append("after update")
    #expect(events == ["update start", "update end", "after update"])
    drainMicrotasks()
    #expect(events == ["update start", "update end", "after update", "onUpdate"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > setEditorState reports an empty editor state outside production"
    ))
  func setEditorStateReportsEmptyState() throws {
    let onError = Calls()
    let editor = try initEditor(onError: { onError.record($0) })
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("before")))
    }
    let emptySerializedEditorState =
      #"{"root":{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
    try editor.setEditorState(try editor.parseEditorState(emptySerializedEditorState))
    #expect(onError.count == 1)
    #expect("\(onError.errors[0])".contains("the editor state is empty"))
    #expect(try editor.read { try getRoot().getTextContent() } == "before")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > editable listener"
    ))
  func editableListener() throws {
    let editor = try initEditor()
    var calls: [Bool] = []
    editor.registerEditableListener { calls.append($0) }
    #expect(editor.isEditable())
    try editor.setEditable(false)
    #expect(editor.isEditable() == false)
    try editor.setEditable(true)
    #expect(calls == [false, true])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not add new listeners while triggering existing"
    ))
  func doesNotAddListenersWhileTriggering() throws {
    // The mutation-listener half waits for the reconciler, which a headless
    // editor does not run.
    var updateListener = 0
    var nodeTransformListener = 0
    var textContentListener = 0
    var editableListener = 0
    var commandListener = 0
    let testCommand = createCommand("TEST_COMMAND", payload: Bool.self)
    let editor = try initEditor(onError: { throw $0 })
    editor.registerUpdateListener { _ in
      updateListener += 1
      editor.registerUpdateListener { _ in updateListener += 1 }
    }
    _ = try editor.registerNodeTransform(ParagraphNode.self) { _ in
      nodeTransformListener += 1
      _ = try editor.registerNodeTransform(ParagraphNode.self) { _ in nodeTransformListener += 1 }
    }
    editor.registerEditableListener { _ in
      editableListener += 1
      editor.registerEditableListener { _ in editableListener += 1 }
    }
    editor.registerTextContentListener { _ in
      textContentListener += 1
      editor.registerTextContentListener { _ in textContentListener += 1 }
    }
    editor.registerCommand(
      testCommand,
      { _, _ in
        commandListener += 1
        editor.registerCommand(
          testCommand,
          { _, _ in
            commandListener += 1
            return false
          }, COMMAND_PRIORITY_LOW)
        return false
      }, COMMAND_PRIORITY_LOW)
    try update(editor) {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("Hello world")))
    }
    try editor.dispatchCommand(testCommand, false)
    try editor.setEditable(false)
    #expect(updateListener == 1)
    #expect(editableListener == 1)
    #expect(commandListener == 1)
    #expect(textContentListener == 1)
    #expect(nodeTransformListener == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > calls command listeners in deque order"
    ))
  func commandListenersInDequeOrder() throws {
    var calls: [String] = []
    var regOrder = 0
    let names = [
      COMMAND_PRIORITY_BEFORE_EDITOR: "before:editor", COMMAND_PRIORITY_BEFORE_LOW: "before:low",
      COMMAND_PRIORITY_EDITOR: "after:editor", COMMAND_PRIORITY_LOW: "after:low",
    ]
    let testCommand = createCommand("TEST_COMMAND", payload: Void.self)
    let editor = try initEditor()
    let listener = { (priority: Int) -> () -> Void in
      let idx = regOrder
      regOrder += 1
      return editor.registerCommand(
        testCommand,
        { _, _ in
          calls.append("\(names[priority]!):\(idx)")
          return false
        }, priority)
    }
    let unreg = mergeRegister(
      listener(COMMAND_PRIORITY_EDITOR),
      listener(COMMAND_PRIORITY_LOW),
      listener(COMMAND_PRIORITY_EDITOR),
      listener(COMMAND_PRIORITY_LOW),
      listener(COMMAND_PRIORITY_BEFORE_EDITOR),
      listener(COMMAND_PRIORITY_BEFORE_LOW),
      listener(COMMAND_PRIORITY_BEFORE_EDITOR),
      listener(COMMAND_PRIORITY_BEFORE_LOW))
    #expect(calls.isEmpty)
    try editor.dispatchCommand(testCommand)
    #expect(
      calls == [
        "before:low:7", "before:low:5", "after:low:1", "after:low:3", "before:editor:6",
        "before:editor:4", "after:editor:0", "after:editor:2",
      ])
    unreg()
    calls = []
    try editor.dispatchCommand(testCommand)
    #expect(calls.isEmpty)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > maps priorities correctly"
    ))
  func mapsPrioritiesCorrectly() throws {
    // Upstream finds the constants by reflecting over the module's exports.
    let priorities: [(String, Int)] = [
      ("COMMAND_PRIORITY_EDITOR", COMMAND_PRIORITY_EDITOR),
      ("COMMAND_PRIORITY_LOW", COMMAND_PRIORITY_LOW),
      ("COMMAND_PRIORITY_NORMAL", COMMAND_PRIORITY_NORMAL),
      ("COMMAND_PRIORITY_HIGH", COMMAND_PRIORITY_HIGH),
      ("COMMAND_PRIORITY_CRITICAL", COMMAND_PRIORITY_CRITICAL),
      ("COMMAND_PRIORITY_BEFORE_EDITOR", COMMAND_PRIORITY_BEFORE_EDITOR),
      ("COMMAND_PRIORITY_BEFORE_LOW", COMMAND_PRIORITY_BEFORE_LOW),
      ("COMMAND_PRIORITY_BEFORE_NORMAL", COMMAND_PRIORITY_BEFORE_NORMAL),
      ("COMMAND_PRIORITY_BEFORE_HIGH", COMMAND_PRIORITY_BEFORE_HIGH),
      ("COMMAND_PRIORITY_BEFORE_CRITICAL", COMMAND_PRIORITY_BEFORE_CRITICAL),
    ]
    var beforePriorities: [(String, Int)] = []
    var afterPriorities: [(String, Int)] = []
    for (k, v) in priorities {
      if k.hasPrefix("COMMAND_PRIORITY_BEFORE") {
        #expect(v < 0)
        beforePriorities.append((k, v))
      } else {
        #expect(v >= 0)
        afterPriorities.append((k, v))
      }
    }
    beforePriorities.sort { $0.1 < $1.1 }
    afterPriorities.sort { $0.1 < $1.1 }
    #expect(beforePriorities.count == 5)
    #expect(afterPriorities.count == 5)
    #expect(
      beforePriorities.map { $0.0.replacingOccurrences(of: "COMMAND_PRIORITY_BEFORE_", with: "") }
        == afterPriorities.map { $0.0.replacingOccurrences(of: "COMMAND_PRIORITY_", with: "") })
    let editor = try initEditor()
    let command = createCommand("TEST_COMMAND", payload: Void.self)
    var listeners: [() -> Void] = []
    var calls: [String] = []
    for count in [0, 1] {
      for arr in [afterPriorities, beforePriorities] {
        for (k, v) in arr {
          listeners.append(
            editor.registerCommand(
              command,
              { _, _ in
                calls.append("\(k) \(count)")
                return false
              }, v))
        }
      }
    }
    try editor.dispatchCommand(command)
    #expect(
      calls == [
        "COMMAND_PRIORITY_BEFORE_CRITICAL 1", "COMMAND_PRIORITY_BEFORE_CRITICAL 0",
        "COMMAND_PRIORITY_CRITICAL 0", "COMMAND_PRIORITY_CRITICAL 1",
        "COMMAND_PRIORITY_BEFORE_HIGH 1", "COMMAND_PRIORITY_BEFORE_HIGH 0",
        "COMMAND_PRIORITY_HIGH 0", "COMMAND_PRIORITY_HIGH 1",
        "COMMAND_PRIORITY_BEFORE_NORMAL 1", "COMMAND_PRIORITY_BEFORE_NORMAL 0",
        "COMMAND_PRIORITY_NORMAL 0", "COMMAND_PRIORITY_NORMAL 1",
        "COMMAND_PRIORITY_BEFORE_LOW 1", "COMMAND_PRIORITY_BEFORE_LOW 0",
        "COMMAND_PRIORITY_LOW 0", "COMMAND_PRIORITY_LOW 1",
        "COMMAND_PRIORITY_BEFORE_EDITOR 1", "COMMAND_PRIORITY_BEFORE_EDITOR 0",
        "COMMAND_PRIORITY_EDITOR 0", "COMMAND_PRIORITY_EDITOR 1",
      ])
    mergeRegister(listeners)()
    calls = []
    try editor.dispatchCommand(command)
    #expect(calls.isEmpty)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete for synchronous updates"
    ))
  func discreteSynchronousUpdates() throws {
    let editor = try initEditor()
    var payloads: [UpdateListenerPayload] = []
    editor.registerUpdateListener { payloads.append($0) }
    let prevEditorState = editor.getEditorState()
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Sync update")))
    }
    #expect(try editor.read(.latest) { try getRoot().getTextContent() } == "Sync update")
    #expect(payloads.count == 1)
    // `computeUpdateListenerPayload(editor, prevEditorState, false)`.
    let payload = try #require(payloads.first)
    #expect(payload.prevEditorState === prevEditorState)
    #expect(payload.editorState === editor.getEditorState())
    #expect(payload.dirtyElements.keysArray.sorted() == ["1", "root"])
    #expect(payload.dirtyLeaves.sorted() == ["2"])
    #expect(payload.normalizedNodes.size == 0)
    #expect(payload.tags.size == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete after a non-discrete update to flush the entire queue"
    ))
  func discreteFlushesQueue() throws {
    let headless = try createTestEditor()
    var onUpdate = 0
    headless.registerUpdateListener { _ in onUpdate += 1 }
    try headless.update {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Async update")))
    }
    try headless.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Sync update")))
    }
    #expect(
      try headless.read(.latest) { try getRoot().getTextContent() }
        == "Async update\n\nSync update")
    #expect(onUpdate == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete after a non-discrete setEditorState to flush the entire queue"
    ))
  func discreteAfterSetEditorState() throws {
    let editor = try initEditor()
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Async update")))
    }
    let headless = try createEditor(
      CreateEditorArgs(
        editorState: editor.getEditorState(), namespace: "test", nodes: defaultTestNodes,
        onError: { throw $0 }, theme: [:]))
    try headless.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Sync update")))
    }
    #expect(
      try headless.read(.latest) { try getRoot().getTextContent() }
        == "Async update\n\nSync update")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete in a nested update to flush the entire queue"
    ))
  func discreteInNestedUpdate() throws {
    let editor = try initEditor()
    var onUpdate = 0
    editor.registerUpdateListener { _ in onUpdate += 1 }
    try editor.update {
      try getRoot().append(try createParagraphNode().append(try createTextNode("Async update")))
      try editor.update(discrete: true) {
        try getRoot().append(try createParagraphNode().append(try createTextNode("Sync update")))
      }
    }
    #expect(
      try editor.read(.latest) { try getRoot().getTextContent() } == "Async update\n\nSync update")
    #expect(onUpdate == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can read in a nested update"
    ))
  func readInNestedUpdate() throws {
    let editor = try initEditor(onError: { throw $0 })
    var shapeAfterRead = ""
    try update(editor) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("foo")))
      try editor.read {}
      shapeAfterRead = try shape(editor)
      try editor.update {
        try getRoot().append(try createParagraphNode().append(try createTextNode("bar")))
      }
    }
    #expect(shapeAfterRead == "<paragraph>[foo]</paragraph>")
    #expect(try shape(editor) == "<paragraph>[foo]</paragraph><paragraph>[bar]</paragraph>")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > reconciles state without root element"
    ))
  func reconcilesWithoutRootElement() throws {
    let editor = try createTestEditor()
    let state = try editor.parseEditorState(
      #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hello world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#
    )
    try editor.setEditorState(state)
    #expect(try editor._editorState.toJSON() == state.toJSON())
    #expect(editor._pendingEditorState == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > should work correctly"
    ))
  func nodeReplacementWorks() throws {
    let onError = Calls()
    let editor = try createReplacementEditor(testTextReplacement(), onError)
    try update(editor) {
      let paragraph = try createParagraphNode()
      let text = try createTextNode("123")
      try getRoot().append(paragraph)
      try paragraph.append(text)
      #expect(text is TestTextNode)
      #expect(try text.getTextContent() == "123")
    }
    #expect(onError.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > should fail if node keys are re-used"
    ))
  func nodeReplacementRejectsReusedKeys() throws {
    let onError = Calls()
    let editor = try createReplacementEditor(testTextReplacement(reuseKey: true), onError)
    var reached = false
    try update(editor) {
      _ = try createTextNode("123")
      reached = true
    }
    #expect(reached == false)
    let message = "\(try #require(onError.errors.first))"
    #expect(message.contains(/TestTextNode.*re-use key.*TextNode/))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > node transform to the nodes specified by \"replace\" should not be applied to the nodes specified by \"with\" when \"withKlass\" is not specified"
    ))
  func replaceTransformSkippedWithoutWithKlass() throws {
    let onError = Calls()
    var warnings: [String] = []
    let editor = try withWarningHandler({ warnings.append($0) }) {
      try createReplacementEditor(testTextReplacement(withKlass: false), onError)
    }
    #expect(warnings == [replaceWithoutWithKlass("TextNode")])
    var transforms = 0
    let removeTransform = try editor.registerNodeTransform(TextNode.self) { _ in transforms += 1 }
    try update(editor) {
      let paragraph = try createParagraphNode()
      let text = try createTextNode("123")
      try getRoot().append(paragraph)
      try paragraph.append(text)
      #expect(text is TestTextNode)
      #expect(try text.getTextContent() == "123")
    }
    #expect(transforms == 0)
    #expect(onError.count == 0)
    removeTransform()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > node transform to the nodes specified by \"replace\" should be applied also to the nodes specified by \"with\" when \"withKlass\" is specified"
    ))
  func replaceTransformAppliedWithWithKlass() throws {
    let onError = Calls()
    let editor = try createReplacementEditor(testTextReplacement(), onError)
    var transforms = 0
    let removeTransform = try editor.registerNodeTransform(TextNode.self) { _ in transforms += 1 }
    try update(editor) {
      let paragraph = try createParagraphNode()
      let text = try createTextNode("123")
      try getRoot().append(paragraph)
      try paragraph.append(text)
      #expect(text is TestTextNode)
    }
    #expect(transforms == 1)
    #expect(onError.count == 0)
    removeTransform()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > applies the replacement when deserializing a base node type via parseEditorState (#8640 regression)"
    ))
  func replacementAppliedOnParse() throws {
    let onError = Calls()
    let editor = try createReplacementEditor(
      [
        .klass(CustomParagraphNode.self),
        .replacement(
          LexicalNodeReplacement(
            replace: ParagraphNode.self, with: { _ in try CustomParagraphNode() },
            withKlass: CustomParagraphNode.self)),
      ], onError)
    try editor.setEditorState(try editor.parseEditorState(paragraphJSON()))
    try editor.read { () throws in
      let paragraph = try getRoot().getFirstChild()
      #expect(paragraph is CustomParagraphNode)
      #expect(paragraph?.getType() == "custom-paragraph")
    }
    #expect(onError.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > $create resolves withKlass directly, and still honors a `with` without one"
    ))
  func createResolvesWithKlass() throws {
    let resolved = try createReplacementEditor(
      [
        .klass(ResolvedParagraph.self),
        .replacement(
          LexicalNodeReplacement(
            replace: ParagraphNode.self, with: { _ in try ResolvedParagraph() },
            withKlass: ResolvedParagraph.self)),
      ], Calls())
    try resolved.update(discrete: true) { () throws in
      #expect(try create(ParagraphNode.self) is ResolvedParagraph)
    }
    var warnings: [String] = []
    let legacy = try withWarningHandler({ warnings.append($0) }) {
      try createReplacementEditor(
        [
          .klass(LegacyParagraph.self),
          .replacement(
            LexicalNodeReplacement(
              replace: ParagraphNode.self, with: { _ in try LegacyParagraph() })),
        ], Calls())
    }
    #expect(warnings == [replaceWithoutWithKlass("ParagraphNode")])
    try legacy.update(discrete: true) { () throws in
      #expect(try create(ParagraphNode.self) is LegacyParagraph)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > applies the replacement on import for `with` without `withKlass`"
    ))
  func replacementAppliedOnImportWithoutWithKlass() throws {
    let onError = Calls()
    var warnings: [String] = []
    let editor = try withWarningHandler({ warnings.append($0) }) {
      try createReplacementEditor(
        [
          .klass(CustomParagraphNode.self),
          .replacement(
            LexicalNodeReplacement(
              replace: ParagraphNode.self, with: { _ in try CustomParagraphNode() })),
        ], onError)
    }
    #expect(warnings == [replaceWithoutWithKlass("ParagraphNode")])
    try editor.setEditorState(try editor.parseEditorState(paragraphJSON()))
    try editor.read { () throws in
      let paragraph = try getRoot().getFirstChild()
      #expect(paragraph is CustomParagraphNode)
      #expect(paragraph?.getType() == "custom-paragraph")
    }
    #expect(onError.count == 0)
  }
}

/// The two committed states the `setEditorState` tag tests start from:
/// returns the "historic" one, leaving "current" committed.
private func historicAndCurrent(_ editor: Editor) throws -> EditorState {
  try editor.update(discrete: true) {
    try getRoot().append(try createParagraphNode().append(try createTextNode("historic")))
  }
  let historicState = editor.getEditorState()
  try editor.update(discrete: true) {
    let root = try getRoot()
    try root.clear()
    try root.append(try createParagraphNode().append(try createTextNode("current")))
  }
  return historicState
}
