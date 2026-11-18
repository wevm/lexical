import Lexical
import LexicalTesting
import Testing

// Port of src/__tests__/browser/SlotWordDeletion.test.ts (a browser test,
// so it isn't in the unit ledger): deletion whose movement lands in another
// slot frame stops at the boundary in the anchor's frame. A browser's
// `Selection.modify` walks the text as it is laid out and can cross a slot
// boundary, which lives only in the model; `LaidOutMovement` walks the
// text nodes in the order the browser test lays them out.

/// A `SelectionMovement` over text nodes in layout order. Each entry is a
/// text node's key and whether a new line starts before it; by word a new
/// line counts as a space, by line it is the line's edge.
private final class LaidOutMovement: SelectionMovement {
  var order: [(NodeKey, Bool)] = []

  func landedPoint(
    from point: Point, isBackward: Bool, granularity: SelectionGranularity
  ) throws -> (NodeKey, Int, PointType)? {
    var text: [Character] = []
    var breaks: Set<Int> = []
    var segments: [(key: NodeKey, start: Int, length: Int)] = []
    for (key, newLine) in order {
      if newLine && !text.isEmpty {
        breaks.insert(text.count)
        text.append(" ")
      }
      let content = Array(try #require(try getNodeByKey(key)).getTextContent())
      segments.append((key, text.count, content.count))
      text += content
    }
    let from = try #require(segments.first { $0.key == point.key })
    let start = from.start + point.offset
    var o = start
    switch granularity {
    case .word:
      if isBackward {
        while o > 0 && text[o - 1] == " " { o -= 1 }
        while o > 0 && text[o - 1] != " " { o -= 1 }
      } else {
        while o < text.count && text[o] == " " { o += 1 }
        while o < text.count && text[o] != " " { o += 1 }
      }
    case .lineboundary:
      if isBackward {
        while o > 0 && !breaks.contains(o - 1) { o -= 1 }
      } else {
        while o < text.count && !breaks.contains(o) { o += 1 }
      }
    default:
      Issue.record("unexpected granularity \(granularity)")
      return nil
    }
    if o == start {
      return nil
    }
    // At the edge of two texts, the one it moved into.
    let at = segments.filter { $0.start <= o && o <= $0.start + $0.length }
    let landed = try #require(isBackward ? at.last : at.first)
    return (landed.key, o - landed.start, .text)
  }
}

private enum Layout: String, CaseIterable {
  case rootSlot = "root slot"
  case elementHost = "element host"
  case slotOnlyHost = "slot-only host"
  case inlineSlot = "inline slot"
  case inlineRootSlot = "inline root slot"
  /// Not in the browser test, from JS's code path: an inline element host
  /// whose slot renders before its children on the line.
  case inlineHost = "inline host"
}

private enum Deletion {
  case word, line
}

/// `pressAt`: mounts `layout`, puts the caret at `offset` in `region` and
/// deletes. Returns the text of every region, `nil` for a removed one, and
/// the editor.
private func pressAt(
  _ layout: Layout, _ region: String, _ offset: Int, _ deletion: Deletion, isBackward: Bool
) throws -> (editor: Editor, original: [String: String?], result: [String: String?]) {
  let editor = try createTestEditor()
  let movement = LaidOutMovement()
  editor.selectionMovement = movement
  var keys: [String: NodeKey] = [:]
  try editor.update(discrete: true) {
    func text(_ name: String, _ value: String) throws -> TextNode {
      let node = try createTextNode(value)
      keys[name] = node.getKey()
      return node
    }
    func paragraph(_ children: LexicalNode...) throws -> ParagraphNode {
      try createParagraphNode().append(contentsOf: children)
    }
    let root = try getRoot().clear()
    switch layout {
    case .rootSlot:
      try root.append(try paragraph(try text("sibling", "Body words")))
      _ = try setSlot(
        root, "footnotes",
        try createTestShadowRootNode().append(try paragraph(try text("note", "Footnote words"))))
      movement.order = [(keys["note"]!, true), (keys["sibling"]!, true)]
    case .elementHost, .slotOnlyHost:
      let host =
        layout == .elementHost
        ? try paragraph(try text("sibling", "After words")) : try createParagraphNode()
      try root.append(try paragraph(try text("before", "Before words")), host)
      if layout == .slotOnlyHost {
        try root.append(try paragraph(try text("sibling", "After words")))
      }
      _ = try setSlot(
        host, "note",
        try createTestShadowRootNode().append(try paragraph(try text("note", "Footnote words"))))
      movement.order = [
        (keys["before"]!, true), (keys["note"]!, true), (keys["sibling"]!, true),
      ]
    case .inlineRootSlot:
      try root.append(try paragraph(try text("sibling", "bar")))
      _ = try setSlot(root, "inline", try paragraph(try text("note", "foo")))
      // Mounted after the body, on the same line.
      movement.order = [(keys["sibling"]!, true), (keys["note"]!, false)]
    case .inlineSlot:
      let host = try paragraph(try text("sibling", "bar"))
      try root.append(host)
      _ = try setSlot(host, "inline", try paragraph(try text("note", "foo")))
      // An element's slots render before its children.
      movement.order = [(keys["note"]!, true), (keys["sibling"]!, false)]
    case .inlineHost:
      let host = try createTestInlineElementNode()
      try host.append(try text("host", "x"))
      try root.append(
        try paragraph(try text("before", "ab "), host, try text("sibling", "ef")))
      _ = try setSlot(host, "note", try paragraph(try text("note", "cd")))
      // On one line: the host's slot, then its children.
      movement.order = [
        (keys["before"]!, true), (keys["note"]!, false), (keys["host"]!, false),
        (keys["sibling"]!, false),
      ]
    }
  }
  func snapshot() throws -> [String: String?] {
    try editor.read {
      try keys.mapValues { key -> String? in
        try (getNodeByKey(key) as? TextNode)?.getTextContent()
      }
    }
  }
  let original = try snapshot()
  try editor.update(discrete: true) {
    let selection = try (try node(keys[region]!) as TextNode).select(offset, offset)
    switch deletion {
    case .word: try selection.deleteWord(isBackward)
    case .line: try selection.deleteLine(isBackward)
    }
  }
  return (editor, original, try snapshot())
}

@Suite struct SlotWordDeletionTests {
  @Test(arguments: [
    (Layout.rootSlot, "sibling"), (.elementHost, "note"), (.elementHost, "sibling"),
    (.slotOnlyHost, "note"), (.slotOnlyHost, "sibling"),
  ])
  fileprivate func wordBackspaceAtStartDeletesNothingAcrossTheSlot(
    _ layout: Layout, _ region: String
  ) throws {
    let (_, original, result) = try pressAt(layout, region, 0, .word, isBackward: true)
    #expect(result == original)
  }

