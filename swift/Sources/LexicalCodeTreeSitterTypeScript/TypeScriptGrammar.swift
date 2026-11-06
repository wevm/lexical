import LexicalCodeTreeSitter
import TreeSitterTypeScript

extension TreeSitterGrammar {
  /// TypeScript, from the vendored grammar in Sources/TreeSitterTypeScript, for the
  /// code languages `typescript`, `ts`.
  public static let typeScript = TreeSitterGrammar(
    languages: ["typescript", "ts"], language: tree_sitter_typescript(),
    highlights: tree_sitter_typescript_highlights())
}
