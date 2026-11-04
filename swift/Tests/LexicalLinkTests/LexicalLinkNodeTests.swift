import Lexical
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts.
// The test of a link around a @lexical/mark MarkNode is skipped until that
// package is ported.

private var editorConfig: EditorConfig {
  EditorConfig(
    namespace: "",
    theme: [
      "link": "my-link-class",
      "text": [
        "bold": "my-bold-class",
        "code": "my-code-class",
        "hashtag": "my-hashtag-class",
        "italic": "my-italic-class",
        "strikethrough": "my-strikethrough-class",
        "underline": "my-underline-class",
        "underlineStrikethrough": "my-underline-strikethrough-class",
      ],
    ])
}

private func firstParagraph() throws -> ParagraphNode {
  try #require(try getRoot().getFirstChild() as? ParagraphNode)
}

@Suite struct LexicalLinkNodeTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.updateFromJSON restores null for empty or absent attributes"
    ))
  func updateFromJSONRestoresNull() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let link = try createLinkNode(
        "https://example.com/", LinkAttributes(rel: "noopener", target: "_blank", title: "example"))
      // The legacy parser applied `value || null`: an empty string imports
      // as null, and an absent property resets to null.
      _ = try link.updateFromJSON(
        JSONObject([
          ("rel", ""), ("target", ""), ("title", ""), ("url", "https://example.com/"),
        ]))
      #expect(try link.getRel() == nil)
      #expect(try link.getTarget() == nil)
      #expect(try link.getTitle() == nil)
      try link.setRel("noopener")
      _ = try link.updateFromJSON(JSONObject([("url", "https://example.com/")]))
      #expect(try link.getRel() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.constructor"
    ))
  func constructor() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("/")
      #expect(linkNode.__type == "link")
      #expect(linkNode.__url == "/")
    }
    #expect(throws: (any Error).self) { try createLinkNode("") }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.clone()"
    ))
  func clone() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("/")
      let linkNodeClone = try cloneWithProperties(linkNode)
      #expect(linkNodeClone !== linkNode)
      #expect(type(of: linkNodeClone) == type(of: linkNode))
      #expect(linkNodeClone.__key == linkNode.__key)
      #expect(linkNodeClone.__parent == linkNode.__parent)
      #expect(linkNodeClone.__url == linkNode.__url)
      #expect(linkNodeClone.__target == linkNode.__target)
      #expect(linkNodeClone.__rel == linkNode.__rel)
      #expect(linkNodeClone.__title == linkNode.__title)
      #expect(try linkNodeClone.exportJSON() == linkNode.exportJSON())
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getURL()"
    ))
  func getURL() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      #expect(try linkNode.getURL() == "https://example.com/foo")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setURL()"
    ))
  func setURL() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      #expect(try linkNode.getURL() == "https://example.com/foo")
      try linkNode.setURL("https://example.com/bar")
      #expect(try linkNode.getURL() == "https://example.com/bar")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getTarget()"
    ))
  func getTarget() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo", LinkAttributes(target: "_blank"))
      #expect(try linkNode.getTarget() == "_blank")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setTarget()"
    ))
  func setTarget() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo", LinkAttributes(target: "_blank"))
      #expect(try linkNode.getTarget() == "_blank")
      try linkNode.setTarget("_self")
      #expect(try linkNode.getTarget() == "_self")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getRel()"
    ))
  func getRel() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo", LinkAttributes(rel: "noopener noreferrer", target: "_blank"))
      #expect(try linkNode.getRel() == "noopener noreferrer")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setRel()"
    ))
  func setRel() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo", LinkAttributes(rel: "noopener", target: "_blank"))
      #expect(try linkNode.getRel() == "noopener")
      try linkNode.setRel("noopener noreferrer")
      #expect(try linkNode.getRel() == "noopener noreferrer")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.getTitle()"
    ))
  func getTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo", LinkAttributes(title: "Hello world"))
      #expect(try linkNode.getTitle() == "Hello world")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.setTitle()"
    ))
  func setTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo", LinkAttributes(title: "Hello world"))
      #expect(try linkNode.getTitle() == "Hello world")
      try linkNode.setTitle("World hello")
      #expect(try linkNode.getTitle() == "World hello")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" class="my-link-class"></a>"#)
      #expect(
        try linkNode.createDOM(EditorConfig(namespace: "", theme: [:])).outerHTML
          == #"<a href="https://example.com/foo"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() with target, rel and title"
    ))
  func createDOMWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo",
        LinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-link-class"></a>"#
      )
      #expect(
        try linkNode.createDOM(EditorConfig(namespace: "", theme: [:])).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world"></a>"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() sanitizes javascript: URLs"
    ))
  func createDOMSanitizesJavascript() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("javascript:alert(0)")
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="about:blank" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() fails closed on unparseable URLs"
    ))
  func createDOMFailsClosed() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      // A `javascript:` URL crafted so that `new URL()` throws (invalid
      // port). It must be neutralized to about:blank.
      let linkNode = try createLinkNode("javascript://:99999999999/%0aalert(document.domain)")
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="about:blank" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme \"java\\u0000script:alert(document.domain)\""
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme \"java\\tscript:alert(document.domain)\""
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme \"\\u0001javascript:alert(document.domain)\""
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme \"java\\nscript:alert(document.domain)\""
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() neutralizes obfuscated dangerous scheme \"data:text/html,<script>alert(1)</script>\""
    ),
    arguments: [
      // Control-character- and whitespace-obfuscated `javascript:` URLs that
      // throw in `new URL()` but are still navigated by some browsers.
      "java\u{0000}script:alert(document.domain)",
      "java\tscript:alert(document.domain)",
      "\u{0001}javascript:alert(document.domain)",
      "java\nscript:alert(document.domain)",
      "data:text/html,<script>alert(1)</script>",
    ])
  func createDOMNeutralizesObfuscatedScheme(_ input: String) throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(input)
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="about:blank" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > https://example.com -> https://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > https://www.example.com -> https://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > example.com -> https://example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > www.example.com -> https://www.example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > mailto:user@example.com -> mailto:user@example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > user@example.com -> mailto:user@example.com"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.createDOM() formats URLs > +1234567890 -> tel:+1234567890"
    ),
    arguments: [
      ("https://example.com", "https://example.com"),
      ("https://www.example.com", "https://www.example.com"),
      ("example.com", "https://example.com"),
      ("www.example.com", "https://www.example.com"),
      ("mailto:user@example.com", "mailto:user@example.com"),
      ("user@example.com", "mailto:user@example.com"),
      ("+1234567890", "tel:+1234567890"),
    ])
  func createDOMFormatsURLs(_ input: String, _ output: String) throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(input)
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="\#(output)" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.updateDOM()"
    ))
  func updateDOM() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      let domElement = try linkNode.createDOM(editorConfig)
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" class="my-link-class"></a>"#)
      let newLinkNode = try createLinkNode("https://example.com/bar")
      let result = try newLinkNode.updateDOM(linkNode, domElement, editorConfig)
      #expect(result == false)
      #expect(
        domElement.outerHTML == #"<a href="https://example.com/bar" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.updateDOM() with target, rel and title"
    ))
  func updateDOMWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo",
        LinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))
      let domElement = try linkNode.createDOM(editorConfig)
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-link-class"></a>"#
      )
      let newLinkNode = try createLinkNode(
        "https://example.com/bar",
        LinkAttributes(rel: "noopener", target: "_self", title: "World hello"))
      let result = try newLinkNode.updateDOM(linkNode, domElement, editorConfig)
      #expect(result == false)
      #expect(
        domElement.outerHTML
          == #"<a href="https://example.com/bar" target="_self" rel="noopener" title="World hello" class="my-link-class"></a>"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.updateDOM() with undefined target, undefined rel and undefined title"
    ))
  func updateDOMWithUndefinedAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode(
        "https://example.com/foo",
        LinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))
      let domElement = try linkNode.createDOM(editorConfig)
      #expect(
        try linkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-link-class"></a>"#
      )
      let newLinkNode = try createLinkNode("https://example.com/bar")
      let result = try newLinkNode.updateDOM(linkNode, domElement, editorConfig)
      #expect(result == false)
      #expect(
        domElement.outerHTML == #"<a href="https://example.com/bar" class="my-link-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.canInsertTextBefore()"
    ))
  func canInsertTextBefore() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      #expect(try linkNode.canInsertTextBefore() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > LinkNode.canInsertTextAfter()"
    ))
  func canInsertTextAfter() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      #expect(try linkNode.canInsertTextAfter() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $createLinkNode()"
    ))
  func createLinkNodeFactory() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("https://example.com/foo")
      let createdLinkNode = try createLinkNode("https://example.com/foo")
      #expect(linkNode.__type == createdLinkNode.__type)
      #expect(linkNode.__parent == createdLinkNode.__parent)
      #expect(linkNode.__url == createdLinkNode.__url)
      #expect(linkNode.__key != createdLinkNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $createLinkNode() with target, rel and title"
    ))
  func createLinkNodeWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let attributes = LinkAttributes(
        rel: "noopener noreferrer", target: "_blank", title: "Hello world")
      let linkNode = try createLinkNode("https://example.com/foo", attributes)
      let createdLinkNode = try createLinkNode("https://example.com/foo", attributes)
      #expect(linkNode.__type == createdLinkNode.__type)
      #expect(linkNode.__parent == createdLinkNode.__parent)
      #expect(linkNode.__url == createdLinkNode.__url)
      #expect(linkNode.__target == createdLinkNode.__target)
      #expect(linkNode.__rel == createdLinkNode.__rel)
      #expect(linkNode.__title == createdLinkNode.__title)
      #expect(linkNode.__key != createdLinkNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $isLinkNode()"
    ))
  func isLinkNodeGuard() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let linkNode = try createLinkNode("")
      #expect(isLinkNode(linkNode))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink applies the title attribute when creating"
    ))
  func toggleLinkAppliesTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let p = try createParagraphNode()
      let textNode = try createTextNode("Some text")
      try p.append(textNode)
      try getRoot().append(p)
      try selectAll()
      try toggleLink("https://lexical.dev/", LinkAttributes(title: "Lexical Website"))
      let linkNode = try #require(try p.getFirstChild() as? LinkNode)
      #expect(isLinkNode(linkNode))
      #expect(try linkNode.getTitle() == "Lexical Website")
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.key == textNode.getKey())
      #expect(selection.anchor.offset == 0)
      #expect(selection.anchor.type == .text)
      #expect(selection.focus.key == textNode.getKey())
      #expect(try selection.focus.offset == textNode.getTextContentSize())
      #expect(selection.focus.type == .text)
    }
    let paragraph = try editor.getEditorState().toJSON()["root"]?.objectValue?["children"]?
      .arrayValue?[0].objectValue
    let link = paragraph?["children"]?.arrayValue?[0].objectValue
    #expect(link?["title"] == .string("Lexical Website"))
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink correctly removes link when link contains heading"
    ))
  func toggleLinkRemovesLinkAroundHeading() throws {
    // This tests the structure: link > heading > text
    // (HeadingNode is one of upstream's default test nodes.)
    let editor = try createLinkTestEditor(nodes: [.klass(HeadingNode.self)])
    try update(editor) {
      let paragraph = try createParagraphNode()
      let linkNode = try createLinkNode("https://example.com/foo")
      let headingNode = try createHeadingNode("h3")
      let textNode = try createTextNode("Example Link")
      try headingNode.append(textNode)
      try linkNode.append(headingNode)
      try paragraph.append(linkNode)
      try getRoot().append(paragraph)
    }

    // Verify initial structure: paragraph > link > heading > text
    try editor.read { () throws -> Void in
      let linkNode = try firstParagraph().getFirstChild()
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getURL() == "https://example.com/foo")
        let headingNode = try linkNode.getFirstChild()
        #expect(headingNode?.getType() == "heading")
        #expect(try headingNode?.getTextContent() == "Example Link")
      }
    }

    // Select all and remove link
    try update(editor) {
      try selectAll()
      try toggleLink(nil)
    }

    // Verify structure after link removal: paragraph > heading > text
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      // Link should be removed, heading should be moved up to paragraph level
      #expect(children.count == 1)
      let headingNode = try #require(children.first)
      #expect(headingNode.getType() == "heading")
      #expect(try headingNode.getTextContent() == "Example Link")
      // Verify no link nodes remain
      #expect(!isLinkNode(headingNode))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink adds link with embedded LineBreakNode"
    ))
  func toggleLinkWithEmbeddedLineBreak() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let paragraph = try createParagraphNode()
      let precedingText = try createTextNode("some ")  // space after
      let textNode = try createTextNode("text")
      try paragraph.append(precedingText, textNode, createLineBreakNode())
      try getRoot().clear().append(paragraph)
      try paragraph.select(1)
      try toggleLink("https://example.com/foo", LinkAttributes(rel: "noreferrer"))
    }

    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      try #require(children.count >= 2)
      let precedingText = children[0]
      let linkNode = children[1]

      // Check first text node
      #expect(try precedingText.getTextContent() == "some ")

      // Check link node and its nested structure
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getURL() == "https://example.com/foo")
        #expect(try linkNode.getRel() == "noreferrer")
        #expect(try linkNode.getChildren().map { try $0.getTextContent() } == ["text", "\n"])
        let selection = try #require(try getSelection() as? RangeSelection)
        #expect(try selection.anchor.key == linkNode.getFirstChildOrThrow().getKey())
        #expect(selection.anchor.offset == 0)
        #expect(selection.anchor.type == .text)
        #expect(selection.focus.key == linkNode.getKey())
        #expect(selection.focus.offset == 2)
        #expect(selection.focus.type == .element)
      }
    }

    try update(editor) {
      try selectAll()
      try toggleLink(nil)
    }

    // Verify structure after link removal
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      #expect(try children.map { try $0.getTextContent() } == ["some text", "\n"])
      try #require(children.count == 2)
      #expect(isTextNode(children[0]))
      #expect(isLineBreakNode(children[1]))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from trailing space only"
    ))
  func toggleLinkRemovesTrailingSpace() throws {
    let editor = try createLinkTestEditor()
    var textNodeKey: NodeKey = ""

    // Create a link with text and a trailing space
    try update(editor) {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode("hello ")
      textNodeKey = textNode.getKey()
      let linkNode = try createLinkNode("https://example.com")
      try linkNode.append(textNode)
      try paragraph.append(linkNode)
      try getRoot().clear().append(paragraph)
    }

    // Verify initial structure
    try editor.read { () throws -> Void in
      let linkNode = try firstParagraph().getFirstChild()
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getTextContent() == "hello ")
      }
    }

    // Select only the trailing space and remove the link from it
    try update(editor) {
      if let textNode = try getNodeByKey(textNodeKey) as? TextNode {
        try textNode.select(5, 6)  // Select the trailing space
        try toggleLink(nil)
      }
    }

    // Verify that the link was removed only from the space
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      // Should have two children: linkNode with 'hello' and a text node with ' '
      try #require(children.count == 2)
      let linkNode = children[0]
      let spaceNode = children[1]

      // First child should still be a link containing 'hello'
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getTextContent() == "hello")
        #expect(try linkNode.getURL() == "https://example.com")
      }

      // Second child should be a text node with just the space
      #expect(isTextNode(spaceNode))
      if let spaceNode = spaceNode as? TextNode {
        #expect(try spaceNode.getTextContent() == " ")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from leading space only"
    ))
  func toggleLinkRemovesLeadingSpace() throws {
    let editor = try createLinkTestEditor()
    var textNodeKey: NodeKey = ""

    // Create a link with a leading space and text
    try update(editor) {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode(" hello")
      textNodeKey = textNode.getKey()
      let linkNode = try createLinkNode("https://example.com")
      try linkNode.append(textNode)
      try paragraph.append(linkNode)
      try getRoot().clear().append(paragraph)
    }

    // Verify initial structure
    try editor.read { () throws -> Void in
      let linkNode = try firstParagraph().getFirstChild()
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getTextContent() == " hello")
      }
    }

    // Select only the leading space and remove the link from it
    try update(editor) {
      if let textNode = try getNodeByKey(textNodeKey) as? TextNode {
        try textNode.select(0, 1)  // Select the leading space
        try toggleLink(nil)
      }
    }

    // Verify that the link was removed only from the space
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      // Should have two children: a text node with ' ' and linkNode with 'hello'
      try #require(children.count == 2)
      let spaceNode = children[0]
      let linkNode = children[1]

      // First child should be a text node with just the space
      #expect(isTextNode(spaceNode))
      if let spaceNode = spaceNode as? TextNode {
        #expect(try spaceNode.getTextContent() == " ")
      }

      // Second child should still be a link containing 'hello'
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getTextContent() == "hello")
        #expect(try linkNode.getURL() == "https://example.com")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link from middle word only, preserving surrounding spaces"
    ))
  func toggleLinkRemovesMiddleWord() throws {
    let editor = try createLinkTestEditor()
    var textNodeKey: NodeKey = ""

    // Create a link with leading space, text, and trailing space
    try update(editor) {
      let paragraph = try createParagraphNode()
      let textNode = try createTextNode(" hello ")
      textNodeKey = textNode.getKey()
      let linkNode = try createLinkNode("https://example.com")
      try linkNode.append(textNode)
      try paragraph.append(linkNode)
      try getRoot().clear().append(paragraph)
    }

    // Verify initial structure
    try editor.read { () throws -> Void in
      let linkNode = try firstParagraph().getFirstChild()
      #expect(isLinkNode(linkNode))
      if let linkNode = linkNode as? LinkNode {
        #expect(try linkNode.getTextContent() == " hello ")
      }
    }

    // Select only 'hello' (without spaces) and remove the link from it
    try update(editor) {
      if let textNode = try getNodeByKey(textNodeKey) as? TextNode {
        try textNode.select(1, 6)  // Select 'hello'
        try toggleLink(nil)
      }
    }

    // Verify that the link was removed only from 'hello'
    try editor.read { () throws -> Void in
      let children = try firstParagraph().getChildren()
      // Should have three children: link with ' ', text with 'hello', link with ' '
      try #require(children.count == 3)
      let leadingSpaceLink = children[0]
      let middleText = children[1]
      let trailingSpaceLink = children[2]

      // First child should be a link with leading space
      #expect(isLinkNode(leadingSpaceLink))
      if let leadingSpaceLink = leadingSpaceLink as? LinkNode {
        #expect(try leadingSpaceLink.getTextContent() == " ")
        #expect(try leadingSpaceLink.getURL() == "https://example.com")
      }

      // Middle child should be plain text 'hello'
      #expect(isTextNode(middleText))
      if let middleText = middleText as? TextNode {
        #expect(try middleText.getTextContent() == "hello")
      }

      // Third child should be a link with trailing space
      #expect(isLinkNode(trailingSpaceLink))
      if let trailingSpaceLink = trailingSpaceLink as? LinkNode {
        #expect(try trailingSpaceLink.getTextContent() == " ")
        #expect(try trailingSpaceLink.getURL() == "https://example.com")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LexicalLinkNode tests > $toggleLink removes link when selection is collapsed"
    ))
  func toggleLinkRemovesLinkWhenCollapsed() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let p = try createParagraphNode()
      let textNode = try createTextNode("textboldtext")
      try p.append(textNode)
      try getRoot().append(p)
      try selectAll()
      try toggleLink("https://lexical.dev/", LinkAttributes(title: "Lexical Website"))

      let linkNode = try #require(try p.getFirstChild() as? LinkNode)
      try textNode.select(4, 8).formatText("bold")
      let sel = createRangeSelection()
      let key = try #require(try linkNode.getChildAtIndex(1)).getKey()
      try sel.anchor.set(key, 4, .text)
      try sel.focus.set(key, 4, .text)
      try setSelection(sel)
      try toggleLink(nil)

      let children = try p.getChildren()
      #expect(children.count == 3)
      for child in children {
        #expect(isTextNode(child))
      }
    }
  }
}

