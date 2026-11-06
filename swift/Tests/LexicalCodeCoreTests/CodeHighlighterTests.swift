import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalMarkdown
import LexicalRichText
import LexicalTesting
import Testing

// Ports of packages/lexical-code-prism/src/__tests__/unit, against
// `CodeHighlighterExtension` with `TestCodeTokenizer` rather than Prism,
// and tests of the `CodeTokenizer` contract: composing tokenizers and a
// language that loads later.

/// A small tokenizer for language `test` (and `javascript`, upstream's
/// default): `let`, `const` and `var` are keywords, runs of digits numbers
/// and `=` an operator.
struct TestCodeTokenizer: CodeTokenizer {
  var defaultLanguage: String? = DEFAULT_CODE_LANGUAGE
  var languages: Set<String> = ["test", DEFAULT_CODE_LANGUAGE]

  func languageState(_ language: String) -> CodeLanguageState {
    languages.contains(language) ? .loaded : .unsupported
  }

  func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    var tokens: [CodeToken] = []
    var plain = ""
    func flush() {
      if !plain.isEmpty { tokens.append(.text(plain)) }
      plain = ""
    }
    var rest = Substring(code)
    while let first = rest.first {
      let word = rest.prefix { $0.isLetter }
      let digits = rest.prefix { $0.isNumber }
      if ["let", "const", "var"].contains(word) {
        flush()
        tokens.append(.token("keyword", String(word)))
        rest = rest.dropFirst(word.count)
      } else if !digits.isEmpty {
        flush()
        tokens.append(.token("number", String(digits)))
        rest = rest.dropFirst(digits.count)
      } else if first == "=" {
        flush()
        tokens.append(.token("operator", "="))
        rest = rest.dropFirst()
      } else {
        let run = word.isEmpty ? rest.prefix(1) : word
        plain += run
        rest = rest.dropFirst(run.count)
      }
    }
    flush()
    return tokens
  }
}

private func buildHighlightEditor(
  name: String, _ tokenizer: any CodeTokenizer = TestCodeTokenizer(),
  tabSize: Int? = nil, nodes: [LexicalNodeConfig] = []
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: name,
      dependencies: [
        RichTextExtension, TabIndentationExtension,
        configExtension(CodeHighlighterExtension) { $0.tokenizer = tokenizer },
        configExtension(CodeIndentExtension) { $0.tabSize = tabSize },
      ],
      nodes: { nodes }))
}

private func codeNode() throws -> CodeNode {
  try #require(try getRoot().getChildren().lazy.compactMap { $0 as? CodeNode }.first)
}

/// [type, text, highlight type] of the code block's children.
private func children() throws -> [[String?]] {
  try codeNode().getChildren().map {
    [$0.getType(), try $0.getTextContent(), ($0 as? CodeHighlightNode)?.__highlightType]
  }
}

