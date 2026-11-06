import LexicalCodeTreeSitter
import TreeSitterJava

extension TreeSitterGrammar {
  /// Java, from the vendored grammar in Sources/TreeSitterJava, for the
  /// code languages `java`.
  public static let java = TreeSitterGrammar(
    languages: ["java"], language: tree_sitter_java(),
    highlights: tree_sitter_java_highlights())
}
