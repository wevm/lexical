import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts.
// Upstream's `build` registers the list, link, table, code and overflow
// nodes in every editor; the editors here register rich text, the list, the
// link, the code block and the horizontal rule. The tests that use the
// other packages are skipped until they are ported.

private func shape(_ node: LexicalNode) throws -> JSONValue {
  if let element = node as? ElementNode {
    return .array([.string(node.getType())] + (try element.getChildren().map(shape)))
  }
  if let text = node as? TextNode {
    return .string(try text.getTextContent())
  }
  return .string(node.getType())
}

private func rootShape(_ editor: Editor) throws -> String {
  try editor.read { JSONValue.array(try getRoot().getChildren().map(shape)).stringify() }
}

private func build(_ initial: @escaping () throws -> Void, _ withStructure: Bool) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "node-structure-test",
      dependencies: [RichTextExtension, ListExtension, LinkExtension, HorizontalRuleExtension]
        + (withStructure ? [NodeStructureExtension] : []),
      nodes: { [.klass(CodeNode.self)] },
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: .update { _ in try initial() })))
}

private func paragraphWithRule() throws {
  try getRoot().clear().append(
    createParagraphNode().append(
      createTextNode("a"), createHorizontalRuleNode(), createTextNode("b")))
}

/// Children put in as they are (no override rewrites them).
@discardableResult
private func put<T: ElementNode>(_ parent: T, _ children: [LexicalNode]) throws -> T {
  try parent.elementSplice(parent.getChildrenSize(), 0, children)
}

private func expectError(_ fragment: String, _ body: () throws -> Void) {
  let error = #expect(throws: LexicalError.self) { try body() }
  #expect(error?.message.contains(fragment) == true, "\(String(describing: error))")
}

// MARK: Node classes

private final class ChipNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ChipNode.self, type: "test-chip", extends: ElementNode.self, inline: true)
  }
}

private final class BoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BoxNode.self, type: "test-box", extends: ElementNode.self)
  }
}

private final class BadByNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      BadByNode.self, type: "test-bad-by", extends: ElementNode.self,
      mergeAdjacent: .by([.field("nope")]))
  }
}

nonisolated(unsafe) private let derivedState = createState(
  "d", parse: { $0?.numberValue ?? 0 }, derived: true)

private final class BadDerivedNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      BadDerivedNode.self, type: "test-bad-derived", extends: ElementNode.self,
      mergeAdjacent: .by([.state(derivedState)]))
  }
}

private final class BadInlineNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BadInlineNode.self, type: "test-bad-inline", extends: ElementNode.self, inline: true)
  }
  override func isInline() throws -> Bool { false }
}

private final class MergingItemNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(MergingItemNode.self, type: "test-item", extends: ElementNode.self, children: .block)
  }
}

private final class MergingGroupNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      MergingGroupNode.self, type: "test-group", extends: ElementNode.self,
      children: .list([.klass(MergingItemNode.self)]), mergeAdjacent: .by([.field("format")]))
  }
}

/// A subclass written before the node-structure keys existed (JS declares
/// no `$config`; Swift names its type in one that declares nothing else).
private final class InlineParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineParagraphNode.self, type: "test-inline-paragraph", extends: ParagraphNode.self)
  }
  override func isInline() throws -> Bool { true }
}

private class TagNode: ElementNode {
  var __tag = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TagNode.self, type: "test-tag", extends: ElementNode.self,
      json: [SchemaField("tag", stringValue(), field: \TagNode.__tag)], inline: true,
      children: .inline, mergeAdjacent: .by([.field("tag")]))
  }
}

private final class SubTagNode: TagNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SubTagNode.self, type: "test-subtag", extends: TagNode.self)
  }
}

private final class CleanTagNode: ElementNode {
  var __tag = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CleanTagNode.self, type: "test-clean-tag", extends: ElementNode.self,
      json: [SchemaField("tag", stringValue(), field: \CleanTagNode.__tag)],
      transform: { node in
        if try (node as! ElementNode).isEmpty() {
          try node.remove()
        }
      }, children: .list([.klass(ParagraphNode.self)]), mergeAdjacent: .by([.field("tag")]))
  }
}

nonisolated(unsafe) private let colorState = createState(
  "color", parse: { value -> String in value?.stringValue ?? "" })

private final class ColorNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      ColorNode.self, type: "test-color", extends: ElementNode.self,
      stateConfigs: [RequiredNodeStateConfig(colorState, flat: true)], inline: true,
      children: .inline, mergeAdjacent: .by([.state(colorState)]))
  }
}

private final class FitItemNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FitItemNode.self, type: "test-fit-item", extends: ElementNode.self, children: .inline)
  }
}

private final class FitBlockItemNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FitBlockItemNode.self, type: "test-fit-block-item", extends: ElementNode.self,
      children: .block)
  }
}

private class FitGroupNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FitGroupNode.self, type: "test-fit-group", extends: ElementNode.self,
      children: .list([.klass(FitItemNode.self)]), mergeAdjacent: .by([.field("format")]))
  }
}

/// Lists children its own merging rule cannot hold.
private final class FitBlockGroupNode: FitGroupNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      FitBlockGroupNode.self, type: "test-fit-block-group", extends: FitGroupNode.self,
      children: .list([.klass(FitBlockItemNode.self)]))
  }
}

private final class BlockTagNode: ElementNode {
  var __tag = ""
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      BlockTagNode.self, type: "test-block-tag", extends: ElementNode.self,
      json: [SchemaField("tag", stringValue(), field: \BlockTagNode.__tag)], children: .inline,
      mergeAdjacent: .by([.field("tag")]))
  }
}

private final class CountedNode: ElementNode {
  var __count: Double = 0
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CountedNode.self, type: "test-counted", extends: ElementNode.self,
      json: [SchemaField("count", numberValue(0), field: \CountedNode.__count, derived: true)],
      mergeAdjacent: .by([.field("count")]))
  }
}

private final class WrapItemNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      WrapItemNode.self, type: "test-wrap-item", extends: ElementNode.self, children: .inline)
  }
}

private final class WrapGroupNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      WrapGroupNode.self, type: "test-wrap-group", extends: ElementNode.self,
      children: .list([.klass(WrapItemNode.self), .klass(ParagraphNode.self)]))
  }
}

// MARK: Tests

/// A LinkNode subclass written before the node-structure keys existed (JS
/// declares no `$config`; Swift names its type in one that declares nothing
/// else), whose `isInline()` returns false.
private final class BlockLinkNode: LinkNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BlockLinkNode.self, type: "test-block-link", extends: LinkNode.self)
  }
  override func isInline() throws -> Bool { false }
}

private final class BadOwnInlineLinkNode: LinkNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      BadOwnInlineLinkNode.self, type: "test-bad-own-inline", extends: LinkNode.self,
      inline: false)
  }
  override func isInline() throws -> Bool { true }
}

private class OwnChipNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(OwnChipNode.self, type: "test-own-chip", extends: ElementNode.self, inline: true)
  }
}

private final class SubChipNode: OwnChipNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SubChipNode.self, type: "test-sub-chip", extends: OwnChipNode.self)
  }
}

private final class ModernLinkNode: LinkNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ModernLinkNode.self, type: "test-modern-link", extends: LinkNode.self)
  }
}

private final class PlainBoxNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainBoxNode.self, type: "test-plain-box", extends: ParagraphNode.self)
  }
}