@Suite struct CodeHighlighterRetainSelectionTests {
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content \"\\nfoo\" (#8943)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content \"\\n\\nfoo\" (#8943)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content \"foo\\nbar\" (#8943)"
    ),
    arguments: [("\nfoo", "X\nfoo"), ("\n\nfoo", "X\n\nfoo"), ("foo\nbar", "Xfoo\nbar")])
  func retainsANonNegativeOffset(codeText: String, expectedText: String) throws {
    let editor = try createCodeTestEditor()
    defer { editor.dispose() }
    _ = try registerCodeHighlighting(editor, TestCodeTokenizer())
    try editor.update(discrete: true) {
      let code = try createCodeNode("javascript")
      try getRoot().clear().append(code)
      try code.append(createTextNode(codeText))
      // The caret at the block's start, before its text is split, so the
      // highlighter has to restore it.
      _ = try code.select(0, 0)
    }
    try editor.read { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      for point in [selection.anchor, selection.focus] {
        #expect(point.offset >= 0)
        if point.type == .text {
          #expect(point.offset <= (try point.getNode().getTextContentSize()))
        }
      }
    }
    try editor.update(discrete: true) {
      try #require(try getSelection() as? RangeSelection).insertText("X")
    }
    #expect(
      try editor.read { try getRoot().getFirstChildOrThrow().getTextContent() } == expectedText)
  }

  /// Puts a caret in an empty paragraph, as when markdown is pasted into a
  /// focused editor: `convertFromMarkdownString` only moves a caret there is.
  private func seedSelection(_ editor: Editor) throws {
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      try getRoot().clear().append(paragraph)
      _ = try paragraph.selectEnd()
    }
  }

  /// Where the caret is, in terms that survive the highlighter's split.
  private func describeCaret() throws -> (
    blocks: [String], inCodeBlock: Bool, offset: Int, text: String
  ) {
    let selection = try #require(try getSelection() as? RangeSelection)
    let anchorNode = try selection.anchor.getNode()
    return (
      try getRoot().getChildren().map { $0.getType() },
      try anchorNode is CodeNode || anchorNode.getParents().contains { $0 is CodeNode },
      selection.anchor.offset, try anchorNode.getTextContent()
    )
  }

  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > importing markdown that ends in a code block leaves the caret at the document start"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > the caret is not dragged into the last of several imported code blocks"
    ),
    arguments: [
      ("hello world\n\n```\necho hello world\n```", ["paragraph", "code"]),
      (
        "hello world\n\n```\nfirst\n```\n\nmiddle\n\n```\nlast\n```",
        ["paragraph", "code", "paragraph", "code"]
      ),
    ])
  func importingMarkdownLeavesTheCaret(markdown: String, blocks: [String]) throws {
    let editor = try buildHighlightEditor(name: "code-prism-retain-selection-test")
    defer { editor.dispose() }
    try seedSelection(editor)
    try editor.update(discrete: true) { try convertFromMarkdownString(markdown, TRANSFORMERS) }
    let caret = try editor.read { try describeCaret() }
    #expect(caret.blocks == blocks)
    #expect(!caret.inCodeBlock)
    #expect(caret.offset == 0)
    #expect(caret.text == "hello world")
  }

  // A control: this caret is in the code block, so it passes with and
  // without the fix, and keeps #6305 from being fixed by never restoring.
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismRetainSelection.test.ts::Prism highlighting only retains a selection it owns (#6305) > a caret inside the code block is still retained across highlighting"
    ))
  func retainsACaretInsideTheCodeBlock() throws {
    let editor = try buildHighlightEditor(name: "code-prism-retain-selection-test")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let code = try createCodeNode("javascript")
      let text = try createTextNode("const x = 1;")
      try code.append(text)
      try getRoot().clear().append(code)
      // After `const `, which becomes a token boundary.
      _ = try text.select(6, 6)
    }
    let caret = try editor.read { try describeCaret() }
    #expect(caret.inCodeBlock)
    // The caret rides along to the token that now holds text offset 6.
    #expect(caret.text == " x ")
    #expect(caret.offset == 1)
  }
}

nonisolated(unsafe) private let noteState = createState("note", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let flatState = createState(
  "flatNote", parse: { $0?.stringValue ?? "" })
nonisolated(unsafe) private let createdState = createState(
  "created", parse: { $0?.stringValue ?? "" })

/// A code highlight node with a flat state (serialized at the top level).
private final class FlatHighlightNode: CodeHighlightNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      FlatHighlightNode.self, type: "test-flat-highlight", extends: CodeHighlightNode.self,
      stateConfigs: [RequiredNodeStateConfig(flatState, flat: true)])
  }
}

/// A TextNode replacement that sets a state of its own when it is created.
private final class CreatedTextNode: TextNode {
  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CreatedTextNode.self, type: "test-created-text", extends: TextNode.self,
      stateConfigs: [RequiredNodeStateConfig(createdState)])
  }
}

