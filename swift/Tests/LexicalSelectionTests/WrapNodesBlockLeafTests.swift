import Foundation
import Lexical
import LexicalExtension
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts.
//
// #9638: a block leaf (a horizontal rule) in an element that is not a block
// container (a box that declares no structure): $wrapNodes took the box for
// the rule's block, moved all the box's children into one new element, those
// before the range included, and put the new elements next to one of them,
// inside that new element. The update never returned. A block leaf there is
// now wrapped on its own, as in a block container.
//
// A cycle in the tree makes the update walk parents forever: a guard turns
// that into an error, so a hang fails the test instead of the run. Upstream
// patches `ElementNode.prototype.getParent`; the port sets the thread's
// `elementGetParentHook`.
private let LIMIT = 1_000_000
private struct ParentCycleError: Error, CustomStringConvertible {
  var description: String { "Error: getParent called more than a million times: a cycle" }
}

/// Runs `body` with the `getParent` call limit; `reset` restarts the count.
private func withParentCallLimit<T>(_ body: (_ reset: () -> Void) throws -> T) rethrows -> T {
  var parentCalls = 0
  let context = ThreadContext.current
  context.elementGetParentHook = {
    parentCalls += 1
    if parentCalls > LIMIT { throw ParentCycleError() }
  }
  defer { context.elementGetParentHook = nil }
  return try body { parentCalls = 0 }
}

/// A plain block element that holds blocks, with no declared structure.
private final class BoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BoxNode.self, type: "test_wrap_box", extends: ElementNode.self)
  }
}

private func build() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "wrap-block-leaf", dependencies: [HorizontalRuleExtension],
      nodes: { [.klass(BoxNode.self), .klass(QuoteNode.self)] },
      // LexicalBuilder's `defaultOnError`, which rethrows.
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
}

/// The texts and rules in document order, and the quotes' texts.
private func outline(_ editor: Editor) throws -> (all: [String], quoted: [String]) {
  try editor.read {
    var all: [String] = []
    var quoted: [String] = []
    func walk(_ node: LexicalNode, _ inQuote: Bool) throws {
      if let text = node as? TextNode {
        all.append(try text.getTextContent())
        if inQuote { quoted.append(try text.getTextContent()) }
      } else if isHorizontalRuleNode(node) {
        all.append("<hr>")
        if inQuote { quoted.append("<hr>") }
      } else if let element = node as? ElementNode {
        let quote = inQuote || element.getType() == "quote"
        for child in try element.getChildren() { try walk(child, quote) }
      }
    }
    try walk(try getRoot(), false)
    return (all, quoted)
  }
}

private func paragraph(_ text: TextNode) throws -> ParagraphNode {
  try createParagraphNode().append(text)
}

// Random documents of paragraphs, rules (adjacent ones too), boxes (nested
// up to twice) and, with `directText`, text directly in a box, and random
// ranges between text and element points: the documents where $wrapNodes
// throws or does not keep every text in order.
private func wrapFailures(_ directText: Bool) throws -> [String] {
  try withParentCallLimit { reset in try wrapFailures(directText, reset) }
}

private func wrapFailures(_ directText: Bool, _ resetParentCalls: () -> Void) throws -> [String] {
  // The JS LCG in doubles: the product exceeds 2^53 and rounds as in JS.
  var seed: Double = 9638
  func random() -> Double {
    seed = (seed * 1_103_515_245 + 12345).truncatingRemainder(dividingBy: 2_147_483_648)
    return seed / 2_147_483_648
  }
  func pick(_ count: Int) -> Int { Int((random() * Double(count)).rounded(.down)) }
  var failures: [String] = []
  for run in 0..<500 {
    resetParentCalls()
    let editor = try build()
    defer { editor.dispose() }
    var n = 0
    func children(_ depth: Int) throws -> [LexicalNode] {
      var out: [LexicalNode] = []
      let count = 1 + pick(4)
      for _ in 0..<count {
        let r = random()
        if r < 0.25 {
          out.append(try createHorizontalRuleNode())
        } else if r < 0.4 && depth < 2 {
          out.append(try create(BoxNode.self).append(contentsOf: try children(depth + 1)))
        } else if directText && r < 0.55 && depth > 0 {
          // Text directly in a box, next to its blocks.
          out.append(try createTextNode("t\(n)"))
          n += 1
        } else {
          out.append(try paragraph(try createTextNode("t\(n)")))
          n += 1
        }
      }
      return out
    }
    var before: [String] = []
    var error: (any Error)?
    do {
      try editor.update(discrete: true) {
        try getRoot().clear().append(contentsOf: try children(0))
        before = try getRoot().getAllTextNodes().map { try $0.getTextContent() }
        var points: [(NodeKey, Int, PointType)] = []
        func collect(_ node: LexicalNode) throws {
          if let text = node as? TextNode {
            for o in 0...(try text.getTextContentSize()) {
              points.append((text.getKey(), o, .text))
            }
          } else if let element = node as? ElementNode {
            for o in 0...(try element.getChildrenSize()) {
              points.append((element.getKey(), o, .element))
            }
            for child in try element.getChildren() { try collect(child) }
          }
        }
        try collect(try getRoot())
        let a = points[pick(points.count)]
        let b = points[pick(points.count)]
        let selection = createRangeSelection()
        try selection.anchor.set(a.0, a.1, a.2)
        try selection.focus.set(b.0, b.1, b.2)
        try setSelection(selection)
        try wrapNodes(selection) { try createQuoteNode() }
      }
    } catch let e {
      error = e
    }
    let after = try outline(editor).all.filter { $0 != "<hr>" }
    // Adjacent texts may merge: compare the text in order.
    if error != nil || after.joined() != before.joined() {
      failures.append(
        "run \(run): \(error.map { String(describing: $0) } ?? after.joined(separator: ","))")
    }
  }
  return failures
}

@Suite struct WrapNodesBlockLeafTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a range across a horizontal rule in a box keeps every paragraph"
    ))
  func rangeAcrossRuleInBox() throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let bb = try createTextNode("bb")
      let cc = try createTextNode("cc")
      try getRoot().clear().append(
        try create(BoxNode.self).append(
          try paragraph(try createTextNode("aa")), try paragraph(bb),
          try createHorizontalRuleNode(), try paragraph(cc)))
      let selection = createRangeSelection()
      try selection.anchor.set(bb.getKey(), 1, .text)
      try selection.focus.set(cc.getKey(), 2, .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    let result = try outline(editor)
    #expect(result.all == ["aa", "bb", "<hr>", "cc"])
    #expect(result.quoted == ["bb", "<hr>", "cc"])
    // The rule stays in the box, next to the paragraphs.
    #expect(
      try editor.read { try getRoot().getChildren().map { $0.getType() } } == ["test_wrap_box"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a range that starts at the rule"
    ))
  func rangeStartsAtRule() throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let cc = try createTextNode("cc")
      let box = try create(BoxNode.self).append(
        try paragraph(try createTextNode("aa")), try createHorizontalRuleNode(), try paragraph(cc))
      try getRoot().clear().append(box)
      let selection = createRangeSelection()
      try selection.anchor.set(box.getKey(), 1, .element)
      try selection.focus.set(cc.getKey(), 2, .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    let result = try outline(editor)
    #expect(result.all == ["aa", "<hr>", "cc"])
    #expect(result.quoted == ["<hr>", "cc"])
  }

  // A box that holds text directly is the block of that text and of the
  // rules next to it: its children move together, as before.
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a box with text and two rules in it, from the second rule (forward)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > a box with text and two rules in it, from the second rule (backward)"
    ),
    arguments: [false, true])
  func boxWithTextAndTwoRules(backward: Bool) throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let cc = try createTextNode("cc")
      let box = try create(BoxNode.self).append(
        try createTextNode("aa"), try createHorizontalRuleNode(), try createHorizontalRuleNode(),
        cc)
      try getRoot().clear().append(box)
      let selection = createRangeSelection()
      let (from, to) =
        backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try from.set(box.getKey(), 2, .element)
      try to.set(cc.getKey(), 2, .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    #expect(try outline(editor).all == ["aa", "<hr>", "<hr>", "cc"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > control: the same range without the rule"
    ))
  func controlWithoutRule() throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let bb = try createTextNode("bb")
      let cc = try createTextNode("cc")
      try getRoot().clear().append(
        try create(BoxNode.self).append(
          try paragraph(try createTextNode("aa")), try paragraph(bb), try paragraph(cc)))
      let selection = createRangeSelection()
      try selection.anchor.set(bb.getKey(), 1, .text)
      try selection.focus.set(cc.getKey(), 2, .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    let result = try outline(editor)
    #expect(result.all == ["aa", "bb", "cc"])
    #expect(result.quoted == ["bb", "cc"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > control: a rule at the root"
    ))
  func controlRuleAtRoot() throws {
    let editor = try build()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let bb = try createTextNode("bb")
      let cc = try createTextNode("cc")
      try getRoot().clear().append(
        try paragraph(try createTextNode("aa")), try paragraph(bb),
        try createHorizontalRuleNode(), try paragraph(cc))
      let selection = createRangeSelection()
      try selection.anchor.set(bb.getKey(), 1, .text)
      try selection.focus.set(cc.getKey(), 2, .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    let result = try outline(editor)
    #expect(result.all == ["aa", "bb", "<hr>", "cc"])
    #expect(result.quoted == ["bb", "<hr>", "cc"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > property: random box, rule and paragraph mixes"
    ))
  func propertyRandomMixes() throws {
    #expect(try wrapFailures(false) == [])
  }

  // With text directly in a box next to its blocks, $wrapNodes reorders text
  // or never returns in some documents on main too (#9665): pinned until
  // that is fixed.
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesBlockLeaf.test.ts::$wrapNodes with a block leaf in an element that is not a block container (#9638) > property: with text directly in a box (#9665)"
    ))
  func propertyWithDirectText() throws {
    let failures = try wrapFailures(true)
    withKnownIssue("#9665") {
      #expect(failures == [])
    }
  }
}
