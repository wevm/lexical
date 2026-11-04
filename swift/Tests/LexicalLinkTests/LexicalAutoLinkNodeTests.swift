import Lexical
import LexicalLink
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts.

private var editorConfig: EditorConfig {
  EditorConfig(
    namespace: "",
    theme: [
      "link": "my-autolink-class",
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

nonisolated(unsafe) private let autoLinkTestState = createState(
  "autoLinkTestState", parse: { $0?.stringValue ?? "" })

@Suite struct LexicalAutoLinkNodeTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.constructor"
    ))
  func constructor() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let actutoLinkNode = try AutoLinkNode("/")

      #expect(actutoLinkNode.getType() == "autolink")
      #expect(actutoLinkNode.__url == "/")
      #expect(actutoLinkNode.__isUnlinked == false)
    }

    #expect(throws: LexicalError.self) { try AutoLinkNode("") }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.constructor with isUnlinked param set to true"
    ))
  func constructorWithIsUnlinked() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("/", AutoLinkAttributes(isUnlinked: true))

      #expect(autoLinkNode.getType() == "autolink")
      #expect(autoLinkNode.__url == "/")
      #expect(autoLinkNode.__isUnlinked == true)
    }

    #expect(throws: LexicalError.self) { try AutoLinkNode("") }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.clone()"
    ))
  func clone() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("/")

      let clone = try cloneWithProperties(autoLinkNode)

      #expect(clone !== autoLinkNode)
      // toStrictEqual: same class and the same own fields.
      #expect(type(of: clone) == AutoLinkNode.self)
      #expect(clone.__key == autoLinkNode.__key)
      #expect(clone.__parent == autoLinkNode.__parent)
      #expect(clone.__url == autoLinkNode.__url)
      #expect(clone.__target == autoLinkNode.__target)
      #expect(clone.__rel == autoLinkNode.__rel)
      #expect(clone.__title == autoLinkNode.__title)
      #expect(clone.__isUnlinked == autoLinkNode.__isUnlinked)
      #expect(try clone.exportJSON() == autoLinkNode.exportJSON())
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getURL()"
    ))
  func getURL() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")

      #expect(try autoLinkNode.getURL() == "https://example.com/foo")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setURL()"
    ))
  func setURL() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")

      #expect(try autoLinkNode.getURL() == "https://example.com/foo")

      try autoLinkNode.setURL("https://example.com/bar")

      #expect(try autoLinkNode.getURL() == "https://example.com/bar")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getTarget()"
    ))
  func getTarget() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(target: "_blank"))

      #expect(try autoLinkNode.getTarget() == "_blank")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setTarget()"
    ))
  func setTarget() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(target: "_blank"))

      #expect(try autoLinkNode.getTarget() == "_blank")

      try autoLinkNode.setTarget("_self")

      #expect(try autoLinkNode.getTarget() == "_self")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getRel()"
    ))
  func getRel() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(rel: "noopener noreferrer", target: "_blank"))

      #expect(try autoLinkNode.getRel() == "noopener noreferrer")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setRel()"
    ))
  func setRel() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(rel: "noopener", target: "_blank"))

      #expect(try autoLinkNode.getRel() == "noopener")

      try autoLinkNode.setRel("noopener noreferrer")

      #expect(try autoLinkNode.getRel() == "noopener noreferrer")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getTitle()"
    ))
  func getTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(title: "Hello world"))

      #expect(try autoLinkNode.getTitle() == "Hello world")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setTitle()"
    ))
  func setTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(title: "Hello world"))

      #expect(try autoLinkNode.getTitle() == "Hello world")

      try autoLinkNode.setTitle("World hello")

      #expect(try autoLinkNode.getTitle() == "World hello")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.getIsUnlinked()"
    ))
  func getIsUnlinked() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("/", AutoLinkAttributes(isUnlinked: true))
      #expect(try autoLinkNode.getIsUnlinked() == true)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.setIsUnlinked()"
    ))
  func setIsUnlinked() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("/")
      #expect(try autoLinkNode.getIsUnlinked() == false)
      try autoLinkNode.setIsUnlinked(true)
      #expect(try autoLinkNode.getIsUnlinked() == true)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" class="my-autolink-class"></a>"#)
      #expect(
        try autoLinkNode.createDOM(EditorConfig(namespace: "", theme: [:])).outerHTML
          == #"<a href="https://example.com/foo"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() for unlinked"
    ))
  func createDOMForUnlinked() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(isUnlinked: true))

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == "<span>\(try autoLinkNode.getTextContent())</span>")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() with target, rel and title"
    ))
  func createDOMWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo",
        AutoLinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-autolink-class"></a>"#
      )
      #expect(
        try autoLinkNode.createDOM(EditorConfig(namespace: "", theme: [:])).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world"></a>"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.createDOM() sanitizes javascript: URLs"
    ))
  func createDOMSanitizesJavascriptURLs() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("javascript:alert(0)")
      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="about:blank" class="my-autolink-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.updateDOM()"
    ))
  func updateDOM() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")

      let domElement = try autoLinkNode.createDOM(editorConfig)

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" class="my-autolink-class"></a>"#)

      let newAutoLinkNode = try AutoLinkNode("https://example.com/bar")
      let result = try newAutoLinkNode.updateDOM(autoLinkNode, domElement, editorConfig)

      #expect(result == false)
      #expect(
        domElement.outerHTML
          == #"<a href="https://example.com/bar" class="my-autolink-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.updateDOM() with target, rel and title"
    ))
  func updateDOMWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo",
        AutoLinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))

      let domElement = try autoLinkNode.createDOM(editorConfig)

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-autolink-class"></a>"#
      )

      let newAutoLinkNode = try AutoLinkNode(
        "https://example.com/bar",
        AutoLinkAttributes(rel: "noopener", target: "_self", title: "World hello"))
      let result = try newAutoLinkNode.updateDOM(autoLinkNode, domElement, editorConfig)

      #expect(result == false)
      #expect(
        domElement.outerHTML
          == #"<a href="https://example.com/bar" target="_self" rel="noopener" title="World hello" class="my-autolink-class"></a>"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.updateDOM() with undefined target, undefined rel and undefined title"
    ))
  func updateDOMWithUndefinedAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo",
        AutoLinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))

      let domElement = try autoLinkNode.createDOM(editorConfig)

      #expect(
        try autoLinkNode.createDOM(editorConfig).outerHTML
          == #"<a href="https://example.com/foo" target="_blank" rel="noopener noreferrer" title="Hello world" class="my-autolink-class"></a>"#
      )

      let newNode = try AutoLinkNode("https://example.com/bar")
      let result = try newNode.updateDOM(autoLinkNode, domElement, editorConfig)

      #expect(result == false)
      #expect(
        domElement.outerHTML
          == #"<a href="https://example.com/bar" class="my-autolink-class"></a>"#)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.updateDOM() with isUnlinked \"true\""
    ))
  func updateDOMWithIsUnlinked() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes(isUnlinked: false))

      let domElement = try autoLinkNode.createDOM(editorConfig)
      #expect(
        domElement.outerHTML
          == #"<a href="https://example.com/foo" class="my-autolink-class"></a>"#)

      let newAutoLinkNode = try AutoLinkNode(
        "https://example.com/bar", AutoLinkAttributes(isUnlinked: true))
      let newDomElement = try newAutoLinkNode.createDOM(editorConfig)
      #expect(
        newDomElement.outerHTML == "<span>\(try newAutoLinkNode.getTextContent())</span>")

      let result = try newAutoLinkNode.updateDOM(autoLinkNode, domElement, editorConfig)
      #expect(result == true)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.canInsertTextBefore()"
    ))
  func canInsertTextBefore() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")

      #expect(try autoLinkNode.canInsertTextBefore() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.canInsertTextAfter()"
    ))
  func canInsertTextAfter() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")
      #expect(try autoLinkNode.canInsertTextAfter() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $createAutoLinkNode()"
    ))
  func createAutoLinkNodeFactory() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("https://example.com/foo")
      let createdAutoLinkNode = try createAutoLinkNode(
        "https://example.com/foo", AutoLinkAttributes())

      #expect(autoLinkNode.getType() == createdAutoLinkNode.getType())
      #expect(autoLinkNode.__parent == createdAutoLinkNode.__parent)
      #expect(autoLinkNode.__url == createdAutoLinkNode.__url)
      #expect(autoLinkNode.__isUnlinked == createdAutoLinkNode.__isUnlinked)
      #expect(autoLinkNode.__key != createdAutoLinkNode.__key)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $createAutoLinkNode() with target, rel, isUnlinked and title"
    ))
  func createAutoLinkNodeWithAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode(
        "https://example.com/foo",
        AutoLinkAttributes(rel: "noopener noreferrer", target: "_blank", title: "Hello world"))

      let createdAutoLinkNode = try createAutoLinkNode(
        "https://example.com/foo",
        AutoLinkAttributes(
          rel: "noopener noreferrer", target: "_blank", title: "Hello world", isUnlinked: true))

      #expect(autoLinkNode.getType() == createdAutoLinkNode.getType())
      #expect(autoLinkNode.__parent == createdAutoLinkNode.__parent)
      #expect(autoLinkNode.__url == createdAutoLinkNode.__url)
      #expect(autoLinkNode.__target == createdAutoLinkNode.__target)
      #expect(autoLinkNode.__rel == createdAutoLinkNode.__rel)
      #expect(autoLinkNode.__title == createdAutoLinkNode.__title)
      #expect(autoLinkNode.__key != createdAutoLinkNode.__key)
      #expect(autoLinkNode.__isUnlinked != createdAutoLinkNode.__isUnlinked)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $isAutoLinkNode()"
    ))
  func isAutoLinkNodeGuard() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let autoLinkNode = try AutoLinkNode("")
      #expect(isAutoLinkNode(autoLinkNode) == true)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > $toggleLink applies the title attribute when creating"
    ))
  func toggleLinkAppliesTitle() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let p = try ParagraphNode()
      try p.append(try TextNode("Some text"))
      try getRoot().append(p)
    }

    try update(editor) {
      try selectAll()
      try toggleLink("https://lexical.dev/", LinkAttributes(title: "Lexical Website"))
    }

    let paragraph = try editor.getEditorState().toJSON()["root"]?.objectValue?["children"]?
      .arrayValue?[0].objectValue
    let link = paragraph?["children"]?.arrayValue?[0].objectValue
    #expect(link?["title"]?.stringValue == "Lexical Website")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::LexicalAutoAutoLinkNode tests > AutoLinkNode.insertNewAfter does not create new paragraph"
    ))
  func insertNewAfterDoesNotCreateParagraph() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let root = try getRoot()
      try root.clear()
      let paragraph = try createParagraphNode()
      let autoLink = try createAutoLinkNode("https://example.com")
      let textNode = try TextNode("https://example.com")
      try autoLink.append(textNode)
      try paragraph.append(autoLink)
      try root.append(paragraph)

      let selection = try createRangeSelection()
      let newNode = try autoLink.insertNewAfter(selection, restoreSelection: false)
      // Should create a sibling AutoLinkNode in the same paragraph
      #expect(isAutoLinkNode(newNode) == true)
      // The original paragraph should now contain two AutoLinkNodes
      #expect(try paragraph.getChildrenSize() == 2)
      // No new paragraph should be created
      #expect(try root.getChildrenSize() == 1)
    }
  }
}

