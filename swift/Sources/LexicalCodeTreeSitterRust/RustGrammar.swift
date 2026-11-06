import LexicalCodeTreeSitter
import TreeSitterRust

extension TreeSitterGrammar {
  /// Rust, from the vendored grammar in Sources/TreeSitterRust, for the
  /// code languages `rust`, `rs`.
  public static let rust = TreeSitterGrammar(
    languages: ["rust", "rs"], language: tree_sitter_rust(),
    highlights: tree_sitter_rust_highlights())
}
