import Foundation
import Lexical
import LexicalHashtag
import LexicalMark
import LexicalTesting
import LexicalText
import Testing

// Port of
// packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts.
//
// Property: in random layouts of entities and text in several formats, with
// NodeState on entities and text and some text in MarkNodes, an edit that
// turns an entity back into text leaves every character of that entity that
// is still there with the entity's NodeState (or inside another entity that
// grew over it, which takes characters with its own NodeState), and inside
// the same MarkNode. Each entity is written with a letter no other text has,
// which tells its characters apart after the edit.

nonisolated(unsafe) private let note = createState("note", parse: { $0?.stringValue ?? "" })

private final class MentionNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(MentionNode.self, type: "property-mention", extends: TextNode.self)
  }
  override func isTextEntity() -> Bool { true }
}

private enum Kind: String, CaseIterable {
  case hashtag, mention
}

private let SEEDS =
  Int(ProcessInfo.processInfo.environment["TEXT_ENTITY_NODE_STATE_SEEDS"] ?? "")
  ?? 200
private let ENTITY_LETTERS = Array("abcdefghijklmnop")
private let PLAIN = [" ", " ", "x", "y", ".", "#", "@", "xy", " x "]
private let TYPED = [" ", "x", "#", ".", "@", "z"]
private let FORMATS = [0, 0, 1, 2]

/// Mulberry32, as upstream's, so a failure names a seed that reproduces it.
private func rng(_ seed: Int) -> () -> Double {
  var a = UInt32(truncatingIfNeeded: seed)
  return {
    a = a &+ 0x6d2b_79f5
    var t = a
    t = (t ^ (t >> 15)) &* (t | 1)
    t ^= t &+ ((t ^ (t >> 7)) &* (t | 61))
    return Double(t ^ (t >> 14)) / 4_294_967_296
  }
}

private func createEntityEditor(_ kind: Kind) throws -> Editor {
  let editor = try createEditor(
    CreateEditorArgs(
      nodes: [.klass(HashtagNode.self), .klass(MentionNode.self), .klass(MarkNode.self)],
      onError: { throw $0 }))
  try editor.setRootElement(TestRootElement())
  switch kind {
  case .hashtag:
    _ = try registerLexicalTextEntity(
      editor, HashtagConfig().getHashtagMatch, HashtagNode.self
    ) { try createHashtagNode($0.getTextContent()) }
  case .mention:
    _ = try registerLexicalTextEntity(editor, regexMatch("@[a-z0-9]+"), MentionNode.self) {
      try create(MentionNode.self).setTextContent($0.getTextContent()).setMode("token")
    }
  }
  return editor
}

private func createEntity(_ kind: Kind, _ text: String) throws -> TextNode {
  switch kind {
  case .hashtag: try createHashtagNode(text)
  case .mention: try create(MentionNode.self).setTextContent(text).setMode("token")
  }
}

private struct Info: Equatable, CustomStringConvertible {
  var entity: Bool
  var mark: String
  var note: String
  var text: String

  var description: String {
    "{entity: \(entity), mark: \(mark), note: \(note), text: \(text.debugDescription)}"
  }
}

private func read(_ editor: Editor) throws -> [Info] {
  try editor.read {
    try getRoot().getAllTextNodes().map { n in
      let parent = try n.getParent()
      return Info(
        entity: try n.isTextEntity(),
        mark: try (parent as? MarkNode)?.getIDs().joined(separator: ",") ?? "",
        note: try getState(n, note), text: try n.getTextContent())
    }
  }
}

private func check(_ label: String, _ before: [Info], _ after: [Info], _ failures: inout [String]) {
  // The entities before the edit, by their own letter
  for node in before where node.entity && !node.note.isEmpty {
    guard let letter = node.text.first(where: { ENTITY_LETTERS.contains($0) }) else { continue }
    for now in after where now.text.contains(letter) {
      if now.note == node.note && now.mark == node.mark { continue }
      // Characters that another entity grew over (an entity takes the text
      // its match extends into with its own NodeState, as before): not at
      // the start of an entity, right after its sigil
      if now.entity, let index = now.text.firstIndex(of: letter),
        now.text.distance(from: now.text.startIndex, to: index) > 1
      {
        continue
      }
      failures.append(
        "\(label): the characters of \(node) are in \(now); before \(before) after \(after)")
      return
    }
  }
}

