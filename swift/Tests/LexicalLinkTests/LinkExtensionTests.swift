import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LinkExtension.test.ts.

private func linkExtension(
  name: String = "[root]", text: String = "Hello",
  link: any AnyLexicalExtensionArgument = LinkExtension, nodes: [LexicalNodeConfig]? = nil
) -> any AnyLexicalExtensionArgument {
  defineExtension(
    name: name, dependencies: [link, RichTextExtension], nodes: nodes.map { nodes in { nodes } },
    editorConfig: InitialEditorConfig(
      initialEditorState: .update { _ in
        try getRoot().append(createParagraphNode().append(createTextNode(text)))
      }))
}

private func paragraph() throws -> ParagraphNode {
  try #require(
    try getRoot().getFirstChild() as? ParagraphNode, "Expecting a ParagraphNode in the root")
}

@Suite struct LinkExtensionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > can convert a text node to a link with $toggleLink"
    ))
  func toggleLinkConverts() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let textNode = try #require(try getRoot().getLastDescendant() as? TextNode)
      try textNode.select(0)
      #expect(try !(textNode.getParent() is LinkNode))
      try toggleLink("https://lexical.dev/")
      var linkNode = try #require(try textNode.getParent() as? LinkNode, "Expected a LinkNode")
      #expect(try linkNode.getURL() == "https://lexical.dev/")
      #expect(try linkNode.getTarget() == nil)
      #expect(try getRoot().getTextContent() == "Hello")
      try toggleLink(nil)
      #expect(try getRoot().getTextContent() == "Hello")
      #expect(try !(textNode.getParent() is LinkNode))
      try toggleLink(
        "https://lexical.dev/", LinkAttributes(rel: "noopener", target: "_blank", title: "title"))
      linkNode = try #require(try textNode.getParent() as? LinkNode, "Expected a LinkNode")
      #expect(try linkNode.getURL() == "https://lexical.dev/")
      #expect(try linkNode.getTarget() == "_blank")
      #expect(try linkNode.getRel() == "noopener")
      #expect(try linkNode.getTitle() == "title")
    }
  }
}