@Suite struct FormatUrlTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::formatUrl > should not modify URLs with protocols"
    ))
  func protocols() {
    #expect(formatUrl("https://example.com") == "https://example.com")
    #expect(formatUrl("http://example.com") == "http://example.com")
    #expect(formatUrl("mailto:user@example.com") == "mailto:user@example.com")
    #expect(formatUrl("tel:+1234567890") == "tel:+1234567890")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::formatUrl > should not modify relative paths"
    ))
  func relativePaths() {
    #expect(formatUrl("/path/to/resource") == "/path/to/resource")
    #expect(formatUrl("/index.html") == "/index.html")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::formatUrl > should add mailto: to email addresses"
    ))
  func emailAddresses() {
    #expect(formatUrl("user@example.com") == "mailto:user@example.com")
    #expect(formatUrl("name.lastname@domain.co.uk") == "mailto:name.lastname@domain.co.uk")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::formatUrl > should add tel: to phone numbers"
    ))
  func phoneNumbers() {
    #expect(formatUrl("+1234567890") == "tel:+1234567890")
    #expect(formatUrl("123-456-7890") == "tel:123-456-7890")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::formatUrl > should add https:// to URLs without protocols"
    ))
  func withoutProtocols() {
    #expect(formatUrl("www.example.com") == "https://www.example.com")
    #expect(formatUrl("example.com") == "https://example.com")
    #expect(formatUrl("subdomain.example.com/path") == "https://subdomain.example.com/path")
  }
}