@Suite struct RegisterLexicalTextEntityNodeStatePropertyTests {
  @Test(
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts::registerLexicalTextEntity keeps NodeState (property) > hashtag: an edit that turns an entity into text keeps its NodeState and marks on its characters"
    ),
    .upstream(
      "lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts::registerLexicalTextEntity keeps NodeState (property) > mention: an edit that turns an entity into text keeps its NodeState and marks on its characters"
    ),
    arguments: Kind.allCases)
  fileprivate func editKeepsNodeStateAndMarks(_ kind: Kind) throws {
    var failures: [String] = []
    var demoted = 0
    var seed = 0
    while seed < SEEDS && failures.count < 3 {
      defer { seed += 1 }
      let random = rng(seed * 11 + (kind == .hashtag ? 1 : 2))
      func pick<V>(_ values: [V]) -> V { values[Int(random() * Double(values.count))] }
      do {
        let editor = try createEntityEditor(kind)
        var letter = 0
        try editor.update(discrete: true) {
          let blocks = 1 + Int(random() * 3)
          for _ in 0..<blocks {
            let paragraph = try createParagraphNode()
            let runs = 1 + Int(random() * 5)
            for _ in 0..<runs {
              var node: TextNode
              if random() < 0.45 && letter < ENTITY_LETTERS.count {
                let own = ENTITY_LETTERS[letter]
                letter += 1
                node = try setState(
                  createEntity(
                    kind, (kind == .hashtag ? "#" : "@") + String(own) + (random() < 0.5 ? "1" : "")
                  ),
                  note, "e\(letter)")
              } else {
                node = try createTextNode(pick(PLAIN) + pick(PLAIN))
                if random() < 0.5 { try setState(node, note, "t\(Int(random() * 3))") }
              }
              _ = try node.setFormat(pick(FORMATS))
              try paragraph.append(
                random() < 0.2 ? createMarkNode(["m\(Int(random() * 2))"]).append(node) : node)
            }
            try getRoot().append(paragraph)
          }
        }
        for edit in 0..<4 {
          let before = try read(editor)
          let op = random()
          var description = ""
          try editor.update(discrete: true) {
            let nodes = try getRoot().getAllTextNodes()
            if nodes.isEmpty { return }
            let node = pick(nodes)
            let index = { (n: TextNode) in nodes.firstIndex { $0 === n }! }
            let offset = Int(random() * Double(try node.getTextContentSize() + 1))
            if op < 0.35 {
              let typed = pick(TYPED)
              description = "type \(typed.debugDescription) at \(index(node)):\(offset)"
              try node.select(offset, offset).insertText(typed)
            } else if op < 0.6 {
              let backward = random() < 0.5
              description = "\(backward ? "backspace" : "delete") at \(index(node)):\(offset)"
              try node.select(offset, offset).deleteCharacter(backward)
            } else if op < 0.8 {
              // Join a block onto the one before it (backspace at its start)
              let blocks = try getRoot().getChildren().compactMap { $0 as? ElementNode }
              let block = pick(blocks)
              description = "join block \(blocks.firstIndex { $0 === block }!)"
              if let first = try block.getFirstDescendant() as? TextNode {
                try first.select(0, 0).deleteCharacter(true)
              }
            } else {
              // Delete a range across nodes
              let other = pick(nodes)
              let (a, b) = index(node) <= index(other) ? (node, other) : (other, node)
              let selection = createRangeSelection()
              let from = min(offset, try a.getTextContentSize())
              let to = Int(random() * Double(try b.getTextContentSize() + 1))
              try selection.anchor.set(a.getKey(), from, .text)
              try selection.focus.set(b.getKey(), a === b ? max(from, to) : to, .text)
              description = "remove \(index(a)):\(from)-\(index(b)):\(to)"
              try setSelection(selection)
              try selection.removeText()
            }
            try setSelection(nil)
          }
          let after = try read(editor)
          if after.filter(\.entity).count < before.filter(\.entity).count { demoted += 1 }
          check("seed \(seed) edit \(edit) \(description)", before, after, &failures)
        }
      } catch {
        failures.append("seed \(seed): \(error)")
      }
    }
    #expect(failures == [])
    // The property was exercised
    #expect(Double(demoted) > Double(SEEDS) / 10)
  }
}