@Suite struct CodeHighlighterLeaveCodeNodeStateTests {
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismLeaveCodeNodeState.test.ts::code highlight nodes turned back into text when they leave a code block keep their NodeState"
    ))
  func keepsNodeStateLeavingTheCodeBlock() throws {
    let editor = try buildHighlightEditor(name: "leave-code-node-state")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let token = try setState(createCodeHighlightNode("note"), noteState, "kept")
      try getRoot().clear().append(createCodeNode().append(token))
    }
    try editor.update(discrete: true) {
      _ = try getRoot().getFirstChildOrThrow().replace(createParagraphNode(), includeChildren: true)
    }
    let nodes = try editor.read {
      try getRoot().getAllTextNodes().map {
        "\(!($0 is CodeHighlightNode)) \(try $0.getTextContent()) \(try getState($0, noteState))"
      }
    }
    #expect(nodes == ["true note kept"])
  }

  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismLeaveCodeNodeState.test.ts::the copy includes flat states and keeps the state of a TextNode replacement"
    ))
  func copiesFlatStatesAndKeepsAReplacementsState() throws {
    let editor = try buildHighlightEditor(
      name: "leave-code-flat-node-state",
      nodes: [
        .klass(FlatHighlightNode.self),
        .replacement(
          LexicalNodeReplacement(
            replace: CodeHighlightNode.self,
            with: { try create(FlatHighlightNode.self).setTextContent($0.getTextContent()) },
            withKlass: FlatHighlightNode.self)),
        .klass(CreatedTextNode.self),
        .replacement(
          LexicalNodeReplacement(
            replace: TextNode.self,
            with: {
              try setState(
                create(CreatedTextNode.self).setTextContent($0.getTextContent()), createdState,
                "yes")
            },
            withKlass: CreatedTextNode.self)),
      ])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(createCodeNode().append(createCodeHighlightNode("note")))
    }
    // Once highlighting has made its tokens.
    try editor.update(discrete: true) {
      for token in try getRoot().getAllTextNodes() {
        _ = try setState(token, flatState, "kept")
        _ = try setState(token, noteState, "kept")
      }
    }
    try editor.update(discrete: true) {
      _ = try getRoot().getFirstChildOrThrow().replace(createParagraphNode(), includeChildren: true)
    }
    let nodes = try editor.read {
      try getRoot().getAllTextNodes().map {
        [
          $0.getType(), try getState($0, flatState), try getState($0, noteState),
          try getState($0, createdState),
        ]
      }
    }
    #expect(nodes == [["test-created-text", "kept", "kept", "yes"]])
  }
}

@Suite struct CodeHighlighterDefaultLanguageTests {
  private func buildNullDefaultEditor() throws -> Editor {
    try buildHighlightEditor(
      name: "prism-default-null", TestCodeTokenizer().withDefaultLanguage(nil))
  }

  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismNullDefaultLanguage.test.ts::Prism defaultLanguage: null (#7235) > leaves `__language` unset and skips highlight mutation"
    ))
  func leavesTheLanguageUnset() throws {
    let editor = try buildNullDefaultEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createCodeNode().append(createTextNode("hello")))
    }
    #expect(try editor.read { try codeNode().getLanguage() } == nil)
  }

  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismNullDefaultLanguage.test.ts::Prism defaultLanguage: null (#7235) > splits text into CodeHighlightNode + LineBreakNode + TabNode for `\\n` / `\\t` so indent + line-move handlers stay compatible"
    ))
  func splitsTextWithoutALanguage() throws {
    let editor = try buildNullDefaultEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createCodeNode().append(createTextNode("a\n\tb")))
    }
    #expect(
      try editor.read { try codeNode().getChildren().map { $0.getType() } } == [
        "code-highlight", "linebreak", "tab", "code-highlight",
      ])
  }

  /// The default language is set on a block without one and highlighted.
  @Test func setsAndHighlightsTheDefaultLanguage() throws {
    let editor = try buildHighlightEditor(name: "default-language")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createCodeNode().append(createTextNode("let x = 10")))
    }
    try editor.read { () throws in
      #expect(try codeNode().getLanguage() == DEFAULT_CODE_LANGUAGE)
      #expect(try codeNode().getIsSyntaxHighlightSupported())
      #expect(
        try children() == [
          ["code-highlight", "let", "keyword"], ["code-highlight", " x ", nil],
          ["code-highlight", "=", "operator"], ["code-highlight", " ", nil],
          ["code-highlight", "10", "number"],
        ])
    }
  }

  /// An edit replaces only the tokens that changed.
  @Test func replacesOnlyTheChangedTokens() throws {
    let editor = try buildHighlightEditor(name: "diff")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createCodeNode("test").append(createTextNode("let x = 1")))
    }
    let before = try editor.read { try codeNode().getChildren().map { $0.getKey() } }
    try editor.update(discrete: true) {
      let last = try #require(try codeNode().getLastChild() as? TextNode)
      _ = try last.select(1, 1)
      try #require(try getSelection() as? RangeSelection).insertText("2")
    }
    let after = try editor.read { try codeNode().getChildren().map { $0.getKey() } }
    #expect(Array(after.dropLast()) == Array(before.dropLast()))
    #expect(try editor.read { try children().last } == ["code-highlight", "12", "number"])
  }
}

