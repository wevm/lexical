import LexicalCodeTreeSitter
import TreeSitterJSON

extension TreeSitterGrammar {
  /// JSON, from the vendored grammar in Sources/TreeSitterJSON, for the
  /// code languages `json`.
  public static let json = TreeSitterGrammar(
    languages: ["json"], language: tree_sitter_json(),
    highlights: tree_sitter_json_highlights())
}
