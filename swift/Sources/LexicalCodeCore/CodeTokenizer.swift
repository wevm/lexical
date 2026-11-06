import Lexical

// The tokenizer interface of packages/lexical-code-prism and
// packages/lexical-code-shiki (`Tokenizer`), without a grammar engine:
// `CodeHighlighterExtension` runs the highlighting loop over any
// `CodeTokenizer`, the way upstream lets Shiki stand in for Prism.
//
// Upstream's two engines differ in how a language becomes available. Prism
// has its grammars loaded or not (`isCodeLanguageLoaded`). Shiki downloads
// one on demand (`loadCodeLanguage`) and highlights the block again once
// it has loaded. `CodeLanguageState` covers both.

/// A token of code, as Prism's `tokenize` returns them: text without a
/// type, or a typed token whose content is text and nested tokens.
public indirect enum CodeToken: Equatable, Sendable {
  case text(String)
  /// A token of `type`, a Prism token name such as `keyword`, `string`,
  /// `comment`, `function`, `class-name`, `number`, `operator` or
  /// `punctuation`. `alias` is the type a `prefix` token of a diff gives
  /// its text.
  case token(type: String, content: [CodeToken], alias: String? = nil)

  /// A token of `type` holding `text`.
  public static func token(_ type: String, _ text: String) -> CodeToken {
    .token(type: type, content: [.text(text)])
  }
}

/// Whether a tokenizer can highlight a language.
public enum CodeLanguageState: Equatable, Sendable {
  /// It has the language's grammar (`isCodeLanguageLoaded`).
  case loaded
  /// It has the grammar, but not yet: `loadLanguage` gets it, as Shiki's
  /// `loadCodeLanguage` imports a grammar.
  case loading
  /// It doesn't know the language. The block is plain text.
  case unsupported
}

/// `Tokenizer` of the code highlighters: what `CodeHighlighterExtension`
/// asks a highlighting engine.
public protocol CodeTokenizer {
  /// The language of a code block without one (`defaultLanguage`), which
  /// the highlighter sets on the block. `nil` opts out: such a block keeps
  /// no language and is split into plain tokens, so a Markdown round trip
  /// keeps a fence without an info string.
  var defaultLanguage: String? { get }

  /// Whether the tokenizer can highlight `language` now.
  func languageState(_ language: String) -> CodeLanguageState

  /// Loads `language`, whose state is `.loading`, and calls `onReady` once
  /// it is `.loaded`, on the thread that owns the editor; the highlighter
  /// then highlights the blocks waiting for it. The default does nothing,
  /// for a tokenizer whose languages are all loaded or unsupported.
  func loadLanguage(_ language: String, onReady: @escaping () -> Void)

  /// `tokenize(code, language)`: `code` as tokens, for a loaded `language`.
  func tokenize(_ code: String, _ language: String) -> [CodeToken]

  /// `$tokenize(codeNode, language)`: the children that highlight
  /// `codeNode`, CodeHighlightNodes, TabNodes and LineBreakNodes, for a
  /// tokenizer that builds them itself. The default, `nil`, has the
  /// highlighter map `tokenize` of the block's text (or plain tokens without
  /// a language) and create nodes only for the tokens that changed.
  func tokenize(codeNode: CodeNode, _ language: String?) throws -> [LexicalNode]?
}

extension CodeTokenizer {
  public func loadLanguage(_ language: String, onReady: @escaping () -> Void) {}

  public func tokenize(codeNode: CodeNode, _ language: String?) throws -> [LexicalNode]? { nil }

  /// This tokenizer with `defaultLanguage` replaced, as upstream spreads
  /// `{...PrismTokenizer, defaultLanguage: null}`.
  public func withDefaultLanguage(_ defaultLanguage: String?) -> any CodeTokenizer {
    DefaultLanguageCodeTokenizer(base: self, defaultLanguage: defaultLanguage)
  }
}

/// A child of a highlighted code block: a token's text and type, a tab or
/// a line break.
enum CodeRun: Equatable {
  case text(String, String?)
  case tab
  case linebreak

  /// Whether `node` is this run, as the highlighter's `isEqual` compares
  /// children: a CodeHighlightNode of the same text and type, a tab or a
  /// line break.
  func matches(_ node: LexicalNode) -> Bool {
    switch self {
    case .text(let text, let type):
      guard let node = node as? CodeHighlightNode else { return false }
      return node.__text == text && node.__highlightType == type
    case .tab: return node is TabNode
    case .linebreak: return node is LineBreakNode
    }
  }

  func createNode() throws -> LexicalNode {
    switch self {
    case .text(let text, let type): try createCodeHighlightNode(text, type)
    case .tab: try createTabNode()
    case .linebreak: try createLineBreakNode()
    }
  }
}