@Suite struct CodeHighlighterUnknownLanguageTests {
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismUnknownLanguage.test.ts::Prism with a language it does not know > splits the text into CodeHighlightNode + LineBreakNode + TabNode"
    ))
  func splitsTheText() throws {
    let editor = try buildHighlightEditor(name: "prism-unknown-language")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // What the markdown shortcut "```After " creates.
      try getRoot().clear().append(createCodeNode("After").append(createTextNode("a\n\tb")))
    }
    try editor.read { () throws in
      #expect(try codeNode().getLanguage() == "After")
      #expect(try !codeNode().getIsSyntaxHighlightSupported())
      // Plain text: no token type from the default language's grammar.
      #expect(
        try children() == [
          ["code-highlight", "a", nil], ["linebreak", "\n", nil], ["tab", "\t", nil],
          ["code-highlight", "b", nil],
        ])
    }
  }

  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/CodePrismUnknownLanguage.test.ts::Prism with a language it does not know > Tab over the whole block indents every line"
    ))
  func tabIndentsEveryLine() throws {
    let editor = try buildHighlightEditor(name: "prism-unknown-language")
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(createCodeNode("After").append(createTextNode("one\ntwo")))
    }
    // Select all, as the browser reports it: text point to text point.
    try editor.update(discrete: true) {
      let first = try #require(try codeNode().getFirstDescendant() as? TextNode)
      let last = try #require(try codeNode().getLastDescendant() as? TextNode)
      try first.select(0, 0).focus.set(last.getKey(), last.getTextContentSize(), .text)
    }
    _ = try editor.dispatchCommand(KEY_TAB_COMMAND, KeyboardEvent("keydown", key: "Tab"))
    #expect(try editor.read { try codeNode().getTextContent() } == "\tone\n\ttwo")
  }
}

/// A tokenizer whose `lazy` language loads when asked: `.loading` until
/// `finishLoading` calls back, as Shiki imports a grammar.
private final class LazyTokenizer: CodeTokenizer {
  let defaultLanguage: String? = nil
  var isLoaded = false
  var loads = 0
  private var waiting: [() -> Void] = []

  func languageState(_ language: String) -> CodeLanguageState {
    language == "lazy" ? (isLoaded ? .loaded : .loading) : .unsupported
  }

  func loadLanguage(_ language: String, onReady: @escaping () -> Void) {
    loads += 1
    waiting.append(onReady)
  }

  func finishLoading() {
    isLoaded = true
    waiting.forEach { $0() }
    waiting = []
  }

  func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    [.token(type: "lazy", content: [.text(code)])]
  }
}

