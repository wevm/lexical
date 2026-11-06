import LexicalCodeTreeSitter
import TreeSitterCSS

extension TreeSitterGrammar {
  /// CSS, from the vendored grammar in Sources/TreeSitterCSS, for the
  /// code languages `css`.
  public static let css = TreeSitterGrammar(
    languages: ["css"], language: tree_sitter_css(),
    highlights: tree_sitter_css_highlights())
}