private final class NoMergeLinkNode: LinkNode {
  override class func config() -> NodeConfig? {
    NodeConfig(NoMergeLinkNode.self, type: "test-nomerge-link", extends: LinkNode.self)
  }
  override func shouldMergeAdjacentLink(_ otherLink: LinkNode) throws -> Bool { false }
}

private final class StatefulLinkNode: LinkNode {
  override class func config() -> NodeConfig? {
    NodeConfig(StatefulLinkNode.self, type: "test-stateful-link", extends: LinkNode.self)
  }
  override func isInline() throws -> Bool { try getLatest().__url != "" }
}

@Suite struct NodeStructureDeclarationTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::declarations > a class that declares inline gets an isInline() that returns it"
    ))
  func declaredInline() throws {
    let editor = try createEditor(
      CreateEditorArgs(
        nodes: [.klass(ChipNode.self), .klass(BoxNode.self)], onError: { error in throw error }))
    try editor.update(discrete: true) { () throws in
      #expect(try create(ChipNode.self).isInline() == true)
      #expect(try create(BoxNode.self).isInline() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > mergeAdjacent.by names an unknown field"
    ))
  func unknownField() {
    expectError("not a schema field") {
      _ = try createEditor(CreateEditorArgs(nodes: [.klass(BadByNode.self)]))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > mergeAdjacent.by names a derived field or state"
    ))
  func derivedFieldOrState() {
    expectError("derived") {
      _ = try createEditor(CreateEditorArgs(nodes: [.klass(BadDerivedNode.self)]))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > declared inline disagrees with isInline()"
    ))
  func inlineDisagrees() {
    expectError("isInline") {
      _ = try createEditor(CreateEditorArgs(nodes: [.klass(BadInlineNode.self)]))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::validation at editor creation > a merging group's children must hold inline content"
    ))
  func mergingGroupChildren() {
    expectError("hold inline content") {
      _ = try createEditor(
        CreateEditorArgs(nodes: [.klass(MergingItemNode.self), .klass(MergingGroupNode.self)]))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a ParagraphNode subclass whose isInline() returns true"
    ))
  func inlineParagraphSubclass() throws {
    let editor = try createEditor(
      CreateEditorArgs(nodes: [.klass(InlineParagraphNode.self)], onError: { error in throw error })
    )
    try editor.update(discrete: true) { () throws in
      let node = try InlineParagraphNode()
      try getRoot().append(createParagraphNode().append(node))
      #expect(try node.isInline() == true)
    }
    #expect(getDeclaredNodeStructure(InlineParagraphNode.self).inline == nil)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a LinkNode subclass whose isInline() returns false"
    ))
  func blockLinkSubclass() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "block-link", dependencies: [RichTextExtension, ListExtension, LinkExtension],
        nodes: { [.klass(BlockLinkNode.self)] },
        editorConfig: InitialEditorConfig(
          onError: { error, _ in throw error },
          initialEditorState: .update { _ in
            try getRoot().append(
              BlockLinkNode("https://a").append(
                createParagraphNode().append(createTextNode("x"))))
          })))
    defer { editor.dispose() }
    #expect(throws: Never.self) {
      _ = try createEditor(
        CreateEditorArgs(nodes: [
          .klass(LinkNode.self), .klass(ListNode.self), .klass(ListItemNode.self),
          .klass(BlockLinkNode.self),
        ]))
    }
    let declared = getDeclaredNodeStructure(BlockLinkNode.self)
    #expect(declared.inline == nil)
    #expect(declared.declaredBy.inline == nil)
    // The other inherited keys stay (they agree with the class).
    #expect(declared.children == .inline)
    #expect(
      declared.mergeAdjacent?.by == [
        .field("url"), .field("target"), .field("rel"), .field("title"),
      ])
    try editor.read { () throws -> Void in
      #expect(try getRoot().getFirstChildOrThrow().isInline() == false)
    }
    // LinkNode itself still declares it.
    #expect(getDeclaredNodeStructure(LinkNode.self).inline == true)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a subclass that declares the key itself is still checked"
    ))
  func ownInlineDeclarationChecked() {
    expectError("isInline") {
      _ = try createEditor(
        CreateEditorArgs(nodes: [.klass(LinkNode.self), .klass(BadOwnInlineLinkNode.self)]))
    }
  }

  // JS checks which prototypes own an `isInline()`; Swift cannot ask whether
  // a class overrides a method, so the port asserts what each class's
  // `isInline()` returns, which is what the synthesized method decides.
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > isInline() is synthesized only for a class that declares inline itself"
    ))
  func isInlineSynthesizedForDeclaringClass() throws {
    let editor = try createEditor(
      CreateEditorArgs(
        nodes: [
          .klass(OwnChipNode.self), .klass(SubChipNode.self), .klass(LinkNode.self),
          .klass(ModernLinkNode.self), .klass(PlainBoxNode.self), .klass(BlockLinkNode.self),
        ], onError: { error in throw error }))
    try editor.update(discrete: true) { () throws -> Void in
      #expect(try create(OwnChipNode.self).isInline() == true)
      // Inherited, not synthesized again: the same answers as the parent.
      #expect(try create(SubChipNode.self).isInline() == true)
      #expect(try create(ModernLinkNode.self).isInline() == true)
      #expect(try create(PlainBoxNode.self).isInline() == false)
      #expect(try createParagraphNode().isInline() == false)
      // BlockLinkNode's own isInline() is untouched.
      #expect(try BlockLinkNode().isInline() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > an inherited inline is dropped when the subclass isInline() cannot be evaluated without an instance"
    ))
  func inheritedInlineDroppedWithoutInstance() {
    #expect(throws: Never.self) {
      _ = try createEditor(
        CreateEditorArgs(nodes: [.klass(LinkNode.self), .klass(StatefulLinkNode.self)]))
    }
    #expect(getDeclaredNodeStructure(StatefulLinkNode.self).inline == nil)
    #expect(getDeclaredNodeStructure(LinkNode.self).inline == true)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > an inherited mergeAdjacent that does not fit the subclass is dropped"
    ))
  func inheritedMergeAdjacentDropped() throws {
    _ = try createEditor(
      CreateEditorArgs(nodes: [
        .klass(FitItemNode.self), .klass(FitBlockItemNode.self), .klass(FitGroupNode.self),
        .klass(FitBlockGroupNode.self),
      ]))
    #expect(
      getDeclaredNodeStructure(FitGroupNode.self).mergeAdjacent
        == MergeAdjacentConfig(by: [.field("format")]))
    #expect(getDeclaredNodeStructure(FitBlockGroupNode.self).mergeAdjacent == nil)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > a derived schema field in mergeAdjacent.by is refused with its own message"
    ))
  func derivedSchemaField() {
    #expect(getDeclaredNodeStructure(CountedNode.self).derived == ["count"])
    expectError("names count, which is derived") {
      _ = try createEditor(CreateEditorArgs(nodes: [.klass(CountedNode.self)]))
    }
  }
}

