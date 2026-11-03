import LexicalTesting
import Testing

@testable import Lexical

// Port of packages/lexical/src/__tests__/unit/LexicalNode.test.ts, the
// `LexicalNode tests` suite. Upstream checks the reconciled DOM after some
// updates; those checks are model assertions here, and the native
// reconciler's output is tested on its own.

/// The `beforeEach` of the suite: a paragraph holding the text node `foo`.
struct NodeTestEnv {
  let editor: Editor
  var paragraphNode: ParagraphNode!
  var textNode: TextNode!

  init() throws {
    editor = try createTestEditor(nodes: lexicalNodeTestNodes)
    var paragraph: ParagraphNode!
    var text: TextNode!
    try update(editor) { () throws in
      paragraph = try ParagraphNode()
      text = try TextNode("foo")
      try paragraph.append(text)
      try getRoot().append(paragraph)
    }
    paragraphNode = paragraph
    textNode = text
  }

  /// `editor.read('latest', fn)`.
  func latest(_ fn: () throws -> Void) throws {
    try editor.read(.latest, fn)
  }

  /// `editor.read('latest', fn)`, returning its value.
  func latestValue<T>(_ fn: () throws -> T) throws -> T {
    try editor.read(.latest, fn)
  }

  /// The text of each paragraph of the committed state.
  func paragraphs() throws -> [String] {
    try latestValue { try getRoot().getChildren().map { try $0.getTextContent() } }
  }
}

