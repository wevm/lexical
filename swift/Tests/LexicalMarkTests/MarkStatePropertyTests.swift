import Foundation
import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHistory
import LexicalMark
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-mark/src/__tests__/unit/MarkStateProperty.test.ts.
//
// A seeded property test of MarkStateExtension: random edit sequences on a
// few paragraphs with overlapping marks, checked after every step against a
// model of the text that tracks the ids of every character (a code point:
// typed text includes emoji). Every third seed replaces TextNode with a
// subclass through a node replacement.
//
// It runs twice: with every id in MarkStateExtension, and with the ids split
// between MarkStateExtension and a second mark flavor (CommentMarks), whose
// ids together must follow the same model.
//
// MARK_STATE_PROPERTY_SEEDS and MARK_STATE_PROPERTY_STEPS run it longer, and
// MARK_STATE_PROPERTY_OPS limits it to a comma separated list of operations.
// The xorshift generator is the upstream one, so a seed makes the same
// choices as upstream wherever the documents agree.

/// A character of the model and the ids it carries, sorted.
private struct Char: Equatable {
  let c: String
  let ids: [String]
}
private typealias Model = [[Char]]

private let IDS = ["c1", "c2", "c3", "c4", "c5"]

private let PropertyCommentMarks = try! defineMarkExtension(
  name: "@test/CommentMarks", stateKey: "commentIds", className: "comment",
  overlapClassName: "comment-overlap")

/// In the two-flavor run, these ids go to CommentMarks.
private let COMMENT_IDS = ["c2", "c4"]

private let OPS = [
  "type", "type", "typeSameUpdate", "typeBold", "typeReplace", "typeReplace", "typeRemove",
  "pasteOutside", "ime", "backspace", "delete", "deleteRange", "enter", "format", "blockType",
  "mark", "unmark", "copyPaste", "cutPaste", "undo", "redo",
]

/// `xorshift(seed)`: the upstream generator, in UInt32 arithmetic.
private final class Xorshift {
  var s: UInt32
  init(_ seed: Int) {
    let s = UInt32(truncatingIfNeeded: seed)
    self.s = s == 0 ? 1 : s
  }
  func next() -> Double {
    s ^= s &<< 13
    s ^= s >> 17
    s ^= s &<< 5
    return Double(s) / 4_294_967_296
  }
  func int(_ n: Int) -> Int { Int((next() * Double(n)).rounded(.down)) }
}

private func union(_ sets: [[String]]) -> [String] {
  jsSort(Array(Set(sets.joined())))
}

private func intersect(_ a: [String], _ b: [String]) -> [String] {
  a.filter { b.contains($0) }
}

private func describeModel(_ model: Model) -> String {
  model.map { block in
    var runs: [String] = []
    var text = ""
    var key: String? = nil
    for ch in block {
      let k = ch.ids.joined(separator: ",")
      if let key, k != key {
        runs.append(quoted(text) + (key.isEmpty ? "" : "[\(key)]"))
        text = ""
      }
      key = k
      text += ch.c
    }
    if let key {
      runs.append(quoted(text) + (key.isEmpty ? "" : "[\(key)]"))
    }
    return "(" + runs.joined(separator: " ") + ")"
  }.joined(separator: " / ")
}

private func chars(_ text: String, _ ids: [String] = []) -> [Char] {
  text.unicodeScalars.map { Char(c: String($0), ids: ids) }
}

/// Insert `chars` at `offset`, as the documented rule says.
private func insertChars(
  _ block: inout [Char], _ offset: Int, _ inserted: [Char], _ replaced: [Char] = []
) {
  let before = offset > 0 ? block[offset - 1].ids : []
  let after = offset < block.count ? block[offset].ids : []
  var shared = intersect(before, after)
  if !replaced.isEmpty {
    shared = union([shared, replaced.map(\.ids).dropFirst().reduce(replaced[0].ids, intersect)])
  }
  block.insert(
    contentsOf: inserted.map { Char(c: $0.c, ids: union([$0.ids, shared])) }, at: offset)
}

private final class PropertyReplacementTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      PropertyReplacementTextNode.self, type: "property-replacement-text", extends: TextNode.self)
  }
}

private struct PointSpec {
  let key: NodeKey
  let offset: Int
  let type: PointType
}

