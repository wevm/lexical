import Lexical
import LexicalTesting
import Testing

// Ports of the createDOM, updateDOM and exportDOM tests of
// nodes/__tests__/unit/LexicalTextNode.test.tsx, LexicalParagraphNode.test.ts,
// LexicalLineBreakNode.test.ts and LexicalRootNode.test.ts,
// __tests__/unit/LexicalNode.test.ts, and LexicalElementHelpers.test.ts.

private var textEditorConfig: EditorConfig {
  EditorConfig(
    namespace: "",
    theme: [
      "text": [
        "bold": "my-bold-class",
        "capitalize": "my-capitalize-class",
        "code": "my-code-class",
        "highlight": "my-highlight-class",
        "italic": "my-italic-class",
        "lowercase": "my-lowercase-class",
        "strikethrough": "my-strikethrough-class",
        "underline": "my-underline-class",
        "underlineStrikethrough": "my-underline-strikethrough-class",
        "uppercase": "my-uppercase-class",
      ]
    ])
}

private var paragraphEditorConfig: EditorConfig {
  EditorConfig(
    namespace: "", theme: ["paragraph": "my-paragraph-class"])
}

struct TextCreateDOMCase: Sendable, CustomTestStringConvertible {
  let name: String
  let format: Int
  let contents: String
  let expectedHTML: String
  var testDescription: String { "\(name) text format type" }

  static let all: [TextCreateDOMCase] = [
    .init(
      name: "no formatting", format: 0, contents: "My text node",
      expectedHTML: "<span>My text node</span>"),
    .init(
      name: "bold", format: IS_BOLD, contents: "My text node",
      expectedHTML: "<strong class=\"my-bold-class\">My text node</strong>"),
    .init(
      name: "bold + empty", format: IS_BOLD, contents: "",
      expectedHTML: "<strong class=\"my-bold-class\"></strong>"),
    .init(
      name: "underline", format: IS_UNDERLINE, contents: "My text node",
      expectedHTML: "<span class=\"my-underline-class\">My text node</span>"),
    .init(
      name: "strikethrough", format: IS_STRIKETHROUGH, contents: "My text node",
      expectedHTML: "<span class=\"my-strikethrough-class\">My text node</span>"),
    .init(
      name: "highlight", format: IS_HIGHLIGHT, contents: "My text node",
      expectedHTML: "<mark><span class=\"my-highlight-class\">My text node</span></mark>"),
    .init(
      name: "italic", format: IS_ITALIC, contents: "My text node",
      expectedHTML: "<em class=\"my-italic-class\">My text node</em>"),
    .init(
      name: "code", format: IS_CODE, contents: "My text node",
      expectedHTML:
        "<code spellcheck=\"false\"><span class=\"my-code-class\">My text node</span></code>"),
    .init(
      name: "lowercase", format: IS_LOWERCASE, contents: "My text node",
      expectedHTML: "<span class=\"my-lowercase-class\">My text node</span>"),
    .init(
      name: "uppercase", format: IS_UPPERCASE, contents: "My text node",
      expectedHTML: "<span class=\"my-uppercase-class\">My text node</span>"),
    .init(
      name: "capitalize", format: IS_CAPITALIZE, contents: "My text node",
      expectedHTML: "<span class=\"my-capitalize-class\">My text node</span>"),
    .init(
      name: "underline + strikethrough", format: IS_UNDERLINE | IS_STRIKETHROUGH,
      contents: "My text node",
      expectedHTML: "<span class=\"my-underline-strikethrough-class\">My text node</span>"),
    .init(
      name: "code + italic", format: IS_CODE | IS_ITALIC, contents: "My text node",
      expectedHTML:
        "<code spellcheck=\"false\"><em class=\"my-code-class my-italic-class\">My text node</em></code>"
    ),
    .init(
      name: "code + underline + strikethrough", format: IS_CODE | IS_UNDERLINE | IS_STRIKETHROUGH,
      contents: "My text node",
      expectedHTML:
        "<code spellcheck=\"false\"><span class=\"my-underline-strikethrough-class my-code-class\">My text node</span></code>"
    ),
    .init(
      name: "highlight + italic", format: IS_HIGHLIGHT | IS_ITALIC, contents: "My text node",
      expectedHTML:
        "<mark><em class=\"my-highlight-class my-italic-class\">My text node</em></mark>"),
    .init(
      name: "code + underline + strikethrough + bold + italic",
      format: IS_CODE | IS_UNDERLINE | IS_STRIKETHROUGH | IS_BOLD | IS_ITALIC,
      contents: "My text node",
      expectedHTML:
        "<code spellcheck=\"false\"><strong class=\"my-underline-strikethrough-class my-bold-class my-code-class my-italic-class\">My text node</strong></code>"
    ),
    .init(
      name: "code + underline + strikethrough + bold + italic + highlight + uppercase",
      format: IS_CODE | IS_UNDERLINE | IS_STRIKETHROUGH | IS_BOLD | IS_ITALIC | IS_HIGHLIGHT
        | IS_UPPERCASE,
      contents: "My text node",
      expectedHTML:
        "<code spellcheck=\"false\"><strong class=\"my-underline-strikethrough-class my-bold-class my-code-class my-highlight-class my-italic-class my-uppercase-class\">My text node</strong></code>"
    ),
  ]
}

