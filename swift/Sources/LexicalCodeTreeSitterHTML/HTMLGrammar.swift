import LexicalCodeTreeSitter
import TreeSitterHTML

extension TreeSitterGrammar {
  /// HTML, from the vendored grammar in Sources/TreeSitterHTML, for the
  /// code languages `html`.
  public static let html = TreeSitterGrammar(
    languages: ["html"], language: tree_sitter_html(),
    highlights: tree_sitter_html_highlights())
}
