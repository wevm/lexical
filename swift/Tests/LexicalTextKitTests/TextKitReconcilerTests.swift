import Lexical
import LexicalTesting
import LexicalTextKit
import Testing

// The incremental reconciler against full renders: the edits it emits,
// applied to a storage, always give the text and runs of the new state, and
// its document maps locations and points as a fresh one does.

/// What a storage shows at one UTF-16 unit: the text node's key and formats.
struct Attributes: Equatable {
  let key: NodeKey
  let format: Int
  let style: String
}

/// A text storage that keeps the text and the run under each unit.
final class FakeStorage: TextKitStorage {
  var units: [UInt16] = []
  var attributes: [Attributes?] = []
  private var isEditing = false
  /// Every edit applied, in order.
  var edits: [(location: Int, length: Int, count: Int)] = []

  func beginEditing() { isEditing = true }
  func endEditing() { isEditing = false }

  func replaceCharacters(location: Int, length: Int, with string: String, runs: [TextKitRun]) {
    #expect(isEditing)
    let new = Array(string.utf16)
    var newAttributes = [Attributes?](repeating: nil, count: new.count)
    for run in runs {
      for i in run.location..<(run.location + run.length) {
        newAttributes[i - location] = Attributes(
          key: run.key, format: run.format, style: run.style)
      }
    }
    units.replaceSubrange(location..<(location + length), with: new)
    attributes.replaceSubrange(location..<(location + length), with: newAttributes)
    edits.append((location, length, new.count))
  }

  var string: String { String(decoding: units, as: UTF16.self) }

  static func attributes(_ document: TextKitDocument) -> [Attributes?] {
    var result = [Attributes?](repeating: nil, count: document.length)
    for run in document.runs {
      for i in run.location..<(run.location + run.length) {
        result[i] = Attributes(key: run.key, format: run.format, style: run.style)
      }
    }
    return result
  }
}

/// SplitMix64, so sessions replay from their seed.
struct SeededGenerator: RandomNumberGenerator {
  var state: UInt64
  mutating func next() -> UInt64 {
    state &+= 0x9E37_79B9_7F4A_7C15
    var z = state
    z = (z ^ (z >> 30)) &* 0xBF58_476D_1CE4_E5B9
    z = (z ^ (z >> 27)) &* 0x94D0_49BB_1331_11EB
    return z ^ (z >> 31)
  }
}

/// A controller whose edits go to a `FakeStorage`.
struct Harness {
  let controller: TextKitInputController
  let storage = FakeStorage()
  var editor: Editor { controller.editor }

  /// `onCommit` is an update listener that runs before the controller's.
  init(paragraphs: Int = 0, onCommit: (() -> Void)? = nil) throws {
    let editor = try createTestEditor()
    registerTextKitSliceEditing(editor)
    if let onCommit {
      editor.registerUpdateListener { _ in onCommit() }
    }
    if paragraphs > 0 {
      try editor.update(discrete: true) {
        let root = try getRoot()
        for i in 0..<paragraphs {
          try root.append(createParagraphNode().append(createTextNode("Paragraph number \(i)")))
        }
      }
    }
    controller = TextKitInputController(editor: editor)
    let storage = storage
    storage.apply([
      TextKitEdit(
        location: 0, length: 0, string: controller.document.string,
        runs: controller.document.runs)
    ])
    controller.onRender = { edits, _ in storage.apply(edits) }
  }

  func fresh() throws -> TextKitDocument {
    try editor.getEditorState().read { try TextKitDocument(root: getRoot()) }
  }

  /// The storage and the reconciler's document match a full render. In a
  /// large document, `around` limits the point checks to the locations
  /// near it and the nodes there.
  func check(around: Int? = nil, _ context: @autoclosure () -> String = "") throws {
    let fresh = try fresh()
    let freshSpans = fresh.spans
    let document = controller.document
    #expect(storage.string == fresh.string, "\(context())")
    #expect(storage.attributes == FakeStorage.attributes(fresh), "\(context())")
    #expect(document.string == fresh.string, "\(context())")
    #expect(document.length == fresh.length, "\(context())")
    #expect(document.runs == fresh.runs, "\(context())")
    #expect(document.spans == freshSpans, "\(context())")
    #expect(controller.reconciler.fallbacks == 0, "\(context())")
    let window =
      around.map { max(0, $0 - 200)...min(fresh.length, $0 + 200) } ?? 0...fresh.length
    for location in window {
      let (a, b) = (document.point(at: location), fresh.point(at: location))
      #expect(a == b, "point(at: \(location)) \(context())")
    }
    try editor.getEditorState().read {
      for (key, node) in editor.getEditorState()._nodeMap {
        if let span = freshSpans[key],
          span.end < window.lowerBound || span.location > window.upperBound
        {
          continue
        }
        #expect(document.span(key) == freshSpans[key], "\(context())")
        let count = (node as? ElementNode)?.__size ?? 0
        for offset in 0...(count + 1) {
          for type in [PointType.text, .element] {
            #expect(
              document.location(key, offset, type) == fresh.location(key, offset, type),
              "location(\(key), \(offset), \(type)) \(context())")
          }
        }
      }
    }
  }
}