/// The runs of `tokens`, each text run typed by its innermost token (a
/// diff's `prefix` by its alias, `unchanged` by none).
func codeRuns(_ tokens: [CodeToken], _ type: String? = nil) throws -> [CodeRun] {
  var runs: [CodeRun] = []
  runs.reserveCapacity(tokens.count)
  try addCodeRuns(tokens, type, &runs)
  return runs
}

private func addCodeRuns(_ tokens: [CodeToken], _ type: String?, _ runs: inout [CodeRun]) throws {
  for token in tokens {
    switch token {
    case .text(let text):
      // Most tokens hold no tab or line break; they skip the split, which
      // costs a UTF-16 copy per token.
      if !text.utf8.contains(where: { $0 == 0x0A || $0 == 0x0D || $0 == 0x09 }) {
        if !text.isEmpty { runs.append(.text(text, type)) }
        continue
      }
      var split: [CodeRun] = []
      try tokenizeRawText(
        text,
        RawTextVisitor(
          linebreak: { split.append(.linebreak) }, tab: { split.append(.tab) },
          text: { split.append(.text($0, type)) }))
      runs.append(contentsOf: split)
    case .token(let tokenType, let content, let alias):
      if content.count == 1, case .text = content[0] {
        try addCodeRuns(content, tokenType == "prefix" && alias != nil ? alias : tokenType, &runs)
      } else {
        try addCodeRuns(content, tokenType == "unchanged" ? nil : tokenType, &runs)
      }
    }
  }
}

/// `$mapTokensToLexicalStructure(tokens)`: the CodeHighlightNodes, TabNodes
/// and LineBreakNodes of `tokens`, each text run typed by its innermost
/// token (a diff's `prefix` by its alias, `unchanged` by none).
public func mapTokensToLexicalStructure(
  _ tokens: [CodeToken], _ type: String? = nil
) throws -> [LexicalNode] {
  try codeRuns(tokens, type).map { try $0.createNode() }
}

/// A tokenizer that knows no language: every code block is split into
/// plain tokens, so the indent and line-move handlers can work on it.
public struct PlainCodeTokenizer: CodeTokenizer {
  public let defaultLanguage: String?

  public init(defaultLanguage: String? = nil) { self.defaultLanguage = defaultLanguage }

  public func languageState(_ language: String) -> CodeLanguageState { .unsupported }

  public func tokenize(_ code: String, _ language: String) -> [CodeToken] { [.text(code)] }
}

/// Tokenizers tried in order for each language: the first that knows the
/// language (loaded or loading) highlights it, so a tokenizer can fall
/// back to another for the languages it lacks.
public struct ComposedCodeTokenizer: CodeTokenizer {
  public let tokenizers: [any CodeTokenizer]
  public let defaultLanguage: String?

  /// `defaultLanguage` defaults to the first tokenizer's.
  public init(_ tokenizers: [any CodeTokenizer], defaultLanguage: String?? = nil) {
    self.tokenizers = tokenizers
    self.defaultLanguage = defaultLanguage ?? tokenizers.first?.defaultLanguage
  }

  /// The tokenizer for `language`, with its state.
  private func select(_ language: String) -> (any CodeTokenizer, CodeLanguageState)? {
    for tokenizer in tokenizers {
      let state = tokenizer.languageState(language)
      if state != .unsupported { return (tokenizer, state) }
    }
    return nil
  }

  public func languageState(_ language: String) -> CodeLanguageState {
    select(language)?.1 ?? .unsupported
  }

  public func loadLanguage(_ language: String, onReady: @escaping () -> Void) {
    select(language)?.0.loadLanguage(language, onReady: onReady)
  }

  public func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    select(language)?.0.tokenize(code, language) ?? [.text(code)]
  }

  public func tokenize(codeNode: CodeNode, _ language: String?) throws -> [LexicalNode]? {
    guard let language = nonEmpty(language) ?? defaultLanguage,
      let (tokenizer, _) = select(language)
    else { return nil }
    return try tokenizer.tokenize(codeNode: codeNode, language)
  }
}

private struct DefaultLanguageCodeTokenizer: CodeTokenizer {
  let base: any CodeTokenizer
  let defaultLanguage: String?

  func languageState(_ language: String) -> CodeLanguageState { base.languageState(language) }

  func loadLanguage(_ language: String, onReady: @escaping () -> Void) {
    base.loadLanguage(language, onReady: onReady)
  }

  func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    base.tokenize(code, language)
  }

  func tokenize(codeNode: CodeNode, _ language: String?) throws -> [LexicalNode]? {
    guard let language = nonEmpty(language) ?? defaultLanguage else { return nil }
    return try base.tokenize(codeNode: codeNode, language)
  }
}

/// `language || …`: an empty language is none, as JS reads `""`.
func nonEmpty(_ language: String?) -> String? {
  language?.isEmpty == false ? language : nil
}