@Suite struct NodeStructureExtensionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a heading inside a link becomes a heading holding the link"
    ))
  func headingInsideLink() throws {
    let editor = try build(
      {
        try getRoot().append(
          createParagraphNode().append(
            createLinkNode("https://a").append(
              createHeadingNode("h1").append(createTextNode("Lexical")))))
      }, true)
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["heading",["link","Lexical"]]]"#)
    #expect(
      try editor.read {
        try (getRoot().getAllTextNodes()[0].getParent() as! LinkNode).getURL()
      } == "https://a")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a caret between two merged links stays between their contents"
    ))
  func caretBetweenMergedLinks() throws {
    let editor = try build(
      {
        let p = try createParagraphNode().append(
          createLinkNode("https://a").append(createTextNode("one")),
          createLinkNode("https://a").append(createTextNode("two")))
        try getRoot().append(p)
        _ = try p.select(1, 1)
      }, true)
    defer { editor.dispose() }
    try editor.read { () throws -> Void in
      let selection = try #require(try getSelection() as? RangeSelection)
      let point = selection.anchor
      let node = try point.getNode()
      // At the boundary: after "one", not after "onetwo".
      let text = try node.getTextContent().jsSlice(0, point.offset)
      #expect((["one", "onetwo"].contains(text) ? text : point.type.rawValue) != "onetwo")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > adjacent equal links merge; different ones do not"
    ))
  func adjacentEqualLinksMerge() throws {
    let editor = try build(
      {
        try getRoot().append(
          createParagraphNode().append(
            createLinkNode("https://a").append(createTextNode("1")),
            createLinkNode("https://a").append(createTextNode("2")),
            createLinkNode("https://b").append(createTextNode("3"))))
      }, true)
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["paragraph",["link","12"],["link","3"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a subclass override of shouldMergeAdjacentLink keeps deciding"
    ))
  func subclassShouldMergeAdjacentLink() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "override",
        dependencies: [RichTextExtension, LinkExtension, NodeStructureExtension],
        nodes: { [.klass(NoMergeLinkNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createParagraphNode().append(
                create(NoMergeLinkNode.self).setURL("https://a").append(createTextNode("1")),
                create(NoMergeLinkNode.self).setURL("https://a").append(createTextNode("2"))))
          })))
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["paragraph",["test-nomerge-link","1"],["test-nomerge-link","2"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > an element caret after a lifted block stays after it"
    ))
  func elementCaretAfterLiftedBlock() throws {
    // What is before and after the caret in its element, and where that
    // element is. A point at an end of a text node (where the normalization
    // puts an element point next to it) reads as the point next to the node.
    func caret(_ editor: Editor) throws -> JSONValue {
      try editor.read {
        let selection = try #require(try getSelection() as? RangeSelection)
        let anchor = selection.anchor
        let node = try anchor.getNode()
        if let text = node as? TextNode {
          let content = try text.getTextContent()
          return .array([
            .string(
              anchor.offset > 0
                ? content.jsSlice(0, anchor.offset)
                : try text.getPreviousSibling()?.getTextContent() ?? "^"),
            .string(
              anchor.offset < content.utf16.count
                ? content.jsSlice(anchor.offset)
                : try text.getNextSibling()?.getTextContent() ?? "$"),
            .number(Double(try text.getParentOrThrow().getIndexWithinParent())),
          ])
        }
        let element = try #require(node as? ElementNode)
        return .array([
          .string(try element.getChildAtIndex(anchor.offset - 1)?.getTextContent() ?? "^"),
          .string(try element.getChildAtIndex(anchor.offset)?.getTextContent() ?? "$"),
          .number(Double(try element.getIndexWithinParent())),
        ])
      }
    }
    func split(_ offset: Int) throws -> Editor {
      try build(
        {
          let p = try createParagraphNode().append(
            createTextNode("a"), createHorizontalRuleNode(), createTextNode("b"))
          try getRoot().append(p)
          _ = try p.select(offset, offset)
        }, true)
    }
    // At the end of a paragraph split around a rule: after "b".
    let end = try split(3)
    defer { end.dispose() }
    #expect(try caret(end) == ["b", "$", 2])
    // Right after the rule: at the start of the second part.
    let after = try split(2)
    defer { after.dispose() }
    #expect(try caret(after) == ["^", "b", 2])
    // Right before it: at the end of the first part.
    let before = try split(1)
    defer { before.dispose() }
    #expect(try caret(before) == ["a", "$", 0])
    // In a list item, after a link that follows the rule.
    let item = try build(
      {
        let li = try createListItemNode()
        try getRoot().append(createListNode("number").append(li))
        try put(
          li,
          [
            try createTextNode("x"), try createHorizontalRuleNode(),
            try createLinkNode("https://a").append(createTextNode("y")),
          ])
        _ = try li.select(3, 3)
      }, true)
    defer { item.dispose() }
    #expect(
      try rootShape(item)
        == #"[["list",["listitem","x"]],"horizontalrule",["list",["listitem",["link","y"]]]]"#)
    #expect(try caret(item) == ["y", "$", 0])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a shadow root quote keeps its blocks"
    ))
  func shadowRootQuote() throws {
    let editor = try build(
      {
        try getRoot().append(
          createQuoteNode(shadowRoot: true).append(
            createParagraphNode().append(createTextNode("one")),
            createParagraphNode().append(createTextNode("two"))))
      }, true)
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["quote",["paragraph","one"],["paragraph","two"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a quote holding paragraphs stays one quote, with line breaks"
    ))
  func quoteHoldingParagraphs() throws {
    let editor = try build(
      {
        try getRoot().append(
          createQuoteNode().append(
            createParagraphNode().append(createTextNode("a")),
            createParagraphNode().append(createTextNode("b"))),
          createQuoteNode().append(
            createTextNode("x"), createParagraphNode().append(createTextNode("c")),
            createTextNode("y")))
      }, true)
    defer { editor.dispose() }
    // As markdown's "> a\n>\n> b" imports (and Shift+Enter in a quote).
    #expect(
      try rootShape(editor)
        == #"[["quote","a","linebreak","b"],["quote","x","linebreak","c","linebreak","y"]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > an empty paragraph in a quote keeps its line"
    ))
  func emptyParagraphInQuote() throws {
    let editor = try build(
      {
        let empty = try createParagraphNode()
        try getRoot().append(
          createQuoteNode().append(
            createParagraphNode(), createParagraphNode().append(createTextNode("a")), empty))
        try empty.select()
      }, true)
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["quote","linebreak","a","linebreak"]]"#)
    try editor.read { () throws in
      // The caret that was in the last (empty) paragraph is on that line.
      let selection = try #require(try getSelection() as? RangeSelection)
      let quote = try #require(try getRoot().getFirstChild())
      #expect(selection.anchor.key == quote.getKey())
      #expect(selection.anchor.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a class that declares mergeAdjacent merges; a subclass that inherits it merges by its own code"
    ))
  func declaredMergeAdjacent() throws {
    func tag(_ klass: TagNode.Type, _ tag: String, _ text: String) throws -> LexicalNode {
      let node = try create(klass)
      node.__tag = tag
      return try node.append(createTextNode(text))
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "tags", dependencies: [RichTextExtension, NodeStructureExtension],
        nodes: { [.klass(TagNode.self), .klass(SubTagNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createParagraphNode().append(
                tag(TagNode.self, "a", "1"), tag(TagNode.self, "a", "2"),
                tag(TagNode.self, "b", "3")),
              createParagraphNode().append(
                tag(SubTagNode.self, "a", "4"), tag(SubTagNode.self, "a", "5")))
          })))
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["paragraph",["test-tag","12"],["test-tag","3"]],["paragraph",["test-subtag","4"],["test-subtag","5"]]]"#
    )
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > the importers read whether an editor enforces the structure"
    ))
  func importersReadEnforcement() throws {
    let on = try build({}, true)
    defer { on.dispose() }
    let off = try build({}, false)
    defer { off.dispose() }
    #expect(INTERNAL_isNodeStructureEnforced(on))
    #expect(!INTERNAL_isNodeStructureEnforced(off))
    #expect(!INTERNAL_isNodeStructureEnforced(try createEditor()))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > a class with an unrelated transform of its own is still merged"
    ))
  func unrelatedTransform() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "clean-tag", dependencies: [RichTextExtension, NodeStructureExtension],
        nodes: { [.klass(CleanTagNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            for text in ["a", "b"] {
              let node = try create(CleanTagNode.self)
              node.__tag = "t"
              try getRoot().append(
                node.append(createParagraphNode().append(createTextNode(text))))
            }
          })))
    defer { editor.dispose() }
    #expect(
      try rootShape(editor) == #"[["test-clean-tag",["paragraph","a"],["paragraph","b"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > mergeAdjacent.by naming a NodeState"
    ))
  func mergeByNodeState() throws {
    func color(_ value: String, _ text: String) throws -> LexicalNode {
      try setState(create(ColorNode.self), colorState, value).append(createTextNode(text))
    }
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "color", dependencies: [RichTextExtension, NodeStructureExtension],
        nodes: { [.klass(ColorNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createParagraphNode().append(
                color("red", "1"), color("red", "2"), color("blue", "3")))
          })))
    defer { editor.dispose() }
    #expect(
      try rootShape(editor) == #"[["paragraph",["test-color","12"],["test-color","3"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > a custom group wraps a stray child in the first class it lists"
    ))
  func customGroupWraps() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "group", dependencies: [RichTextExtension, NodeStructureExtension],
        nodes: { [.klass(WrapItemNode.self), .klass(WrapGroupNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let group = try create(WrapGroupNode.self)
            try getRoot().append(group)
            try group.elementSplice(
              0, 0,
              [
                createTextNode("a"), createLineBreakNode(),
                createParagraphNode().append(createTextNode("p")), createTextNode("b"),
              ])
          })))
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["test-wrap-group",["test-wrap-item","a","linebreak"],["paragraph","p"],["test-wrap-item","b"]]]"#
    )
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > typed blocks inside a quote are split around and keep their type"
    ))
  func typedBlocksInQuote() throws {
    let editor = try build(
      {
        try getRoot().append(
          createQuoteNode().append(
            createParagraphNode().append(createTextNode("a")),
            createHeadingNode("h2").append(createTextNode("h")),
            createParagraphNode().append(createTextNode("b")),
            createCodeNode().append(createTextNode("c")),
            createQuoteNode().append(createTextNode("q")),
            createParagraphNode().append(createTextNode("d"))))
      }, true)
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["quote","a"],["heading","h"],["quote","b"],["code","c"],["quote","q"],["quote","d"]]"#
    )
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > paragraphs in other blocks that hold inline content become lines"
    ))
  func paragraphsInInlineBlocks() throws {
    let editor = try build(
      {
        try getRoot().append(
          createCodeNode().append(
            createParagraphNode().append(createTextNode("one")),
            createParagraphNode().append(createTextNode("two"))),
          createHeadingNode("h1").append(
            createTextNode("x"), createLineBreakNode(),
            createParagraphNode().append(createTextNode("y"))))
      }, true)
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["code","one","linebreak","two"],["heading","x","linebreak","linebreak","y"]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a paragraph in a block in a holder: the result does not depend on which is repaired first"
    ))
  func repairOrderIndependent() throws {
    let doc: () throws -> Void = {
      _ = try getRoot().append(
        createCodeNode().append(
          createParagraphNode().append(
            createHeadingNode("h1").append(createTextNode("h")), createTextNode("a"))))
    }
    let built = try build(doc, true)
    defer { built.dispose() }
    let want = try rootShape(built)
    #expect(want == #"[["heading","h"],["code","a"]]"#)
    // The same document built with the extension off, then every element
    // marked dirty inner first once it is on.
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "order",
        dependencies: [
          RichTextExtension, configExtension(NodeStructureExtension) { $0.disabled = true },
        ],
        nodes: { [.klass(CodeNode.self)] },
        editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try doc() })))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      var all: [LexicalNode] = []
      func walk(_ n: LexicalNode) throws {
        if let element = n as? ElementNode {
          try element.getChildren().forEach(walk)
        }
        all.append(n)
      }
      try getRoot().getChildren().forEach(walk)
      try all.forEach { try $0.markDirty() }
    }
    try getExtensionDependencyFromEditor(editor, NodeStructureExtension).output.disabled.set(false)
    #expect(try rootShape(editor) == want)
  }
}