// MARK: LinkNode transform (Regression #8083)

private let transformExtension = defineExtension(
  name: "[test-link-transform]", dependencies: [LinkExtension, RichTextExtension])

@Suite struct LinkNodeTransformTests {
  /// The positions of #8305: visually equivalent cursor positions at the
  /// boundary between two adjacent identical links.
  enum MergeBoundary: CaseIterable, Sendable {
    case textPointAtEndOfFirstLinkText
    case textPointAtStartOfSecondLinkText
    case elementPointAtLastChildOfFirstLink
    case elementPointBetweenTheTwoLinks
    case elementPointAtFirstChildOfSecondLink
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > extracts block child (HeadingNode) from link"
    ))
  func extractsHeading() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var textKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let text = try createTextNode("Lexical")
      textKey = text.getKey()
      let heading = try createHeadingNode("h1")
      try heading.append(text)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try text.select(3, 3)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      let heading = try #require(
        children.first as? HeadingNode, "First child must be a HeadingNode")
      #expect(try heading.getChildrenSize() == 1)
      let headingChild = try #require(
        try heading.getFirstChild() as? LinkNode, "Heading child must be a LinkNode")
      #expect(try headingChild.getTextContent() == "Lexical")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == textKey)
      #expect(selection.anchor.offset == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > extracts block child (ParagraphNode) from link"
    ))
  func extractsParagraph() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var textKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let text = try createTextNode("Lexical")
      textKey = text.getKey()
      let innerParagraph = try createParagraphNode()
      try innerParagraph.append(text)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(innerParagraph)
      let outerParagraph = try createParagraphNode()
      try outerParagraph.append(link)
      try root.clear().append(outerParagraph)
      try text.select(0, 7)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      let para = try #require(
        children.first as? ParagraphNode, "First child must be a ParagraphNode")
      #expect(try para.getChildrenSize() == 1)
      let paraChild = try #require(
        try para.getFirstChild() as? LinkNode, "Paragraph child must be a LinkNode")
      #expect(try paraChild.getTextContent() == "Lexical")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == textKey)
      #expect(selection.anchor.offset == 0)
      #expect(selection.focus.type == .text)
      #expect(selection.focus.key == textKey)
      #expect(selection.focus.offset == 7)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > handles siblings after block child"
    ))
  func siblingsAfterBlockChild() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var afterTextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let headingText = try createTextNode("Heading")
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let afterText = try createTextNode(" after")
      afterTextKey = afterText.getKey()
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading, afterText)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try afterText.select(2, 5)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let heading = try #require(children[0] as? HeadingNode, "First child must be a HeadingNode")
      let headingLink = try #require(
        try heading.getFirstChild() as? LinkNode, "Heading child must be a LinkNode")
      #expect(try headingLink.getTextContent() == "Heading")
      let trailingPara = try #require(
        children[1] as? ParagraphNode, "Second child must be a ParagraphNode")
      let trailingLink = try #require(
        try trailingPara.getFirstChild() as? LinkNode,
        "Trailing paragraph child must be a LinkNode")
      #expect(try trailingLink.getTextContent() == " after")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == afterTextKey)
      #expect(selection.anchor.offset == 2)
      #expect(selection.focus.type == .text)
      #expect(selection.focus.key == afterTextKey)
      #expect(selection.focus.offset == 5)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection in paragraph split to the right of LinkNode"
    ))
  func elementSelectionInSplitParagraph() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var afterTextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let textBefore = try createTextNode("before")
      let headingText = try createTextNode("Heading")
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading)
      let textAfter = try createTextNode("after")
      afterTextKey = textAfter.getKey()
      let paragraph = try createParagraphNode()
      try paragraph.append(textBefore, link, textAfter)
      try root.clear().append(paragraph)
      try paragraph.select(2, 2)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 3)
      #expect(try children[2].getTextContent() == "after")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == afterTextKey)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection in LinkNode to the right of non-inline node"
    ))
  func elementSelectionRightOfNonInline() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var afterTextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let headingText = try createTextNode("Heading")
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let afterText = try createTextNode(" after")
      afterTextKey = afterText.getKey()
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading, afterText)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try link.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let trailingPara = try #require(
        children[1] as? ParagraphNode, "Second child must be a ParagraphNode")
      _ = try #require(
        try trailingPara.getFirstChild() as? LinkNode,
        "Trailing paragraph child must be a LinkNode")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == afterTextKey)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection with multiple non-inline siblings"
    ))
  func elementSelectionMultipleNonInline() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var heading2TextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let heading1Text = try createTextNode("First")
      let heading1 = try createHeadingNode("h1")
      try heading1.append(heading1Text)
      let heading2Text = try createTextNode("Second")
      heading2TextKey = heading2Text.getKey()
      let heading2 = try createHeadingNode("h2")
      try heading2.append(heading2Text)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading1, heading2)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try link.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      try #require(children.count == 2)
      let first = try #require(children[0] as? HeadingNode, "First child must be a HeadingNode")
      #expect(try first.getTextContent() == "First")
      let second = try #require(children[1] as? HeadingNode, "Second child must be a HeadingNode")
      #expect(try second.getTextContent() == "Second")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == heading2TextKey)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > fixes element selection when no siblings to the right of LinkNode"
    ))
  func elementSelectionNoSiblings() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let root = try getRoot()
      let headingText = try createTextNode("Heading")
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try paragraph.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      let heading = try #require(
        children.first as? HeadingNode, "First child must be a HeadingNode")
      let headingChild = try #require(
        try heading.getFirstChild() as? LinkNode, "Heading child must be a LinkNode")
      #expect(try headingChild.getTextContent() == "Heading")
      _ = try #require(try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > selection in paragraph right of link with trailing text in link"
    ))
  func selectionRightOfLinkWithTrailingText() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var afterTextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let before = try createTextNode("before")
      let headingText = try createTextNode("Heading")
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let trailingText = try createTextNode(" trailing")
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading, trailingText)
      let after = try createTextNode("after")
      afterTextKey = after.getKey()
      let paragraph = try createParagraphNode()
      try paragraph.append(before, link, after)
      try root.clear().append(paragraph)
      try paragraph.select(2, 2)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == afterTextKey)
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > selection at end of link with heading only child"
    ))
  func selectionAtEndOfLinkWithHeadingOnly() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var headingTextKey: NodeKey = ""
    try editor.update(discrete: true) {
      let root = try getRoot()
      let headingText = try createTextNode("Heading")
      headingTextKey = headingText.getKey()
      let heading = try createHeadingNode("h1")
      try heading.append(headingText)
      let link = try createLinkNode("https://lexical.dev")
      try link.append(heading)
      let paragraph = try createParagraphNode()
      try paragraph.append(link)
      try root.clear().append(paragraph)
      try link.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 1)
      _ = try #require(children.first as? HeadingNode, "First child must be a HeadingNode")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Selection must be a RangeSelection")
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == headingTextKey)
      #expect(selection.anchor.offset == 7)
    }
  }

  // Regression #8305: All five visually-equivalent cursor positions at the
  // boundary between two adjacent identical links must resolve to the merge
  // boundary (textA at offset 5) after the links merge.
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: text point at end of first link text"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: text point at start of second link text"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point at last child of first link"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point between the two links"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 cursor at merge boundary: element point at first child of second link"
    ), arguments: MergeBoundary.allCases)
  func cursorAtMergeBoundary(_ boundary: MergeBoundary) throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var textAKey: NodeKey = ""
    try editor.update(discrete: true) {
      let textA = try createTextNode("link1")
      textAKey = textA.getKey()
      let textB = try createTextNode("link2")
      let linkA = try createLinkNode("https://lexical.dev").append(textA)
      let linkB = try createLinkNode("https://lexical.dev").append(textB)
      let paragraph = try createParagraphNode().append(linkA, linkB)
      try getRoot().clear().append(paragraph)
      switch boundary {
      case .textPointAtEndOfFirstLinkText: try textA.select(5, 5)
      case .textPointAtStartOfSecondLinkText: try textB.select(0, 0)
      case .elementPointAtLastChildOfFirstLink: try linkA.select(1, 1)
      case .elementPointBetweenTheTwoLinks: try paragraph.select(1, 1)
      case .elementPointAtFirstChildOfSecondLink: try linkB.select(0, 0)
      }
    }
    try editor.read { () throws -> Void in
      let paragraph = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "Expected ParagraphNode")
      #expect(try paragraph.getChildrenSize() == 1)
      let mergedLink = try #require(try paragraph.getFirstChild() as? LinkNode, "Expected LinkNode")
      #expect(try mergedLink.getTextContent() == "link1link2")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Expected RangeSelection")
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == textAKey)
      #expect(selection.anchor.offset == 5)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 edge case: second link starts with LineBreakNode"
    ))
  func secondLinkStartsWithLineBreak() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    var textAKey: NodeKey = ""
    try editor.update(discrete: true) {
      let textA = try createTextNode("link1")
      textAKey = textA.getKey()
      let paragraph = try createParagraphNode().append(
        createLinkNode("https://lexical.dev").append(textA),
        createLinkNode("https://lexical.dev").append(
          createLineBreakNode(), createTextNode("link2")))
      try getRoot().clear().append(paragraph)
      try paragraph.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let paragraph = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "Expected ParagraphNode")
      #expect(try paragraph.getChildrenSize() == 1)
      _ = try #require(try paragraph.getFirstChild() as? LinkNode, "Expected LinkNode")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Expected RangeSelection")
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.key == textAKey)
      #expect(selection.anchor.offset == 5)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode transform (Regression #8083) > #8305 edge case: first link ends with LineBreakNode"
    ))
  func firstLinkEndsWithLineBreak() throws {
    let editor = try buildEditorFromExtensions(transformExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode().append(
        createLinkNode("https://lexical.dev").append(
          createTextNode("link1"), createLineBreakNode()),
        createLinkNode("https://lexical.dev").append(createTextNode("link2")))
      try getRoot().clear().append(paragraph)
      try paragraph.select(1, 1)
    }
    try editor.read { () throws -> Void in
      let paragraph = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "Expected ParagraphNode")
      #expect(try paragraph.getChildrenSize() == 1)
      _ = try #require(try paragraph.getFirstChild() as? LinkNode, "Expected LinkNode")
      let selection = try #require(
        try getSelection() as? RangeSelection, "Expected RangeSelection")
      #expect(selection.isCollapsed())
      #expect(selection.anchor.type == .text)
      #expect(selection.anchor.offset == 0)
    }
  }
}