@Suite struct CodeTokenizerContractTests {
  /// The first tokenizer that knows a language highlights it; a language
  /// none knows is plain.
  @Test func composedTokenizersFallBackPerLanguage() throws {
    let first = TestCodeTokenizer(defaultLanguage: nil, languages: ["test"])
    let lazy = LazyTokenizer()
    lazy.isLoaded = true
    let composed = ComposedCodeTokenizer([first, lazy])
    #expect(composed.defaultLanguage == nil)
    #expect(composed.languageState("test") == .loaded)
    #expect(composed.languageState("lazy") == .loaded)
    #expect(composed.languageState("other") == .unsupported)
    let editor = try buildHighlightEditor(name: "composed", composed)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(
        createCodeNode("test").append(createTextNode("let")),
        createCodeNode("lazy").append(createTextNode("let")),
        createCodeNode("other").append(createTextNode("let")))
    }
    let types = try editor.read {
      try getRoot().getChildren().compactMap { $0 as? CodeNode }.map {
        try $0.getChildren().map { ($0 as? CodeHighlightNode)?.__highlightType }
      }
    }
    #expect(types == [["keyword"], ["lazy"], [nil]])
  }

  /// A block in a language still loading stays as it is, and is
  /// highlighted once the tokenizer reports the grammar ready.
  @Test func highlightsALazyLanguageOnceLoaded() throws {
    let lazy = LazyTokenizer()
    let editor = try buildHighlightEditor(name: "lazy", lazy)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createCodeNode("lazy").append(createTextNode("a\nb")))
    }
    try editor.read { () throws in
      #expect(try children() == [["text", "a\nb", nil]])
      #expect(try !codeNode().getIsSyntaxHighlightSupported())
    }
    #expect(lazy.loads == 1)
    lazy.finishLoading()
    try editor.read { () throws in
      #expect(try codeNode().getIsSyntaxHighlightSupported())
      #expect(
        try children() == [
          ["code-highlight", "a", "lazy"], ["linebreak", "\n", nil],
          ["code-highlight", "b", "lazy"],
        ])
    }
  }

  /// Nested tokens type their text by the innermost token, a diff's
  /// prefix by its alias.
  @Test func mapsNestedTokens() throws {
    let editor = try createCodeTestEditor()
    defer { editor.dispose() }
    var nodes: [[String?]] = []
    try editor.update(discrete: true) {
      nodes = try mapTokensToLexicalStructure([
        .token(type: "function", content: [.token("punctuation", "("), .text("x\ty")]),
        .token(type: "prefix", content: [.text("+")], alias: "inserted"),
        .token(type: "unchanged", content: [.token("keyword", "if"), .text(" z")]),
      ]).map {
        [$0.getType(), try $0.getTextContent(), ($0 as? CodeHighlightNode)?.__highlightType]
      }
    }
    #expect(
      nodes == [
        ["code-highlight", "(", "punctuation"], ["code-highlight", "x", "function"],
        ["tab", "\t", nil], ["code-highlight", "y", "function"],
        ["code-highlight", "+", "inserted"], ["code-highlight", "if", "keyword"],
        ["code-highlight", " z", nil],
      ])
  }
}

/// A Tabs scenario: the text before and after, with `>` a line start, `-` a
/// tab and `|` the selection's ends, and the command.
struct TabScenario: Sendable, CustomTestStringConvertible {
  let before: String
  let after: String
  let operation: String
  let direction: CaretDirection
  var testDescription: String { "\(operation): \(before) => \(after) (\(direction))" }
}