@Suite struct LinkExtensionMergeTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges adjacent LinkNodes with identical attributes"
    ))
  func mergesIdentical() throws {
    let attributes = LinkAttributes(rel: "noreferrer", target: "_blank", title: "Lexical")
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      try p.append(
        createLinkNode("https://lexical.dev/", attributes).append(createTextNode("Hello ")),
        createLinkNode("https://lexical.dev/", attributes).append(createTextNode("World")))
    }
    try editor.read { () throws -> Void in
      let children = try paragraph().getChildren()
      #expect(children.count == 1)
      let link = try #require(children.first as? LinkNode, "Expected a LinkNode")
      #expect(try link.getURL() == "https://lexical.dev/")
      #expect(try link.getTarget() == "_blank")
      #expect(try link.getRel() == "noreferrer")
      #expect(try link.getTitle() == "Lexical")
      #expect(try link.getTextContent() == "Hello World")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNodes with different URLs"
    ))
  func differentURLs() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      try p.append(
        createLinkNode("https://lexical.dev/").append(createTextNode("Hello ")),
        createLinkNode("https://other.dev/").append(createTextNode("World")))
    }
    #expect(try editor.read { try paragraph().getChildren().count } == 2)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNode with AutoLinkNode"
    ))
  func linkAndAutoLink() throws {
    let editor = try buildEditorFromExtensions(
      linkExtension(name: "[root-autolink]", nodes: [.klass(AutoLinkNode.self)]))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      try p.append(
        createLinkNode("https://lexical.dev/").append(createTextNode("Hello ")),
        createAutoLinkNode("https://lexical.dev/").append(createTextNode("World")))
    }
    #expect(try editor.read { try paragraph().getChildren().count } == 2)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > does not merge LinkNodes with different rel"
    ))
  func differentRel() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      try p.append(
        createLinkNode("https://lexical.dev/", LinkAttributes(rel: "noreferrer")).append(
          createTextNode("Hello ")),
        createLinkNode("https://lexical.dev/", LinkAttributes(rel: "noopener")).append(
          createTextNode("World")))
    }
    #expect(try editor.read { try paragraph().getChildren().count } == 2)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges three adjacent identical links into one"
    ))
  func mergesThree() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      for text in ["A", "B", "C"] {
        try p.append(createLinkNode("https://lexical.dev/").append(createTextNode(text)))
      }
    }
    try editor.read { () throws -> Void in
      let children = try paragraph().getChildren()
      #expect(children.count == 1)
      #expect(children.first is LinkNode)
      #expect(try children.first?.getTextContent() == "ABC")
    }
  }

  /// The merged link's text, with the caret 8 into it.
  private func expectCaretInMergedLink(_ editor: Editor) throws {
    try editor.read { () throws -> Void in
      let children = try paragraph().getChildren()
      #expect(children.count == 1)
      #expect(try children.first?.getTextContent() == "Hello World")
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.offset == 8)
      #expect(try selection.anchor.getNode().getTextContent() == "Hello World")
      #expect(try selection.anchor.getNode().getParent() is LinkNode)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > preserves selection when cursor is in the second link during merge"
    ))
  func selectionInSecondLink() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      let text2 = try createTextNode("World")
      try p.append(
        createLinkNode("https://lexical.dev/").append(createTextNode("Hello ")),
        createLinkNode("https://lexical.dev/").append(text2))
      try text2.select(2, 2)
    }
    try expectCaretInMergedLink(editor)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > merges when only the second link is changed to match the first"
    ))
  func mergesAfterURLChange() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      try p.append(
        createLinkNode("https://lexical.dev/").append(createTextNode("Hello ")),
        createLinkNode("https://other.dev/").append(createTextNode("World")))
    }
    #expect(try editor.read { try paragraph().getChildren().count } == 2)
    try editor.update(discrete: true) {
      let link2 = try #require(try paragraph().getChildren()[1] as? LinkNode, "Expected LinkNode")
      try link2.setURL("https://lexical.dev/")
    }
    try editor.read { () throws -> Void in
      let children = try paragraph().getChildren()
      #expect(children.count == 1)
      #expect(children.first is LinkNode)
      #expect(try children.first?.getTextContent() == "Hello World")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > merge adjacent LinkNodes > preserves selection when cursor is in a link that merges into its left neighbor"
    ))
  func selectionWhenMergingLeft() throws {
    let editor = try buildEditorFromExtensions(linkExtension())
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let p = try paragraph()
      try p.clear()
      let text2 = try createTextNode("World")
      try p.append(
        createLinkNode("https://lexical.dev/").append(createTextNode("Hello ")),
        createLinkNode("https://other.dev/").append(text2))
      try text2.select(2, 2)
    }
    try editor.update(discrete: true) {
      let link2 = try #require(try paragraph().getChildren()[1] as? LinkNode, "Expected LinkNode")
      try link2.setURL("https://lexical.dev/")
    }
    try expectCaretInMergedLink(editor)
  }
}

@Suite struct LinkExtensionPasteTests {
  private let pasteUrl = "https://lexical.dev/"

