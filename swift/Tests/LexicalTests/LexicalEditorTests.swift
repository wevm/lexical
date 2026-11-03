import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalEditor.test.tsx. Upstream mounts each
// editor in a React root; these editors are headless, so expected HTML is
// asserted as the model it renders, and the DOM half of a test waits for the
// views. The mutation listener tests, which need the reconciler, mount their
// editors (LexicalEditorMutationTests.swift).

/// `init(onError, nodes, onWarn)`: upstream's test editor, whose default
/// `onError` swallows errors (`vi.fn()`).
func initEditor(
  onError: ErrorHandler? = nil, nodes: [LexicalNodeConfig] = [], onWarn: ErrorHandler? = nil
) throws -> Editor {
  resetRandomKey()
  drainMicrotasks()
  return try createEditor(
    CreateEditorArgs(
      namespace: "", nodes: defaultTestNodes + nodes, onError: onError ?? { _ in },
      onWarn: onWarn, theme: [:]))
}

/// A counter standing in for `vi.fn()`.
final class Calls {
  var count = 0
  var errors: [any Error] = []
  func record() { count += 1 }
  func record(_ error: any Error) {
    count += 1
    errors.append(error)
  }
}

private func textContents() throws -> [String] {
  try getRoot().getAllTextNodes().map { try $0.getTextContent() }
}

