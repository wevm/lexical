import LexicalCodeTreeSitter
import TreeSitterJavaScript

extension TreeSitterGrammar {
  /// JavaScript, from the vendored grammar in Sources/TreeSitterJavaScript, for the
  /// code languages `js`, `javascript`, `jsx`.
  public static let javaScript = TreeSitterGrammar(
    languages: ["js", "javascript", "jsx"], language: tree_sitter_javascript(),
    highlights: tree_sitter_javascript_highlights())
}
