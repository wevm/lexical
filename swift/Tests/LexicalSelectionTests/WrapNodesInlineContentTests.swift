import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts.
// The cases that build a LinkNode, MarkNode, OverflowNode or list wait for
// those packages, and are skipped in parity/skips.json with the packages
// they need. The two cost cases count LexicalNode.getParent calls with a spy,
// which has no Swift equivalent, and are skipped as deviations.

/// `InlineShadowNode` from the upstream test.
private final class InlineShadowNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineShadowNode.self, type: "test_wrap_inline_shadow", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
  override func isShadowRoot() -> Bool { true }
  override func canBeEmpty() throws -> Bool { true }
}

/// A block's content as a string: types, nesting and text.
private func content(_ node: LexicalNode) throws -> String {
  if let element = node as? ElementNode {
    return "\(element.getType())(\(try element.getChildren().map(content).joined(separator: ",")))"
  }
  if let text = node as? TextNode {
    return "\"\(try text.getTextContent())\""
  }
  return node.getType()
}

/// A block's content as `content` prints it, with the scope's own content
/// left out.
private func outside(_ node: LexicalNode, _ scopeKey: NodeKey) throws -> String {
  if node.getKey() == scopeKey {
    return "scope"
  }
  if let element = node as? ElementNode {
    let children = try element.getChildren().map { try outside($0, scopeKey) }
    return "\(element.getType())(\(children.joined(separator: ",")))"
  }
  return try content(node)
}

private func textOf(_ parent: ElementNode, _ text: String) throws -> TextNode {
  guard let found = try parent.getAllTextNodes().first(where: { try $0.getTextContent() == text })
  else {
    throw LexicalError("no text node \(text)")
  }
  return found
}

/// The text nodes under `node`, in document order, as key:text.
private func texts(_ node: LexicalNode) throws -> [String] {
  if let element = node as? ElementNode {
    return try element.getChildren().flatMap(texts)
  }
  if let text = node as? TextNode {
    return ["\(text.getKey()):\(try text.getTextContent())"]
  }
  return []
}

private func buildEditor(
  _ name: String, _ structure: Bool, _ nodes: [LexicalNodeConfig]
) throws
  -> Editor
{
  try buildEditorFromExtensions(
    defineExtension(
      name: name, dependencies: structure ? [NodeStructureExtension] : [], nodes: { nodes }))
}

private func structureSuffix(_ structure: Bool) -> String {
  structure ? ", NodeStructureExtension" : ""
}

struct WrapScopeCase: Sendable, CustomTestStringConvertible {
  let scope: String
  let content: String
  let place: String
  let structure: Bool
  var testDescription: String { "\(scope), \(content), \(place)\(structureSuffix(structure))" }

  /// The scopes that use only ported nodes (not the one in a mark).
  static let all: [WrapScopeCase] = [false, true].flatMap { structure in
    [
      "inline, in the paragraph", "block, at the root", "block, the paragraph's slot value",
    ].flatMap { scope in
      [
        ("empty", "caret in the empty scope"), ("holding a paragraph", "caret in the inner text"),
        ("holding a paragraph", "the whole inner text"),
        ("holding text", "caret in the inner text"), ("holding text", "the whole inner text"),
      ].map { WrapScopeCase(scope: scope, content: $0.0, place: $0.1, structure: structure) }
    }
  }
}

/// A part of a mixed container: direct text, or a paragraph holding it.
enum WrapMixPart: Sendable {
  case text(String)
  case paragraph(String)
}

struct WrapMixCase: Sendable, CustomTestStringConvertible {
  let container: String
  let mix: Int
  let span: String
  let backward: Bool
  let structure: Bool
  var testDescription: String {
    "\(container), mix \(mix), \(span)\(backward ? ", backward" : "")\(structureSuffix(structure))"
  }

