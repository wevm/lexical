import LexicalCodeTreeSitter
import TreeSitterPython

extension TreeSitterGrammar {
  /// Python, from the vendored grammar in Sources/TreeSitterPython, for the
  /// code languages `py`, `python`.
  public static let python = TreeSitterGrammar(
    languages: ["py", "python"], language: tree_sitter_python(),
    highlights: tree_sitter_python_highlights())
}
