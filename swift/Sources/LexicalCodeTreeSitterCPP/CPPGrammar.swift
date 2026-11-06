import LexicalCodeTreeSitter
import TreeSitterCPP

extension TreeSitterGrammar {
  /// C++, from the vendored grammar in Sources/TreeSitterCPP, for the
  /// code languages `cpp`, `c++`.
  /// Its highlights follow C's, which it extends.
  public static let cpp = TreeSitterGrammar(
    languages: ["cpp", "c++"], language: tree_sitter_cpp(),
    highlights: tree_sitter_cpp_highlights())
}