// MARK: LinkNode.extractWithChild (Issue #5046)

private func setupLinkInParagraph() throws -> (linkNode: LinkNode, textNode: TextNode) {
  let root = try getRoot()
  let paragraph = try createParagraphNode()
  let linkNode = try createLinkNode("https://example.com")
  let textNode = try createTextNode("hello link")
  try linkNode.append(textNode)
  try paragraph.append(linkNode)
  try root.append(paragraph)
  return (linkNode, textNode)
}

@Suite struct LinkNodeExtractWithChildTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Chrome-like — anchor/focus both on inner TextNode"
    ))
  func chromeLike() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let (linkNode, textNode) = try setupLinkInParagraph()
      let selection = try textNode.select(0, 10)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone"))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Firefox-like — anchor/focus both on LinkNode itself (element point)"
    ))
  func firefoxLike() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let (linkNode, textNode) = try setupLinkInParagraph()
      // 0,1 (asymmetric offsets) keeps select() in the element-point
      // branch; 0,0 would redirect into the first/last child because
      // LinkNode.canBeEmpty() is false.
      let selection = try linkNode.select(0, 1)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone"))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > Safari-like — asymmetric (anchor on LinkNode, focus on inner TextNode)"
    ))
  func safariLike() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let (linkNode, textNode) = try setupLinkInParagraph()
      let selection = try linkNode.select(0, 1)
      try selection.focus.set(textNode.getKey(), 10, .text)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone"))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > selection entirely outside the link returns false"
    ))
  func selectionOutsideLink() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let linkNode = try createLinkNode("https://example.com")
      let textNode = try createTextNode("hello link")
      try linkNode.append(textNode)
      let outsideText = try createTextNode("outside")
      try paragraph.append(linkNode)
      try paragraph.append(outsideText)
      try root.append(paragraph)
      let selection = try outsideText.select(0, 7)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone") == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > collapsed (zero-length) selection inside the link returns false"
    ))
  func collapsedSelectionInsideLink() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let (linkNode, textNode) = try setupLinkInParagraph()
      let selection = try textNode.select(5, 5)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone") == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > AutoLinkNode (LinkNode subclass) inherits the Firefox-like fix"
    ))
  func autoLinkInheritsFix() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let autoLink = try createAutoLinkNode("https://example.com")
      let textNode = try createTextNode("https://example.com")
      try autoLink.append(textNode)
      try paragraph.append(autoLink)
      try root.append(paragraph)
      let selection = try autoLink.select(0, 1)
      #expect(try autoLink.extractWithChild(textNode, selection, "clone"))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > partial text selection inside link with Firefox-like element anchor still preserves wrap"
    ))
  func partialTextSelectionWithElementAnchor() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let (linkNode, textNode) = try setupLinkInParagraph()
      // anchor on LinkNode (element), focus partway through inner text
      let selection = try linkNode.select(0, 1)
      try selection.focus.set(textNode.getKey(), 4, .text)
      #expect(try linkNode.extractWithChild(textNode, selection, "clone"))
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::LinkNode.extractWithChild (Issue #5046 — preserve link wrap across browsers) > selecting one of two adjacent links does not pull in the other"
    ))
  func adjacentLinks() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) {
      let root = try getRoot()
      let paragraph = try createParagraphNode()
      let link1 = try createLinkNode("https://first.example")
      let text1 = try createTextNode("first")
      try link1.append(text1)
      let between = try createTextNode(" and ")
      let link2 = try createLinkNode("https://second.example")
      let text2 = try createTextNode("second")
      try link2.append(text2)
      try paragraph.append(link1)
      try paragraph.append(between)
      try paragraph.append(link2)
      try root.append(paragraph)
      // Firefox-like selection that wraps link1 entirely
      let selection = try link1.select(0, 1)
      // link1 includes its own child, link2 does not
      #expect(try link1.extractWithChild(text1, selection, "clone"))
      #expect(try link2.extractWithChild(text2, selection, "clone") == false)
    }
  }
}

