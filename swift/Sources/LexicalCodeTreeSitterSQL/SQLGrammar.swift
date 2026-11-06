import LexicalCodeTreeSitter
import TreeSitterSQL

extension TreeSitterGrammar {
  /// SQL, from the vendored grammar in Sources/TreeSitterSQL, for the
  /// code languages `sql`.
  public static let sql = TreeSitterGrammar(
    languages: ["sql"], language: tree_sitter_sql(),
    highlights: tree_sitter_sql_highlights())
}
