import Lexical
import LexicalTesting
import Testing

// Port of nodes/__tests__/unit/LexicalTextNode.test.tsx, except the
// createDOM, updateDOM and exportDOM tests, which are dom-only.

/// `init()`: a paragraph holding an empty unmergeable text node.
private func textNodeEditor() throws -> Editor {
  let editor = try createTestEditor()
  try update(editor) {
    let paragraph = try createParagraphNode()
    let text = try createTextNode()
    try text.toggleUnmergeable()
    try paragraph.append(text)
    try getRoot().append(paragraph)
  }
  return editor
}

/// The empty text node of `init()`.
private func firstText() throws -> TextNode {
  let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
  return try #require(try paragraph.getFirstChild() as? TextNode)
}

struct FormatFlagCase: Sendable, CustomTestStringConvertible {
  let name: TextFormatType
  let flag: Int
  var testDescription: String { name }

  static let all: [FormatFlagCase] = [
    ("bold", IS_BOLD), ("italic", IS_ITALIC), ("strikethrough", IS_STRIKETHROUGH),
    ("underline", IS_UNDERLINE), ("code", IS_CODE), ("subscript", IS_SUBSCRIPT),
    ("superscript", IS_SUPERSCRIPT), ("highlight", IS_HIGHLIGHT), ("lowercase", IS_LOWERCASE),
    ("uppercase", IS_UPPERCASE), ("capitalize", IS_CAPITALIZE),
  ].map { FormatFlagCase(name: $0.0, flag: $0.1) }
}

struct SplitTextCase: Sendable, CustomTestStringConvertible {
  let text: String
  let offsets: [Int]
  let expected: [String]
  var testDescription: String { "\"\(text)\" splitText(\(offsets))" }

  static let all: [SplitTextCase] = [
    SplitTextCase(text: "a", offsets: [], expected: ["a"]),
    SplitTextCase(text: "a", offsets: [1], expected: ["a"]),
    SplitTextCase(text: "a", offsets: [5], expected: ["a"]),
    SplitTextCase(text: "Hello World", offsets: [], expected: ["Hello World"]),
    SplitTextCase(text: "Hello World", offsets: [3], expected: ["Hel", "lo World"]),
    SplitTextCase(text: "Hello World", offsets: [3, 3], expected: ["Hel", "lo World"]),
    SplitTextCase(text: "Hello World", offsets: [3, 7], expected: ["Hel", "lo W", "orld"]),
    SplitTextCase(text: "Hello World", offsets: [7, 3], expected: ["Hel", "lo W", "orld"]),
    SplitTextCase(text: "Hello World", offsets: [3, 7, 99], expected: ["Hel", "lo W", "orld"]),
  ]
}

struct SplitSelectCase: Sendable, CustomTestStringConvertible {
  let text: String
  let offsets: [Int]
  let select: (Int, Int)
  let anchor: (Int, Int)
  let focus: (Int, Int)
  var testDescription: String { "\"\(text)\" splitText(\(offsets)) with select\(select)" }

  static let all: [SplitSelectCase] = [
    SplitSelectCase(text: "Hello", offsets: [4], select: (3, 3), anchor: (0, 3), focus: (0, 3)),
    SplitSelectCase(text: "Hello", offsets: [4], select: (5, 5), anchor: (1, 1), focus: (1, 1)),
    SplitSelectCase(
      text: "Hello World", offsets: [4], select: (2, 7), anchor: (0, 2), focus: (1, 3)),
    SplitSelectCase(
      text: "Hello World", offsets: [4], select: (2, 4), anchor: (0, 2), focus: (0, 4)),
    SplitSelectCase(
      text: "Hello World", offsets: [4], select: (7, 2), anchor: (1, 3), focus: (0, 2)),
    SplitSelectCase(
      text: "Hello World", offsets: [4, 6], select: (2, 9), anchor: (0, 2), focus: (2, 3)),
    SplitSelectCase(
      text: "Hello World", offsets: [4, 6], select: (9, 2), anchor: (2, 3), focus: (0, 2)),
    SplitSelectCase(
      text: "Hello World", offsets: [4, 6], select: (9, 9), anchor: (2, 3), focus: (2, 3)),
  ]
}