private let tabSuite: [(String, String, String)] = [
  (">|", ">-|", "tab"), (">|", ">-|", "indent"), (">|", ">|", "outdent"),
  (">|line1", ">-|line1", "tab"), (">|line1", ">-|line1", "indent"),
  (">|line1", ">|line1", "outdent"), (">-li|ne1", ">-li-|ne1", "tab"),
  (">-li|ne1", ">--li|ne1", "indent"), (">-li|ne1", ">li|ne1", "outdent"),
  (">|-line1|", ">|--line1|", "tab"), (">|-line1|", ">|--line1|", "indent"),
  (">|-line1|", ">|line1|", "outdent"), (">-li|ne1|", ">-li-|", "tab"),
  (">-li|ne1|", ">--li|ne1|", "indent"), (">-li|ne1|", ">li|ne1|", "outdent"),
  (">|word1| word2", ">-| word2", "tab"), (">|line1>|", ">|-line1>|", "tab"),
  (">|line1>|", ">|-line1>|", "indent"), (">|line1>|line2", ">|-line1>|line2", "tab"),
  (">|line1>|line2", ">|-line1>|line2", "indent"), (">line1|>line2|", ">-line1|>-line2|", "tab"),
  (">line1|>line2|", ">-line1|>-line2|", "indent"), (">|>li|ne1", ">|>-li|ne1", "tab"),
  (">|>li|ne1", ">|>-li|ne1", "indent"), (">|>|line1", ">|->|line1", "tab"),
  (">|>|line1", ">|->|line1", "indent"), (">|>>|line1", ">|>>|line1", "tab"),
  (">|>>|line1", ">|>>|line1", "indent"), (">|line1>li|ne2", ">|-line1>-li|ne2", "tab"),
  (">|line1>li|ne2", ">|-line1>-li|ne2", "indent"),
  (">|-line1>li|ne2", ">|line1>li|ne2", "outdent"),
  (">li|ne1>li|ne2", ">-li|ne1>-li|ne2", "tab"), (">li|ne1>li|ne2", ">-li|ne1>-li|ne2", "indent"),
  (">-li|ne1>>-li|ne2", ">--li|ne1>>--li|ne2", "tab"),
  (">-li|ne1>>-li|ne2", ">--li|ne1>>--li|ne2", "indent"),
  (">-li|ne1>>-li|ne2", ">li|ne1>>li|ne2", "outdent"),
  (">|-line1>->-line2|", ">|--line1>-->--line2|", "tab"),
]

private let tabScenarios = tabSuite.flatMap { before, after, operation in
  [CaretDirection.next, .previous].map {
    TabScenario(before: before, after: after, operation: operation, direction: $0)
  }
}

private func rawTextWithSelection(_ input: String) -> String {
  String(input.dropFirst()).replacingOccurrences(of: ">", with: "\n")
    .replacingOccurrences(of: "-", with: "\t")
}

private func readable(_ text: String) -> String {
  text.replacingOccurrences(of: "\t", with: "-").replacingOccurrences(of: "\n", with: ">")
}

