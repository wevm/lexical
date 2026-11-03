import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-utils/src/__tests__/unit
// LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts: every scenario runs
// with every combination of the `first`/`last` split edges and
// `removeEmptyDestination`, in a rich-text editor.

private func serialize(_ node: LexicalNode) throws -> String {
  guard let element = node as? ElementNode else { return try node.getTextContent() }
  return "[\(try element.getChildren().map(serialize).joined())]"
}

private func rootSerialize() throws -> String {
  try getRoot().getChildren().map(serialize).joined()
}

private func createLiftable(_ text: String) throws -> LexicalNode {
  try createParagraphNode().append(try createTextNode(text))
}

/// How `shouldSplit` answers on each edge, and `removeEmptyDestination`.
private struct SplitFlags {
  let first: Bool
  let last: Bool
  let removeEmptyDestination: Bool
}

private struct Scenario {
  let label: String
  let run: (SplitAtPointCaretNextOptions) throws -> Void
  let expected: (SplitFlags) -> String
}

/// The expectation shared by the scenarios with an empty destination.
private func emptyDestination(_ flags: SplitFlags) -> String {
  flags.removeEmptyDestination
    ? "[x]" : "\(flags.first ? "[]" : "")[x]\(flags.last || !flags.first ? "[]" : "")"
}

/// The expectation of the scenarios that insert after a sibling.
private func afterSibling(_ before: String) -> (SplitFlags) -> String {
  { flags in "\(before)[x]\(flags.last && !flags.removeEmptyDestination ? "[]" : "")" }
}

private func beforeSibling(_ after: String) -> (SplitFlags) -> String {
  { flags in "\(flags.first ? "[]" : "")[x]\(after)" }
}

nonisolated(unsafe) private let scenarios: [Scenario] = [
  Scenario(
    label: "sole child, caret on node (previous) — caret.origin === node",
    run: { options in
      let x = try createLiftable("x")
      _ = try getRoot().append(try createParagraphNode().append(x))
      try insertNodeToNearestRootAtCaret(x, getSiblingCaret(x, .previous), options)
    }, expected: emptyDestination),
  Scenario(
    label: "sole child, caret on node (next) — caret.origin === node",
    run: { options in
      let x = try createLiftable("x")
      _ = try getRoot().append(try createParagraphNode().append(x))
      try insertNodeToNearestRootAtCaret(x, getSiblingCaret(x, .next), options)
    }, expected: emptyDestination),
  Scenario(
    label: "empty, caret on paragraph (next)",
    run: { options in
      let x = try createLiftable("x")
      let p = try createParagraphNode()
      _ = try getRoot().append(p)
      try insertNodeToNearestRootAtCaret(x, getChildCaret(p, .next), options)
    }, expected: emptyDestination),
  Scenario(
    label: "empty, caret on paragraph (previous)",
    run: { options in
      let x = try createLiftable("x")
      let p = try createParagraphNode()
      _ = try getRoot().append(p)
      try insertNodeToNearestRootAtCaret(x, getChildCaret(p, .previous), options)
    }, expected: emptyDestination),
  Scenario(
    label: "caret on root (next)",
    run: { options in
      let x = try createLiftable("x")
      try insertNodeToNearestRootAtCaret(x, getChildCaret(try getRoot(), .next), options)
    }, expected: { _ in "[x]" }),
  Scenario(
    label: "sibling before, caret on node (next) — caret.origin === node",
    run: { options in
      let x = try createLiftable("x")
      _ = try getRoot().append(try createParagraphNode().append(try createTextNode("a"), x))
      try insertNodeToNearestRootAtCaret(x, getSiblingCaret(x, .next), options)
    }, expected: afterSibling("[a]")),
  Scenario(
    label: "sibling before, caret on node (previous) — caret.origin === node",
    run: { options in
      let x = try createLiftable("x")
      _ = try getRoot().append(try createParagraphNode().append(try createTextNode("a"), x))
      try insertNodeToNearestRootAtCaret(x, getSiblingCaret(x, .previous), options)
    }, expected: afterSibling("[a]")),
  Scenario(
    label: "sibling before, caret on sibling (previous)",
    run: { options in
      let x = try createLiftable("x")
      let sib = try createTextNode("a")
      _ = try getRoot().append(try createParagraphNode().append(sib, x))
      try insertNodeToNearestRootAtCaret(x, getSiblingCaret(sib, .previous), options)
    }, expected: beforeSibling("[a]")),
  Scenario(
    label: "sibling before, caret on text start (next)",
    run: { options in
      let x = try createLiftable("x")
      let sib = try createTextNode("ab")
      _ = try getRoot().append(try createParagraphNode().append(sib, x))
      try insertNodeToNearestRootAtCaret(x, getTextPointCaret(sib, .next, 0), options)
    }, expected: beforeSibling("[ab]")),
  Scenario(
    label: "sibling before, caret on text middle (next)",
    run: { options in
      let x = try createLiftable("x")
      let sib = try createTextNode("ab")
      _ = try getRoot().append(try createParagraphNode().append(sib, x))
      try insertNodeToNearestRootAtCaret(x, getTextPointCaret(sib, .next, 1), options)
    }, expected: { _ in "[a][x][b]" }),
  Scenario(
    label: "sibling before, caret on text end (next)",
    run: { options in
      let x = try createLiftable("x")
      let sib = try createTextNode("ab")
      _ = try getRoot().append(try createParagraphNode().append(sib, x))
      try insertNodeToNearestRootAtCaret(
        x, getTextPointCaret(sib, .next, .direction(.next)), options)
    }, expected: afterSibling("[ab]")),
]

struct InsertAtCaretCase: CustomTestStringConvertible, Sendable {
  let first: Bool
  let last: Bool
  let removeEmptyDestination: Bool
  let scenario: Int

  var variantName: String {
    let names = [
      ("first", first), ("last", last), ("removeEmptyDestination", removeEmptyDestination),
    ].compactMap { $0.1 ? $0.0 : nil }
    return names.isEmpty ? "no-splits-or-remove" : names.joined(separator: " ")
  }
  var testDescription: String {
    "ParagraphNode with \(variantName) > \(scenarios[scenario].label)"
  }
}

/// The cases in upstream order: each option variant, then each scenario.
private let insertAtCaretCases: [InsertAtCaretCase] = [true, false].flatMap { first in
  [true, false].flatMap { last in
    [true, false].flatMap { removeEmptyDestination in
      scenarios.indices.map {
        InsertAtCaretCase(
          first: first, last: last, removeEmptyDestination: removeEmptyDestination, scenario: $0)
      }
    }
  }
}

@Suite struct InsertNodeToNearestRootAtCaretTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text end (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sole child, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sole child, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > empty, caret on paragraph (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > empty, caret on paragraph (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > caret on root (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on node (next) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on node (previous) — caret.origin === node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on sibling (previous)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text start (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text middle (next)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text end (next)"
    ),
    arguments: insertAtCaretCases)
  func edgeCases(_ testCase: InsertAtCaretCase) throws {
    let flags = SplitFlags(
      first: testCase.first, last: testCase.last,
      removeEmptyDestination: testCase.removeEmptyDestination)
    let options = SplitAtPointCaretNextOptions(
      shouldSplit: { _, edge in
        (flags.first && edge == .first) || (flags.last && edge == .last)
      }, removeEmptyDestination: flags.removeEmptyDestination)
    let scenario = scenarios[testCase.scenario]
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[insert-to-nearest-root]", dependencies: [RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in try scenario.run(options) })
      ))
    defer { editor.dispose() }
    #expect(try editor.read(rootSerialize) == scenario.expected(flags))
  }
}