@Suite struct LexicalNodeTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.constructor")
  )
  func constructor() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      let node = try PlainNode(key: "__custom_key__")
      #expect(node.__type == "node")
      #expect(node.__key == "__custom_key__")
      #expect(node.__parent == nil)
    }
    try env.editor.getEditorState().read { () throws in
      #expect(throws: (any Error).self) { try PlainNode() }
      #expect(throws: (any Error).self) { try PlainNode(key: "__custom_key__") }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.constructor: type change detected"
    ))
  func constructorTypeChange() throws {
    let env = try NodeTestEnv()
    let textNode = env.textNode!
    try update(env.editor) { () throws in
      let validNode = try TextNode(textNode.__text, key: textNode.__key)
      #expect(try textNode.getLatest() === textNode)
      #expect(try validNode.getLatest() === textNode)
      #expect {
        try TestNode(key: textNode.__key)
      } throws: { error in
        let message = "\(error)"
        return message.contains("TestNode") && message.contains("re-use key")
          && message.contains("TextNode")
      }
    }
  }

  final class VersionedTextNode: TextNode {
    var __version = 0
    override class func config() -> NodeConfig? {
      NodeConfig(VersionedTextNode.self, type: "vtext", extends: TextNode.self)
    }
    override func afterCloneFrom(_ node: LexicalNode) throws {
      try super.afterCloneFrom(node)
      __version = (node as! VersionedTextNode).__version + 1
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.afterCloneFrom()"
    ))
  func afterCloneFrom() throws {
    let editor = try createEditor(
      CreateEditorArgs(nodes: [.klass(VersionedTextNode.self)], onError: { throw $0 }))
    var versionedTextNode: VersionedTextNode!
    try editor.update(discrete: true) { () throws in
      versionedTextNode = try VersionedTextNode("test")
      try getRoot().append(createParagraphNode().append(versionedTextNode))
      #expect(versionedTextNode.__version == 0)
    }
    try editor.update(discrete: true) { () throws in
      #expect(try versionedTextNode.getLatest().__version == 0)
      #expect(try versionedTextNode.setTextContent("update").setMode("token").__version == 1)
    }
    try editor.update(discrete: true) { () throws in
      var latest = try versionedTextNode.getLatest()
      #expect(versionedTextNode.__version == 0)
      #expect(versionedTextNode.__mode == 0)
      #expect(try versionedTextNode.getMode() == "token")
      #expect(latest.__version == 1)
      #expect(latest.__mode == 1)
      latest = try latest.setTextContent("another update")
      #expect(latest.__version == 2)
      #expect(try latest.getWritable().__version == 2)
      #expect(try versionedTextNode.getLatest().getWritable().__version == 2)
      #expect(try versionedTextNode.getLatest().__version == 2)
      #expect(versionedTextNode.__mode == 0)
      #expect(try versionedTextNode.getLatest().__mode == 1)
      #expect(try versionedTextNode.getMode() == "token")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getType()"))
  func getType() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      let node = try PlainNode(key: "__custom_key__")
      #expect(node.getType() == node.__type)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isAttached()"
    ))
  func isAttached() throws {
    let env = try NodeTestEnv()
    var node: LexicalNode!
    try update(env.editor) { () throws in
      node = try PlainNode(key: "__custom_key__")
    }
    try env.latest { () throws in
      #expect(try node.isAttached() == false)
      #expect(try env.textNode.isAttached())
      #expect(try env.paragraphNode.isAttached())
    }
    #expect(throws: (any Error).self) { try env.textNode.isAttached() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected()"
    ))
  func isSelected() throws {
    let env = try NodeTestEnv()
    var node: LexicalNode!
    try update(env.editor) { () throws in
      node = try PlainNode(key: "__custom_key__")
    }
    try env.latest { () throws in
      #expect(try node.isSelected() == false)
      #expect(try env.textNode.isSelected() == false)
      #expect(try env.paragraphNode.isSelected() == false)
    }
    try update(env.editor) { () throws in
      try env.textNode.select(0, 0)
    }
    try env.latest { () throws in
      #expect(try env.textNode.isSelected())
    }
    #expect(throws: (any Error).self) { try env.textNode.isSelected() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): selected text node"
    ))
  func isSelectedTextNode() throws {
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(try env.paragraphNode.isSelected() == false)
      #expect(try env.textNode.isSelected() == false)
    }
    try update(env.editor) { () throws in
      try env.textNode.select(0, 0)
    }
    try env.latest { () throws in
      #expect(try env.textNode.isSelected())
      #expect(try env.paragraphNode.isSelected() == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): selected block node range"
    ))
  func isSelectedBlockRange() throws {
    let env = try NodeTestEnv()
    var newParagraphNode: ParagraphNode!
    var newTextNode: TextNode!
    try update(env.editor) { () throws in
      #expect(try env.paragraphNode.isSelected() == false)
      #expect(try env.textNode.isSelected() == false)
      newParagraphNode = try ParagraphNode()
      newTextNode = try TextNode("bar")
      try newParagraphNode.append(newTextNode)
      try env.paragraphNode.insertAfter(newParagraphNode)
      #expect(try newParagraphNode.isSelected() == false)
      #expect(try newTextNode.isSelected() == false)
    }
    try update(env.editor) { () throws in
      try env.textNode.select(0, 0)
      let selection = try #require(try getSelection() as? RangeSelection)
      try selection.anchor.set(env.textNode.getKey(), 1, .text)
      try selection.focus.set(newTextNode.getKey(), 1, .text)
    }
    try update(env.editor) { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == env.textNode.getKey())
      #expect(selection.focus.key == newTextNode.getKey())
      #expect(try env.paragraphNode.isSelected())
      #expect(try env.textNode.isSelected())
      #expect(try newParagraphNode.isSelected())
      #expect(try newTextNode.isSelected())
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with custom range selection"
    ))
  func isSelectedCustomRange() throws {
    let env = try NodeTestEnv()
    var newParagraphNode: ParagraphNode!
    var newTextNode: TextNode!
    try update(env.editor) { () throws in
      newParagraphNode = try ParagraphNode()
      newTextNode = try TextNode("bar")
      try newParagraphNode.append(newTextNode)
      try env.paragraphNode.insertAfter(newParagraphNode)
      #expect(try newParagraphNode.isSelected() == false)
      #expect(try newTextNode.isSelected() == false)
    }
    try update(env.editor) { () throws in
      let rangeSelection = createRangeSelection()
      try rangeSelection.anchor.set(env.textNode.getKey(), 1, .text)
      try rangeSelection.focus.set(newTextNode.getKey(), 1, .text)
      #expect(try env.paragraphNode.isSelected(rangeSelection))
      #expect(try env.textNode.isSelected(rangeSelection))
      #expect(try newParagraphNode.isSelected(rangeSelection))
      #expect(try newTextNode.isSelected(rangeSelection))
    }
  }

  struct InlineDecoratorCase: Sendable, CustomTestStringConvertible {
    let label: String
    let isSelected: Bool
    /// Selects with the three paragraphs (p1, p2 holding the decorator, p3).
    let update: @Sendable ([ParagraphNode]) throws -> Void
    var testDescription: String { "\(isSelected ? "is" : "isn't") selected \(label)" }
  }

  static let inlineDecoratorCases: [InlineDecoratorCase] = [
    .init(label: "whole editor", isSelected: true) { _ in try getRoot().select(0) },
    .init(label: "containing paragraph", isSelected: true) { p in try p[1].select(0) },
    .init(label: "before and containing", isSelected: true) { p in
      try p[1].select(0).anchor.set(p[0].getKey(), 0, .element)
    },
    .init(label: "containing and after", isSelected: true) { p in
      try p[1].select(0).focus.set(p[2].getKey(), 0, .element)
    },
    .init(label: "before and after", isSelected: true) { p in
      try p[0].select(0).focus.set(p[2].getKey(), 0, .element)
    },
    .init(label: "collapsed before", isSelected: false) { p in try p[1].select(0, 0) },
    .init(label: "in another element", isSelected: false) { p in try p[0].select(0) },
    .init(label: "before", isSelected: false) { p in
      try p[0].select(0).focus.set(p[1].getKey(), 0, .element)
    },
    .init(label: "collapsed after", isSelected: false) { p in try p[1].selectEnd() },
    .init(label: "after", isSelected: false) { p in
      try p[2].select(0).anchor.set(p[1].getKey(), p[1].getChildrenSize(), .element)
    },
  ]

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected whole editor"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected containing paragraph"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected before and containing"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected containing and after"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > is selected before and after"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected collapsed before"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected in another element"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected before"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected collapsed after"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSelected(): with inline decorator node > isn't selected after"
    ),
    arguments: inlineDecoratorCases)
  func isSelectedWithInlineDecorator(_ testCase: InlineDecoratorCase) throws {
    let env = try NodeTestEnv()
    let editor = env.editor
    var paragraphs: [ParagraphNode] = []
    var inlineDecoratorNode: InlineDecoratorNode!
    try editor.update { () throws in
      inlineDecoratorNode = try InlineDecoratorNode()
      paragraphs = [
        try createParagraphNode(), try createParagraphNode().append(inlineDecoratorNode),
        try createParagraphNode(),
      ]
      try getRoot().clear().append(contentsOf: paragraphs)
    }
    try editor.update { try testCase.update(paragraphs) }
    let verify = {
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(try inlineDecoratorNode.isSelected() == testCase.isSelected)
    }
    try editor.read(verify)
    try editor.update { () throws in
      let selection = try getSelection()
      if let selection = selection as? RangeSelection {
        let backwards = createRangeSelection()
        try backwards.anchor.set(selection.focus.key, selection.focus.offset, selection.focus.type)
        try backwards.focus.set(
          selection.anchor.key, selection.anchor.offset, selection.anchor.type)
        try setSelection(backwards)
      }
      #expect(selection is RangeSelection)
    }
    try editor.read(verify)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getKey()"))
  func getKey() throws {
    let env = try NodeTestEnv()
    #expect(env.textNode.getKey() == env.textNode.__key)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParent()")
  )
  func getParent() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      #expect(try PlainNode().getParent() == nil)
    }
    try env.latest { () throws in
      #expect(try env.textNode.getParent() === env.paragraphNode)
      #expect(try env.paragraphNode.getParent() === getRoot())
    }
    #expect(throws: (any Error).self) { try env.textNode.getParent() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParentOrThrow()"
    ))
  func getParentOrThrow() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      let node = try PlainNode()
      #expect(throws: (any Error).self) { try node.getParentOrThrow() }
    }
    try env.latest { () throws in
      #expect(try env.textNode.getParentOrThrow() === env.paragraphNode)
      #expect(try env.paragraphNode.getParentOrThrow() === getRoot())
    }
    #expect(throws: (any Error).self) { try env.textNode.getParentOrThrow() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTopLevelElement()"
    ))
  func getTopLevelElement() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      #expect(try PlainNode().getTopLevelElement() == nil)
    }
    try env.latest { () throws in
      #expect(try env.textNode.getTopLevelElement() === env.paragraphNode)
      #expect(try env.paragraphNode.getTopLevelElement() === env.paragraphNode)
    }
    #expect(throws: (any Error).self) { try env.textNode.getTopLevelElement() }
    try update(env.editor) { () throws in
      let node = try BlockDecoratorNode()
      #expect(try node.getTopLevelElement() == nil)
      try getRoot().append(node)
      #expect(try node.getTopLevelElement() === node)
    }
    try env.latest { () throws in
      var elementNodes = 0
      var decoratorNodes = 0
      for child in try getRoot().getChildren() {
        #expect(try child.getTopLevelElement() === child)
        if child is ElementNode {
          elementNodes += 1
        } else if child is DecoratorNode {
          decoratorNodes += 1
        } else {
          Issue.record("Expecting all children to be ElementNode or DecoratorNode")
        }
      }
      #expect(decoratorNodes == 1)
      #expect(elementNodes == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTopLevelElementOrThrow()"
    ))
  func getTopLevelElementOrThrow() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      let node = try PlainNode()
      #expect(throws: (any Error).self) { try node.getTopLevelElementOrThrow() }
    }
    try env.latest { () throws in
      #expect(try env.textNode.getTopLevelElementOrThrow() === env.paragraphNode)
      #expect(try env.paragraphNode.getTopLevelElementOrThrow() === env.paragraphNode)
    }
    #expect(throws: (any Error).self) { try env.textNode.getTopLevelElementOrThrow() }
    try update(env.editor) { () throws in
      let node = try InlineDecoratorNode()
      #expect(throws: (any Error).self) { try node.getTopLevelElementOrThrow() }
      try getRoot().append(node)
      #expect(try node.getTopLevelElementOrThrow() === node)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getParents()"
    ))
  func getParents() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      #expect(try PlainNode().getParents().isEmpty)
    }
    #expect(try env.paragraphs() == ["foo"])
    try env.latest { () throws in
      let rootNode = try getRoot()
      #expect(try env.textNode.getParents().map(\.__key) == [env.paragraphNode.__key, "root"])
      #expect(try env.textNode.getParents()[1] === rootNode)
      #expect(try env.paragraphNode.getParents().map(\.__key) == ["root"])
    }
    #expect(throws: (any Error).self) { try env.textNode.getParents() }
  }

  /// Appends unmergeable `bar` (and `baz`) text nodes to the paragraph.
  private func appendUnmergeable(_ env: NodeTestEnv, _ texts: [String]) throws -> [TextNode] {
    var nodes: [TextNode] = []
    try update(env.editor) { () throws in
      nodes = try texts.map { try TextNode($0).toggleUnmergeable() }
      try env.paragraphNode.append(contentsOf: nodes)
    }
    return nodes
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getPreviousSibling()"
    ))
  func getPreviousSibling() throws {
    let env = try NodeTestEnv()
    let barTextNode = try appendUnmergeable(env, ["bar"])[0]
    #expect(try env.latestValue { try env.paragraphNode.getChildren().count } == 2)
    try env.latest { () throws in
      let previous = try #require(try barTextNode.getPreviousSibling())
      #expect(previous.is(env.textNode))
      #expect(previous.__next == "3")
      #expect(try env.textNode.getPreviousSibling() == nil)
    }
    #expect(throws: (any Error).self) { try env.textNode.getPreviousSibling() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getPreviousSiblings()"
    ))
  func getPreviousSiblings() throws {
    let env = try NodeTestEnv()
    let nodes = try appendUnmergeable(env, ["bar", "baz"])
    let (barTextNode, bazTextNode) = (nodes[0], nodes[1])
    try env.latest { () throws in
      let previous = try bazTextNode.getPreviousSiblings()
      #expect(previous.map(\.__key) == [env.textNode.__key, barTextNode.__key])
      #expect(previous[0].__next == "3")
      #expect(previous[1].__prev == "2")
      #expect(try barTextNode.getPreviousSiblings().map(\.__key) == [env.textNode.__key])
      #expect(try env.textNode.getPreviousSiblings().isEmpty)
    }
    #expect(throws: (any Error).self) { try env.textNode.getPreviousSiblings() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNextSibling()"
    ))
  func getNextSibling() throws {
    let env = try NodeTestEnv()
    let barTextNode = try appendUnmergeable(env, ["bar"])[0]
    try env.latest { () throws in
      #expect(try barTextNode.getNextSibling() == nil)
      #expect(try env.textNode.getNextSibling() === barTextNode)
    }
    #expect(throws: (any Error).self) { try env.textNode.getNextSibling() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNextSiblings()"
    ))
  func getNextSiblings() throws {
    let env = try NodeTestEnv()
    let nodes = try appendUnmergeable(env, ["bar", "baz"])
    let (barTextNode, bazTextNode) = (nodes[0], nodes[1])
    try env.latest { () throws in
      #expect(try bazTextNode.getNextSiblings().isEmpty)
      #expect(try barTextNode.getNextSiblings().map(ObjectIdentifier.init) == [.init(bazTextNode)])
      #expect(
        try env.textNode.getNextSiblings().map(ObjectIdentifier.init)
          == [.init(barTextNode), .init(bazTextNode)])
    }
    #expect(throws: (any Error).self) { try env.textNode.getNextSiblings() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isBefore()"))
  func isBefore() throws {
    let env = try NodeTestEnv()
    let nodes = try appendUnmergeable(env, ["bar", "baz"])
    let (barTextNode, bazTextNode) = (nodes[0], nodes[1])
    let textNode = env.textNode!
    try env.latest { () throws in
      #expect(try textNode.isBefore(textNode) == false)
      #expect(try textNode.isBefore(barTextNode))
      #expect(try textNode.isBefore(bazTextNode))
      #expect(try barTextNode.isBefore(bazTextNode))
      #expect(try bazTextNode.isBefore(barTextNode) == false)
      #expect(try bazTextNode.isBefore(textNode) == false)
    }
    #expect(throws: (any Error).self) { try textNode.isBefore(barTextNode) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isParentOf()"
    ))
  func isParentOf() throws {
    let env = try NodeTestEnv()
    let (paragraphNode, textNode) = (env.paragraphNode!, env.textNode!)
    try env.latest { () throws in
      let rootNode = try getRoot()
      #expect(try rootNode.isParentOf(textNode))
      #expect(try rootNode.isParentOf(paragraphNode))
      #expect(try paragraphNode.isParentOf(textNode))
      #expect(try paragraphNode.isParentOf(rootNode) == false)
      #expect(try textNode.isParentOf(paragraphNode) == false)
      #expect(try textNode.isParentOf(rootNode) == false)
    }
    #expect(throws: (any Error).self) { try paragraphNode.isParentOf(textNode) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getNodesBetween()"
    ))
  func getNodesBetween() throws {
    let env = try NodeTestEnv()
    let textNode = env.textNode!
    var barTextNode: TextNode!
    var bazTextNode: TextNode!
    var newParagraphNode: ParagraphNode!
    var quxTextNode: TextNode!
    try update(env.editor) { () throws in
      barTextNode = try TextNode("bar").toggleUnmergeable()
      bazTextNode = try TextNode("baz").toggleUnmergeable()
      newParagraphNode = try ParagraphNode()
      quxTextNode = try TextNode("qux").toggleUnmergeable()
      try getRoot().append(newParagraphNode)
      try env.paragraphNode.append(barTextNode, bazTextNode)
      try newParagraphNode.append(quxTextNode)
    }
    #expect(try env.paragraphs() == ["foobarbaz", "qux"])
    try env.latest { () throws in
      func keys(_ nodes: [LexicalNode]) -> [NodeKey] { nodes.map(\.__key) }
      #expect(try keys(textNode.getNodesBetween(textNode)) == [textNode.__key])
      #expect(
        try keys(textNode.getNodesBetween(barTextNode)) == [textNode.__key, barTextNode.__key])
      #expect(
        try keys(textNode.getNodesBetween(bazTextNode))
          == [textNode.__key, barTextNode.__key, bazTextNode.__key])
      let between = try textNode.getNodesBetween(quxTextNode)
      #expect(
        keys(between)
          == [
            textNode.__key, barTextNode.__key, bazTextNode.__key, env.paragraphNode.__key,
            newParagraphNode.__key, quxTextNode.__key,
          ])
      #expect(try between[3] === env.paragraphNode.getLatest())
    }
    #expect(throws: (any Error).self) { try textNode.getNodesBetween(bazTextNode) }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isToken()"))
  func isToken() throws {
    let env = try NodeTestEnv()
    var tokenTextNode: TextNode!
    try update(env.editor) { () throws in
      tokenTextNode = try TextNode("token").setMode("token")
      try env.paragraphNode.append(tokenTextNode)
    }
    #expect(try env.latestValue { try env.paragraphNode.getChildrenSize() } == 2)
    try env.latest { () throws in
      #expect(try env.textNode.isToken() == false)
      #expect(try tokenTextNode.isToken())
    }
    #expect(throws: (any Error).self) { try env.textNode.isToken() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isSegmented()"
    ))
  func isSegmented() throws {
    let env = try NodeTestEnv()
    var segmentedTextNode: TextNode!
    try update(env.editor) { () throws in
      segmentedTextNode = try TextNode("segmented").setMode("segmented")
      try env.paragraphNode.append(segmentedTextNode)
    }
    #expect(try env.latestValue { try env.paragraphNode.getChildrenSize() } == 2)
    try env.latest { () throws in
      #expect(try env.textNode.isSegmented() == false)
      #expect(try segmentedTextNode.isSegmented())
    }
    #expect(throws: (any Error).self) { try env.textNode.isSegmented() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.isDirectionless()"
    ))
  func isDirectionless() throws {
    let env = try NodeTestEnv()
    var directionlessTextNode: TextNode!
    try update(env.editor) { () throws in
      directionlessTextNode = try TextNode("directionless").toggleDirectionless()
      try directionlessTextNode.toggleUnmergeable()
      try env.paragraphNode.append(directionlessTextNode)
    }
    #expect(try env.latestValue { try env.paragraphNode.getChildrenSize() } == 2)
    try env.latest { () throws in
      #expect(try env.textNode.isDirectionless() == false)
      #expect(try directionlessTextNode.isDirectionless())
    }
    #expect(throws: (any Error).self) { try directionlessTextNode.isDirectionless() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getLatest()")
  )
  func getLatest() throws {
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(try env.textNode.getLatest() === env.textNode)
    }
    #expect(throws: (any Error).self) { try env.textNode.getLatest() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getLatest(): garbage collected node"
    ))
  func getLatestCollected() throws {
    let env = try NodeTestEnv()
    var node: LexicalNode!
    var text: TextNode!
    var block: TestElementNode!
    try update(env.editor) { () throws in
      node = try PlainNode()
      _ = try node.getLatest()
      text = try TextNode("")
      _ = try text.getLatest()
      block = try TestElementNode()
      _ = try block.getLatest()
    }
    try update(env.editor) { () throws in
      #expect(throws: (any Error).self) { try node.getLatest() }
      #expect(throws: (any Error).self) { try text.getLatest() }
      #expect(throws: (any Error).self) { try block.getLatest() }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTextContent()"
    ))
  func getTextContent() throws {
    let env = try NodeTestEnv()
    try update(env.editor) { () throws in
      #expect(try PlainNode().getTextContent() == "")
    }
    try env.latest { () throws in
      #expect(try env.textNode.getTextContent() == "foo")
    }
    #expect(throws: (any Error).self) { try env.textNode.getTextContent() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.getTextContentSize()"
    ))
  func getTextContentSize() throws {
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(try env.textNode.getTextContentSize() == 3)
    }
    #expect(throws: (any Error).self) { try env.textNode.getTextContentSize() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.remove()"))
  func remove() throws {
    let env = try NodeTestEnv()
    try env.latest { () throws in
      #expect(throws: (any Error).self) { try env.textNode.remove() }
    }
    #expect(try env.paragraphs() == ["foo"])
    try update(env.editor) { () throws in
      let node = try PlainNode()
      try node.remove()
      #expect(try node.getParent() == nil)
      try env.textNode.remove()
      #expect(try env.textNode.getParent() == nil)
      #expect(env.editor._dirtyLeaves.has(env.textNode.getKey()))
    }
    #expect(try env.paragraphs() == [""])
    #expect(throws: (any Error).self) { try env.textNode.remove() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace()"))
  func replace() throws {
    let env = try NodeTestEnv()
    // Upstream calls replace() with no argument, which Swift's types rule out;
    // in a read, any replacement throws.
    try env.latest { () throws in
      #expect(throws: (any Error).self) { try env.textNode.replace(TextNode()) }
    }
    #expect(throws: (any Error).self) { try env.textNode.remove() }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.replace(): from another parent"
    ))
  func replaceFromAnotherParent() throws {
    let env = try NodeTestEnv()
    #expect(try env.paragraphs() == ["foo"])
    var barTextNode: TextNode!
    try update(env.editor) { () throws in
      let barParagraphNode = try ParagraphNode()
      barTextNode = try TextNode("bar")
      try barParagraphNode.append(barTextNode)
      try getRoot().append(barParagraphNode)
    }
    #expect(try env.paragraphs() == ["foo", "bar"])
    try update(env.editor) { () throws in
      try env.textNode.replace(barTextNode)
    }
    #expect(try env.paragraphs() == ["bar", ""])
  }
}