  /// The upstream MIXES without a paragraph at index 1, which upstream puts
  /// in a MarkNode.
  static let mixes: [Int: [WrapMixPart]] = [
    1: [.paragraph("AA"), .text("BB"), .paragraph("CC")],
    2: [.text("AA"), .text("BB"), .paragraph("CC"), .text("DD")],
    4: [.text("AA"), .text("BB"), .text("CC")],
  ]

  static let all: [WrapMixCase] = [false, true].flatMap { structure in
    [
      "block shadow root at the root", "block shadow root, a paragraph's slot value",
      "inline shadow root in a paragraph",
    ].flatMap { container in
      [1, 2, 4].flatMap { mix in
        ["all of it", "first text to the middle", "middle to the last text"].flatMap { span in
          [false, true].map {
            WrapMixCase(
              container: container, mix: mix, span: span, backward: $0, structure: structure)
          }
        }
      }
    }
  }
}

struct WrapBlockEndCase: Sendable, CustomTestStringConvertible {
  var first = "paragraph"
  let end: String
  let backward: Bool
  let structure: Bool
  var testDescription: String {
    "\(first), to \(end)\(backward ? ", backward" : "")\(structureSuffix(structure))"
  }

  static let all: [WrapBlockEndCase] = [false, true].flatMap { structure in
    [
      "text in a paragraph", "an empty paragraph", "an empty paragraph in a shadow root",
      "text in a shadow root",
    ].flatMap { end in
      [false, true].map { WrapBlockEndCase(end: end, backward: $0, structure: structure) }
    }
  }

  /// The same cases with a list as the first block.
  static let lists: [WrapBlockEndCase] = all.map {
    var c = $0
    c.first = "list"
    return c
  }
}

struct WrapListPointCase: Sendable, CustomTestStringConvertible {
  let item: Int
  let offset: Int
  let backward: Bool
  let structure: Bool
  var testDescription: String {
    "item \(item), list offset \(offset)\(backward ? ", backward" : "")\(structureSuffix(structure))"
  }

  static let all: [WrapListPointCase] = [false, true].flatMap { structure in
    [0, 1, 2].flatMap { item in
      [0, 1, 2, 3].flatMap { offset in
        [false, true].map {
          WrapListPointCase(item: item, offset: offset, backward: $0, structure: structure)
        }
      }
    }
  }
}

