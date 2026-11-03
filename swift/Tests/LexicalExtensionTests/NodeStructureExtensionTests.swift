import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/NodeStructureExtension.test.ts.
// Upstream's `build` registers the list, link, table, code and overflow
// nodes in every editor; the tests here use none of them, so their editors
// register rich text and the horizontal rule only. The tests that use those
// packages are skipped until they are ported.

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
      dependencies: [RichTextExtension, HorizontalRuleExtension]
        + (withStructure ? [NodeStructureExtension] : []),
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
      dependencies: [RichTextExtension, HorizontalRuleExtension]
        + (withStructure ? [NodeStructureExtension] : []),
      editorConfig: InitialEditorConfig(onError: { error, _ in throw error })))
}

/// Load a document as an update applied with skipTransforms leaves it (not
/// normalized), then mark every node dirty, outer first.
private func loadThenNormalize(
  _ editor: Editor, _ build: @escaping () throws -> [LexicalNode],
  _ select: @escaping () throws -> Void = {}
) throws {
  try editor.update(discrete: true, skipTransforms: true) {
    try getRoot().clear().append(contentsOf: build())
    try select()
  }
  try editor.update(discrete: true) {
    var nodes: [LexicalNode] = []
    func walk(_ node: LexicalNode) throws {
      nodes.append(node)
      if let element = node as? ElementNode {
        try element.getChildren().forEach(walk)
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