/// The nodes of the current state that pass `filter` (call it in a read or
/// update).
private func nodes(_ filter: (LexicalNode) -> Bool) throws -> [LexicalNode] {
  var result: [LexicalNode] = []
  func visit(_ node: LexicalNode) throws {
    if filter(node) { result.append(node) }
    for child in try (node as? ElementNode)?.getChildren() ?? [] {
      try visit(child)
    }
  }
  try visit(getRoot())
  return result
}

@Suite struct TextKitReconcilerTests {
  /// One random editing step.
  private func step(
    _ h: Harness, _ rng: inout SeededGenerator, _ saved: inout [EditorState]
  )
    throws -> String
  {
    let c = h.controller
    let length = c.document.length
    let texts = ["a", "bc", " ", "é", "😀", "xyz"]
    switch Int.random(in: 0..<20, using: &rng) {
    case 0, 1, 2:
      let location = Int.random(in: 0...length, using: &rng)
      let span = Bool.random(using: &rng) ? 0 : Int.random(in: 0...(length - location), using: &rng)
      try c.setSelectedRange(location: location, length: span)
      return "select \(location) \(span)"
    case 3, 4, 5:
      let text = texts.randomElement(using: &rng)!
      try c.insertText(text)
      return "type \(text)"
    case 6:
      try c.deleteBackward()
      return "deleteBackward"
    case 7:
      try c.deleteForward()
      return "deleteForward"
    case 8:
      try c.insertNewline()
      return "insertNewline"
    case 9:
      try c.insertLineBreak()
      return "insertLineBreak"
    case 10:
      let format: TextFormatType = Bool.random(using: &rng) ? "bold" : "italic"
      try c.toggleFormat(format)
      return "toggle \(format)"
    case 11:
      let isInline = Bool.random(using: &rng)
      try h.editor.update(discrete: true) {
        try getSelection()?.insertNodes([createTestDecoratorNode().setIsInline(isInline)])
      }
      return "insert decorator inline: \(isInline)"
    case 12:
      var seed = rng
      try h.editor.update(discrete: true) {
        let decorators = try nodes { $0 is TestDecoratorNode }
        guard let node = decorators.randomElement(using: &seed) as? TestDecoratorNode else {
          return
        }
        try node.setIsInline(!node.isInline())
      }
      rng = seed
      return "toggle decorator block"
    case 13:
      let text = texts.randomElement(using: &rng)!
      try h.editor.update(discrete: true) {
        let link = try createTestInlineElementNode()
        try link.append(createTextNode(text))
        try getSelection()?.insertNodes([link])
      }
      return "insert inline element \(text)"
    case 14:
      var seed = rng
      try h.editor.update(discrete: true) {
        let root = try getRoot()
        let block = try createTestElementNode()
        try block.append(createParagraphNode().append(createTextNode("in")))
        try block.append(createParagraphNode())
        let children = try root.getChildren()
        if let anchor = children.randomElement(using: &seed) {
          try anchor.insertBefore(block)
        } else {
          try root.append(block)
        }
      }
      rng = seed
      return "insert nested block"
    case 15:
      var seed = rng
      try h.editor.update(discrete: true) {
        let all = try nodes { !($0 is RootNode) }
        try all.randomElement(using: &seed)?.remove()
      }
      rng = seed
      return "remove node"
    case 16:
      var seed = rng
      try h.editor.update(discrete: true) {
        let blocks = try getRoot().getChildren()
        guard blocks.count > 1, let moved = blocks.randomElement(using: &seed),
          let target = blocks.randomElement(using: &seed), !moved.is(target)
        else { return }
        try target.insertAfter(moved)
      }
      rng = seed
      return "move block"
    case 17:
      var seed = rng
      try h.editor.update(discrete: true) {
        let texts = try nodes { $0 is TextNode }
        guard let node = texts.randomElement(using: &seed) as? TextNode else { return }
        try node.setTextContent(node.getTextContent() + "!")
      }
      rng = seed
      return "append to text node"
    case 18:
      saved.append(h.editor.getEditorState())
      return "save state"
    default:
      guard let state = saved.randomElement(using: &rng) else { return "no saved state" }
      try h.editor.setEditorState(state)
      return "setEditorState"
    }
  }

  @Test(arguments: 0..<24)
  func randomSessionsMatchFullRenders(seed: UInt64) throws {
    var rng = SeededGenerator(state: seed)
    let h = try Harness()
    var saved: [EditorState] = []
    var log: [String] = []
    for _ in 0..<120 {
      var description: String
      do {
        description = try step(h, &rng, &saved)
      } catch {
        description = "failed: \(error)"
      }
      log.append(description)
      try h.check("seed \(seed): \(log.suffix(6).joined(separator: "; "))")
    }
  }