@Suite struct WrapNodesInlineContentTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, empty, caret in the empty scope"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, empty, caret in the empty scope"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, empty, caret in the empty scope"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, caret in the inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, the whole inner text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, empty, caret in the empty scope, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding a paragraph, the whole inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > inline, in the paragraph, holding text, the whole inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, empty, caret in the empty scope, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding a paragraph, the whole inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, at the root, holding text, the whole inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, empty, caret in the empty scope, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding a paragraph, the whole inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, caret in the inner text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes keeps a selection in a shadow root inside that scope > block, the paragraph's slot value, holding text, the whole inner text, NodeStructureExtension"
    ),
    arguments: WrapScopeCase.all)
  func keepsSelectionInShadowRootScope(_ c: WrapScopeCase) throws {
    let editor = try buildEditor(
      "@wrapNodes-shadow-scope-test", c.structure,
      [.klass(InlineShadowNode.self), .klass(QuoteNode.self), .klass(TestShadowRootNode.self)])
    defer { editor.dispose() }
    let filled = c.content != "empty"
    var outsideBefore = ""
    var parentKey: NodeKey = ""
    var scopeKey: NodeKey = ""
    try editor.update(discrete: true) {
      try getRoot().clear()
      let shadow: ElementNode =
        c.scope.hasPrefix("inline")
        ? try create(InlineShadowNode.self) : try createTestShadowRootNode()
      if c.content == "holding a paragraph" {
        try shadow.append(try createParagraphNode().append(try createTextNode("inside")))
      } else if c.content == "holding text" {
        try shadow.append(try createTextNode("inside"))
      }
      // Token mode keeps the two outside texts apart when nothing is left
      // between them.
      let parent = try createParagraphNode().append(
        try createTextNode("outside before").setMode("token"))
      if c.scope == "inline, in the paragraph" {
        try parent.append(shadow)
      } else if c.scope.hasSuffix("slot value") {
        try setSlot(parent, "scope", shadow)
      }
      try parent.append(try createTextNode("outside after").setMode("token"))
      try getRoot().append(parent)
      if c.scope == "block, at the root" {
        try getRoot().append(shadow)
      }
      parentKey = parent.getKey()
      scopeKey = shadow.getKey()
      outsideBefore = try outside(parent, scopeKey)
    }
    try editor.update(discrete: true) {
      let shadow = try #require(try getNodeByKey(scopeKey) as? ElementNode)
      let selection = createRangeSelection()
      if !filled {
        try selection.anchor.set(scopeKey, 0, .element)
        try selection.focus.set(scopeKey, 0, .element)
      } else {
        let inside = try textOf(shadow, "inside")
        let caret = c.place == "caret in the inner text"
        try selection.anchor.set(inside.getKey(), caret ? 3 : 0, .text)
        try selection.focus.set(inside.getKey(), caret ? 3 : 6, .text)
      }
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    try editor.read { () throws in
      let parent = try #require(try getNodeByKey(parentKey) as? ElementNode)
      let shadow = try #require(try getNodeByKey(scopeKey) as? ElementNode)
      #expect(try parent.isAttached())
      #expect(try outside(parent, scopeKey) == outsideBefore)
      if c.scope.hasSuffix("slot value") {
        #expect(try getSlot(parent, "scope")?.getKey() == scopeKey)
      }
      #expect(
        try content(shadow) == "\(shadow.getType())(quote(\(filled ? "\"inside\"" : "")))")
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 1, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 2, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root at the root, mix 4, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 1, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 2, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > block shadow root, a paragraph's slot value, mix 4, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 1, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 2, middle to the last text, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, all of it, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, first text to the middle, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a block container that mixes inline content and blocks > inline shadow root in a paragraph, mix 4, middle to the last text, backward, NodeStructureExtension"
    ),
    arguments: WrapMixCase.all)
  func mixedContainer(_ c: WrapMixCase) throws {
    let editor = try buildEditor(
      "@wrapNodes-mixed-container-test", c.structure,
      [.klass(InlineShadowNode.self), .klass(QuoteNode.self), .klass(TestShadowRootNode.self)])
    defer { editor.dispose() }
    var scopeKey: NodeKey = ""
    var before: [String] = []
    try editor.update(discrete: true) {
      try getRoot().clear()
      let scope: ElementNode =
        c.container.hasPrefix("inline")
        ? try create(InlineShadowNode.self) : try createTestShadowRootNode()
      for part in WrapMixCase.mixes[c.mix]! {
        switch part {
        case .text(let text):
          try scope.append(try createTextNode(text).setMode("token"))
        case .paragraph(let text):
          try scope.append(
            try createParagraphNode().append(try createTextNode(text).setMode("token")))
        }
      }
      let host = try createParagraphNode().append(try createTextNode("outside").setMode("token"))
      if c.container == "block shadow root at the root" {
        try getRoot().append(host, scope)
      } else if c.container.hasPrefix("inline") {
        try host.append(scope)
        try getRoot().append(host)
      } else {
        try setSlot(host, "body", scope)
        try getRoot().append(host)
      }
      scopeKey = scope.getKey()
    }
    try editor.update(discrete: true) {
      let scope = try #require(try getNodeByKey(scopeKey) as? ElementNode)
      before = try texts(scope)
      let all = try scope.getAllTextNodes()
      let middle = all[all.count / 2]
      let (start, end) =
        c.span == "all of it"
        ? (all[0], all[all.count - 1])
        : c.span == "first text to the middle" ? (all[0], middle) : (middle, all[all.count - 1])
      let selection = createRangeSelection()
      let (anchor, focus) = c.backward ? (end, start) : (start, end)
      try selection.anchor.set(
        anchor.getKey(), c.backward ? anchor.getTextContentSize() : 0, .text)
      try selection.focus.set(focus.getKey(), c.backward ? 0 : focus.getTextContentSize(), .text)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    try editor.read { () throws in
      let scope = try #require(try getNodeByKey(scopeKey) as? ElementNode)
      #expect(try scope.isAttached())
      #expect(try texts(scope) == before)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a paragraph, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to an empty paragraph in a shadow root, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > paragraph, to text in a shadow root, backward, NodeStructureExtension"
    ),
    arguments: WrapBlockEndCase.all)
  func rangeStartingAtBlockEnd(_ c: WrapBlockEndCase) throws {
    try startingAtBlockEnd(c)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph in a shadow root, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to an empty paragraph, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a paragraph, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range that starts at the end of a block > list, to text in a shadow root, backward, NodeStructureExtension"
    ),
    arguments: WrapBlockEndCase.lists)
  func rangeStartingAtListEnd(_ c: WrapBlockEndCase) throws {
    try startingAtBlockEnd(c)
  }

  // A range with one end in a list item's text and the other an element
  // point on the list itself, in either direction and at every offset:
  // wrapNodes does not throw, and every text stays, in order.
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 0, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 1, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 2, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 0, list offset 3, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 0, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 1, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 2, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 1, list offset 3, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 0, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 1, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 2, backward, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over a range from a list item to an element point on its list > item 2, list offset 3, backward, NodeStructureExtension"
    ),
    arguments: WrapListPointCase.all)
  func rangeFromListItemToListPoint(_ c: WrapListPointCase) throws {
    let editor = try buildEditor(
      "@wrapNodes-list-point-test", c.structure,
      [.klass(ListNode.self), .klass(ListItemNode.self), .klass(QuoteNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet").append(
        contentsOf: ["aa", "bb", "cc"].map { try createListItemNode().append(createTextNode($0)) })
      try getRoot().clear().append(createParagraphNode().append(createTextNode("before")), list)
      let text = try #require(
        try (list.getChildAtIndex(c.item) as! ElementNode).getFirstDescendant())
      let selection = createRangeSelection()
      let (textPoint, listPoint) =
        c.backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try textPoint.set(text.getKey(), 1, .text)
      try listPoint.set(list.getKey(), c.offset, .element)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    #expect(
      try editor.read { try getRoot().getAllTextNodes().map { try $0.getTextContent() } } == [
        "before", "aa", "bb", "cc",
      ])
  }

  private func startingAtBlockEnd(_ c: WrapBlockEndCase) throws {
    let editor = try buildEditor(
      "@wrapNodes-start-at-block-end-test", c.structure,
      [
        .klass(ListNode.self), .klass(ListItemNode.self), .klass(QuoteNode.self),
        .klass(TestShadowRootNode.self),
      ])
    defer { editor.dispose() }
    var firstKey: NodeKey = ""
    var before: [String] = []
    try editor.update(discrete: true) {
      let block: ElementNode =
        c.first == "list"
        ? try createListNode("bullet").append(
          createListItemNode().append(createTextNode("aa")),
          createListItemNode().append(createTextNode("bb")))
        : try createParagraphNode().append(try createTextNode("aa bb"))
      let empty = try createParagraphNode()
      let ee = try createTextNode("ee")
      let tail: [LexicalNode] =
        c.end.hasSuffix("shadow root")
        ? [try createTestShadowRootNode().append(empty, try createParagraphNode().append(ee))]
        : [empty, try createParagraphNode().append(ee)]
      try getRoot().clear().append(
        contentsOf: [
          block, try createParagraphNode().append(try createTextNode("cc")),
          try createParagraphNode().append(try createTextNode("dd")),
        ] + tail + [try createParagraphNode().append(try createTextNode("ff"))])
      firstKey = block.getKey()
      before = try texts(getRoot())
      let selection = createRangeSelection()
      let (atEnd, other) =
        c.backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try atEnd.set(block.getKey(), block.getChildrenSize(), .element)
      if c.end.hasPrefix("text") {
        try other.set(ee.getKey(), 1, .text)
      } else {
        try other.set(empty.getKey(), 0, .element)
      }
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
    }
    try editor.read { () throws in
      #expect(try texts(getRoot()) == before)
      let block = try getRoot().getFirstChildOrThrow()
      #expect(block.getKey() == firstKey)
      #expect(block.getType() == c.first)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes leaves the selection inside the new tree > three root paragraphs, from the first text to the root's end, backward, NodeStructureExtension"
    ),
    arguments: [false, true], [false, true])
  func leavesSelectionInsideNewTree(_ backward: Bool, _ structure: Bool) throws {
    let editor = try buildEditor(
      "@wrapNodes-selection-restore-test", structure, [.klass(QuoteNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraphs = try ["aa", "bb", "cc"].map {
        try createParagraphNode().append(try createTextNode($0))
      }
      try getRoot().clear().append(contentsOf: paragraphs)
      let selection = createRangeSelection()
      let (textPoint, rootPoint) =
        backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try textPoint.set(paragraphs[0].getFirstChildOrThrow().getKey(), 0, .text)
      try rootPoint.set(getRoot().getKey(), 3, .element)
      try setSelection(selection)
      try wrapNodes(selection) { try createQuoteNode() }
      let after = try getSelection()
      #expect(after is RangeSelection)
      if let after = after as? RangeSelection {
        for point in [after.anchor, after.focus] {
          let node = try point.getNode()
          #expect(try node.isAttached())
          let size =
            try (node as? ElementNode)?.getChildrenSize() ?? node.getTextContentSize()
          #expect(point.offset <= size)
        }
      }
    }
    #expect(
      try editor.read { try getRoot().getAllTextNodes().map { try $0.getTextContent() } }
        == ["aa", "bb", "cc"])
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, backward"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, NodeStructureExtension"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/WrapNodesInlineContent.test.ts::$wrapNodes over the whole root, with a wrapping element > root element points 0 to 3, backward, NodeStructureExtension"
    ),
    arguments: [false, true], [false, true])
  func wholeRootWithWrappingElement(_ backward: Bool, _ structure: Bool) throws {
    let editor = try buildEditor(
      "@wrapNodes-root-points-test", structure,
      [.klass(QuoteNode.self), .klass(TestShadowRootNode.self)])
    defer { editor.dispose() }
    var selected = ""
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        contentsOf: try ["aa", "bb", "cc"].map {
          try createParagraphNode().append(try createTextNode($0))
        })
      let root = try getRoot().getKey()
      let selection = createRangeSelection()
      try selection.anchor.set(root, backward ? 3 : 0, .element)
      try selection.focus.set(root, backward ? 0 : 3, .element)
      try setSelection(selection)
      try wrapNodes(selection, { try createQuoteNode() }, try createTestShadowRootNode())
      if let after = try getSelection() as? RangeSelection {
        let inRange = try [after.anchor, after.focus].allSatisfy { point in
          let node = try point.getNode()
          let size =
            try (node as? ElementNode)?.getChildrenSize() ?? node.getTextContentSize()
          return try node.isAttached() && point.offset <= size
        }
        selected =
          "\(after.isCollapsed() ? "collapsed" : "range") \(inRange ? "in range" : "out of range")"
        try after.insertText("Z")
      }
    }
    // As on main: a collapsed selection, so typing keeps the wrapped text.
    #expect(selected == "collapsed in range")
    // The typed text goes next to the wrapped text, not over it.
    let all = try editor.read { try getRoot().getAllTextNodes().map { try $0.getTextContent() } }
    #expect(all.filter { $0 != "Z" } == ["aa", "bb", "cc"])
    #expect(all.contains("Z"))
  }
}
