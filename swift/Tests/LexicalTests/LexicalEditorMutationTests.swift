import LexicalTesting
import Testing

@testable import Lexical

// Port of the mutation listener tests of src/__tests__/unit/LexicalEditor.test.tsx.
// Upstream mounts each editor in a React root; these editors are mounted on a
// `TestRootElement`, so commits run the reconciler, which is what reports
// mutations.

/// `init(onError, nodes, onWarn)`, mounted.
private func initMounted(
  onError: ErrorHandler? = nil, nodes: [LexicalNodeConfig] = [], onWarn: ErrorHandler? = nil
) throws -> Editor {
  let editor = try initEditor(onError: onError, nodes: nodes, onWarn: onWarn)
  try editor.setRootElement(TestRootElement())
  return editor
}

/// `await editor.update(fn)`.
private func awaitUpdate(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(fn)
  drainMicrotasksOnly()
}

/// The calls a mutation listener received.
private final class MutationCalls {
  var calls: [(mutations: [NodeKey: NodeMutation], tags: Set<UpdateTag>)] = []
  var count: Int { calls.count }
  func listener(_ mutations: JSMap<NodeKey, NodeMutation>, _ payload: MutationListenerPayload) {
    var map: [NodeKey: NodeMutation] = [:]
    for (key, mutation) in mutations {
      map[key] = mutation
    }
    calls.append((map, Set(payload.updateTags)))
  }
  subscript(_ index: Int) -> [NodeKey: NodeMutation] { calls[index].mutations }
}

private func className(_ klass: ObjectIdentifier, _ editor: Editor) -> String {
  for (_, registeredNode) in editor._nodes where ObjectIdentifier(registeredNode.klass) == klass {
    return "\(registeredNode.klass)"
  }
  return "?"
}

/// An update listener payload in comparable form.
private struct Payload: Equatable {
  var dirtyElements: [NodeKey: Bool]
  var dirtyLeaves: Set<NodeKey>
  var mutatedNodes: [String: [NodeKey: NodeMutation]]
  var normalizedNodes: Set<NodeKey>
  var tags: Set<UpdateTag>
  var editorState: ObjectIdentifier
  var prevEditorState: ObjectIdentifier

  init(_ payload: UpdateListenerPayload, _ editor: Editor) {
    dirtyElements = [:]
    for (key, value) in payload.dirtyElements {
      dirtyElements[key] = value
    }
    dirtyLeaves = Set(payload.dirtyLeaves)
    mutatedNodes = [:]
    for (klass, mutations) in payload.mutatedNodes ?? MutatedNodes() {
      var map: [NodeKey: NodeMutation] = [:]
      for (key, mutation) in mutations {
        map[key] = mutation
      }
      mutatedNodes[className(klass, editor)] = map
    }
    normalizedNodes = Set(payload.normalizedNodes)
    tags = Set(payload.tags)
    editorState = ObjectIdentifier(payload.editorState)
    prevEditorState = ObjectIdentifier(payload.prevEditorState)
  }

  init(
    dirtyElements: [NodeKey: Bool], dirtyLeaves: Set<NodeKey>,
    mutatedNodes: [String: [NodeKey: NodeMutation]], editorState: EditorState,
    prevEditorState: EditorState
  ) {
    self.dirtyElements = dirtyElements
    self.dirtyLeaves = dirtyLeaves
    self.mutatedNodes = mutatedNodes
    self.normalizedNodes = []
    self.tags = []
    self.editorState = ObjectIdentifier(editorState)
    self.prevEditorState = ObjectIdentifier(prevEditorState)
  }
}