@Suite struct CodeHighlighterTabsTests {
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >| => >-| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >| => >-| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >| => >-| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >| => >-| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >| => >| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >| => >| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1 => >-|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1 => >-|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1 => >-|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1 => >-|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|line1 => >|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|line1 => >|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1 => >-li-|ne1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1 => >-li-|ne1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1 => >--li|ne1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1 => >--li|ne1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1 => >li|ne1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1 => >li|ne1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1| => >|--line1| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1| => >|--line1| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|-line1| => >|--line1| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|-line1| => >|--line1| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1| => >|line1| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1| => >|line1| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1| => >-li-| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1| => >-li-| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1| => >--li|ne1| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1| => >--li|ne1| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1| => >li|ne1| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1| => >li|ne1| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|word1| word2 => >-| word2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|word1| word2 => >-| word2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>| => >|-line1>| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>| => >|-line1>| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>| => >|-line1>| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>| => >|-line1>| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>|line2 => >|-line1>|line2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>|line2 => >|-line1>|line2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>|line2 => >|-line1>|line2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>|line2 => >|-line1>|line2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >line1|>line2| => >-line1|>-line2| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >line1|>line2| => >-line1|>-line2| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >line1|>line2| => >-line1|>-line2| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >line1|>line2| => >-line1|>-line2| (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>li|ne1 => >|>-li|ne1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>li|ne1 => >|>-li|ne1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>li|ne1 => >|>-li|ne1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>li|ne1 => >|>-li|ne1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>|line1 => >|->|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>|line1 => >|->|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>|line1 => >|->|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>|line1 => >|->|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>>|line1 => >|>>|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>>|line1 => >|>>|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>>|line1 => >|>>|line1 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>>|line1 => >|>>|line1 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>li|ne2 => >|-line1>-li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>li|ne2 => >|-line1>-li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>li|ne2 => >|-line1>-li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>li|ne2 => >|-line1>-li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1>li|ne2 => >|line1>li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1>li|ne2 => >|line1>li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1>>-li|ne2 => >li|ne1>>li|ne2 (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1>>-li|ne2 => >li|ne1>>li|ne2 (backwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1>->-line2| => >|--line1>-->--line2| (forwards)"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1>->-line2| => >|--line1>-->--line2| (backwards)"
    ),
    arguments: tabScenarios)
  func tabs(_ scenario: TabScenario) throws {
    let editor = try createCodeTestEditor()
    defer { editor.dispose() }
    _ = try registerRichText(editor)
    _ = try registerTabIndentation(editor)
    _ = try registerCodeHighlighting(editor, TestCodeTokenizer())
    let rawText = rawTextWithSelection(scenario.before)
    // Fill the editor with the scenario's text.
    try editor.update(discrete: true) {
      let code = try createCodeNode()
      try getRoot().append(code)
      _ = try code.selectStart()
      try #require(try getSelection()).insertRawText(
        rawText.replacingOccurrences(of: "|", with: ""))
    }
    // A selection where the scenario's `|`s are.
    try editor.update(discrete: true) {
      let code = try codeNode()
      let selection = createRangeSelection()
      if try code.isEmpty() {
        try selection.anchor.set(code.getKey(), 0, .element)
        try selection.focus.set(code.getKey(), 0, .element)
      } else {
        let units = Array(rawText.utf16)
        let bar = "|".utf16.first!
        let selFirst = units.firstIndex(of: bar)!
        var selLast = units.lastIndex(of: bar)!
        // Past the first `|`, which the text does not have.
        if selLast != selFirst { selLast -= 1 }
        var matching = try code.getFirstChild()
        var parentIndex = 0
        var offset = 0
        while let node = matching, try offset + node.getTextContentSize() < selFirst {
          offset += try node.getTextContentSize()
          matching = try node.getNextSibling()
          parentIndex += 1
        }
        if let node = matching, !(node is LineBreakNode) {
          try selection.anchor.set(node.getKey(), selFirst - offset, .text)
        } else {
          try selection.anchor.set(code.getKey(), matching == nil ? 0 : parentIndex, .element)
        }
        while let node = matching, try offset + node.getTextContentSize() < selLast {
          offset += try node.getTextContentSize()
          matching = try node.getNextSibling()
          parentIndex += 1
        }
        if let node = matching, !(node is LineBreakNode) {
          try selection.focus.set(node.getKey(), selLast - offset, .text)
        } else {
          try selection.focus.set(code.getKey(), matching == nil ? 0 : parentIndex + 1, .element)
        }
      }
      _ = try setSelectionFromCaretRange(
        getCaretRangeInDirection(caretRangeFromSelection(selection), scenario.direction))
      switch scenario.operation {
      case "indent": _ = try editor.dispatchCommand(INDENT_CONTENT_COMMAND)
      case "outdent": _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND)
      default: _ = try editor.dispatchCommand(KEY_TAB_COMMAND, KeyboardEvent("keydown", key: "Tab"))
      }
    }
    try editor.read { () throws in
      let output = scenario.after.replacingOccurrences(of: "|", with: "")
      let parts = scenario.after.split(separator: "|", omittingEmptySubsequences: false)
      let selectedOutput = parts.count == 3 ? String(parts[1]) : ""
      #expect(try ">" + readable(codeNode().getTextContent()) == output)
      let selection = try #require(try getSelection())
      #expect(try readable(selection.getTextContent()) == selectedOutput)
      let isBackward = try selection.isBackward()
      if scenario.direction == .next {
        #expect(!isBackward)
      } else if parts.count == 3 {
        #expect(isBackward)
      }
    }
  }
}

