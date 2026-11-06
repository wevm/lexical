import LexicalCodeTreeSitter
import TreeSitterGo

extension TreeSitterGrammar {
  /// Go, from the vendored grammar in Sources/TreeSitterGo, for the
  /// code languages `go`, `golang`.
  public static let go = TreeSitterGrammar(
    languages: ["go", "golang"], language: tree_sitter_go(),
    highlights: tree_sitter_go_highlights())
}
