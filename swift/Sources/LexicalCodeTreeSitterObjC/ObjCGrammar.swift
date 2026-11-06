import LexicalCodeTreeSitter
import TreeSitterObjC

extension TreeSitterGrammar {
  /// Objective-C, from the vendored grammar in Sources/TreeSitterObjC, for the
  /// code languages `objc`, `objectivec`.
  /// Its highlights follow C's, which it extends.
  public static let objC = TreeSitterGrammar(
    languages: ["objc", "objectivec"], language: tree_sitter_objc(),
    highlights: tree_sitter_objc_highlights())
}