// MARK: $toggleLink with a NodeSelection

private let nodeSelectionExtension = defineExtension(
  name: "[root-node-selection]", dependencies: [LinkExtension, RichTextExtension],
  editorConfig: InitialEditorConfig(
    initialEditorState: .update { _ in
      try getRoot().clear().append(createParagraphNode().append(createTextNode("hello")))
    }))

private func selectTheTextNode() throws {
  let textNode = try #require(try getRoot().getLastDescendant() as? TextNode, "Expected a TextNode")
  let selection = createNodeSelection()
  selection.add(textNode.getKey())
  try setSelection(selection)
}

private func getLink() throws -> LinkNode {
  let paragraph = try #require(
    try getRoot().getFirstChild() as? ParagraphNode, "Expected a ParagraphNode")
  return try #require(try paragraph.getFirstChild() as? LinkNode, "Expected a LinkNode")
}

@Suite struct ToggleLinkNodeSelectionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > applies the title when creating a link"
    ))
  func appliesTitleWhenCreating() throws {
    let editor = try buildEditorFromExtensions(nodeSelectionExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try selectTheTextNode()
      try toggleLink(
        "https://lexical.dev/", LinkAttributes(rel: "noopener", target: "_blank", title: "Lexical"))
    }
    try editor.read { () throws -> Void in
      let link = try getLink()
      #expect(try link.getURL() == "https://lexical.dev/")
      #expect(try link.getRel() == "noopener")
      #expect(try link.getTarget() == "_blank")
      #expect(try link.getTitle() == "Lexical")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > applies the title when updating an existing link"
    ))
  func appliesTitleWhenUpdating() throws {
    let editor = try buildEditorFromExtensions(nodeSelectionExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try selectTheTextNode()
      try toggleLink("https://lexical.dev/", LinkAttributes(title: "First"))
    }
    try editor.update(discrete: true) {
      try selectTheTextNode()
      try toggleLink("https://lexical.dev/docs", LinkAttributes(title: "Second"))
    }
    try editor.read { () throws -> Void in
      let link = try getLink()
      #expect(try link.getURL() == "https://lexical.dev/docs")
      #expect(try link.getTitle() == "Second")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink with a NodeSelection > leaves an existing title alone when none is supplied"
    ))
  func leavesExistingTitle() throws {
    let editor = try buildEditorFromExtensions(nodeSelectionExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try selectTheTextNode()
      try toggleLink("https://lexical.dev/", LinkAttributes(title: "Kept"))
    }
    try editor.update(discrete: true) {
      try selectTheTextNode()
      try toggleLink("https://lexical.dev/docs")
    }
    try editor.read { () throws -> Void in
      let link = try getLink()
      #expect(try link.getURL() == "https://lexical.dev/docs")
      #expect(try link.getTitle() == "Kept")
    }
  }
}