@Suite(.serialized) struct LexicalEditorTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > registerNodeTransform > Calls the RootNode transform last on every update"
    ))
  func rootTransformRunsLast() throws {
    let editor = try initEditor(onError: { throw $0 })
    var events: [String] = []
    let transform = { (node: LexicalNode) in
      events.append("transform \(node.getType()) \(node.getKey())")
    }
    _ = try editor.registerNodeTransform(RootNode.self, transform)
    _ = try editor.registerNodeTransform(ParagraphNode.self, transform)
    _ = try editor.registerNodeTransform(TextNode.self, transform)
    _ = try editor.registerNodeTransform(ParagraphNode.self) { node in
      if let lastChild = try node.getLastChild() as? TextNode,
        try lastChild.getTextContent() == "[third]"
      {
        try node.append(try createTextNode("fourth").setMode("token"))
      }
    }
    drainMicrotasks()
    events = []
    var paragraphKey: NodeKey = ""
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      paragraphKey = paragraph.getKey()
      try getRoot().clear().append(
        try paragraph.append(
          try createTextNode("first").setMode("token"),
          try createTextNode("second").setMode("token")))
    }
    var keys = try editor.read { try getRoot().getAllTextNodes().map { $0.getKey() } }
    #expect(
      events == [
        "transform text \(keys[0])", "transform text \(keys[1])",
        "transform paragraph \(paragraphKey)", "transform root root",
      ])
    events = []
    _ = try editor.registerNodeTransform(TextNode.self) { node in
      let textContent = try node.getTextContent()
      if textContent.hasPrefix("[") { return }
      _ = try node.setTextContent("[\(textContent)]")
    }
    drainMicrotasks()
    keys = try editor.read { try getRoot().getAllTextNodes().map { $0.getKey() } }
    #expect(
      events == [
        "transform text \(keys[0])", "transform text \(keys[1])", "transform text \(keys[0])",
        "transform text \(keys[1])", "transform root root",
      ])
    #expect(try editor.read(textContents) == ["[first]", "[second]"])
    events = []
    try update(editor) {
      for node in try getRoot().getAllTextNodes() {
        let text = try node.getTextContent()
        _ = try node.setTextContent(":\(text.dropFirst().dropLast()):")
      }
      try (try getNodeByKey(paragraphKey) as! ParagraphNode).append(
        try createTextNode("third").setMode("token"))
    }
    keys = try editor.read { try getRoot().getAllTextNodes().map { $0.getKey() } }
    #expect(
      events == [
        "transform text \(keys[0])", "transform text \(keys[1])", "transform text \(keys[2])",
        "transform text \(keys[0])", "transform text \(keys[1])", "transform text \(keys[2])",
        "transform paragraph \(paragraphKey)", "transform root root",
        "transform text \(keys[3])", "transform text \(keys[2])", "transform text \(keys[3])",
        "transform paragraph \(paragraphKey)", "transform root root",
      ])
    #expect(try editor.read(textContents) == ["[:first:]", "[:second:]", "[third]", "[fourth]"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > Can read the editor state"
    ))
  func readEditorState() throws {
    // The DOM half (`$getNearestNodeFromDOMNode`) waits for the views.
    let editor = try initEditor(onError: { throw $0 })
    #expect(try editor.read { try getRoot().getTextContent() } == "")
    #expect(try editor.read { try getEditor() } === editor)
    let onUpdate = Calls()
    try editor.update(onUpdate: { onUpdate.record() }) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("This works!"))
    }
    #expect(onUpdate.count == 0)
    #expect(try editor.read { try getRoot().getTextContent() } == "This works!")
    #expect(onUpdate.count == 1)
    drainMicrotasks()
    #expect(onUpdate.count == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > runs transforms the editor state"
    ))
  func readRunsTransforms() throws {
    let editor = try initEditor(onError: { throw $0 })
    #expect(try editor.read { try getRoot().getTextContent() } == "")
    _ = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "This works!" {
        _ = try node.replace(try createTextNode("Transforms work!"))
      }
    }
    let onUpdate = Calls()
    try editor.update(onUpdate: { onUpdate.record() }) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("This works!"))
    }
    #expect(onUpdate.count == 0)
    #expect(try editor.read { try getRoot().getTextContent() } == "Transforms work!")
    #expect(onUpdate.count == 1)
    drainMicrotasks()
    #expect(onUpdate.count == 1)
    #expect(try editor.read { try getRoot().getTextContent() } == "Transforms work!")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > can be nested in an update or read"
    ))
  func readNestedInUpdate() throws {
    let editor = try initEditor(onError: { throw $0 })
    try editor.update { () throws in
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      try root.append(paragraph)
      try paragraph.append(try createTextNode("This works!"))
      try editor.read { () throws in
        #expect(try getRoot().getTextContent() == "This works!")
      }
      try editor.read { () throws in
        try editor.update { () throws in
          #expect(try getRoot().getTextContent() == "This works!")
        }
      }
      expectThrows {
        try root.append(try createParagraphNode().append(try createTextNode("update-read-update")))
      }
    }
    try editor.read { () throws in
      try editor.read { () throws in
        #expect(try getRoot().getTextContent() == "This works!")
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Should handle nested updates in the correct sequence"
    ))
  func nestedUpdateSequence() throws {
    let editor = try initEditor()
    let onUpdate = Calls()
    var log: [String] = []
    _ = editor.registerUpdateListener { _ in onUpdate.record() }
    try editor.update {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("This works!"))
    }
    try editor.update(onUpdate: { log.append("D1") }) {
      log.append("A1")
      try getRoot().markDirty()
      try editor.update(onUpdate: { log.append("E1") }) {
        log.append("B1")
        try editor.update(onUpdate: { log.append("F1") }) { log.append("C1") }
      }
    }
    drainMicrotasks()
    #expect(onUpdate.count == 1)
    #expect(log == ["A1", "B1", "C1", "D1", "E1", "F1"])
    log = []
    try editor.update(onUpdate: {
      log.append("B2")
      try editor.update(onUpdate: { log.append("F2") }) {
        try setCompositionKey("root")
        log.append("D2")
      }
      log.append("C2")
      try editor.update(onUpdate: { log.append("G2") }) { log.append("E2") }
    }) {
      log.append("A2")
      try getRoot().markDirty()
    }
    drainMicrotasks()
    #expect(log == ["A2", "B2", "C2", "D2", "E2", "F2", "G2"])
    log = []
    _ = try editor.registerNodeTransform(TextNode.self) { _ in
      log.append("TextTransform A3")
      try editor.update(onUpdate: { log.append("TextTransform C3") }) {
        log.append("TextTransform B3")
      }
    }
    drainMicrotasks()
    #expect(log == ["TextTransform A3", "TextTransform B3", "TextTransform C3"])
    log = []
    try editor.update(onUpdate: { log.append("B3") }) {
      log.append("A3")
      try getRoot().getLastDescendant()!.markDirty()
    }
    drainMicrotasks()
    #expect(log == ["A3", "TextTransform A3", "TextTransform B3", "B3", "TextTransform C3"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > nested update after selection update triggers exactly 1 update"
    ))
  func nestedUpdateAfterSelection() throws {
    // `computeUpdateListenerPayload(editor, prevEditorState, false)`: every
    // node is dirty, nothing was normalized, and there are no tags.
    let editor = try initEditor()
    var payloads: [UpdateListenerPayload] = []
    _ = editor.registerUpdateListener { payloads.append($0) }
    let prevEditorState = editor.getEditorState()
    try editor.update {
      try setSelection(createRangeSelection())
      try editor.update {
        try getRoot().append(try createParagraphNode().append(try createTextNode("Sync update")))
      }
    }
    drainMicrotasks()
    #expect(try editor.read(.latest) { try getRoot().getTextContent() } == "Sync update")
    #expect(payloads.count == 1)
    let payload = try #require(payloads.first)
    #expect(payload.prevEditorState === prevEditorState)
    #expect(payload.editorState === editor.getEditorState())
    #expect(payload.dirtyElements.keysArray.sorted() == ["1", "root"])
    #expect(payload.dirtyLeaves.array == ["2"])
    #expect(payload.normalizedNodes.isEmpty)
    #expect(payload.tags.isEmpty)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > update does not call onUpdate callback when no dirty nodes"
    ))
  func noOnUpdateWithoutDirtyNodes() throws {
    let editor = try initEditor()
    let calls = Calls()
    try editor.update(onUpdate: { calls.record() }) {}
    #expect(calls.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on the other"
    ))
  func threeDependentTransforms() throws {
    let editor = try initEditor()
    let italics = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "foo", try node.hasFormat("bold"),
        try !node.hasFormat("italic")
      {
        _ = try node.toggleFormat("italic")
      }
    }
    let bold = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "foo", try !node.hasFormat("bold") {
        _ = try node.toggleFormat("bold")
      }
    }
    let underline = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "foo", try node.hasFormat("bold"),
        try !node.hasFormat("underline")
      {
        _ = try node.toggleFormat("underline")
      }
    }
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("foo"))
    }
    italics()
    bold()
    underline()
    #expect(try shape(editor) == "<paragraph>[foo]</paragraph>")
    #expect(
      try editor.read { try (getRoot().getFirstDescendant() as! TextNode).getFormat() }
        == IS_BOLD | IS_ITALIC | IS_UNDERLINE)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on the other (2)"
    ))
  func threeDependentTransforms2() throws {
    let editor = try initEditor()
    var skipFirst = true
    let paragraphTransform = try editor.registerNodeTransform(ParagraphNode.self) { paragraph in
      if skipFirst {
        skipFirst = false
        return
      }
      if try paragraph.isEmpty() {
        try paragraph.append(try createTextNode("foo"))
      }
    }
    let bold = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "foo", try !node.hasFormat("bold") {
        _ = try node.toggleFormat("bold")
      }
    }
    let italics = try editor.registerNodeTransform(ParagraphNode.self) { paragraph in
      if let child = try paragraph.getLastDescendant() as? TextNode, try child.hasFormat("bold"),
        try !child.hasFormat("italic")
      {
        _ = try child.toggleFormat("italic")
      }
    }
    try update(editor) { try getRoot().append(try createParagraphNode()) }
    try update(editor) { try getRoot().getFirstChild()!.markDirty() }
    paragraphTransform()
    bold()
    italics()
    #expect(try shape(editor) == "<paragraph>[foo]</paragraph>")
    #expect(
      try editor.read { try (getRoot().getFirstDescendant() as! TextNode).getFormat() }
        == IS_BOLD | IS_ITALIC)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on previously merged text content"
    ))
  func transformsOnMergedText() throws {
    let editor = try initEditor()
    var hasRun = [false, false, false]
    let splitAndPad = { (node: TextNode) throws in
      let parts = try node.splitText(2)
      _ = try parts[0].insertBefore(try createTextNode(""))
      _ = try parts[1].insertAfter(try createTextNode(""))
      _ = try parts[1].insertAfter(try createTextNode("!"))
      _ = try parts[1].insertAfter(try createTextNode(""))
    }
    let foo = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "Foo", !hasRun[0] {
        try splitAndPad(node)
        hasRun[0] = true
      }
    }
    let megaFoo = try editor.registerNodeTransform(ParagraphNode.self) { paragraph in
      if let child = try paragraph.getFirstChild() as? TextNode,
        try child.getTextContent() == "Foo!",
        !hasRun[1]
      {
        try splitAndPad(child)
        hasRun[1] = true
      }
    }
    let boldFoo = try editor.registerNodeTransform(TextNode.self) { node in
      if try node.getTextContent() == "Foo!!", !hasRun[2] {
        _ = try node.toggleFormat("bold")
        let parts = try node.splitText(2)
        _ = try parts[0].insertBefore(try createTextNode(""))
        _ = try parts[1].insertAfter(try createTextNode(""))
        hasRun[2] = true
      }
    }
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("Foo"))
    }
    foo()
    megaFoo()
    boldFoo()
    #expect(try shape(editor) == "<paragraph>[Foo!!]</paragraph>")
    #expect(
      try editor.read { try (getRoot().getFirstDescendant() as! TextNode).getFormat() } == IS_BOLD)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > text transform runs when node is removed"
    ))
  func textTransformOnRemove() throws {
    let editor = try initEditor()
    let executeTransform = Calls()
    var hasBeenRemoved = false
    let remove = try editor.registerNodeTransform(TextNode.self) { _ in
      if hasBeenRemoved { executeTransform.record() }
    }
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(
        try createTextNode("Foo").toggleUnmergeable(), try createTextNode("Bar").toggleUnmergeable()
      )
    }
    try update(editor) {
      try getRoot().getLastDescendant()!.remove()
      hasBeenRemoved = true
    }
    #expect(executeTransform.count == 1)
    remove()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms only run on nodes that were explicitly marked as dirty"
    ))
  func transformsOnlyOnDirtyNodes() throws {
    let editor = try initEditor()
    var counting = false
    let paragraphCalls = Calls()
    let textCalls = Calls()
    let removeParagraph = try editor.registerNodeTransform(ParagraphNode.self) { _ in
      if counting { paragraphCalls.record() }
    }
    let removeText = try editor.registerNodeTransform(TextNode.self) { _ in
      if counting { textCalls.record() }
    }
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("Foo"))
    }
    try update(editor) {
      let paragraph = try getRoot().getFirstChild() as! ParagraphNode
      _ = try (try paragraph.getFirstChild() as! TextNode).getWritable()
      counting = true
    }
    #expect(paragraphCalls.count == 0)
    #expect(textCalls.count == 1)
    removeParagraph()
    removeText()
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on remove"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on replace"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on insertBefore"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on insertAfter"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on splitText"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on append"
    ),
    arguments: [
      ("remove", [2, 1, 2, 1, 1, 1]), ("replace", [2, 2, 2, 2, 1, 2]),
      ("insertBefore", [2, 2, 2, 2, 2, 1]), ("insertAfter", [2, 2, 2, 1, 2, 2]),
      ("splitText", [2, 1, 2, 1, 1, 1, 1, 1]), ("append", [1, 1, 2, 1, 1, 1, 1]),
    ])
  func transformsOnSiblings(operation: String, expected: [Int]) throws {
    let editor = try initEditor()
    var keys: [NodeKey] = []
    var counts = [Int](repeating: 0, count: 6)
    try update(editor) {
      let root = try getRoot()
      let paragraph0 = try createParagraphNode()
      let paragraph1 = try createParagraphNode()
      var textNodes: [LexicalNode] = []
      for i in 0..<6 {
        let node = try createTextNode(String(i)).toggleUnmergeable()
        textNodes.append(node)
        keys.append(node.getKey())
      }
      try root.append(paragraph0, paragraph1)
      for node in textNodes[0..<3] { try paragraph0.append(node) }
      for node in textNodes[3...] { try paragraph1.append(node) }
    }
    let remove = try editor.registerNodeTransform(TextNode.self) { node in
      counts[Int(node.__text)!] += 1
    }
    defer { remove() }
    // Upstream's async beforeEach lets the registration's update commit.
    drainMicrotasks()
    try update(editor) {
      let textNode1 = try getNodeByKey(keys[1])!
      let textNode4 = try getNodeByKey(keys[4])!
      switch operation {
      case "remove": try textNode1.remove()
      case "replace": _ = try textNode4.replace(textNode1)
      case "insertBefore": _ = try textNode4.insertBefore(textNode1)
      case "insertAfter": _ = try textNode4.insertAfter(textNode1)
      case "splitText":
        let text = textNode1 as! TextNode
        _ = try text.setTextContent("67")
        _ = try text.splitText(1)
        counts += [0, 0]
      default:
        try (try getRoot().getFirstChild() as! ParagraphNode).append(
          try createTextNode("6").toggleUnmergeable())
        counts.append(0)
      }
    }
    #expect(counts == expected)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Detects infinite recursivity on transforms"
    ))
  func infiniteTransformRecursion() throws {
    let errors = Calls()
    let editor = try initEditor(onError: { errors.record($0) })
    let bold = try editor.registerNodeTransform(TextNode.self) { _ = try $0.toggleFormat("bold") }
    #expect(errors.count == 0)
    try update(editor) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      try paragraph.append(try createTextNode("foo"))
    }
    #expect(errors.count == 1)
    bold()
  }

  /// `runCascade`: an update listener that always enqueues another update.
  private func runCascade(onWarn: ErrorHandler? = nil) throws -> (Editor, Calls, () -> Void) {
    let errors = Calls()
    let editor = try initEditor(onError: { errors.record($0) }, onWarn: onWarn)
    let unregister = editor.registerUpdateListener { _ in
      try editor.update { try getRoot().markDirty() }
    }
    #expect(errors.count == 0)
    return (editor, errors, unregister)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Detects infinite recursivity on update listeners (dev: default onWarn throws)"
    ))
  func infiniteUpdateListenerRecursion() throws {
    let (editor, errors, unregister) = try runCascade()
    var caught: [any Error] = []
    try withUncaughtErrorHandler({ caught.append($0) }) {
      try editor.update { try getRoot().markDirty() }
      drainMicrotasks()
    }
    #expect(caught.count == 1)
    let message = "\(caught.first.map { "\($0)" } ?? "")"
    #expect(message.contains("endlessly enqueueing"))
    #expect(message.contains("Editor namespace: \(editor._config.namespace)"))
    unregister()
    try editor.update(discrete: true) { try getRoot().markDirty() }
    drainMicrotasks()
    #expect(errors.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > routes the recursion guard through a custom onWarn for embedder telemetry"
    ))
  func recursionGuardThroughOnWarn() throws {
    let warnings = Calls()
    let (editor, errors, unregister) = try runCascade(onWarn: { warnings.record($0) })
    var caught: [any Error] = []
    try withUncaughtErrorHandler({ caught.append($0) }) {
      try editor.update { try getRoot().markDirty() }
      drainMicrotasks()
    }
    #expect(warnings.count == 1)
    let message = "\(warnings.errors.first.map { "\($0)" } ?? "")"
    #expect(message.contains("endlessly enqueueing"))
    #expect(message.contains("Editor namespace: \(editor._config.namespace)"))
    #expect(caught.isEmpty)
    #expect(errors.count == 0)
    unregister()
    try editor.update(discrete: true) { try getRoot().markDirty() }
    drainMicrotasks()
    #expect(errors.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not trip the recursion guard on bounded re-enqueueing across separate actions"
    ))
  func boundedReenqueueing() throws {
    let errors = Calls()
    let warnings = Calls()
    let editor = try initEditor(onError: { errors.record($0) }, onWarn: { warnings.record($0) })
    let unregister = editor.registerUpdateListener { _ in try editor.update {} }
    for _ in 0..<150 {
      try editor.update { try getRoot().markDirty() }
      drainMicrotasks()
    }
    unregister()
    #expect(warnings.count == 0)
    #expect(errors.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not trip the recursion guard on a fast input burst driven by commands"
    ))
  func commandBurst() throws {
    let errors = Calls()
    let warnings = Calls()
    let editor = try initEditor(onError: { errors.record($0) }, onWarn: { warnings.record($0) })
    let burst = createCommand("BURST_COMMAND") as LexicalCommand<Void>
    let unregisterCommand = editor.registerCommand(
      burst,
      { _, _ in
        try editor.update { try getRoot().markDirty() }
        return true
      }, COMMAND_PRIORITY_EDITOR)
    let unregisterUpdate = editor.registerUpdateListener { _ in try editor.update {} }
    for _ in 0..<150 {
      _ = try editor.dispatchCommand(burst)
      drainMicrotasks()
    }
    unregisterUpdate()
    unregisterCommand()
    #expect(warnings.count == 0)
    #expect(errors.count == 0)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > applies (and warns in DEV) when a command dispatched from a read-only context mutates the editor"
    ))
  func readOnlyDispatchWarnsAndApplies() throws {
    let editor = try initEditor()
    let command = createCommand("READONLY_MUTATE_COMMAND") as LexicalCommand<Void>
    let unregister = editor.registerCommand(
      command,
      { _, _ in
        try getRoot().clear().append(
          try createParagraphNode().append(try createTextNode("mutated")))
        return true
      }, COMMAND_PRIORITY_EDITOR)
    defer { unregister() }
    var warnings: [String] = []
    try withWarningHandler({ warnings.append($0) }) {
      try editor.read { _ = try editor.dispatchCommand(command) }
      drainMicrotasks()
      #expect(warnings.count == 1)
      #expect(warnings.first?.contains("read-only context") == true)
      #expect(try editor.read { try getRoot().getTextContent() } == "mutated")
      warnings = []
      _ = try editor.dispatchCommand(command)
      #expect(warnings.isEmpty)
    }
  }
}
