import Lexical
import LexicalDecoratorBlockNode
import LexicalExtension
import LexicalRichText
import Testing

/// A block embed with one field, as the playground's are.
private final class TestBlockNode: DecoratorBlockNode {
  var __id = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TestBlockNode.self, type: "test_block", extends: DecoratorBlockNode.self,
      json: [SchemaField("id", stringValue(), field: \TestBlockNode.__id)])
  }
}

@MainActor
@Suite struct DecoratorBlockNodeTests {
  private func makeEditor() throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "[test-decorator-block]",
        dependencies: [RichTextExtension, BlockWithAlignableContentsExtension],
        nodes: { [.klass(TestBlockNode.self)] }))
  }

  /// Puts a paragraph and a block in the document; returns the block's key.
  private func insertBlock(_ editor: Editor) throws -> NodeKey {
    var key = ""
    try editor.update(discrete: true) {
      let block = try create(TestBlockNode.self)
      block.__id = "x"
      key = block.getKey()
      try getRoot().clear().append(createParagraphNode().append(createTextNode("a")), block)
    }
    return key
  }

  private func format(_ editor: Editor, _ key: NodeKey) throws -> String {
    try editor.read { try (getNodeByKey(key) as! TestBlockNode).getFormat() }
  }

  /// The subclass's fields come first, then the base's `format`, as the
  /// web playground's `{"videoID":…,"format":"center","type":"youtube",…}`.
  @Test func serializesItsFormatAfterTheSubclassFields() throws {
    let editor = try makeEditor()
    let key = try insertBlock(editor)
    try editor.update(discrete: true) {
      try (getNodeByKey(key) as! TestBlockNode).setFormat("center")
    }
    let json = try editor.read { try getNodeByKey(key)!.exportJSON().stringify() }
    #expect(json == #"{"id":"x","format":"center","type":"test_block","version":1}"#)
    // A format outside the enum parses as the default.
    let state = try editor.parseEditorState(
      #"{"root":{"children":[{"id":"y","format":"middle","type":"test_block","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
    )
    let parsed = try state.read { try (getRoot().getFirstChild() as! TestBlockNode).getFormat() }
    #expect(parsed == "")
  }

  @Test func alignsASelectedBlock() throws {
    let editor = try makeEditor()
    let key = try insertBlock(editor)
    try editor.update(discrete: true) {
      let selection = createNodeSelection()
      selection.add(key)
      try setSelection(selection)
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "right")
    }
    #expect(try format(editor, key) == "right")
    // Under a range over the paragraph only, the block keeps its format and
    // rich text aligns the paragraph.
    try editor.update(discrete: true) {
      _ = try getRoot().getFirstChild()!.selectEnd()
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "center")
    }
    #expect(try format(editor, key) == "right")
    #expect(
      try editor.read { try (getRoot().getFirstChild() as! ParagraphNode).getFormatType() }
        == "center")
    // A range over both aligns both.
    try editor.update(discrete: true) {
      _ = try getRoot().select(0, 2)
      _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, "justify")
    }
    #expect(try format(editor, key) == "justify")
    #expect(
      try editor.read { try (getRoot().getFirstChild() as! ParagraphNode).getFormatType() }
        == "justify")
  }

  @Test func aClickTogglesTheBlocksSelection() throws {
    let editor = try makeEditor()
    let key = try insertBlock(editor)
    func click(shift: Bool = false) throws {
      try editor.update(discrete: true) {
        _ = try editor.dispatchCommand(
          CLICK_COMMAND, MouseEvent("click", shiftKey: shift, targetKey: key))
      }
    }
    func selected() throws -> [NodeKey]? {
      try editor.read {
        try (getSelection() as? NodeSelection).map { try $0.getNodes().map { $0.getKey() } }
      }
    }
    try editor.update(discrete: true) { _ = try getRoot().getFirstChild()!.selectEnd() }
    try click()
    #expect(try selected() == [key])
    try click()
    // An empty node selection commits as none, as in JS.
    #expect(try editor.read { try getSelection() } == nil)
    try click(shift: true)
    #expect(try selected() == [key])
  }
}