/// `computeUpdateListenerPayload(editor, prevEditorState, true)`: every
/// node of the new state dirty, nodes of the previous state destroyed and
/// nodes of the new one created or updated.
private func computeUpdateListenerPayload(
  _ editor: Editor, _ prevEditorState: EditorState
) throws
  -> Payload
{
  var dirtyElements: [NodeKey: Bool] = [:]
  var dirtyLeaves = Set<NodeKey>()
  var mutatedNodes: [String: [NodeKey: NodeMutation]] = [:]
  for (key, node) in prevEditorState._nodeMap {
    mutatedNodes["\(type(of: node))", default: [:]][key] = "destroyed"
  }
  let editorState = editor.getEditorState()
  for (key, node) in editorState._nodeMap {
    if node is ElementNode {
      dirtyElements[key] = true
    } else {
      dirtyLeaves.insert(key)
    }
    mutatedNodes["\(type(of: node))", default: [:]][key] =
      prevEditorState._nodeMap.has(key) ? "updated" : "created"
  }
  // This looks like a corner case in element tracking where dirtyElements
  // has keys that were destroyed!
  for (klass, mutations) in mutatedNodes where klass != "TextNode" {
    for (key, mutation) in mutations where mutation == "destroyed" {
      dirtyElements[key] = true
    }
  }
  return Payload(
    dirtyElements: dirtyElements, dirtyLeaves: dirtyLeaves, mutatedNodes: mutatedNodes,
    editorState: editorState, prevEditorState: prevEditorState)
}