/// `select(anchorOffset, focusOffset)` and the offsets it resolves to.
struct SelectCase: Sendable, CustomTestStringConvertible {
  let anchor: Int?
  let focus: Int?
  let expected: (Int, Int)
  var testDescription: String {
    "select(\(String(describing: anchor)), \(String(describing: focus)))"
  }

  static let all: [SelectCase] = [
    SelectCase(anchor: 2, focus: 4, expected: (2, 4)),
    SelectCase(anchor: 4, focus: 2, expected: (4, 2)),
    SelectCase(anchor: nil, focus: 2, expected: (11, 2)),
    SelectCase(anchor: 2, focus: nil, expected: (2, 11)),
    SelectCase(anchor: nil, focus: nil, expected: (11, 11)),
  ]
}

/// "Hello World" appended to the root in a new paragraph.
private func helloWorld() throws -> TextNode {
  let paragraphNode = try createParagraphNode()
  let textNode = try createTextNode("Hello World")
  try paragraphNode.append(textNode)
  try getRoot().append(paragraphNode)
  return textNode
}

@Suite struct LexicalTextNodeTests {
  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > exportJSON() > should return and object conforming to the expected schema"
    ))
  func exportJSONSchema() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let json = try createTextNode().exportJSON()
      #expect(
        json.stringify()
          == #"{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > root.getTextContent() > writable nodes"
    ))
  func rootTextContentWritableNodes() throws {
    let editor = try textNodeEditor()
    var nodeKey = ""
    try update(editor) {
      let textNode = try createTextNode("Text")
      nodeKey = textNode.getKey()
      #expect(try textNode.getTextContent() == "Text")
      #expect(textNode.__text == "Text")
      try #require(try getRoot().getFirstChild() as? ElementNode).append(textNode)
    }
    #expect(try getEditorStateTextContent(editor.getEditorState()) == "Text")
    try update(editor) { try #require(try getNodeByKey(nodeKey)).markDirty() }
    #expect(try getEditorStateTextContent(editor.getEditorState()) == "Text")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > root.getTextContent() > prepend node"
    ))
  func rootTextContentPrependNode() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      try #require(try getRoot().getFirstChild() as? ElementNode).append(
        createTextNode("World").toggleUnmergeable())
    }
    try update(editor) {
      let previous = try #require(
        try (getRoot().getFirstChild() as? ElementNode)?.getFirstChild())
      try previous.insertBefore(createTextNode("Hello ").toggleUnmergeable())
    }
    #expect(try getEditorStateTextContent(editor.getEditorState()) == "Hello World")
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setTextContent() > writable nodes"
    ))
  func setTextContentWritable() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try createTextNode("My new text node")
      try textNode.setTextContent("My newer text node")
      #expect(try textNode.getTextContent() == "My newer text node")
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > getFormatFlags(bold)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > getFormatFlags(italic)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > getFormatFlags(strikethrough)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > getFormatFlags(underline)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > getFormatFlags(code)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > getFormatFlags(subscript)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > getFormatFlags(superscript)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > getFormatFlags(highlight)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > getFormatFlags(lowercase)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > getFormatFlags(uppercase)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > getFormatFlags(capitalize)"
    ), arguments: FormatFlagCase.all)
  func getFormatFlags(_ c: FormatFlagCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try firstText()
      let newFormat = try textNode.getFormatFlags(c.name, nil)
      #expect(newFormat == c.flag)
      try textNode.setFormat(newFormat)
      #expect(try textNode.getFormatFlags(c.name, nil) == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > predicate for bold"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > predicate for italic"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > predicate for strikethrough"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > predicate for underline"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > predicate for code"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > predicate for subscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > predicate for superscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > predicate for highlight"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > predicate for lowercase"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > predicate for uppercase"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > predicate for capitalize"
    ), arguments: FormatFlagCase.all)
  func formatPredicate(_ c: FormatFlagCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try firstText()
      try textNode.setFormat(c.flag)
      #expect(try textNode.hasFormat(c.name))
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > bold flag > toggling for bold"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > italic flag > toggling for italic"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > strikethrough flag > toggling for strikethrough"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > underline flag > toggling for underline"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > code flag > toggling for code"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > subscript flag > toggling for subscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > superscript flag > toggling for superscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > highlight flag > toggling for highlight"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > lowercase flag > toggling for lowercase"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > uppercase flag > toggling for uppercase"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalize flag > toggling for capitalize"
    ), arguments: FormatFlagCase.all)
  func formatToggling(_ c: FormatFlagCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try firstText()
      #expect(try textNode.hasFormat(c.name) == false)
      try textNode.toggleFormat(c.name)
      #expect(try textNode.hasFormat(c.name))
      try textNode.toggleFormat(c.name)
      #expect(try textNode.hasFormat(c.name) == false)
    }
  }

  /// Toggles `first` then `second`, and checks which of the two are set.
  struct ToggleCase: Sendable, CustomTestStringConvertible {
    let first: TextFormatType
    let second: TextFormatType
    let expectFirst: Bool
    let expectSecond: Bool
    var testDescription: String { "\(first) then \(second)" }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setting subscript clears superscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > setting superscript clears subscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > clearing subscript does not set superscript"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > clearing superscript does not set subscript"
    ),
    arguments: [
      ToggleCase(first: "superscript", second: "subscript", expectFirst: false, expectSecond: true),
      ToggleCase(first: "subscript", second: "superscript", expectFirst: false, expectSecond: true),
      ToggleCase(first: "subscript", second: "subscript", expectFirst: false, expectSecond: false),
      ToggleCase(
        first: "superscript", second: "superscript", expectFirst: false, expectSecond: false),
    ])
  func scriptFormatsExclusive(_ c: ToggleCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try helloWorld()
      try textNode.toggleFormat(c.first)
      try textNode.toggleFormat(c.second)
      #expect(try textNode.hasFormat(c.second) == c.expectSecond)
      let other: TextFormatType = c.second == "subscript" ? "superscript" : "subscript"
      #expect(try textNode.hasFormat(other) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > capitalization formats are mutually exclusive"
    ))
  func capitalizationExclusive() throws {
    let editor = try textNodeEditor()
    let formats: [TextFormatType] = ["lowercase", "uppercase", "capitalize"]
    try update(editor) {
      let textNode = try helloWorld()
      for formatToSet in formats {
        try textNode.toggleFormat(formatToSet)
        for format in formats where format != formatToSet {
          #expect(try textNode.hasFormat(format) == false)
        }
        #expect(try textNode.hasFormat(formatToSet))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > selectPrevious()"
    ))
  func selectPrevious() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode("Hello World")
      let textNode2 = try createTextNode("Goodbye Earth")
      try paragraphNode.append(textNode, textNode2)
      try getRoot().append(paragraphNode)
      var selection = try textNode2.selectPrevious()
      #expect(try selection.anchor.getNode() === textNode.getLatest())
      #expect(selection.anchor.offset == 11)
      #expect(try selection.focus.getNode() === textNode.getLatest())
      #expect(selection.focus.offset == 11)
      selection = try textNode.selectPrevious()
      #expect(try selection.anchor.getNode() === paragraphNode.getLatest())
      #expect(selection.anchor.offset == 0)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > selectNext()"
    ))
  func selectNext() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode("Hello World")
      let textNode2 = try createTextNode("Goodbye Earth")
      try paragraphNode.append(textNode, textNode2)
      try getRoot().append(paragraphNode)
      var selection = try textNode.selectNext(1, 3)
      #expect(try selection.anchor.getNode() === textNode2.getLatest())
      #expect(selection.anchor.offset == 1)
      #expect(try selection.focus.getNode() === textNode2.getLatest())
      #expect(selection.focus.offset == 3)
      selection = try textNode2.selectNext()
      #expect(try selection.anchor.getNode() === paragraphNode.getLatest())
      #expect(selection.anchor.offset == 2)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #2"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #3"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #4"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > select() > select(...%p) #5"
    ), arguments: SelectCase.all)
  func select(_ c: SelectCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let textNode = try helloWorld()
      let selection = try textNode.select(c.anchor, c.focus)
      #expect(try selection.focus.getNode() === textNode.getLatest())
      #expect(selection.anchor.offset == c.expected.0)
      #expect(selection.focus.offset == c.expected.1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > convert segmented node into plain text"
    ))
  func splitSegmented() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let segmentedNode = try createTestSegmentedNode("Hello World")
      let paragraphNode = try createParagraphNode()
      try paragraphNode.append(segmentedNode)
      let split = try segmentedNode.splitText(5)
      let children = try paragraphNode.getAllTextNodes()
      #expect(try paragraphNode.getTextContent() == "Hello World")
      #expect(try children[0].isSimpleText())
      #expect(try children[0].getTextContent() == "Hello")
      #expect(split[0] === children[0])
      #expect(split[1] === children[1])
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"a\" splitText(...%p)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"a\" splitText(...%p) #2"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"a\" splitText(...%p) #3"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) #2"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) #3"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) #4"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) #5"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) #6"
    ), arguments: SplitTextCase.all)
  func splitText(_ c: SplitTextCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode(c.text)
      try paragraphNode.append(textNode)
      let splitNodes = try textNode.splitText(offsets: c.offsets)
      #expect(try paragraphNode.getChildren().count == c.expected.count)
      #expect(try splitNodes.map { try $0.getTextContent() } == c.expected)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > splitText moves composition key to last node"
    ))
  func splitTextMovesCompositionKey() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode("12345")
      try paragraphNode.append(textNode)
      try setCompositionKey(textNode.getKey())
      let split = try textNode.splitText(1)
      #expect(try getCompositionKey() == split[1].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello\" splitText(...%p) with select(...%p)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello\" splitText(...%p) with select(...%p) #2"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p)"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p) #2"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p) #3"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p) #4"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p) #5"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > \"Hello World\" splitText(...%p) with select(...%p) #6"
    ), arguments: SplitSelectCase.all)
  func splitTextWithSelection(_ c: SplitSelectCase) throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let textNode = try createTextNode(c.text)
      try paragraphNode.append(textNode)
      try getRoot().append(paragraphNode)
      let selection = try textNode.select(c.select.0, c.select.1)
      let childrenNodes = try textNode.splitText(offsets: c.offsets)
      #expect(try selection.anchor.getNode() === childrenNodes[c.anchor.0])
      #expect(selection.anchor.offset == c.anchor.1)
      #expect(try selection.focus.getNode() === childrenNodes[c.focus.0])
      #expect(selection.focus.offset == c.focus.1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > with detached parent"
    ))
  func splitTextDetached() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let splits = try createTextNode("foo").splitText(1, 2)
      #expect(try splits.map { try $0.getTextContent() } == ["f", "o", "o"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > copies state to all nodes"
    ),
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > splitText() > copies state to all nodes (segmented)"
    ),
    arguments: [false, true])
  func splitTextCopiesState(_ segmented: Bool) throws {
    let editor = try textNodeEditor()
    let state = createState("state", parse: { $0 }, unparse: { $0 })
    try update(editor) {
      let textNode =
        try segmented ? createTestSegmentedNode("hello world") : createTextNode("hello world")
      try setState(textNode, state, "foo")
      let splits = try textNode.splitText(3, 5)
      #expect(try splits.allSatisfy { try getState($0, state) == "foo" })
      if !segmented {
        try setState(splits[0], state, "bar")
        #expect(try getState(splits[0], state) == "bar")
        #expect(try getState(splits[1], state) == "foo")
        #expect(try getState(splits[2], state) == "foo")
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/nodes/__tests__/unit/LexicalTextNode.test.tsx::LexicalTextNode tests > mergeWithSibling"
    ))
  func mergeWithSibling() throws {
    let editor = try textNodeEditor()
    try update(editor) {
      let paragraph = try #require(try getRoot().getFirstChild() as? ElementNode)
      let textNode1 = try createTextNode("1")
      let textNode2 = try createTextNode("2")
      let textNode3 = try createTextNode("3")
      try paragraph.append(textNode1, textNode2, textNode3)
      try textNode2.select()
      let selection = try #require(try getSelection() as? RangeSelection)
      try textNode2.mergeWithSibling(textNode1)
      #expect(try selection.anchor.getNode() === textNode2.getLatest())
      #expect(selection.anchor.offset == 1)
      #expect(selection.focus.offset == 1)
      try textNode2.mergeWithSibling(textNode3)
      #expect(try selection.anchor.getNode() === textNode2.getLatest())
      #expect(selection.anchor.offset == 1)
      #expect(selection.focus.offset == 1)
    }
    #expect(try getEditorStateTextContent(editor.getEditorState()) == "123")
  }
}