/// `$runOutdentScenario` of lexical-code-core's outdentTestUtils: a code
/// block of `rawText`, a caret at `cursorOffset`, then OUTDENT; the text
/// and the caret's offset in the block after it.
private func runOutdentScenario(
  _ editor: Editor, _ rawText: String, _ cursorOffset: Int
) throws -> (text: String, cursor: Int) {
  try editor.update(discrete: true) {
    let code = try createCodeNode()
    try getRoot().clear().append(code)
    try code.selectStart().insertRawText(rawText)
  }
  try editor.update(discrete: true) {
    let code = try codeNode()
    var matching = try code.getFirstChild()
    var offset = 0
    while let node = matching, try offset + node.getTextContentSize() < cursorOffset {
      offset += try node.getTextContentSize()
      matching = try node.getNextSibling()
    }
    let point: PointCaret =
      if let node = matching as? TextNode {
        try getTextPointCaret(node, .next, cursorOffset - offset)
      } else if let node = matching {
        getSiblingCaret(node, .next)
      } else {
        getChildCaret(code, .next)
      }
    _ = try setSelectionFromCaretRange(getCollapsedCaretRange(point))
    _ = try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND)
  }
  return try editor.read { () throws in
    let selection = try #require(try getSelection() as? RangeSelection)
    try #require(selection.focus.type == .text)
    var cursor = 0
    var node = try codeNode().getFirstChild()
    while let current = node, current is TextNode || current is LineBreakNode {
      if selection.focus.key == current.getKey() {
        cursor += selection.focus.offset
        break
      }
      cursor += try current.getTextContentSize()
      node = try current.getNextSibling()
    }
    return (try codeNode().getTextContent(), cursor)
  }
}

struct OutdentScenario: Sendable, CustomTestStringConvertible {
  let name: String
  let rawText: String
  let cursorOffset: Int
  let tabSize: Int?
  let expectedText: String
  var expectedCursor: Int? = nil
  var testDescription: String { name }
}

@Suite struct CodeHighlighterOutdentTests {
  @Test(
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 strips two leading spaces"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 strips one indent level from …"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 best-effort: line with one le…"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=4 strips four leading spaces"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=4 best-effort: two leading spac…"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > no-op when line has no leading whitespa…"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > TabNode at the start takes precedence o…"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=undefined preserves existing Ta…"
    ),
    .upstream(
      "lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=0 is rejected by the guard (non…"
    ),
    arguments: [
      OutdentScenario(
        name: "tabSize=2 strips two leading spaces", rawText: "  hello", cursorOffset: 2,
        tabSize: 2, expectedText: "hello"),
      OutdentScenario(
        name: "tabSize=2 strips one indent level from four leading spaces", rawText: "    hello",
        cursorOffset: 4, tabSize: 2, expectedText: "  hello", expectedCursor: 2),
      OutdentScenario(
        name: "tabSize=2 best-effort: line with one leading space is fully stripped",
        rawText: " hello", cursorOffset: 1, tabSize: 2, expectedText: "hello"),
      OutdentScenario(
        name: "tabSize=4 strips four leading spaces", rawText: "    hello", cursorOffset: 4,
        tabSize: 4, expectedText: "hello"),
      OutdentScenario(
        name: "tabSize=4 best-effort: two leading spaces are fully stripped", rawText: "  hello",
        cursorOffset: 2, tabSize: 4, expectedText: "hello"),
      OutdentScenario(
        name: "no-op when line has no leading whitespace", rawText: "hello", cursorOffset: 0,
        tabSize: 2, expectedText: "hello"),
      OutdentScenario(
        name: "TabNode at the start takes precedence over space stripping", rawText: "\t  hello",
        cursorOffset: 1, tabSize: 2, expectedText: "  hello"),
      OutdentScenario(
        name: "tabSize=undefined preserves existing TabNode-only behavior", rawText: "  hello",
        cursorOffset: 2, tabSize: nil, expectedText: "  hello"),
      OutdentScenario(
        name: "tabSize=0 is rejected by the guard (non-positive)", rawText: "    hello",
        cursorOffset: 4, tabSize: 0, expectedText: "    hello"),
    ])
  func outdentsSpaceIndentedLines(_ scenario: OutdentScenario) throws {
    let editor = try buildHighlightEditor(name: "prism-outdent", tabSize: scenario.tabSize)
    defer { editor.dispose() }
    let (text, cursor) = try runOutdentScenario(editor, scenario.rawText, scenario.cursorOffset)
    #expect(text == scenario.expectedText)
    if let expectedCursor = scenario.expectedCursor { #expect(cursor == expectedCursor) }
  }
}