@Suite struct NodeStructureRegistrationTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > registerNodeStructure: idempotent, and disposed by its last registration"
    ))
  func idempotentRegistration() throws {
    let editor = try createEditor(
      CreateEditorArgs(nodes: [.klass(HorizontalRuleNode.self)], onError: { error in throw error })
    )
    func broken() throws {
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("a"), createHorizontalRuleNode()))
    }
    func sh(_ n: LexicalNode) throws -> JSONValue {
      if let element = n as? ElementNode {
        return .array([.string(n.getType())] + (try element.getChildren().map(sh)))
      }
      return .string(n.getType())
    }
    func rootTypes() throws -> String {
      try editor.read { JSONValue.array(try getRoot().getChildren().map(sh)).stringify() }
    }
    let first = try registerNodeStructure(editor)
    let second = try registerNodeStructure(editor)
    try editor.update(discrete: true, broken)
    #expect(try rootTypes() == #"[["paragraph","text"],"horizontalrule"]"#)
    first()
    first()
    try editor.update(discrete: true, broken)
    #expect(try rootTypes() == #"[["paragraph","text"],"horizontalrule"]"#)
    second()
    #expect(!INTERNAL_isNodeStructureEnforced(editor))
    try editor.update(discrete: true, broken)
    #expect(try rootTypes() == #"[["paragraph","text","horizontalrule"]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > disabled: the declarations are only read; turning it on normalizes the document"
    ))
  func disabledThenEnabled() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "disabled",
        dependencies: [
          RichTextExtension, HorizontalRuleExtension,
          configExtension(NodeStructureExtension) { $0.disabled = true },
        ],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in try paragraphWithRule() })))
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["paragraph","a","horizontalrule","b"]]"#)
    #expect(!INTERNAL_isNodeStructureEnforced(editor))
    try getExtensionDependencyFromEditor(editor, NodeStructureExtension).output.disabled.set(false)
    #expect(
      try rootShape(editor) == #"[["paragraph","a"],"horizontalrule",["paragraph","b"]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > each editor enforces its own registration (nested editors too)"
    ))
  func eachEditorItsOwn() throws {
    let parent = try build({}, true)
    defer { parent.dispose() }
    let nested = try createEditor(
      CreateEditorArgs(
        nodes: [.klass(HorizontalRuleNode.self)], onError: { error in throw error },
        parentEditor: parent))
    try nested.update(discrete: true, paragraphWithRule)
    func nestedTypes() throws -> [String] {
      try nested.read { try getRoot().getChildren().map { $0.getType() } }
    }
    #expect(try nestedTypes() == ["paragraph"])
    let dispose = try registerNodeStructure(nested)
    #expect(try nestedTypes() == ["paragraph", "horizontalrule", "paragraph"])
    dispose()
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::registration > INTERNAL_$normalizeNodeStructure normalizes a subtree now, without the extension"
    ))
  func normalizeSubtreeNow() throws {
    // No extension: the code that changed the document calls it itself.
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "internal", dependencies: [RichTextExtension, HorizontalRuleExtension],
        nodes: { [.klass(BlockTagNode.self)] }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      func tag(_ children: LexicalNode...) throws -> LexicalNode {
        let node = try create(BlockTagNode.self)
        node.__tag = "t"
        return try node.append(contentsOf: children)
      }
      try getRoot().clear().append(
        tag(createTextNode("a")), tag(createTextNode("b"), createHorizontalRuleNode()))
    }
    // Nothing enforces the structure yet.
    #expect(
      try rootShape(editor)
        == #"[["test-block-tag","a"],["test-block-tag","b","horizontalrule"]]"#)
    // The second node is split around the rule, then merged with its equal
    // sibling. (Core then joins the two text nodes.)
    try editor.update(discrete: true) {
      try INTERNAL_normalizeNodeStructure(getRoot().getLastChildOrThrow())
    }
    let normalized = #"[["test-block-tag","ab"],"horizontalrule"]"#
    #expect(try rootShape(editor) == normalized)
    try editor.update(discrete: true) { try INTERNAL_normalizeNodeStructure(getRoot()) }
    #expect(try rootShape(editor) == normalized)
  }
}