  @Test func typingInALargeDocumentTouchesOneTextNode() throws {
    let h = try Harness(paragraphs: 5_000)
    let before = h.controller.document
    let (paragraph, text) = try h.editor.read {
      let paragraph = try #require(try getRoot().getChildAtIndex(2_500) as? ElementNode)
      return (paragraph.getKey(), try #require(try paragraph.getFirstChild()).getKey())
    }
    let span = try #require(before.span(text))
    try h.controller.setSelectedRange(location: span.end, length: 0)
    h.storage.edits = []
    try h.controller.insertText("x")
    #expect(h.storage.edits.count == 1)
    let edit = try #require(h.storage.edits.first)
    #expect(edit.location == span.location && edit.length == span.length)
    #expect(edit.count == span.length + 1)
    let after = h.controller.document
    #expect(after.span(paragraph)?.length == before.span(paragraph)!.length + 1)
    #expect(try h.controller.selectedRange()! == (span.end + 1, 0))
    try h.check(around: span.end)
  }

  @Test func enterAndBackspaceSpliceOneParagraph() throws {
    let h = try Harness(paragraphs: 5_000)
    let text = try h.editor.read {
      try #require(try (getRoot().getChildAtIndex(1_000) as? ElementNode)?.getFirstChild())
        .getKey()
    }
    let span = try #require(h.controller.document.span(text))
    try h.controller.setSelectedRange(location: span.location + 9, length: 0)
    h.storage.edits = []
    try h.controller.insertNewline()
    let touched = h.storage.edits.reduce(0) { $0 + $1.length + $1.count }
    #expect(touched < 100, "\(h.storage.edits)")
    try h.check(around: span.location)
    h.storage.edits = []
    try h.controller.deleteBackward()
    #expect(h.storage.edits.reduce(0) { $0 + $1.length + $1.count } < 100)
    #expect(h.controller.document.string == (try h.fresh().string))
    #expect(h.controller.document.span(text)?.length == span.length)
    try h.check(around: span.location)
  }

  @Test func formattingReplacesTheTextWithNewRuns() throws {
    let h = try Harness()
    try h.controller.insertText("Hello world")
    try h.controller.setSelectedRange(location: 6, length: 5)
    h.storage.edits = []
    try h.controller.toggleFormat("bold")
    #expect(h.storage.edits.count == 2)
    #expect(h.controller.document.runs.map(\.format) == [0, 1])
    try h.check()
  }

  @Test func selectionChangesEmitNoEdits() throws {
    let h = try Harness(paragraphs: 10)
    var renders: [[TextKitEdit]] = []
    let storage = h.storage
    h.controller.onRender = { edits, _ in
      renders.append(edits)
      storage.apply(edits)
    }
    try h.controller.setSelectedRange(location: 3, length: 4)
    #expect(renders.count == 1 && renders[0].isEmpty)
    try h.controller.moveSelection(.move, isBackward: false, granularity: .character)
    #expect(renders.allSatisfy { $0.isEmpty })
  }

  @Test func setEditorStateRendersInFull() throws {
    let h = try Harness(paragraphs: 3)
    let saved = h.editor.getEditorState()
    try h.controller.insertText("more")
    let length = h.storage.units.count
    h.storage.edits = []
    try h.editor.setEditorState(saved)
    #expect(h.storage.edits.count == 1)
    #expect(h.storage.edits.first?.location == 0)
    #expect(h.storage.edits.first?.length == length)
    try h.check()
  }

  /// Reported, not gating (DESIGN.md, Benchmarks): 200 keystrokes in the
  /// middle of 5,000 paragraphs, from dispatch to the edits, and the
  /// reconcile alone (from the commit's first update listener to the edits).
  @Test func reportsKeystrokeTiming() throws {
    let clock = ContinuousClock()
    var commit = clock.now
    let h = try Harness(paragraphs: 5_000, onCommit: { commit = clock.now })
    let paragraph = try h.editor.read {
      try #require(try getRoot().getChildAtIndex(2_500)).getKey()
    }
    let span = try #require(h.controller.document.span(paragraph))
    try h.controller.setSelectedRange(location: span.end, length: 0)
    var keystrokes: [Duration] = []
    var reconciles: [Duration] = []
    // The fake storage is not a text view, so the edits reach it after
    // the timed keystrokes.
    var edits: [[TextKitEdit]] = []
    h.controller.onRender = { new, _ in
      reconciles.append(clock.now - commit)
      edits.append(new)
    }
    for _ in 0..<200 {
      let start = clock.now
      try h.controller.insertText("a")
      keystrokes.append(clock.now - start)
    }
    edits.forEach(h.storage.apply)
    func p95(_ values: [Duration]) -> Double {
      let value = values.sorted()[values.count * 95 / 100]
      return Double(value.components.seconds) * 1e3 + Double(value.components.attoseconds) / 1e15
    }
    print(
      "TextKit p95 over 200 keystrokes on 5,000 paragraphs: keystroke \(p95(keystrokes)) ms, "
        + "reconcile \(p95(reconciles)) ms")
    try h.check(around: span.end)
  }
}