@Suite(.serialized) struct LexicalEditorMutationTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener"))
  func mutationListener() throws {
    let editor = try initMounted()
    let paragraphNodeMutations = MutationCalls()
    let textNodeMutations = MutationCalls()
    var onUpdate: [Payload] = []
    editor.registerUpdateListener { onUpdate.append(Payload($0, editor)) }
    try editor.registerMutationListener(ParagraphNode.self, paragraphNodeMutations.listener)
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    var paragraphKeys: [NodeKey] = []
    var textNodeKeys: [NodeKey] = []
    var prevEditorState = editor.getEditorState()

    // No await intentional (batch with next)
    try editor.update {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("foo")
      try root.append(paragraph)
      try paragraph.append(textNode)
      paragraphKeys.append(paragraph.getKey())
      textNodeKeys.append(textNode.getKey())
    }
    try awaitUpdate(editor) {
      let textNode = try #require(try getNodeByKey(textNodeKeys[0]) as? TextNode)
      let textNode2 = try createTextNode("bar").toggleFormat("bold")
      let textNode3 = try createTextNode("xyz").toggleFormat("italic")
      try textNode.insertAfter(textNode2)
      try textNode2.insertAfter(textNode3)
      textNodeKeys.append(textNode2.getKey())
      textNodeKeys.append(textNode3.getKey())
    }
    #expect(onUpdate.count == 1)
    #expect(onUpdate.last == (try computeUpdateListenerPayload(editor, prevEditorState)))

    prevEditorState = editor.getEditorState()
    try awaitUpdate(editor) { try getRoot().clear() }
    #expect(onUpdate.count == 2)
    #expect(onUpdate.last == (try computeUpdateListenerPayload(editor, prevEditorState)))

    prevEditorState = editor.getEditorState()
    try awaitUpdate(editor) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      paragraphKeys.append(paragraph.getKey())
      // Created and deleted in the same update (not attached to node)
      textNodeKeys.append(try createTextNode("zzz").getKey())
      try root.append(paragraph)
    }
    #expect(onUpdate.count == 3)
    #expect(onUpdate.last == (try computeUpdateListenerPayload(editor, prevEditorState)))

    #expect(paragraphNodeMutations.count == 3)
    #expect(textNodeMutations.count == 2)
    #expect(paragraphNodeMutations[0] == [paragraphKeys[0]: "created"])
    #expect(paragraphNodeMutations[1][paragraphKeys[0]] == "destroyed")
    #expect(paragraphNodeMutations[2] == [paragraphKeys[1]: "created"])
    #expect(
      textNodeMutations[0] == [
        textNodeKeys[0]: "created", textNodeKeys[1]: "created", textNodeKeys[2]: "created",
      ])
    #expect(
      textNodeMutations[1] == [
        textNodeKeys[0]: "destroyed", textNodeKeys[1]: "destroyed", textNodeKeys[2]: "destroyed",
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener on newly initialized editor"
    ))
  func mutationListenerOnNewEditor() throws {
    let editor = try createEditor()
    let textNodeMutations = MutationCalls()
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    #expect(textNodeMutations.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener with setEditorState"
    ))
  func mutationListenerWithSetEditorState() throws {
    let editor = try initMounted()
    try awaitUpdate(editor) { try getRoot().append(createParagraphNode()) }
    let initialEditorState = editor.getEditorState()
    let textNodeMutations = MutationCalls()
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    var textNodeKeys: [NodeKey] = []
    try awaitUpdate(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let textNode1 = try createTextNode("foo")
      try paragraph.append(textNode1)
      textNodeKeys.append(textNode1.getKey())
    }
    let fooEditorState = editor.getEditorState()
    try editor.setEditorState(initialEditorState)
    // This line should have no effect on the mutation listeners
    let parsedFooEditorState = try editor.parseEditorState(try fooEditorState.toJSON().stringify())
    try awaitUpdate(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let textNode2 = try createTextNode("bar").toggleFormat("bold")
      let textNode3 = try createTextNode("xyz").toggleFormat("italic")
      try paragraph.append(textNode2, textNode3)
      textNodeKeys.append(contentsOf: [textNode2.getKey(), textNode3.getKey()])
    }
    try editor.setEditorState(parsedFooEditorState)

    #expect(textNodeMutations.count == 4)
    #expect(textNodeMutations[0] == [textNodeKeys[0]: "created"])
    #expect(textNodeMutations[1] == [textNodeKeys[0]: "destroyed"])
    #expect(textNodeMutations[2] == [textNodeKeys[1]: "created", textNodeKeys[2]: "created"])
    // +1 newly generated key by parseEditorState
    #expect(textNodeMutations[3].count == 3)
    #expect(textNodeMutations[3][textNodeKeys[1]] == "destroyed")
    #expect(textNodeMutations[3][textNodeKeys[2]] == "destroyed")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener set for original node should work with the replaced node"
    ))
  func mutationListenerForReplacedOriginal() throws {
    let editor = try initMounted(nodes: textReplacement())
    let textNodeMutations = MutationCalls()
    let textNodeMutationsB = MutationCalls()
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    var textNodeKeys: [NodeKey] = []
    // No await intentional (batch with next)
    try editor.update {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("foo")
      try getRoot().append(paragraph)
      try paragraph.append(textNode)
      textNodeKeys.append(textNode.getKey())
    }
    try awaitUpdate(editor) {
      let textNode = try #require(try getNodeByKey(textNodeKeys[0]) as? TextNode)
      let textNode2 = try createTextNode("bar").toggleFormat("bold")
      let textNode3 = try createTextNode("xyz").toggleFormat("italic")
      try textNode.insertAfter(textNode2)
      try textNode2.insertAfter(textNode3)
      textNodeKeys.append(textNode2.getKey())
      textNodeKeys.append(textNode3.getKey())
    }
    try editor.registerMutationListener(TextNode.self, textNodeMutationsB.listener)
    try awaitUpdate(editor) { try getRoot().clear() }
    try awaitUpdate(editor) {
      let paragraph = try createParagraphNode()
      // Created and deleted in the same update (not attached to node)
      textNodeKeys.append(try createTextNode("zzz").getKey())
      try getRoot().append(paragraph)
    }
    let created = [
      textNodeKeys[0]: "created", textNodeKeys[1]: "created", textNodeKeys[2]: "created",
    ]
    let destroyed = [
      textNodeKeys[0]: "destroyed", textNodeKeys[1]: "destroyed", textNodeKeys[2]: "destroyed",
    ]
    #expect(textNodeMutations.count == 2)
    #expect(textNodeMutationsB.count == 2)
    #expect(textNodeMutations[0] == created)
    #expect(textNodeMutations.calls[0].tags.isEmpty)
    #expect(textNodeMutations[1] == destroyed)
    #expect(textNodeMutations.calls[1].tags.isEmpty)
    #expect(textNodeMutationsB[0] == created)
    #expect(textNodeMutationsB.calls[0].tags == ["registerMutationListener"])
    #expect(textNodeMutationsB[1] == destroyed)
    #expect(textNodeMutationsB.calls[1].tags.isEmpty)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener should work with the replaced node"
    ))
  func mutationListenerForReplacement() throws {
    let editor = try initMounted(nodes: textReplacement())
    let textNodeMutations = MutationCalls()
    let textNodeMutationsB = MutationCalls()
    try editor.registerMutationListener(TestTextNode.self, textNodeMutations.listener)
    var textNodeKeys: [NodeKey] = []
    try awaitUpdate(editor) {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("foo")
      try getRoot().append(paragraph)
      try paragraph.append(textNode)
      textNodeKeys.append(textNode.getKey())
    }
    try editor.registerMutationListener(TestTextNode.self, textNodeMutationsB.listener)
    #expect(textNodeMutations.count == 1)
    #expect(textNodeMutations[0] == [textNodeKeys[0]: "created"])
    #expect(textNodeMutations.calls[0].tags.isEmpty)
    #expect(textNodeMutationsB[0] == [textNodeKeys[0]: "created"])
    #expect(textNodeMutationsB.calls[0].tags == ["registerMutationListener"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listeners does not trigger when other node types are mutated"
    ))
  func mutationListenersByType() throws {
    let editor = try initMounted()
    let paragraphNodeMutations = MutationCalls()
    let textNodeMutations = MutationCalls()
    try editor.registerMutationListener(ParagraphNode.self, paragraphNodeMutations.listener)
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    try awaitUpdate(editor) { try getRoot().append(createParagraphNode()) }
    #expect(paragraphNodeMutations.count == 1)
    #expect(textNodeMutations.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listeners with normalization"
    ))
  func mutationListenersWithNormalization() throws {
    let editor = try initMounted()
    let textNodeMutations = MutationCalls()
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    var textNodeKeys: [NodeKey] = []
    try awaitUpdate(editor) {
      let paragraph = try createParagraphNode()
      let textNode1 = try createTextNode("foo")
      let textNode2 = try createTextNode("bar")
      textNodeKeys.append(contentsOf: [textNode1.getKey(), textNode2.getKey()])
      try getRoot().append(paragraph)
      try paragraph.append(textNode1, textNode2)
    }
    try awaitUpdate(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let textNode3 = try createTextNode("xyz").toggleFormat("bold")
      try paragraph.append(textNode3)
      textNodeKeys.append(textNode3.getKey())
    }
    try awaitUpdate(editor) {
      let textNode3 = try #require(try getNodeByKey(textNodeKeys[2]) as? TextNode)
      try textNode3.toggleFormat("bold")  // Normalize with foobar
    }
    #expect(textNodeMutations.count == 3)
    #expect(textNodeMutations[0] == [textNodeKeys[0]: "created"])
    #expect(textNodeMutations[1].count == 2)
    #expect(textNodeMutations[1][textNodeKeys[2]] == "created")
    #expect(textNodeMutations[2].count == 2)
    #expect(textNodeMutations[2][textNodeKeys[0]] == "updated")
    #expect(textNodeMutations[2][textNodeKeys[2]] == "destroyed")
  }

  // The table half of this test uses the table nodes for structure only:
  // TestElementNode stands in for the table, TestRowNode and TestCellNode
  // for its rows and cells, and a format change for `toggleHeaderStyle`.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation \"update\" listener"
    ))
  func mutationUpdateListener() throws {
    let editor = try initMounted(nodes: [.klass(TestRowNode.self), .klass(TestCellNode.self)])
    let paragraphNodeMutations = MutationCalls()
    let textNodeMutations = MutationCalls()
    try editor.registerMutationListener(ParagraphNode.self, paragraphNodeMutations.listener)
    try editor.registerMutationListener(TextNode.self, textNodeMutations.listener)
    var paragraphNodeKeys: [NodeKey] = []
    var textNodeKeys: [NodeKey] = []
    try awaitUpdate(editor) {
      let paragraph = try createParagraphNode()
      let textNode1 = try createTextNode("foo")
      textNodeKeys.append(textNode1.getKey())
      paragraphNodeKeys.append(paragraph.getKey())
      try getRoot().append(paragraph)
      try paragraph.append(textNode1)
    }
    #expect(paragraphNodeMutations.count == 1)
    #expect(textNodeMutations.count == 1)
    #expect(textNodeMutations[0].count == 1)
    #expect(paragraphNodeMutations[0].count == 1)
    // Change first text node's content.
    try awaitUpdate(editor) {
      let textNode1 = try #require(try getNodeByKey(textNodeKeys[0]) as? TextNode)
      try textNode1.setTextContent("Test")
    }
    // Append text node to paragraph.
    try awaitUpdate(editor) {
      let paragraphNode1 = try #require(try getNodeByKey(paragraphNodeKeys[0]) as? ParagraphNode)
      try paragraphNode1.append(createTextNode("foo"))
    }
    #expect(textNodeMutations.count == 3)
    // Show TextNode was updated when text content changed.
    #expect(textNodeMutations[1][textNodeKeys[0]] == "updated")
    #expect(paragraphNodeMutations.count == 2)
    // Show ParagraphNode was updated when new text node was appended.
    #expect(paragraphNodeMutations[1][paragraphNodeKeys[0]] == "updated")

    var tableCellKey = ""
    var tableRowKey = ""
    let tableCellMutations = MutationCalls()
    let tableRowMutations = MutationCalls()
    try editor.registerMutationListener(TestCellNode.self, tableCellMutations.listener)
    try editor.registerMutationListener(TestRowNode.self, tableRowMutations.listener)
    // Create Table
    try awaitUpdate(editor) {
      let tableCell = try TestCellNode()
      let tableRow = try TestRowNode()
      let table = try createTestElementNode()
      try tableRow.append(tableCell)
      try table.append(tableRow)
      try getRoot().append(table)
      tableRowKey = tableRow.getKey()
      tableCellKey = tableCell.getKey()
    }
    // Add New Table Cell To Row
    try awaitUpdate(editor) {
      let tableRow = try #require(try getNodeByKey(tableRowKey) as? TestRowNode)
      try tableRow.append(TestCellNode())
    }
    // Update Table Cell
    try awaitUpdate(editor) {
      let tableCell = try #require(try getNodeByKey(tableCellKey) as? TestCellNode)
      try tableCell.setFormat("center")
    }
    #expect(tableCellMutations.count == 3)
    // Show table cell is updated when header value changes.
    #expect(tableCellMutations[2][tableCellKey] == "updated")
    #expect(tableRowMutations.count == 2)
    // Show row is updated when a new child is added.
    #expect(tableRowMutations[1][tableRowKey] == "updated")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > allows using the same listener for multiple node types"
    ))
  func sameListenerForMultipleTypes() throws {
    let editor = try initMounted()
    let listener = MutationCalls()
    try editor.registerMutationListener(TextNode.self, listener.listener)
    try editor.registerMutationListener(ParagraphNode.self, listener.listener)
    var paragraphKey = ""
    var textNodeKey = ""
    try awaitUpdate(editor) {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("Hello world")
      paragraphKey = paragraph.getKey()
      textNodeKey = textNode.getKey()
      try getRoot().append(paragraph)
      try paragraph.append(textNode)
    }
    #expect(listener.count == 2)
    #expect(listener[0] == [textNodeKey: "created"])
    #expect(listener[1] == [paragraphKey: "created"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > calls mutation listener with initial state"
    ))
  func mutationListenerInitialState() throws {
    let mutationListenerA = MutationCalls()
    let mutationListenerB = MutationCalls()
    let mutationListenerC = MutationCalls()
    let editor = try initMounted()
    try editor.registerMutationListener(TextNode.self, mutationListenerA.listener)
    #expect(mutationListenerA.count == 0)
    try awaitUpdate(editor) {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("Hello world")))
    }
    #expect(mutationListenerA.count == 1)
    #expect(mutationListenerA.calls.last?.tags.contains("registerMutationListener") == false)
    try editor.registerMutationListener(TextNode.self, mutationListenerB.listener)
    try editor.registerMutationListener(
      TextNode.self, mutationListenerC.listener, skipInitialization: true)
    #expect(mutationListenerA.count == 1)
    #expect(mutationListenerB.count == 1)
    #expect(mutationListenerB.calls.last?.tags.contains("registerMutationListener") == true)
    #expect(mutationListenerC.count == 0)
    try awaitUpdate(editor) {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("Another update!")))
    }
    #expect(mutationListenerA.count == 2)
    #expect(mutationListenerB.count == 2)
    #expect(mutationListenerC.count == 1)
    for listener in [mutationListenerA, mutationListenerB, mutationListenerC] {
      #expect(listener.calls.last?.tags.contains("registerMutationListener") == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > still detects a runaway cascade that dispatches commands from a mutation listener"
    ))
  func runawayCascadeFromMutationListener() throws {
    let errors = Calls()
    let warnings = Calls()
    let editor = try initMounted(
      onError: { errors.record($0) }, onWarn: { warnings.record($0) })
    let noopCommand = createCommand("NOOP_COMMAND") as LexicalCommand<Void>
    var textKey = ""
    try editor.update(discrete: true) {
      let text = try createTextNode("a")
      try getRoot().clear().append(try createParagraphNode().append(text))
      textKey = text.getKey()
    }
    drainMicrotasks()
    let unregisterMutation = try editor.registerMutationListener(TextNode.self) { _, _ in
      _ = try editor.dispatchCommand(noopCommand)
    }
    // Unbounded: flips the text content again on every commit, forever.
    let unregisterUpdate = editor.registerUpdateListener { _ in
      try editor.update {
        if let text = try getNodeByKey(textKey) as? TextNode {
          try text.setTextContent(try text.getTextContent() == "a" ? "b" : "a")
        }
      }
    }
    try editor.update {
      if let text = try getNodeByKey(textKey) as? TextNode {
        try text.setTextContent("b")
      }
    }
    // Drain only microtasks: the runaway never yields to the event loop.
    drainMicrotasksOnly()
    unregisterUpdate()
    unregisterMutation()
    drainMicrotasks()
    #expect(warnings.count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > does not flush an update started by a commit listener until every listener of that commit has run (#7709)"
    ))
  func readDoesNotFlushCommitListenerUpdate() throws {
    let editor = try initMounted(onError: { throw $0 })
    try awaitUpdate(editor) {
      try getRoot().clear().append(try createParagraphNode().append(try createTextNode("first")))
    }
    var updates: [(EditorState, EditorState)] = []
    editor.registerUpdateListener { updates.append(($0.prevEditorState, $0.editorState)) }
    var appended = false
    try editor.registerMutationListener(
      ParagraphNode.self,
      { _, _ in
        if !appended {
          appended = true
          try editor.update {
            try getRoot().append(
              try createParagraphNode().append(try createTextNode("third")))
          }
        }
      }, skipInitialization: true)
    var reads: [String] = []
    try editor.registerMutationListener(
      ParagraphNode.self,
      { _, _ in reads.append(try editor.read { try getRoot().getTextContent() }) },
      skipInitialization: true)
    var warnings: [String] = []
    try withWarningHandler({ warnings.append($0) }) {
      try editor.update(discrete: true) {
        try getRoot().append(try createParagraphNode().append(try createTextNode("second")))
      }
      // The read must not commit the update started by the first mutation
      // listener.
      #expect(updates.count == 1)
      #expect(updates[0].1 === editor.getEditorState())
      #expect(reads == ["first\n\nsecond"])
      #expect(warnings.count == 1)
      #expect(warnings.first?.contains("editor.read()") == true)
      // The pending update commits afterwards, continuing from that state.
      drainMicrotasksOnly()
      #expect(updates.count == 2)
      #expect(updates[1].0 === updates[0].1)
      #expect(updates[1].1 === editor.getEditorState())
      #expect(reads == ["first\n\nsecond", "first\n\nsecond\n\nthird"])
      #expect(warnings.count == 1)
    }
  }
}

/// A replacement of `TextNode` by `TestTextNode`, keyed anew.
private func textReplacement() -> [LexicalNodeConfig] {
  [
    .klass(TestTextNode.self),
    .replacement(
      LexicalNodeReplacement(
        replace: TextNode.self,
        with: { node in try TestTextNode(try (node as! TextNode).getTextContent()) },
        withKlass: TestTextNode.self)),
  ]
}

/// Stands in for TableRowNode.
private final class TestRowNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestRowNode.self, type: "test_row", extends: ElementNode.self)
  }
}

/// Stands in for TableCellNode.
private final class TestCellNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestCellNode.self, type: "test_cell", extends: ElementNode.self)
  }
}