struct TextUpdateDOMCase: Sendable, CustomTestStringConvertible {
  let name: String
  let prev: (format: Int, text: String)
  let next: (format: Int, text: String)
  let result: Bool
  let expectedHTML: String?
  var testDescription: String { name }

  static let all: [TextUpdateDOMCase] = [
    .init(
      name: "different tags", prev: (IS_BOLD, "My text node"), next: (IS_ITALIC, "My text node"),
      result: true, expectedHTML: nil),
    .init(
      name: "no change in tags", prev: (IS_BOLD, "My text node"), next: (IS_BOLD, "My text node"),
      result: false, expectedHTML: "<strong class=\"my-bold-class\">My text node</strong>"),
    .init(
      name: "change in text", prev: (IS_BOLD, "My text node"), next: (IS_BOLD, "My new text node"),
      result: false, expectedHTML: "<strong class=\"my-bold-class\">My new text node</strong>"),
    .init(
      name: "removing code block", prev: (IS_CODE | IS_BOLD, "My text node"),
      next: (IS_BOLD, "My new text node"), result: true, expectedHTML: nil),
  ]
}

struct TextExportDOMCase: Sendable, CustomTestStringConvertible {
  let name: String
  let format: Int
  var testDescription: String { name }

  static let all: [TextExportDOMCase] = [
    .init(name: "lowercase format", format: IS_LOWERCASE),
    .init(name: "uppercase format", format: IS_UPPERCASE),
    .init(name: "capitalize format", format: IS_CAPITALIZE),
    .init(name: "combined bold and lowercase format", format: IS_BOLD | IS_LOWERCASE),
  ]
}

@Suite struct LexicalTextNodeDOMTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > no formatting text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > bold text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > bold + empty text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > underline text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > strikethrough text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > highlight text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > italic text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > lowercase text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > uppercase text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > capitalize text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > underline + strikethrough text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + italic text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > highlight + italic text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough + bold + italic text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > code + underline + strikethrough + bold + italic + highlight + uppercase text format type"
    ), arguments: TextCreateDOMCase.all)
  func createDOM(_ c: TextCreateDOMCase) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let textNode = try createTextNode(c.contents)
      try textNode.setFormat(c.format)
      let element = try textNode.createDOM(textEditorConfig)
      #expect(element.outerHTML == c.expectedHTML)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > applies styles with direct DOM property updates"
    ))
  func createDOMAppliesStyles() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let textNode = try createTextNode("My text node")
      try textNode.setStyle("color: red; background-color: blue !important; --custom: value;")
      let element = try textNode.createDOM(textEditorConfig)
      #expect(element.style["color"] == "red")
      #expect(element.style["background-color"] == "blue")
      #expect(element.style.getPropertyPriority("background-color") == "important")
      #expect(element.style.getPropertyValue("--custom") == "value")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > has parent node > no formatting text format type"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > createDOM() > has parent node > no formatting + empty string text format type"
    ), arguments: [("My text node", "<span>My text node</span>"), ("", "<span></span>")])
  func createDOMWithParent(_ contents: String, _ expectedHTML: String) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode(contents)
      try paragraphNode.append(textNode)
      let element = try textNode.createDOM(textEditorConfig)
      #expect(element.outerHTML == expectedHTML)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > different tags"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > no change in tags"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > change in text"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > removing code block"
    ), arguments: TextUpdateDOMCase.all)
  func updateDOM(_ c: TextUpdateDOMCase) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let prevTextNode = try createTextNode(c.prev.text)
      try prevTextNode.setMode("normal")
      try prevTextNode.setFormat(c.prev.format)
      let element = try prevTextNode.createDOM(textEditorConfig)
      let textNode = try createTextNode(c.next.text)
      try textNode.setMode("normal")
      try textNode.setFormat(c.next.format)
      #expect(try textNode.updateDOM(prevTextNode, element, textEditorConfig) == c.result)
      if !c.result {
        #expect(element.outerHTML == c.expectedHTML)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > updateDOM() > updates and removes styles with direct DOM property updates"
    ))
  func updateDOMUpdatesStyles() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let prevTextNode = try createTextNode("My text node")
      try prevTextNode.setStyle("color: red; --custom: value;")
      let element = try prevTextNode.createDOM(textEditorConfig)
      let nextTextNode = try createTextNode("My text node")
      try nextTextNode.setStyle("padding: 1px;")
      #expect(try nextTextNode.updateDOM(prevTextNode, element, textEditorConfig) == false)
      #expect(element.style["color"] == "")
      #expect(element.style.getPropertyValue("--custom") == "")
      #expect(element.style["padding"] == "1px")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > lowercase format"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > uppercase format"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > capitalize format"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportDOM() > combined bold and lowercase format"
    ), arguments: TextExportDOMCase.all)
  func exportDOM(_ c: TextExportDOMCase) throws {
    let editor = try createTestEditor()
    try update(editor) {
      let textNode = try createTextNode("Sample Text")
      try textNode.setFormat(c.format)
      let element = try #require(try textNode.exportDOM(editor).element as? DOMElement)
      switch c.format {
      case IS_LOWERCASE: #expect(element.style["text-transform"] == "lowercase")
      case IS_UPPERCASE: #expect(element.style["text-transform"] == "uppercase")
      case IS_CAPITALIZE: #expect(element.style["text-transform"] == "capitalize")
      default:
        #expect(element.tagName.lowercased() == "b")
        let child = try #require(element.firstChild as? DOMElement)
        #expect(child.style["text-transform"] == "lowercase")
      }
    }
  }
}

@Suite struct LexicalCoreNodeDOMTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.createDOM()"
    ))
  func paragraphCreateDOM() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraphNode = try ParagraphNode()
      #expect(
        try paragraphNode.createDOM(paragraphEditorConfig).outerHTML
          == "<p class=\"my-paragraph-class\"></p>")
      #expect(try paragraphNode.createDOM(EditorConfig(namespace: "")).outerHTML == "<p></p>")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.updateDOM()"
    ))
  func paragraphUpdateDOM() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let paragraphNode = try ParagraphNode()
      let domElement = try paragraphNode.createDOM(paragraphEditorConfig)
      #expect(domElement.outerHTML == "<p class=\"my-paragraph-class\"></p>")
      let newParagraphNode = try ParagraphNode()
      #expect(
        try newParagraphNode.updateDOM(paragraphNode, domElement, paragraphEditorConfig) == false)
      #expect(domElement.outerHTML == "<p class=\"my-paragraph-class\"></p>")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.createDOM()"
    ))
  func lineBreakCreateDOM() throws {
    let editor = try createTestEditor()
    try update(editor) { () throws in
      #expect(try createLineBreakNode().createDOM(EditorConfig(namespace: "")).outerHTML == "<br>")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalLineBreakNode.test.ts::LexicalLineBreakNode tests > LineBreakNode.updateDOM()"
    ))
  func lineBreakUpdateDOM() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let node = try createLineBreakNode()
      let dom = try node.createDOM(EditorConfig(namespace: ""))
      #expect(try node.updateDOM(node, dom, EditorConfig(namespace: "")) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.createDOM()"
    ))
  func rootCreateDOM() throws {
    let editor = try createTestEditor()
    try editor.read {
      #expect(throws: LexicalError.self) { try getRoot().createDOM(EditorConfig(namespace: "")) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalRootNode.test.ts::LexicalRootNode tests > RootNode.updateDOM()"
    ))
  func rootUpdateDOM() throws {
    let editor = try createTestEditor()
    try editor.read {
      let root = try getRoot()
      let dom = getDocument().createElement("div")
      #expect(try root.updateDOM(root, dom, EditorConfig(namespace: "")) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.createDOM()")
  )
  func lexicalNodeCreateDOM() throws {
    let editor = try createTestEditor(nodes: [.klass(PlainNode.self)])
    try update(editor) {
      let node = try PlainNode()
      #expect(throws: LexicalError.self) { try node.createDOM(EditorConfig(namespace: ""), editor) }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.updateDOM()")
  )
  func lexicalNodeUpdateDOM() throws {
    let editor = try createTestEditor(nodes: [.klass(PlainNode.self)])
    try update(editor) {
      let node = try PlainNode()
      let dom = getDocument().createElement("div")
      #expect(throws: LexicalError.self) {
        try node.updateDOM(node, dom, EditorConfig(namespace: ""))
      }
    }
  }
}

@Suite struct LexicalElementHelpersTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > basic"
    ))
  func basic() {
    let element = getDocument().createElement("div")
    addClassNamesToElement(element, "test-class")
    #expect(element.className == "test-class")
    removeClassNamesFromElement(element, "test-class")
    #expect(element.className == "")
  }

  /// Upstream also passes `false` and `true`, which a Swift `String?` can't
  /// hold; they add nothing there either.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > empty"
    ))
  func empty() {
    let element = getDocument().createElement("div")
    addClassNamesToElement(element, nil, nil, "", " ", "  \t\n")
    #expect(element.className == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > multiple"
    ))
  func multiple() {
    let element = getDocument().createElement("div")
    addClassNamesToElement(element, "a", "b", "c")
    #expect(element.className == "a b c")
    removeClassNamesFromElement(element, "a", "b", "c")
    #expect(element.className == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > addClassNamesToElement() and removeClassNamesFromElement() > space separated"
    ))
  func spaceSeparated() {
    let element = getDocument().createElement("div")
    addClassNamesToElement(element, "a b c")
    #expect(element.className == "a b c")
    removeClassNamesFromElement(element, "a b c")
    #expect(element.className == "")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalElementHelpers.test.ts::LexicalElementHelpers tests > multiple spaces"
    ))
  func multipleSpaces() {
    let classNames = " a  b   c \t\n  "
    let element = getDocument().createElement("div")
    addClassNamesToElement(element, classNames)
    #expect(element.className == "a b c")
    removeClassNamesFromElement(element, classNames)
    #expect(element.className == "")
  }
}
