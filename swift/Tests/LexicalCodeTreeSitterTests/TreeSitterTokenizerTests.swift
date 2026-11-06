import Lexical
import LexicalCodeCore
import LexicalCodeTreeSitter
import LexicalCodeTreeSitterBash
import LexicalCodeTreeSitterCSS
import LexicalCodeTreeSitterGo
import LexicalCodeTreeSitterHTML
import LexicalCodeTreeSitterJSON
import LexicalCodeTreeSitterJavaScript
import LexicalCodeTreeSitterPython
import LexicalCodeTreeSitterRust
import LexicalCodeTreeSitterSwift
import LexicalCodeTreeSitterTypeScript
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing
import TreeSitterJavaScript

private let allGrammars: [TreeSitterGrammar] = [
  .bash, .css, .go, .html, .javaScript, .json, .python, .rust, .swift, .typeScript,
]

/// The typed tokens of `code`, as `type:text`.
private func typed(_ grammar: TreeSitterGrammar, _ code: String) -> [String] {
  grammar.tokenize(code).compactMap {
    if case .token(let type, let content, _) = $0, case .text(let text)? = content.first {
      return "\(type):\(text)"
    }
    return nil
  }
}

@Suite struct TreeSitterTokenizerTests {
  @Test(arguments: allGrammars)
  func highlightsQueryCompiles(_ grammar: TreeSitterGrammar) {
    #expect(grammar.isValid, "\(grammar.languages)")
  }

  /// A snippet of each language gets the Prism names the playground theme
  /// colors, and its text is kept.
  @Test(arguments: [
    (
      TreeSitterGrammar.javaScript, "const x = 'a'; // c",
      ["keyword:const", "string:'a'", "comment:// c"]
    ),
    (.typeScript, "let n: number = 1", ["keyword:let", "builtin:number", "number:1"]),
    (.swift, "func f() { return \"s\" }", ["keyword:func", "function:f", "keyword:return"]),
    (.python, "def f():\n  return 1", ["keyword:def", "function:f", "keyword:return", "number:1"]),
    (.json, "{\"a\": true}", ["constant:true"]),
    (.html, "<p class=\"x\">", ["tag:p", "attr:class"]),
    (.css, "a { color: red; }", ["tag:a", "property:color"]),
    (
      .rust, "fn main() { let x = 1; }",
      ["keyword:fn", "function:main", "keyword:let", "constant:1"]
    ),
    (.go, "func main() { return }", ["keyword:func", "function:main", "keyword:return"]),
    (.bash, "echo \"hi\" # c", ["function:echo", "string:\"hi\"", "comment:# c"]),
  ])
  func mapsCapturesToPrismNames(grammar: TreeSitterGrammar, code: String, expected: [String]) {
    let tokens = grammar.tokenize(code)
    let text = tokens.map {
      switch $0 {
      case .text(let text): text
      case .token(_, let content, _):
        content.map { if case .text(let t) = $0 { t } else { "" } }.joined()
      }
    }.joined()
    #expect(text == code)
    let got = typed(grammar, code)
    for token in expected { #expect(got.contains(token), "\(token) in \(got)") }
  }

  /// UTF-16 offsets: text after an emoji (a surrogate pair) is typed at the
  /// right place.
  @Test func keepsOffsetsAfterAnEmoji() {
    #expect(typed(.javaScript, "'😀'; let y") == ["string:'😀'", "punctuation:;", "keyword:let"])
  }

  /// Typing a program one character at a time, then deleting it from the
  /// middle, types every step as a fresh parse does, also where an edit
  /// retypes later lines (an open comment or string).
  @Test func incrementalHighlightingMatchesAFreshOne() {
    func fresh() -> TreeSitterGrammar {
      TreeSitterGrammar(
        languages: ["js"], language: tree_sitter_javascript(),
        highlights: tree_sitter_javascript_highlights())
    }
    let grammar = fresh()
    let program = "const A = 1;\n/* x\nlet y = 'z';\nfunction f(n) { return n }\n"
    var code = "let tail = 2;\nclass B {}"
    var steps = 0
    for character in program {
      code.insert(character, at: code.index(code.startIndex, offsetBy: steps))
      steps += 1
      #expect(grammar.tokenize(code) == fresh().tokenize(code), "\(code)")
    }
    for _ in 0..<20 {
      code.remove(at: code.index(code.startIndex, offsetBy: 14))
      #expect(grammar.tokenize(code) == fresh().tokenize(code), "\(code)")
    }
  }

  /// In an editor, a block's language picks its grammar; one without a
  /// grammar is plain, and one grammar falls back to another tokenizer.
  @Test func highlightsCodeBlocksInAnEditor() throws {
    let tokenizer = ComposedCodeTokenizer([TreeSitterTokenizer(allGrammars), PlainCodeTokenizer()])
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "tree-sitter",
        dependencies: [
          RichTextExtension, configExtension(CodeHighlighterExtension) { $0.tokenizer = tokenizer },
        ]))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createCodeNode("py").append(createTextNode("def f(): pass")),
        createCodeNode("cobol").append(createTextNode("MOVE A TO B")))
    }
    let types = try editor.read {
      try getRoot().getChildren().map {
        try ($0 as! CodeNode).getChildren().compactMap {
          ($0 as? CodeHighlightNode)?.__highlightType
        }
      }
    }
    #expect(types[0].contains("keyword"))
    #expect(types[0].contains("function"))
    #expect(types[1].isEmpty)
  }
}