@Suite struct AutoLinkNodeInsertNewAfterCarriesTheNodeOverTests {
  private func seed() throws -> AutoLinkNode {
    let root = try getRoot()
    try root.clear()
    let paragraph = try createParagraphNode()
    let autoLink = try createAutoLinkNode(
      "https://example.com",
      AutoLinkAttributes(rel: "noopener", target: "_blank", title: "Example", isUnlinked: true))
    try autoLink.setStyle("color: red")
    try autoLink.setTextStyle("font-size: 20px")
    try autoLink.setTextFormat(1)
    try setState(autoLink, autoLinkTestState, "carried")
    try autoLink.append(try TextNode("https://example.com"))
    try paragraph.append(autoLink)
    try root.append(paragraph)
    return autoLink
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the element style, text style and text format"
    ))
  func keepsStyleAndFormat() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let next = try seed().insertNewAfter(createRangeSelection(), restoreSelection: false)
      #expect(isAutoLinkNode(next) == true)
      let element = try #require(next as? ElementNode)
      #expect(try element.getStyle() == "color: red")
      #expect(try element.getTextStyle() == "font-size: 20px")
      #expect(try element.getTextFormat() == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the NodeState"
    ))
  func keepsNodeState() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let next = try #require(
        try seed().insertNewAfter(createRangeSelection(), restoreSelection: false))
      #expect(try getState(next, autoLinkTestState) == "carried")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkNode.test.ts::AutoLinkNode.insertNewAfter carries the node over > keeps the link attributes (control)"
    ))
  func keepsLinkAttributes() throws {
    let editor = try createLinkTestEditor()
    try update(editor) {
      let next = try seed().insertNewAfter(createRangeSelection(), restoreSelection: false)
      #expect(isAutoLinkNode(next) == true)
      let autoLink = try #require(next as? AutoLinkNode)
      #expect(try autoLink.getURL() == "https://example.com")
      #expect(try autoLink.getRel() == "noopener")
      #expect(try autoLink.getTarget() == "_blank")
      #expect(try autoLink.getTitle() == "Example")
      #expect(try autoLink.getIsUnlinked() == true)
    }
  }
}