// MARK: carets, empty inline elements and dirty order

private func buildCaretEditor(_ withStructure: Bool = true) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "node-structure-carets",
      dependencies: [RichTextExtension, ListExtension, LinkExtension, HorizontalRuleExtension]
        + (withStructure ? [NodeStructureExtension] : []),
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
}

/// Load a document as an update applied with skipTransforms leaves it (not
/// normalized), then mark every node dirty in `order`.
private func loadThenNormalize(
  _ editor: Editor, _ build: @escaping () throws -> [LexicalNode],
  _ select: @escaping () throws -> Void = {}, order: String = "outer first"
) throws {
  try editor.update(discrete: true, skipTransforms: true) {
    try getRoot().clear().append(contentsOf: build())
    try select()
  }
  try editor.update(discrete: true) {
    var nodes: [LexicalNode] = []
    func walk(_ node: LexicalNode) throws {
      if order == "outer first" {
        nodes.append(node)
      }
      if let element = node as? ElementNode {
        try element.getChildren().forEach(walk)
      }
      if order == "inner first" {
        nodes.append(node)
      }
    }
    try getRoot().getChildren().forEach(walk)
    try nodes.forEach { try $0.markDirty() }
  }
}

private func content(_ editor: Editor) throws -> String {
  try editor.read {
    var out = ""
    func walk(_ node: LexicalNode) throws {
      if let element = node as? ElementNode {
        try element.getChildren().forEach(walk)
      } else if node is HorizontalRuleNode {
        out += "#"
      } else if let text = node as? TextNode {
        // Line breaks are left out: joining paragraphs adds them.
        out += try text.getTextContent()
      }
    }
    try getRoot().getChildren().forEach(walk)
    return out
  }
}

@Suite struct NodeStructureCaretTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an empty link alone in a list is removed before its holder is repaired, whatever the dirty order"
    ))
  func emptyLinkAloneInList() throws {
    for order in ["outer first", "inner first"] {
      let editor = try buildCaretEditor()
      defer { editor.dispose() }
      try loadThenNormalize(
        editor,
        {
          [
            try put(createListNode("bullet"), [try createLinkNode("https://a")]),
            try put(createParagraphNode(), [try createTextNode("z")]),
          ]
        }, order: order)
      #expect(try rootShape(editor) == #"[["paragraph","z"]]"#)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret after a paragraph split around a rule keeps its place in the content"
    ))
  func caretAfterParagraphSplit() throws {
    let editor = try buildCaretEditor()
    defer { editor.dispose() }
    try loadThenNormalize(
      editor,
      {
        [
          try put(
            createParagraphNode(),
            [createTextNode("a"), createHorizontalRuleNode(), createTextNode("b")])
        ]
      },
      {
        let root = try getRoot()
        let selection = createRangeSelection()
        try selection.anchor.set(root.getKey(), 1, .element)
        try selection.focus.set(root.getKey(), 1, .element)
        try setSelection(selection)
      })
    try editor.update(discrete: true) { try getSelection()!.insertText("Z") }
    #expect(try content(editor) == "a#bZ")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > a node whose rule changes is checked again, though no child was written"
    ))
  func ruleChangeChecked() throws {
    let editor = try buildCaretEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(createQuoteNode().append(createTextNode("a")))
    }
    try editor.update(discrete: true) {
      // A shadow root holds blocks: its text is wrapped in a paragraph.
      try (getRoot().getFirstChild() as! QuoteNode).setIsShadowRoot(true)
    }
    #expect(try rootShape(editor) == #"[["quote",["paragraph","a"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > inline content at the root is wrapped whenever core would wrap it"
    ))
  func rootInlineContentWrapped() throws {
    func run(_ withStructure: Bool, _ edit: @escaping () throws -> Void) throws -> String {
      let editor = try buildCaretEditor(withStructure)
      defer { editor.dispose() }
      try editor.update(discrete: true) {
        try put(getRoot().clear(), [createParagraphNode().append(createTextNode("p"))])
      }
      // Left at the root by an update that runs no transform.
      try editor.update(discrete: true, skipTransforms: true) {
        try put(getRoot(), [createTextNode("stray")])
      }
      try editor.update(discrete: true, edit)
      return try rootShape(editor)
    }
    let edits: [() throws -> Void] = [
      // An edit somewhere else.
      { try (getRoot().getFirstDescendant() as! TextNode).setTextContent("pp") },
      // An update that only marks the root dirty.
      { try getRoot().markDirty() },
    ]
    for edit in edits {
      #expect(try run(true, edit) == run(false, edit))
    }
    #expect(try run(true, edits[0]) == #"[["paragraph","pp"],["paragraph","stray"]]"#)
  }
}

// MARK: lists

/// Each top-level list as its items' values, anything else as its type.
private func listValues(_ editor: Editor) throws -> JSONValue {
  try editor.read {
    .array(
      try getRoot().getChildren().map { node in
        guard let list = node as? ListNode else { return .string(node.getType()) }
        return .array(try list.getChildren().map { .int(try ($0 as! ListItemNode).getValue()) })
      })
  }
}

/// Each top-level list as its start and its items' text, anything else as
/// its type.
private func listStarts(_ editor: Editor) throws -> JSONValue {
  try editor.read {
    .array(
      try getRoot().getChildren().map { node in
        guard let list = node as? ListNode else { return .string(node.getType()) }
        return .array(
          [.int(try list.getStart())]
            + (try list.getChildren().map { .string(try $0.getTextContent()) }))
      })
  }
}

private enum Item {
  case text(String)
  case nodes(() throws -> [LexicalNode])
}

private func numbered(_ start: Int, _ items: Item...) throws {
  try getRoot().append(
    createListNode("number", start: start).append(
      contentsOf: items.map { item in
        switch item {
        case .text(let text): return try createListItemNode().append(createTextNode(text))
        case .nodes(let make): return try createListItemNode().append(contentsOf: make())
        }
      }))
}

private func listItem(_ text: String) throws -> ListItemNode {
  try createListItemNode().append(createTextNode(text))
}

/// A ListNode subclass written before `$config`: it declares its type,
/// clone and JSON by hand, and stores a field its schema does not know.
private final class LabeledListNode: ListNode {
  var __label = ""
  override class func getType() -> String { "test-labeled-list" }
  override class func clone(_ node: LexicalNode) throws -> LexicalNode {
    let node = node as! LabeledListNode
    let clone = try LabeledListNode(node.__listType, start: node.__start, key: node.__key)
    clone.__label = node.__label
    return clone
  }
  override class func importJSON(_ json: JSONObject) throws -> LexicalNode {
    let node = try LabeledListNode().updateFromJSON(json)
    node.__label = json["label"]?.stringValue ?? ""
    return node
  }
  override func exportJSON(_ compact: Bool = false) throws -> JSONObject {
    var json = try super.exportJSON(compact)
    json["label"] = .string(__label)
    return json
  }
}