/// UTF-16 `units[from..<to]` as a string.
private func slice(_ units: [UInt16], _ from: Int, _ to: Int? = nil) -> String {
  String(decoding: units[from..<(to ?? units.count)], as: UTF16.self)
}

private final class PropertyRun {
  let twoFlavors: Bool
  let random: Xorshift
  let editor: Editor
  var log: [String] = []

  init(seed: Int, twoFlavors: Bool) throws {
    self.twoFlavors = twoFlavors
    random = Xorshift(seed)
    let withReplacement = seed % 3 == 0
    editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]",
        dependencies: [
          RichTextExtension, configExtension(HistoryExtension) { $0.delay = 0 },
          MarkStateExtension,
        ] + (twoFlavors ? [PropertyCommentMarks] : []),
        nodes: {
          withReplacement
            ? [
              .klass(PropertyReplacementTextNode.self),
              .replacement(
                LexicalNodeReplacement(
                  replace: TextNode.self,
                  with: { node in
                    try create(PropertyReplacementTextNode.self).setTextContent(
                      (node as! TextNode).getTextContent())
                  },
                  withKlass: PropertyReplacementTextNode.self)),
            ] : []
        },
        editorConfig: InitialEditorConfig(
          theme: ["mark": .string("mark"), "markOverlap": .string("mark-overlap")])))
    try editor.setRootElement(TestRootElement())
  }

  func flavorOf(_ id: String) -> MarkFlavorExtension {
    twoFlavors && COMMENT_IDS.contains(id) ? PropertyCommentMarks : MarkStateExtension
  }

  /// The flavors of the run, with their theme classes.
  var flavors: [(MarkFlavorExtension, String, String)] {
    twoFlavors
      ? [
        (MarkStateExtension, "mark", "mark-overlap"),
        (PropertyCommentMarks, "comment", "comment-overlap"),
      ] : [(MarkStateExtension, "mark", "mark-overlap")]
  }

  /// The ids of every flavor on a node, sorted.
  func nodeIDs(_ node: LexicalNode) throws -> [String] {
    union(try flavors.map { try getNodeMarkIDs($0.0, node) })
  }

  func pick<T>(_ items: [T]) -> T { items[random.int(items.count)] }

  func fail(_ message: String) -> LexicalError {
    LexicalError("\(message)\n\(log.suffix(20).joined(separator: "\n"))")
  }

  func update(_ fn: @escaping () throws -> Void) throws {
    try editor.update(discrete: true, fn)
  }

  func documentModel() throws -> Model {
    try getRoot().getChildren().map { block in
      try (block as! ElementNode).getAllTextNodes().flatMap { node in
        chars(try node.getTextContent(), try nodeIDs(node))
      }
    }
  }

  /// A point at a character (code point) offset of a block. At a boundary
  /// between two TextNodes, `preferNext` picks the start of the next one.
  func point(_ block: Int, _ offset: Int, _ preferNext: Bool) throws -> PointSpec {
    let element = try getRoot().getChildAtIndex(block) as! ElementNode
    let texts = try element.getAllTextNodes()
    if texts.isEmpty {
      return PointSpec(key: element.getKey(), offset: 0, type: .element)
    }
    var acc = 0
    for (i, text) in texts.enumerated() {
      let scalars = Array(try text.getTextContent().unicodeScalars)
      let size = scalars.count
      if offset < acc + size || (offset == acc + size && (!preferNext || i == texts.count - 1)) {
        let utf16 = scalars[0..<(offset - acc)].reduce(0) { $0 + String($1).utf16.count }
        return PointSpec(key: text.getKey(), offset: utf16, type: .text)
      }
      acc += size
    }
    throw LexicalError("offset \(offset) out of range in block \(block)")
  }

  func makeSelection(
    _ s: (Int, Int, Bool), _ e: (Int, Int, Bool), _ backward: Bool = false
  ) throws -> (RangeSelection, PointSpec, PointSpec) {
    let selection = createRangeSelection()
    let start = try point(s.0, s.1, s.2)
    let end = try point(e.0, e.1, e.2)
    let (anchor, focus) = backward ? (end, start) : (start, end)
    try selection.anchor.set(anchor.key, anchor.offset, anchor.type)
    try selection.focus.set(focus.key, focus.offset, focus.type)
    return (selection, start, end)
  }

  @discardableResult
  func select(
    _ s: (Int, Int, Bool), _ e: (Int, Int, Bool), _ backward: Bool = false
  ) throws
    -> RangeSelection
  {
    let selection = try makeSelection(s, e, backward).0
    try setSelection(selection)
    return selection
  }

  func check(_ model: Model) throws {
    let doc = try editor.read(documentModel)
    if describeModel(doc) != describeModel(model) {
      throw fail("document\n  got:  \(describeModel(doc))\n  want: \(describeModel(model))")
    }
    func normalize(_ map: [String: [NodeKey]]) -> String {
      map.keys.sorted().map { "\($0):\(map[$0]!.sorted().joined(separator: ","))" }
        .joined(separator: ";")
    }
    for (flavor, mark, overlap) in flavors {
      let output = try getExtensionDependencyFromEditor(editor, flavor).output.markIDNodeKeys
        .peek()
      var got: [String: [NodeKey]] = [:]
      for (id, keys) in output {
        got[id] = keys
      }
      try editor.read {
        // markIDNodeKeys against a full recomputation
        var expected: [String: [NodeKey]] = [:]
        for node in try getRoot().getAllTextNodes() {
          for id in try getNodeMarkIDs(flavor, node) {
            expected[id, default: []].append(node.getKey())
          }
        }
        if normalize(got) != normalize(expected) {
          throw fail(
            "\(flavor.name) markIDNodeKeys\n  got:  \(normalize(got))\n  want: \(normalize(expected))"
          )
        }
        // The DOM of each flavor
        for node in try getRoot().getAllTextNodes() {
          let ids = try getNodeMarkIDs(flavor, node)
          let dom = try renderDOM(editor, node)
          let classList = dom.className.split(separator: " ").map(String.init)
          if dom.getAttribute(flavor.attribute) != (ids.isEmpty ? nil : ids.joined(separator: " "))
            || classList.contains(mark) != !ids.isEmpty
            || classList.contains(overlap) != (ids.count > 1)
          {
            throw fail("\(flavor.name) DOM of \(quoted(try node.getTextContent()))")
          }
        }
      }
    }
    try editor.read {
      // No equal siblings left unmerged
      for block in try getRoot().getChildren() {
        let children = try (block as! ElementNode).getChildren()
        for (i, child) in children.enumerated() {
          guard let node = child as? TextNode, i + 1 < children.count,
            let next = children[i + 1] as? TextNode
          else { continue }
          if try node.isSimpleText() && next.isSimpleText()
            && node.getFormat() == next.getFormat() && node.getStyle() == next.getStyle()
            && flavors.allSatisfy({
              try getNodeMarkIDs($0.0, node) == getNodeMarkIDs($0.0, next)
            })
          {
            throw fail("unmerged \(try node.getTextContent())|\(try next.getTextContent())")
          }
        }
      }
    }
    // getMarkIDsAtSelection against the characters it covers
    for _ in 0..<4 {
      let b1 = random.int(model.count)
      let b2 = min(model.count - 1, b1 + (random.next() < 0.3 ? 1 : 0))
      let o1 = random.int(model[b1].count + 1)
      var o2 = random.int(model[b2].count + 1)
      if b1 == b2 && o2 < o1 {
        o2 = o1 + random.int(model[b1].count - o1 + 1)
      }
      let backward = random.next() < 0.5
      let covered =
        b1 == b2
        ? Array(model[b1][o1..<o2]) : Array(model[b1][o1...]) + Array(model[b2][..<o2])
      try editor.read {
        let (selection, start, _) = try makeSelection((b1, o1, true), (b2, o2, false), backward)
        if selection.isCollapsed() {
          if let node = try selection.anchor.getNode() as? TextNode, start.offset > 0,
            try start.offset < node.getTextContentSize()
          {
            let got = union(try flavors.map { try getMarkIDsAtSelection($0.0, selection) })
              .joined(separator: ",")
            if got != model[b1][o1].ids.joined(separator: ",") {
              throw fail("ids at the caret \(b1):\(o1): \(got)")
            }
          }
          return
        }
        if covered.isEmpty {
          return
        }
        let any = union(covered.map(\.ids)).joined(separator: ",")
        let all = covered.map(\.ids).dropFirst().reduce(covered[0].ids, intersect)
          .joined(separator: ",")
        // The flavors have disjoint ids, so the ids of all of them shared by
        // every character are the union of each flavor's shared ids
        let gotAny = union(try flavors.map { try getMarkIDsAtSelection($0.0, selection, .any) })
          .joined(separator: ",")
        let gotAll = union(try flavors.map { try getMarkIDsAtSelection($0.0, selection, .all) })
          .joined(separator: ",")
        if gotAny != any || gotAll != all {
          throw fail(
            "ids of \(b1):\(o1)-\(b2):\(o2): any \(gotAny) (want \(any)), all \(gotAll) (want \(all))"
          )
        }
      }
    }
  }

  func run(_ steps: Int, _ ops: [String]) throws {
    defer { editor.dispose() }
    try update {
      let root = try getRoot().clear()
      for _ in 0..<3 {
        let length = 4 + self.random.int(10)
        let text = (0..<length).map { _ in
          String(UnicodeScalar(UInt8(97 + self.random.int(26))))
        }.joined()
        _ = try root.append(createParagraphNode().append(createTextNode(text)))
      }
    }
    func blockLength(_ b: Int) throws -> Int {
      try editor.read { try getRoot().getChildAtIndex(b)!.getTextContentSize() }
    }
    for _ in 0..<6 {
      let id = pick(IDS)
      let b1 = random.int(3)
      let b2 = min(2, b1 + (random.next() < 0.3 ? 1 : 0))
      let o1 = random.int(try blockLength(b1))
      let o2 =
        b1 == b2
        ? o1 + 1 + random.int(try blockLength(b1) - o1) : random.int(try blockLength(b2) + 1)
      try update {
        let backward = self.random.next() < 0.5
        try addMarkID(
          self.flavorOf(id), id, self.select((b1, o1, true), (b2, o2, false), backward))
      }
    }
    _ = try editor.dispatchCommand(CLEAR_HISTORY_COMMAND)
    var model = try editor.read(documentModel)
    try check(model)

    let history = try getExtensionDependencyFromEditor(editor, HistoryExtension).output
      .historyState
    func undoDepth() -> Int { history.peek().undoStack.count }
    var past: [Model] = []
    var future: [Model] = []
    /// An update that may add an undo entry, with its effect on the model.
    func commit(_ fn: @escaping () throws -> Void, _ apply: () -> Void) throws {
      let before = model
      let depth = undoDepth()
      try update(fn)
      apply()
      if undoDepth() > depth {
        past.append(before)
        future.removeAll()
      }
    }
    var typed = 0
    // Typed letters are lowercase half of the time, so they can equal the
    // letters around them or the letters they replace
    func nextLetter() -> String {
      if random.next() < 0.5 {
        return String(UnicodeScalar(UInt8(97 + random.int(26))))
      }
      defer { typed += 1 }
      return String(UnicodeScalar(UInt8(65 + typed % 26)))
    }
    // Sometimes an emoji, some of which share their first code unit
    func nextChar() -> String {
      random.next() < 0.1 ? pick(["\u{1F600}", "\u{1F603}", "\u{1F642}"]) : nextLetter()
    }
    func removeEverywhere(_ id: String) {
      model = model.map { $0.map { Char(c: $0.c, ids: $0.ids.filter { $0 != id }) } }
    }

    for step in 0..<steps {
      let op = pick(ops)
      let b = random.int(model.count)
      let length = model[b].count
      let offset = random.int(length + 1)
      let preferNext = random.next() < 0.5
      let caret = (b, offset, preferNext)
      var note = "\(step) \(op) \(b):\(offset)\(preferNext ? ">" : "<")"
      switch op {
      case "type", "typeSameUpdate":
        let c = nextChar()
        if op == "type" {
          try update { try self.select(caret, caret) }
        }
        try commit(
          {
            if op == "typeSameUpdate" {
              try self.select(caret, caret)
            }
            _ = try self.editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, c)
          }, { insertChars(&model[b], offset, chars(c)) })
      case "typeBold":
        // A format toggled at the caret puts the text in a new TextNode
        let c = nextChar()
        try update { try self.select(caret, caret) }
        try commit(
          {
            (try getSelection() as? RangeSelection)?.toggleFormat("bold")
            _ = try self.editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, c)
          }, { insertChars(&model[b], offset, chars(c)) })
      case "typeReplace":
        // A selection replaced by typed or autocorrected text, which often
        // shares letters with the text it replaces
        let b2 = b < model.count - 1 && random.next() < 0.2 ? b + 1 : b
        var end = random.int(model[b2].count + 1)
        if b2 == b {
          if offset == length {
            continue
          }
          end = offset + 1 + random.int(length - offset)
        }
        let selected =
          b2 == b
          ? Array(model[b][offset..<end]) : Array(model[b][offset...]) + Array(model[b2][..<end])
        let word = selected.map(\.c)
        let kind = random.int(6)
        var text: String
        if kind == 0 || word.isEmpty {
          text = nextChar()
        } else if kind == 1 {
          // Its first letter, as when retyping a word
          text = word[0]
        } else if kind == 2 {
          // Its last letter
          text = word[word.count - 1]
        } else if kind == 3 {
          // Two letters swapped, as autocorrect fixes them
          let at = random.int(word.count)
          var swapped = word
          if at + 1 < swapped.count {
            swapped.swapAt(at, at + 1)
          }
          text = swapped.joined()
        } else if kind == 4 {
          // Its start, then new letters
          let count = 1 + random.int(word.count)
          text = word[..<count].joined()
          text += nextChar()
        } else {
          // New letters, then its end
          text = nextChar()
          text += word[random.int(word.count)...].joined()
        }
        // The selection is committed first (as when the user selects), or
        // set in the same update (as a beforeinput target range sets it)
        let separate = random.next() < 0.5
        // A string, or the replacement text of an autocorrection
        let payload: InsertionPayload = random.next() < 0.5 ? .text(text) : .replacement(text)
        let isReplacement: Bool
        if case .replacement = payload { isReplacement = true } else { isReplacement = false }
        note +=
          " to \(b2):\(end) \(quoted(text))\(separate ? " committed" : "")\(isReplacement ? " replacement" : "")"
        if separate {
          try update { try self.select(caret, (b2, end, false)) }
        }
        let inserted = text
        try commit(
          {
            if !separate {
              try self.select(caret, (b2, end, false))
            }
            try dispatchInsertion(self.editor, payload)
          },
          {
            if b2 == b {
              model[b].removeSubrange(offset..<end)
            } else {
              model[b] = Array(model[b][..<offset]) + Array(model[b2][end...])
              model.remove(at: b2)
            }
            insertChars(&model[b], offset, chars(inserted), selected)
          })
      case "typeRemove":
        // An id removed in the same update as typed text, before or after
        // it, replacing a selection or at the caret
        let end =
          offset < length && random.next() < 0.5 ? offset + 1 + random.int(length - offset) : offset
        let c = nextChar()
        let id = pick(IDS)
        let removeFirst = random.next() < 0.5
        note += " to \(end) \(quoted(c)) \(id)\(removeFirst ? " first" : "")"
        try update { try self.select(caret, (b, end, false)) }
        try commit(
          {
            if removeFirst {
              try removeMarkID(self.flavorOf(id), id)
            }
            _ = try self.editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, c)
            if !removeFirst {
              try removeMarkID(self.flavorOf(id), id)
            }
          },
          {
            removeEverywhere(id)
            let selected = Array(model[b][offset..<end])
            model[b].removeSubrange(offset..<end)
            insertChars(&model[b], offset, chars(c), selected)
          })
      case "pasteOutside":
        // A paste event, which dispatches PASTE_COMMAND outside an update,
        // over a selection or at the caret, often of text that starts with
        // the letter it replaces
        let end =
          offset < length && random.next() < 0.6 ? offset + 1 + random.int(length - offset) : offset
        let selected = Array(model[b][offset..<end])
        var text: String
        if !selected.isEmpty && random.next() < 0.5 {
          text = selected[0].c + nextChar()
        } else {
          text = nextChar()
          text += nextChar()
        }
        note += " to \(end) \(quoted(text))"
        try update { try self.select(caret, (b, end, false)) }
        let before = model
        let depth = undoDepth()
        _ = try editor.dispatchCommand(PASTE_COMMAND, pasteEvent(text))
        try editor.read {}
        model[b].removeSubrange(offset..<end)
        insertChars(&model[b], offset, chars(text), selected)
        if undoDepth() > depth {
          past.append(before)
          future.removeAll()
        }
      case "ime":
        if length == 0 {
          continue
        }
        let c = nextLetter()
        try update { try self.select(caret, caret) }
        try commit({ try dispatchCompositionStart(self.editor) }, {})
        try commit(
          {
            // What the IME writes into the composing node
            guard let selection = try getSelection() as? RangeSelection else {
              throw LexicalError("no selection while composing")
            }
            let node = try selection.anchor.getNode() as! TextNode
            let at = selection.anchor.offset
            let units = Array(try node.getTextContent().utf16)
            let cu = c.utf16.count
            if at > 0 && units[at - 1] == 0x200B {
              _ = try node.setTextContent(slice(units, 0, at - 1) + c + slice(units, at))
              _ = try node.select(at - 1 + cu, at - 1 + cu)
            } else if at < units.count && units[at] == 0x200B {
              _ = try node.setTextContent(slice(units, 0, at) + c + slice(units, at + 1))
              _ = try node.select(at + cu, at + cu)
            } else {
              _ = try node.setTextContent(slice(units, 0, at) + c + slice(units, at))
              _ = try node.select(at + cu, at + cu)
            }
          }, {})
        try commit({ try setCompositionKey(nil) }, { insertChars(&model[b], offset, chars(c)) })
      case "backspace", "delete":
        let backward = op == "backspace"
        if backward && b == 0 && offset == 0 {
          continue
        }
        if backward && offset > 0 {
          try update { try self.select((b, offset - 1, true), (b, offset, false)) }
        } else if !backward && offset < length {
          try update { try self.select((b, offset, true), (b, offset + 1, false)) }
        } else {
          try update { try self.select(caret, caret) }
        }
        try commit(
          { _ = try self.editor.dispatchCommand(DELETE_CHARACTER_COMMAND, backward) },
          {
            if backward && offset > 0 {
              model[b].remove(at: offset - 1)
            } else if backward {
              model[b - 1].append(contentsOf: model[b])
              model.remove(at: b)
            } else if offset < length {
              model[b].remove(at: offset)
            } else if b < model.count - 1 {
              model[b].append(contentsOf: model[b + 1])
              model.remove(at: b + 1)
            }
          })
      case "deleteRange":
        let b2 = min(model.count - 1, b + (random.next() < 0.4 ? 1 : 0))
        var o2 = random.int(model[b2].count + 1)
        if b2 == b && o2 < offset {
          o2 = offset + random.int(length - offset + 1)
        }
        if b2 == b && o2 == offset {
          continue
        }
        note += " to \(b2):\(o2)"
        let backward = random.next() < 0.5
        try commit(
          {
            try self.select((b, offset, true), (b2, o2, false), backward)
            _ = try self.editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
          },
          {
            if b2 == b {
              model[b].removeSubrange(offset..<o2)
            } else {
              model[b] = Array(model[b][..<offset]) + Array(model[b2][o2...])
              model.remove(at: b2)
            }
          })
      case "enter":
        try commit(
          {
            try self.select(caret, caret)
            _ = try self.editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
          },
          {
            let rest = Array(model[b][offset...])
            model[b].removeSubrange(offset...)
            model.insert(rest, at: b + 1)
          })
      case "format":
        if length == 0 {
          continue
        }
        let o1 = random.int(length)
        let o2 = o1 + 1 + random.int(length - o1)
        let format = random.next() < 0.5 ? "bold" : "italic"
        note += " \(format) [\(o1),\(o2))"
        let backward = random.next() < 0.5
        try commit(
          {
            try self.select((b, o1, true), (b, o2, false), backward)
            _ = try self.editor.dispatchCommand(FORMAT_TEXT_COMMAND, format)
          }, {})
      case "blockType":
        let heading = random.next() < 0.5
        try commit(
          {
            try self.select(caret, caret)
            try setBlocksType(getSelection()) { () throws -> ElementNode in
              heading ? try createHeadingNode("h1") : try createParagraphNode()
            }
          }, {})
      case "mark", "unmark":
        let id = pick(IDS)
        let b2 = min(model.count - 1, b + (random.next() < 0.3 ? 1 : 0))
        var o2 = random.int(model[b2].count + 1)
        if b2 == b && o2 < offset {
          o2 = offset + random.int(length - offset + 1)
        }
        let backward = random.next() < 0.5
        let everywhere = op == "unmark" && random.next() < 0.3
        note += " \(id) to \(b2):\(o2)\(everywhere ? " everywhere" : "")"
        try commit(
          {
            let selection = try self.select((b, offset, true), (b2, o2, false), backward)
            if op == "mark" {
              try addMarkID(self.flavorOf(id), id, selection)
            } else {
              try removeMarkID(self.flavorOf(id), id, everywhere ? nil : selection)
            }
          },
          {
            func apply(_ ch: Char) -> Char {
              Char(c: ch.c, ids: op == "mark" ? union([ch.ids, [id]]) : ch.ids.filter { $0 != id })
            }
            if everywhere {
              model = model.map { $0.map(apply) }
            } else if b2 == b {
              model[b].replaceSubrange(offset..<o2, with: model[b][offset..<o2].map(apply))
            } else {
              model[b].replaceSubrange(offset..., with: model[b][offset...].map(apply))
              model[b2].replaceSubrange(..<o2, with: model[b2][..<o2].map(apply))
            }
          })
      case "copyPaste", "cutPaste":
        if length == 0 {
          continue
        }
        let o1 = random.int(length)
        let o2 = o1 + 1 + random.int(length - o1)
        let copied = Array(model[b][o1..<o2])
        var json: SerializedSelectedNodes? = nil
        try commit(
          {
            let selection = try self.select((b, o1, true), (b, o2, false))
            json = try generateJSONFromSelectedNodes(self.editor, selection)
            if op == "cutPaste" {
              try selection.removeText()
            }
          },
          {
            if op == "cutPaste" {
              model[b].removeSubrange(o1..<o2)
            }
          })
        let tb = random.int(model.count)
        let to = random.int(model[tb].count + 1)
        note += " [\(o1),\(o2)) to \(tb):\(to)"
        try commit(
          {
            let selection = try self.select((tb, to, preferNext), (tb, to, preferNext))
            try insertGeneratedNodes(
              self.editor, generateNodesFromSerializedNodes(json!.nodes), selection)
          }, { insertChars(&model[tb], to, copied) })
      default:  // undo, redo
        let depth = undoDepth()
        let redoDepth = history.peek().redoStack.count
        _ = try editor.dispatchCommand(op == "undo" ? UNDO_COMMAND : REDO_COMMAND)
        try editor.read {}
        if op == "undo" && undoDepth() < depth {
          future.append(model)
          model = past.removeLast()
        } else if op == "redo" && history.peek().redoStack.count < redoDepth {
          past.append(model)
          model = future.removeLast()
        }
      }
      if model.isEmpty {
        model = [[]]
      }
      log.append("\(note) => \(describeModel(model))")
      try check(model)
    }
  }
}

@Suite struct MarkStatePropertyTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateProperty.test.ts::MarkStateExtension property test > 40 random edit sequences of 30 steps keep the ids of every character"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateProperty.test.ts::MarkStateExtension property test with two mark flavors > 40 random edit sequences of 30 steps keep the ids of every character"
    ),
    arguments: [false, true])
  func randomEditSequences(_ twoFlavors: Bool) throws {
    let env = ProcessInfo.processInfo.environment
    let seeds = env["MARK_STATE_PROPERTY_SEEDS"].flatMap { Int($0) } ?? 40
    let steps = env["MARK_STATE_PROPERTY_STEPS"].flatMap { Int($0) } ?? 30
    let ops = (env["MARK_STATE_PROPERTY_OPS"] ?? "").split(separator: ",").map(String.init)
      .filter { OPS.contains($0) }
    var failures: [String] = []
    for seed in 1...seeds {
      do {
        try PropertyRun(seed: seed, twoFlavors: twoFlavors).run(steps, ops.isEmpty ? OPS : ops)
      } catch {
        failures.append("seed \(seed): \(error)")
      }
    }
    #expect(failures.prefix(3).joined(separator: "\n\n") == "")
    #expect(failures.count == 0)
  }
}