// MARK: $toggleLink(null) with a selection touching a link boundary

private let unlinkBoundaryExtension = defineExtension(
  name: "[test-unlink-link-boundary]", dependencies: [LinkExtension])

private func buildParagraph(_ layout: [ToggleLinkNullLinkBoundaryTests.LayoutPart]) throws {
  let paragraph = try createParagraphNode()
  for (i, part) in layout.enumerated() {
    switch part {
    case .text(let text):
      try paragraph.append(createTextNode(text))
    case .link(let texts):
      let link = try createLinkNode("https://example.com/\(i)")
      // Alternate formats so adjacent text nodes are not merged
      for (j, text) in texts.enumerated() {
        let textNode = try createTextNode(text)
        try link.append(j % 2 == 1 ? textNode.setFormat("bold") : textNode.setFormat(0))
      }
      try paragraph.append(link)
    }
  }
  try getRoot().clear().append(paragraph)
}

private func getTextNode(_ text: String) throws -> TextNode {
  try #require(
    try getRoot().getAllTextNodes().first { try $0.getTextContent() == text },
    "Expected a TextNode with text \(text)")
}

private func describeParagraph() throws -> [ToggleLinkNullLinkBoundaryTests.LayoutPart] {
  let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ParagraphNode)
  return try paragraph.getChildren().map { child in
    if let link = child as? LinkNode {
      return .link(try link.getChildren().map { try $0.getTextContent() })
    }
    return .text(try child.getTextContent())
  }
}

