import LexicalCodeTreeSitter
import TreeSitterC

extension TreeSitterGrammar {
  /// C, from the vendored grammar in Sources/TreeSitterC, for the
  /// code languages `c`.
  public static let c = TreeSitterGrammar(
    languages: ["c"], language: tree_sitter_c(),
    highlights: tree_sitter_c_highlights())
}