private final class SubListNode: ListNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SubListNode.self, type: "test-sub-list", extends: ListNode.self)
  }
}

@Suite struct NodeStructureListTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::declarations > derived fields"
    ))
  func derivedFields() {
    #expect(getDeclaredNodeStructure(ListItemNode.self).derived == ["indent", "value"])
    #expect(getDeclaredNodeStructure(ListNode.self).derived == [])
    let derived = createState("n", parse: { $0?.numberValue ?? 0 }, derived: true)
    let plain = createState("n", parse: { $0?.numberValue ?? 0 })
    #expect(derived.derived)
    #expect(!plain.derived)
  }

  // Swift returns the declaration as a value: a copy changed by the caller
  // leaves the cached one as it was (JS freezes it).
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::getDeclaredNodeStructure > returns immutable results"
    ))
  func immutableResults() {
    var declared = getDeclaredNodeStructure(ListItemNode.self).derived
    declared.insert("checked")
    #expect(getDeclaredNodeStructure(ListItemNode.self).derived == ["indent", "value"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::existing subclasses that inherit a declaration > a ListNode subclass with an extra field and a legacy exportJSON"
    ))
  func legacyListSubclass() throws {
    let nodes: [LexicalNodeConfig] = [
      .klass(ListNode.self), .klass(ListItemNode.self), .klass(LabeledListNode.self),
    ]
    let editor = try createEditor(CreateEditorArgs(nodes: nodes, onError: { error in throw error }))
    try editor.update(discrete: true) {
      let list = try LabeledListNode("number")
      list.__label = "t"
      try getRoot().append(list.append(listItem("a")))
    }
    let json = try editor.getEditorState().toJSON()
    let root: JSONObject? = json["root"]?.objectValue
    let children: [JSONValue]? = root?["children"]?.arrayValue
    let first: JSONObject? = children?.first?.objectValue
    #expect(first?["label"] == "t")
    #expect(first?["listType"] == "number")
    #expect(first?["type"] == "test-labeled-list")
    let restored = try createEditor(
      CreateEditorArgs(nodes: nodes, onError: { error in throw error }))
    try restored.setEditorState(restored.parseEditorState(json))
    #expect(try restored.getEditorState().toJSON().stringify() == json.stringify())
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > paragraphs in a list item become lines of the item"
    ))
  func paragraphsInListItem() throws {
    let editor = try build(
      {
        let x = try createTextNode("x")
        try getRoot().append(createListNode("bullet").append(createListItemNode().append(x)))
        // insertAfter, not ListItemNode.append (which unwraps blocks itself).
        let p = try createParagraphNode().append(createTextNode("p"))
        try x.insertAfter(p)
        try p.insertAfter(createParagraphNode().append(createTextNode("q")))
      }, true)
    defer { editor.dispose() }
    // As the HTML import of <li>x<p>p</p><p>q</p></li> gives.
    #expect(
      try rootShape(editor) == #"[["list",["listitem","x","linebreak","p","linebreak","q"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a list item inside a paragraph is lifted without throwing"
    ))
  func listItemInParagraph() throws {
    let editor = try build(
      {
        try getRoot().append(
          createParagraphNode().append(
            createTextNode("a"), listItem("item"), createTextNode("b")))
      }, true)
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["paragraph","a"],["list",["listitem","item"]],["paragraph","b"]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > adjacent lists of one type merge, whatever their start"
    ))
  func adjacentListsMerge() throws {
    let editor = try build(
      {
        try getRoot().append(
          createListNode("number", start: 1).append(listItem("a")),
          createListNode("number", start: 5).append(listItem("b")),
          createListNode("bullet").append(listItem("c")))
      }, true)
    defer { editor.dispose() }
    #expect(
      try rootShape(editor)
        == #"[["list",["listitem","a"],["listitem","b"]],["list",["listitem","c"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator splits an ordered list wherever it sits in the item; numbering continues"
    ))
  func blockDecoratorSplitsList() throws {
    func hr() throws -> LexicalNode { try createHorizontalRuleNode() }
    func t(_ text: String) throws -> LexicalNode { try createTextNode(text) }
    // The list starts at 5: x is 5, the item holding the rule is 6.
    let cases: [(String, () throws -> [LexicalNode], JSONValue)] = [
      ("first", { [try hr(), try t("b")] }, [[5, "x"], "horizontalrule", [6, "b", "y"]]),
      (
        "middle", { [try t("a"), try hr(), try t("b")] },
        [[5, "x", "a"], "horizontalrule", [7, "b", "y"]]
      ),
      ("last", { [try t("a"), try hr()] }, [[5, "x", "a"], "horizontalrule", [7, "y"]]),
      ("alone", { [try hr()] }, [[5, "x"], "horizontalrule", [6, "y"]]),
    ]
    for (name, item, expected) in cases {
      let editor = try build({ try numbered(5, .text("x"), .nodes(item), .text("y")) }, true)
      defer { editor.dispose() }
      #expect(try listStarts(editor) == expected, "\(name)")
      // The item values follow: the second list continues the numbering.
      let values = JSONValue.array(
        expected.arrayValue!.map { entry in
          guard let list = entry.arrayValue else { return entry }
          let start = Int(list[0].numberValue!)
          return .array((0..<(list.count - 1)).map { .int(start + $0) })
        })
      #expect(try listValues(editor) == values, "\(name)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator in the first or last item of an ordered list"
    ))
  func blockDecoratorAtListEnds() throws {
    let cases: [(() throws -> Void, JSONValue)] = [
      (
        { try numbered(3, .nodes({ [try createHorizontalRuleNode()] }), .text("y")) },
        ["horizontalrule", [3, "y"]]
      ),
      (
        { try numbered(3, .text("x"), .nodes({ [try createHorizontalRuleNode()] })) },
        [[3, "x"], "horizontalrule"]
      ),
    ]
    for (initial, expected) in cases {
      let editor = try build(initial, true)
      defer { editor.dispose() }
      #expect(try listStarts(editor) == expected)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block decorator put in a list item later splits the same way"
    ))
  func blockDecoratorPutLater() throws {
    let editor = try build({ try numbered(5, .text("x"), .text("a"), .text("y")) }, true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try getRoot().getFirstChildOrThrow() as! ListNode
      let item = try list.getChildAtIndex(1) as! ElementNode
      try item.getFirstChildOrThrow().insertBefore(createHorizontalRuleNode())
    }
    #expect(try listStarts(editor) == [[5, "x"], "horizontalrule", [6, "a", "y"]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a block lifted out of a list item splits the list and keeps its type"
    ))
  func blockLiftedOutOfListItem() throws {
    func h() throws -> LexicalNode { try createHeadingNode("h2").append(createTextNode("h")) }
    let cases: [(() throws -> [LexicalNode], JSONValue)] = [
      ({ [try h(), try createTextNode("a")] }, [[1, "x"], "heading", [2, "a", "y"]]),
      ({ [try createTextNode("a"), try h()] }, [[1, "x", "a"], "heading", [3, "y"]]),
      ({ [try createCodeNode().append(createTextNode("c"))] }, [[1, "x"], "code", [2, "y"]]),
    ]
    for (item, expected) in cases {
      let editor = try build({ try numbered(1, .text("x"), .nodes(item), .text("y")) }, true)
      defer { editor.dispose() }
      #expect(try listStarts(editor) == expected)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a text caret survives the split of its node"
    ))
  func textCaretSurvivesSplit() throws {
    let editor = try build({ try numbered(1, .text("x"), .text("y")) }, true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try getRoot().getAllTextNodes()[0]
      _ = try text.select(1, 1)
      try text.insertAfter(createHorizontalRuleNode())
    }
    try editor.update(discrete: true) { try getSelection()!.insertText("1") }
    #expect(try listStarts(editor) == [[1, "x1"], "horizontalrule", [2, "y"]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::NodeStructureExtension > a list item holding a paragraph and a nested list gets no stray line break"
    ))
  func paragraphAndNestedList() throws {
    let editor = try build(
      {
        let li = try createListItemNode()
        try getRoot().append(createListNode("bullet").append(li))
        try li.elementSplice(
          0, 0,
          [
            try createParagraphNode().append(createTextNode("a")),
            try createListNode("bullet").append(listItem("b")),
          ])
      }, true)
    defer { editor.dispose() }
    // The nested list then moves into an item of its own (a list item
    // holding a nested list holds nothing else).
    #expect(
      try rootShape(editor)
        == #"[["list",["listitem","a"],["listitem",["list",["listitem","b"]]]]]"#)
  }
}