@Suite struct ToggleLinkNullLinkBoundaryTests {
  /// One entry of upstream's `Layout`: a text node, or a link with the texts
  /// of its children.
  enum LayoutPart: Equatable, Sendable {
    case text(String)
    case link([String])
  }

  struct UnlinkBoundaryCase: Sendable, CustomTestStringConvertible {
    let name: String
    let layout: [LayoutPart]
    let anchor: (String, Int)
    let focus: (String, Int)
    let expected: [LayoutPart]
    let selectedText: String
    var testDescription: String { name }
  }

  static let cases: [UnlinkBoundaryCase] = [
    UnlinkBoundaryCase(
      name: "forward selection ending at offset 0 of the link text",
      layout: [.text("ab"), .link(["cd"]), .text("ef")], anchor: ("ab", 1), focus: ("cd", 0),
      expected: [.text("ab"), .link(["cd"]), .text("ef")], selectedText: "b"),
    UnlinkBoundaryCase(
      name: "backward selection ending at offset 0 of the link text",
      layout: [.text("ab"), .link(["cd"]), .text("ef")], anchor: ("cd", 0), focus: ("ab", 1),
      expected: [.text("ab"), .link(["cd"]), .text("ef")], selectedText: "b"),
    UnlinkBoundaryCase(
      name: "forward selection starting at the end of the link text",
      layout: [.text("ab"), .link(["cd"]), .text("ef")], anchor: ("cd", 2), focus: ("ef", 1),
      expected: [.text("ab"), .link(["cd"]), .text("ef")], selectedText: "e"),
    UnlinkBoundaryCase(
      name: "backward selection starting at the end of the link text",
      layout: [.text("ab"), .link(["cd"]), .text("ef")], anchor: ("ef", 1), focus: ("cd", 2),
      expected: [.text("ab"), .link(["cd"]), .text("ef")], selectedText: "e"),
    UnlinkBoundaryCase(
      name: "selection ending at offset 0 of a multi-child link",
      layout: [.text("ab"), .link(["cd", "ef"])], anchor: ("ab", 1), focus: ("cd", 0),
      expected: [.text("ab"), .link(["cd", "ef"])], selectedText: "b"),
    UnlinkBoundaryCase(
      name: "selection starting at the end of a multi-child link",
      layout: [.link(["ab", "cd"]), .text("ef")], anchor: ("cd", 2), focus: ("ef", 1),
      expected: [.link(["ab", "cd"]), .text("ef")], selectedText: "e"),
    UnlinkBoundaryCase(
      name: "unlinks a selected link while leaving a boundary-touching link intact",
      layout: [.link(["ab"]), .text("cd"), .link(["ef"])], anchor: ("ab", 1), focus: ("ef", 0),
      expected: [.link(["a"]), .text("bcd"), .link(["ef"])], selectedText: "bcd"),
  ]

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > forward selection ending at offset 0 of…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > backward selection ending at offset 0 o…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > forward selection starting at the end o…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > backward selection starting at the end …"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > selection ending at offset 0 of a multi…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > selection starting at the end of a mult…"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts::$toggleLink(null) with a selection touching a link boundary > unlinks a selected link while leaving a…"
    ), arguments: cases)
  func unlinkAtBoundary(_ testCase: UnlinkBoundaryCase) throws {
    let editor = try buildEditorFromExtensions(unlinkBoundaryExtension)
    defer { editor.dispose() }
    try editor.update(discrete: true) { try buildParagraph(testCase.layout) }
    try editor.update(discrete: true) {
      let selection = createRangeSelection()
      try selection.anchor.set(
        getTextNode(testCase.anchor.0).getKey(), testCase.anchor.1, .text)
      try selection.focus.set(getTextNode(testCase.focus.0).getKey(), testCase.focus.1, .text)
      try setSelection(selection)
      try toggleLink(nil)
    }
    try editor.read { () throws -> Void in
      #expect(try describeParagraph() == testCase.expected)
      let selection = try #require(
        try getSelection() as? RangeSelection, "Expected a RangeSelection")
      #expect(try selection.getTextContent() == testCase.selectedText)
    }
  }
}