  @Test func wordBackspaceInAWordThatStartsInAnInlineSlotDeletesUpToTheSlot() throws {
    let (_, _, result) = try pressAt(.inlineSlot, "sibling", 2, .word, isBackward: true)
    #expect(result == ["note": "foo", "sibling": "r"])
  }

  @Test func wordDeleteInAnInlineSlotWordThatContinuesPastTheSlotDeletesToTheSlotEnd() throws {
    let (_, _, result) = try pressAt(.inlineSlot, "note", 1, .word, isBackward: false)
    #expect(result == ["note": "f", "sibling": "bar"])
  }

  @Test(arguments: [Layout.elementHost, .slotOnlyHost])
  fileprivate func wordDeleteAtTheEndOfTheParagraphBeforeTheHostDeletesNothing(
    _ layout: Layout
  ) throws {
    let (editor, original, result) = try pressAt(
      layout, "before", "Before words".count, .word, isBackward: false)
    #expect(result == original)
    // Text that moved into another block would keep its content.
    #expect(try editor.read { try getRoot().getChildrenSize() } == (layout == .elementHost ? 2 : 3))
  }

  @Test func wordDeleteInABodyWordThatRunsIntoAnInlineRootSlotDeletesToTheSlot() throws {
    let (_, _, result) = try pressAt(.inlineRootSlot, "sibling", 1, .word, isBackward: false)
    #expect(result == ["note": "foo", "sibling": "b"])
  }

  // The emptied text node is removed.
  @Test(arguments: [(0, nil), (1, "b"), (3, "bar")] as [(Int, String?)])
  func forwardLineDeletionInABodyLineThatEndsInAnInlineRootSlotDeletesToTheSlot(
    _ offset: Int, _ sibling: String?
  ) throws {
    let (_, _, result) = try pressAt(.inlineRootSlot, "sibling", offset, .line, isBackward: false)
    #expect(result == ["note": "foo", "sibling": sibling])
  }

  @Test func backwardLineDeletionInAnInlineRootSlotAfterTheBodyDeletesToTheSlotStart() throws {
    let (_, _, result) = try pressAt(.inlineRootSlot, "note", 2, .line, isBackward: true)
    #expect(result == ["note": "o", "sibling": "bar"])
  }

  // From JS's code path: beside a host, a deletion stops after it, where a
  // selection would stop before it and take the host's text too.
  @Test func wordBackspaceInAWordThatStartsInAnInlineHostsSlotStopsBesideTheHost() throws {
    let (editor, _, result) = try pressAt(.inlineHost, "sibling", 1, .word, isBackward: true)
    #expect(result == ["before": "ab ", "note": "cd", "host": "x", "sibling": "f"])
    #expect(try rootText(editor) == "ab cdxf")
  }
}