@Suite struct NodeStructureListMergingTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > AutoLinkNode never merges"
    ))
  func autoLinkNeverMerges() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "autolink",
        dependencies: [RichTextExtension, LinkExtension, NodeStructureExtension],
        nodes: { [.klass(AutoLinkNode.self)] },
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              createParagraphNode().append(
                createAutoLinkNode("https://a").append(createTextNode("1")),
                createAutoLinkNode("https://a").append(createTextNode("2"))))
          })))
    defer { editor.dispose() }
    #expect(try rootShape(editor) == #"[["paragraph",["autolink","1"],["autolink","2"]]]"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > a ListNode subclass merges as it does without the extension"
    ))
  func subclassMergesAsWithout() throws {
    func list(_ sub: Bool, _ text: String) throws -> LexicalNode {
      try (sub ? create(SubListNode.self) : createListNode()).setListType("bullet")
        .append(listItem(text))
    }
    for (first, second) in [(false, false), (true, true), (false, true)] {
      let shapes = try [false, true].map { withStructure in
        let editor = try buildEditorFromExtensions(
          defineExtension(
            name: "sub-list",
            dependencies: [RichTextExtension, ListExtension]
              + (withStructure ? [NodeStructureExtension] : []),
            nodes: { [.klass(SubListNode.self)] },
            editorConfig: InitialEditorConfig(
              onError: { error, _ in throw error },
              initialEditorState: .update { _ in
                try getRoot().append(list(first, "a"), list(second, "b"))
              })))
        defer { editor.dispose() }
        return try editor.read { try getRoot().getChildren().map { try $0.getTextContent() } }
      }
      #expect(shapes[1] == shapes[0], "\(first) \(second)")
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > ListNode: the extension and the list transform merge the same lists"
    ))
  func extensionAndTransformMerge() throws {
    // A list brought next to another one by a repair (lifted out of a
    // paragraph) merges just as two lists that were adjacent already.
    let editor = try build(
      {
        try getRoot().append(createListNode("number", start: 2).append(listItem("a")))
        let p = try createParagraphNode()
        try getRoot().append(p)
        try p.elementSplice(0, 0, [try createListNode("number", start: 7).append(listItem("b"))])
        try getRoot().append(
          createListNode("number", start: 9).append(listItem("c")),
          createListNode("bullet").append(listItem("d")))
      }, true)
    defer { editor.dispose() }
    #expect(try listStarts(editor) == [[2, "a", "b", "c"], [1, "d"]])
    #expect(try listValues(editor) == [[2, 3, 4], [1]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::merging > ListNode: the extension merges lists exactly as the list transform does, nested lists at the seam included"
    ))
  func nestedListsAtSeam() throws {
    func doc() throws {
      func nested(_ text: String) throws -> LexicalNode {
        try createListItemNode().append(createListNode("bullet").append(listItem(text)))
      }
      try getRoot().append(
        createListNode("number", start: 3).append(listItem("x"), nested("a")),
        createListNode("number", start: 8).append(nested("b"), listItem("y")))
    }
    let on = try build(doc, true)
    defer { on.dispose() }
    let off = try build(doc, false)
    defer { off.dispose() }
    #expect(
      try rootShape(on)
        == #"[["list",["listitem","x"],["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem","y"]]]"#
    )
    #expect(try rootShape(off) == rootShape(on))
    #expect(
      try on.getEditorState().toJSON().stringify() == off.getEditorState().toJSON().stringify())
  }
}

@Suite struct NodeStructureListCaretTests {
  /// The shape of the document, with each link's URL.
  private func annotated(_ editor: Editor) throws -> String {
    func walk(_ node: LexicalNode) throws -> JSONValue {
      if let link = node as? LinkNode {
        return .array([.string("link:\(try link.getURL())")] + (try link.getChildren().map(walk)))
      }
      if let element = node as? ElementNode {
        return .array([.string(element.getType())] + (try element.getChildren().map(walk)))
      }
      return try shape(node)
    }
    return try editor.read { JSONValue.array(try getRoot().getChildren().map(walk)).stringify() }
  }

  private func load(_ editor: Editor, _ build: @escaping () throws -> [LexicalNode]) throws {
    try editor.update(discrete: true) { try put(getRoot().clear(), build()) }
  }

  private func li(_ children: LexicalNode...) throws -> ListItemNode {
    try put(createListItemNode(), children)
  }

  private func ul(_ children: LexicalNode...) throws -> ListNode {
    try put(createListNode("bullet"), children)
  }

