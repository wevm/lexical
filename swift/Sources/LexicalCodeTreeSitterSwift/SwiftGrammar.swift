import LexicalCodeTreeSitter
import TreeSitterSwift

extension TreeSitterGrammar {
  /// Swift, from the vendored grammar in Sources/TreeSitterSwift, for the
  /// code languages `swift`.
  public static let swift = TreeSitterGrammar(
    languages: ["swift"], language: tree_sitter_swift(),
    highlights: tree_sitter_swift_highlights())
}
