import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalNormalizationComposition.test.ts.
// Without DEFER_COMPOSITION_MERGE_TAG, an IME composition does not change how
// text nodes are normalized: every scenario runs once with the composition
// key on the composed node and once without, and the two runs must produce
// identical editor states (JSON) and node identities.

nonisolated(unsafe) private let ops: [(name: String, run: (TextNode) throws -> Void)] = [
  ("insert before", { composed in _ = try composed.insertBefore(try createTextNode("j1z")) }),
  ("insert after", { composed in _ = try composed.insertAfter(try createTextNode("j3z")) }),
  (
    "insert on both sides",
    { composed in
      _ = try composed.insertBefore(try createTextNode("j1z"))
      _ = try composed.insertAfter(try createTextNode("j3z"))
    }
  ),
  // The bold node between plain text and the composed node goes away.
  ("remove the bold node before", { composed in try composed.getPreviousSibling()!.remove() }),
  ("remove the bold node after", { composed in try composed.getNextSibling()!.remove() }),
  (
    "unbold the node before",
    { composed in _ = try (try composed.getPreviousSibling() as! TextNode).setFormat(0) }
  ),
  // A line join from another source: the next line's text moves next to the
  // composition.
  (
    "join the next line",
    { composed in
      let line = try composed.getParentOrThrow()
      let next = try line.getNextSibling() as! ElementNode
      try line.append(contentsOf: try next.getChildren())
      try next.remove()
    }
  ),
]

private struct Step: Equatable {
  let json: String
  let nodes: [String]
}

/// The text node holding the composed text (it may have been merged).
private func composedText() throws -> TextNode {
  for node in try getRoot().getAllTextNodes() where try node.getTextContent().contains("に") {
    return node
  }
  throw LexicalError("the composed text is gone")
}

private func run(
  _ opIndex: Int, _ tag: UpdateTag?, _ skipTransforms: Bool, _ composing: Bool
)
  throws -> [Step]
{
  let op = ops[opIndex]
  let editor = try createEditor(
    CreateEditorArgs(namespace: "test", onError: { throw $0 }))
  var labels: [NodeKey: String] = [:]
  let snapshot = {
    try editor.read {
      Step(
        json: try editor.getEditorState().toJSON().stringify(),
        nodes: try getRoot().getChildren().flatMap { try ($0 as! ElementNode).getChildren() }.map {
          n in
          "\(labels[n.getKey()] ?? "new"):\(try n.getTextContent())"
            + (try (n as? TextNode)?.getFormat() ?? 0 != 0 ? "{b}" : "")
        })
    }
  }
  var steps: [Step] = []
  try editor.update(discrete: true) {
    let before = try createTextNode("j1z")
    let bold1 = try createTextNode("B").toggleFormat("bold")
    let composed = try createTextNode("にj2z")
    let bold2 = try createTextNode("C").toggleFormat("bold")
    let after = try createTextNode("j3z")
    let joined = try createTextNode("k1z")
    let isBold = op.name.contains("bold")
    try getRoot().clear().append(
      try createParagraphNode().append(
        contentsOf: isBold ? [before, bold1, composed, bold2, after] : [composed]),
      try createParagraphNode().append(joined))
    for (node, label) in [
      (before, "before"), (bold1, "bold1"), (composed, "composed"), (bold2, "bold2"),
      (after, "after"), (joined, "joined"),
    ] {
      labels[node.getKey()] = label
    }
    _ = try composed.select(1, 1)
    if composing {
      try setCompositionKey(composed.getKey())
    } else {
      _ = try composed.getWritable()
    }
  }
  steps.append(try snapshot())
  try editor.update(
    { try op.run(try composedText()) },
    EditorUpdateOptions(skipTransforms: skipTransforms, tag: tag.map { [$0] } ?? [], discrete: true)
  )
  steps.append(try snapshot())
  // The next composition step: the composed node's text changes.
  try editor.update(discrete: true) {
    let node = try composedText()
    _ = try node.setTextContent(
      try node.getTextContent().replacingOccurrences(of: "に", with: "にほ"))
  }
  steps.append(try snapshot())
  // The composition ends.
  try editor.update(discrete: true) {
    if composing {
      try setCompositionKey(nil)
    } else {
      _ = try composedText().getWritable()
    }
  }
  steps.append(try snapshot())
  return steps
}

@Suite struct LexicalNormalizationCompositionTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-end, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag (none)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag (none), skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag collaboration"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag collaboration, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag historic"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag historic, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag history-merge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag history-merge, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag paste"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag paste, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag skip-collab"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag skip-collab, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-start, skipTransforms"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-end, skipTransforms"
    ),
    arguments: [
      (0, nil, false),
      (0, nil, true),
      (0, "collaboration", false),
      (0, "collaboration", true),
      (0, "historic", false),
      (0, "historic", true),
      (0, "history-merge", false),
      (0, "history-merge", true),
      (0, "paste", false),
      (0, "paste", true),
      (0, "skip-collab", false),
      (0, "skip-collab", true),
      (0, "composition-start", false),
      (0, "composition-start", true),
      (0, "composition-end", false),
      (0, "composition-end", true),
      (1, nil, false),
      (1, nil, true),
      (1, "collaboration", false),
      (1, "collaboration", true),
      (1, "historic", false),
      (1, "historic", true),
      (1, "history-merge", false),
      (1, "history-merge", true),
      (1, "paste", false),
      (1, "paste", true),
      (1, "skip-collab", false),
      (1, "skip-collab", true),
      (1, "composition-start", false),
      (1, "composition-start", true),
      (1, "composition-end", false),
      (1, "composition-end", true),
      (2, nil, false),
      (2, nil, true),
      (2, "collaboration", false),
      (2, "collaboration", true),
      (2, "historic", false),
      (2, "historic", true),
      (2, "history-merge", false),
      (2, "history-merge", true),
      (2, "paste", false),
      (2, "paste", true),
      (2, "skip-collab", false),
      (2, "skip-collab", true),
      (2, "composition-start", false),
      (2, "composition-start", true),
      (2, "composition-end", false),
      (2, "composition-end", true),
      (3, nil, false),
      (3, nil, true),
      (3, "collaboration", false),
      (3, "collaboration", true),
      (3, "historic", false),
      (3, "historic", true),
      (3, "history-merge", false),
      (3, "history-merge", true),
      (3, "paste", false),
      (3, "paste", true),
      (3, "skip-collab", false),
      (3, "skip-collab", true),
      (3, "composition-start", false),
      (3, "composition-start", true),
      (3, "composition-end", false),
      (3, "composition-end", true),
      (4, nil, false),
      (4, nil, true),
      (4, "collaboration", false),
      (4, "collaboration", true),
      (4, "historic", false),
      (4, "historic", true),
      (4, "history-merge", false),
      (4, "history-merge", true),
      (4, "paste", false),
      (4, "paste", true),
      (4, "skip-collab", false),
      (4, "skip-collab", true),
      (4, "composition-start", false),
      (4, "composition-start", true),
      (4, "composition-end", false),
      (4, "composition-end", true),
      (5, nil, false),
      (5, nil, true),
      (5, "collaboration", false),
      (5, "collaboration", true),
      (5, "historic", false),
      (5, "historic", true),
      (5, "history-merge", false),
      (5, "history-merge", true),
      (5, "paste", false),
      (5, "paste", true),
      (5, "skip-collab", false),
      (5, "skip-collab", true),
      (5, "composition-start", false),
      (5, "composition-start", true),
      (5, "composition-end", false),
      (5, "composition-end", true),
      (6, nil, false),
      (6, nil, true),
      (6, "collaboration", false),
      (6, "collaboration", true),
      (6, "historic", false),
      (6, "historic", true),
      (6, "history-merge", false),
      (6, "history-merge", true),
      (6, "paste", false),
      (6, "paste", true),
      (6, "skip-collab", false),
      (6, "skip-collab", true),
      (6, "composition-start", false),
      (6, "composition-start", true),
      (6, "composition-end", false),
      (6, "composition-end", true),
    ] as [(Int, UpdateTag?, Bool)])
  func composingMatchesNotComposing(opIndex: Int, tag: UpdateTag?, skipTransforms: Bool) throws {
    #expect(
      try run(opIndex, tag, skipTransforms, true) == run(opIndex, tag, skipTransforms, false))
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > text inserted before the composed node merges with it at once"
    ))
  func insertedTextMergesAtOnce() throws {
    let steps = try run(0, nil, false, true)
    #expect(
      steps.map(\.nodes) == [
        ["composed:にj2z", "joined:k1z"],
        ["new:j1zにj2z", "joined:k1z"],
        ["new:j1zにほj2z", "joined:k1z"],
        ["new:j1zにほj2z", "joined:k1z"],
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > a line joined into the composing line merges at once"
    ))
  func joinedLineMergesAtOnce() throws {
    let steps = try run(6, nil, true, true)
    #expect(
      steps.map(\.nodes) == [
        ["composed:にj2z", "joined:k1z"],
        ["composed:にj2zk1z"],
        ["composed:にほj2zk1z"],
        ["composed:にほj2zk1z"],
      ])
  }
}