  private func nested(_ text: String, _ rest: LexicalNode...) throws -> ListItemNode {
    try put(
      createListItemNode(),
      [try put(createListNode("number"), [try li(createTextNode(text))])] + rest)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > a list item outside a list, in a link, keeps the link as it does without the extension"
    ))
  func orphanItemInLink() throws {
    // The shape pasting <a href="https://a">l<li>x</li></a> builds.
    let build: () throws -> [LexicalNode] = {
      [
        try createParagraphNode().append(
          put(createLinkNode("https://a"), [try createTextNode("l"), try li(createTextNode("x"))]))
      ]
    }
    let on = try buildCaretEditor()
    defer { on.dispose() }
    let off = try buildCaretEditor(false)
    defer { off.dispose() }
    try load(on, build)
    try load(off, build)
    #expect(
      try annotated(on)
        == #"[["paragraph",["link:https://a","l"]],["list",["listitem",["link:https://a","x"]]]]"#)
    #expect(try annotated(on) == annotated(off))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > lists that merge keep what follows or precedes a nested list in an item at the seam"
    ))
  func mergingListsKeepSeamContent() throws {
    // That content moves into an item of its own (a list item holding a
    // nested list holds nothing else), so the wrapper items that then meet
    // join their nested lists.
    // Removing what separated them.
    do {
      let editor = try buildCaretEditor()
      defer { editor.dispose() }
      try load(editor) {
        [
          try ul(nested("a")), try createParagraphNode().append(createTextNode("sep")),
          try ul(nested("b", try createTextNode("tail"))),
        ]
      }
      try editor.update(discrete: true) { try getRoot().getChildAtIndex(1)!.remove() }
      #expect(
        try rootShape(editor)
          == #"[["list",["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem","tail"]]]"#
      )
    }
    // Loaded next to each other, the first item at the seam holding more.
    do {
      let editor = try buildCaretEditor()
      defer { editor.dispose() }
      try load(editor) {
        [try ul(nested("a", try createTextNode("x"))), try ul(nested("b"))]
      }
      #expect(
        try rootShape(editor)
          == #"[["list",["listitem",["list",["listitem","a"]]],["listitem","x"],["listitem",["list",["listitem","b"]]]]]"#
      )
    }
    // A list lifted out of a link next to an equal one.
    do {
      let editor = try buildCaretEditor()
      defer { editor.dispose() }
      try load(editor) {
        [
          try createParagraphNode().append(
            put(createLinkNode("https://a"), [try ul(nested("a"))])),
          try ul(nested("b", try createTextNode("tail"))),
        ]
      }
      #expect(
        try annotated(editor)
          == #"[["list",["listitem",["list",["listitem",["link:https://a","a"]],["listitem","b"]]],["listitem","tail"]]]"#
      )
    }
    // Items that only wrap a nested list still join, as without the
    // extension.
    let chain: () throws -> [LexicalNode] = {
      [
        try ul(nested("a")),
        try ul(nested("b"), nested("c"), li(createTextNode("d"))),
      ]
    }
    let on = try buildCaretEditor()
    defer { on.dispose() }
    let off = try buildCaretEditor(false)
    defer { off.dispose() }
    try load(on, chain)
    try load(off, chain)
    #expect(
      try rootShape(on)
        == #"[["list",["listitem",["list",["listitem","a"],["listitem","b"]]],["listitem",["list",["listitem","c"]]],["listitem","d"]]]"#
    )
    #expect(
      try on.getEditorState().toJSON().stringify() == off.getEditorState().toJSON().stringify())
  }

  private func typeAtElementCaret(
    _ build: @escaping () throws -> [LexicalNode], _ path: [Int], _ offset: Int
  ) throws -> String {
    let editor = try buildCaretEditor()
    defer { editor.dispose() }
    try loadThenNormalize(
      editor, build,
      {
        var node: LexicalNode = try getRoot()
        for index in path {
          node = try (node as! ElementNode).getChildAtIndex(index)!
        }
        let selection = createRangeSelection()
        try selection.anchor.set(node.getKey(), offset, .element)
        try selection.focus.set(node.getKey(), offset, .element)
        try setSelection(selection)
      })
    try editor.update(discrete: true) { try getSelection()!.insertText("Z") }
    return try content(editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret after a quote lifted out of a list item and split around a rule keeps its place in the content"
    ))
  func caretAfterLiftedQuote() throws {
    let result = try typeAtElementCaret(
      {
        [
          try put(
            createListNode("bullet"),
            [
              try put(
                createListItemNode(),
                [
                  try createTextNode("x"),
                  try put(
                    createQuoteNode(),
                    [
                      try put(createParagraphNode(), [try createTextNode("a")]),
                      try createHorizontalRuleNode(), try createTextNode("b"),
                    ]),
                ])
            ])
        ]
      }, [0, 0], 2)
    #expect(result == "xa#bZ")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > an element caret before a list whose first item starts with a rule keeps its place in the content"
    ))
  func caretBeforeListStartingWithRule() throws {
    let result = try typeAtElementCaret(
      {
        [
          try put(
            createListNode("number"),
            [
              try put(
                createListItemNode(), [try createHorizontalRuleNode(), try createTextNode("y")])
            ]
          )
        ]
      }, [], 0)
    #expect(result == "Z#y")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::carets, empty inline elements and dirty order > items a list renumbers are not visited; what changed still is"
    ))
  func renumberedItemsNotVisited() throws {
    let editor = try buildCaretEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createListNode("number").append(contentsOf: ["a", "b", "c", "d"].map(listItem)))
    }
    // A new item renumbers every item after it, which leaves them as they
    // were otherwise; a rule added in the same update still splits the list.
    try editor.update(discrete: true) {
      _ = try getRoot().getAllTextNodes()[0].select(1, 1)
      _ = try (getSelection() as! RangeSelection).insertParagraph()
      try getRoot().getAllTextNodes()[1].insertAfter(createHorizontalRuleNode())
    }
    #expect(try listStarts(editor) == [[1, "a", "", "b"], "horizontalrule", [4, "c", "d"]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > items the seam joins are still normalized: the result is a fixed point"
    ))
  func seamFixedPoint() throws {
    func li(_ children: LexicalNode...) throws -> ListItemNode {
      try put(createListItemNode(), children)
    }
    func ul(_ children: LexicalNode...) throws -> ListNode {
      try put(createListNode("bullet"), children)
    }
    func nested(_ text: String) throws -> ListItemNode {
      try li(put(createListNode("number"), [try li(createTextNode(text))]))
    }
    func doc() throws -> [LexicalNode] {
      [
        try ul(nested("a")),
        try ul(
          li(
            put(
              createListNode("number"),
              [
                try li(
                  createParagraphNode().append(createTextNode("p")),
                  createParagraphNode().append(createTextNode("q")))
              ]))),
      ]
    }
    let loaded = try buildCaretEditor()
    defer { loaded.dispose() }
    try loaded.update(discrete: true) { try put(getRoot().clear(), doc()) }
    let normalized = try buildCaretEditor(false)
    defer { normalized.dispose() }
    try normalized.update(discrete: true, skipTransforms: true) {
      try put(getRoot().clear(), doc())
      try INTERNAL_normalizeNodeStructure(getRoot())
    }
    let expected =
      #"[["list",["listitem",["list",["listitem","a"],["listitem","p","linebreak","q"]]]]]"#
    #expect(try rootShape(loaded) == expected)
    #expect(try rootShape(normalized) == expected)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts::formatting, list seams, orphan list items and the root > a caret next to a list item outside a list keeps its place"
    ))
  func caretNextToOrphanItem() throws {
    func typeAt(_ build: @escaping () throws -> ElementNode, _ offset: Int) throws -> (String, Bool)
    {
      let editor = try buildCaretEditor()
      defer { editor.dispose() }
      try editor.update(discrete: true) {
        let holder = try build()
        try put(getRoot().clear(), [holder])
        _ = try holder.select(offset, offset)
      }
      try editor.update(discrete: true) { try getSelection()!.insertText("Z") }
      return (
        try rootShape(editor),
        try editor.read { try (getSelection() as! RangeSelection).anchor.getNode().isAttached() }
      )
    }
    func a() throws -> LexicalNode { try createTextNode("a") }
    func x() throws -> LexicalNode { try createTextNode("x") }
    // After an empty item: still after "a".
    #expect(
      try typeAt({ try put(createParagraphNode(), [try a(), try createListItemNode()]) }, 2)
        == (#"[["paragraph","aZ"],["list",["listitem"]]]"#, true))
    #expect(
      try typeAt({ try put(createQuoteNode(), [try a(), try createListItemNode()]) }, 2)
        == (#"[["quote","aZ"],["list",["listitem"]]]"#, true))
    // Before an item: after "a"; after it: after its content.
    #expect(
      try typeAt(
        { try put(createParagraphNode(), [try a(), try put(createListItemNode(), [try x()])]) }, 1)
        == (#"[["paragraph","aZ"],["list",["listitem","x"]]]"#, true))
    #expect(
      try typeAt(
        { try put(createParagraphNode(), [try a(), try put(createListItemNode(), [try x()])]) }, 2)
        == (#"[["paragraph","a"],["list",["listitem","x"]],["paragraph","Z"]]"#, true))
    // The only content of its paragraph: no lost selection.
    #expect(
      try typeAt({ try put(createParagraphNode(), [try createListItemNode()]) }, 1)
        == (#"[["list",["listitem"]],["paragraph","Z"]]"#, true))
  }
}
