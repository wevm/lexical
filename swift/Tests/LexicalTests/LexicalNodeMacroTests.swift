import Lexical
import Testing

// The `@LexicalNode` macro on real node classes: the config it generates
// must behave like the hand-written `SchemaField` configs.

@LexicalNode(type: "macro_caption", extends: ElementNode.self)
final class MacroCaptionNode: ElementNode {
  @Field(stringValue()) var __caption: String = ""
  @Field("level", numberValue(1)) var __depth: Double = 1
  @Field(try arrayValue(stringValue()), accessors: false) var __tags: [JSONValue] = []
}

@LexicalNode(type: "macro_text", extends: TextNode.self)
final class MacroTextNode: TextNode {
  @Field(booleanValue()) var __flag: Bool = false
}

private func macroEditor() throws -> Editor {
  try createTestEditor(nodes: [.klass(MacroCaptionNode.self), .klass(MacroTextNode.self)])
}

@Suite struct LexicalNodeMacroTests {
  // The expected JSON is what JS writes for the same $config (checked with
  // the upstream sources).
  @Test func exportsFieldsInDeclarationOrder() throws {
    let editor = try macroEditor()
    try update(editor) {
      let node = try MacroCaptionNode()
      try node.setCaption("Hi")
      try node.setDepth(2)
      #expect(try node.getCaption() == "Hi")
      #expect(try node.getDepth() == 2)
      #expect(
        try node.exportJSON().stringify()
          == #"{"children":[],"caption":"Hi","level":2,"tags":[],"direction":null,"format":"","indent":0,"type":"macro_caption","version":1}"#
      )
      // The compact form leaves the defaults (tags, direction, format, indent) out.
      #expect(
        try node.exportJSON(true).stringify()
          == #"{"type":"macro_caption","children":[],"caption":"Hi","level":2}"#)
    }
  }

  @Test func roundTripsAndClones() throws {
    let editor = try macroEditor()
    var key = ""
    try update(editor) {
      let caption = try MacroCaptionNode().setCaption("c")
      let text = try MacroTextNode()
      try text.setTextContent("t")
      try text.setFlag(true)
      try caption.append(text)
      try getRoot().append(caption)
      key = text.getKey()
    }
    // A later update clones the nodes; the fields come with them.
    try update(editor) { try #require(try getNodeByKey(key)).markDirty() }
    let json = try editor.getEditorState().toJSON().stringify()
    #expect(
      json.contains(
        #"{"flag":true,"detail":0,"format":0,"mode":"normal","style":"","text":"t","type":"macro_text","version":1}"#
      ))
    let parsed = try editor.parseEditorState(json)
    #expect(try parsed.toJSON().stringify() == json)
    try parsed.read {
      let caption = try #require(try getRoot().getFirstChild() as? MacroCaptionNode)
      #expect(try caption.getCaption() == "c")
      let text = try #require(try caption.getFirstChild() as? MacroTextNode)
      #expect(try text.getFlag())
    }
  }
}