  private func dispatchPaste(_ editor: Editor) throws {
    let clipboardData = DataTransfer()
    clipboardData.setData("text/plain", pasteUrl)
    clipboardData.setData("text/html", pasteUrl)
    _ = try editor.dispatchCommand(
      PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: clipboardData)))
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > PASTE_COMMAND with URLs > does not convert pasted URL to link inside a code block"
    ))
  func noLinkInCodeBlock() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root-code]",
        dependencies: [
          configExtension(LinkExtension) { $0.validateUrl = { _ in true } }, RichTextExtension,
          CodeExtension,
        ],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let code = try createCodeNode()
            try code.append(createCodeHighlightNode("const x = 5;"))
            try getRoot().append(code)
          })))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let codeHighlight = try #require(
        try getRoot().getFirstDescendant() as? TextNode, "Expected a TextNode")
      try codeHighlight.select(0, 5)
      try dispatchPaste(editor)
    }
    try editor.read { () throws -> Void in
      let code = try #require(try getRoot().getFirstChild() as? CodeNode, "Expected a CodeNode")
      #expect(!(try code.getChildren().contains { $0 is LinkNode }))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > PASTE_COMMAND with URLs > wraps selected plain text in a link when a URL is pasted"
    ))
  func wrapsSelection() throws {
    let pasteUrl = "https://lexical.dev/"
    let editor = try buildEditorFromExtensions(
      linkExtension(
        name: "[root-paste]", text: "click here",
        link: configExtension(LinkExtension) { $0.validateUrl = { _ in true } }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try #require(try getRoot().getFirstDescendant() as? TextNode)
      try text.select(0, text.getTextContentSize())
      let clipboardData = DataTransfer()
      clipboardData.setData("text/plain", pasteUrl)
      clipboardData.setData("text/html", pasteUrl)
      _ = try editor.dispatchCommand(
        PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: clipboardData)))
    }
    try editor.read { () throws -> Void in
      let link = try #require(try paragraph().getFirstChild() as? LinkNode, "Expected a LinkNode")
      #expect(try link.getURL() == pasteUrl)
      #expect(try link.getTextContent() == "click here")
    }
  }
}

@Suite struct LinkExtensionConfiguredAttributesTests {
  private func editor() throws -> Editor {
    try buildEditorFromExtensions(
      linkExtension(
        name: "[root-attributes]",
        link: configExtension(LinkExtension) {
          $0.attributes = LinkAttributes(
            rel: "noopener", target: "_blank", title: "Configured title")
        }))
  }

  private func toggle(_ editor: Editor, _ payload: ToggleLinkPayload) throws {
    try editor.update(discrete: true) {
      let textNode = try #require(try getRoot().getLastDescendant() as? TextNode)
      try textNode.select(0, textNode.getTextContentSize())
      _ = try editor.dispatchCommand(TOGGLE_LINK_COMMAND, payload)
    }
  }

  private func link() throws -> LinkNode {
    try #require(try paragraph().getFirstChild() as? LinkNode, "Expected a LinkNode")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > applies the configured attributes for a string payload"
    ))
  func stringPayload() throws {
    let editor = try editor()
    defer { editor.dispose() }
    try toggle(editor, .url("https://lexical.dev/"))
    try editor.read { () throws -> Void in
      let link = try link()
      #expect(try link.getRel() == "noopener")
      #expect(try link.getTarget() == "_blank")
      #expect(try link.getTitle() == "Configured title")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > applies the configured attributes for an object payload"
    ))
  func objectPayload() throws {
    let editor = try editor()
    defer { editor.dispose() }
    try toggle(editor, .attributes(url: "https://lexical.dev/", LinkAttributes()))
    try editor.read { () throws -> Void in
      let link = try link()
      #expect(try link.getRel() == "noopener")
      #expect(try link.getTarget() == "_blank")
      #expect(try link.getTitle() == "Configured title")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtension.test.ts::Link > TOGGLE_LINK_COMMAND with configured attributes > lets the object payload override the configured attributes"
    ))
  func objectPayloadOverrides() throws {
    let editor = try editor()
    defer { editor.dispose() }
    try toggle(editor, .attributes(url: "https://lexical.dev/", LinkAttributes(target: .some(nil))))
    try editor.read { () throws -> Void in
      let link = try link()
      #expect(try link.getRel() == "noopener")
      #expect(try link.getTarget() == nil)
      #expect(try link.getTitle() == "Configured title")
    }
  }
}
