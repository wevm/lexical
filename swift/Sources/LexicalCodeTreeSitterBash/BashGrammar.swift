import LexicalCodeTreeSitter
import TreeSitterBash

extension TreeSitterGrammar {
  /// Bash, from the vendored grammar in Sources/TreeSitterBash, for the
  /// code languages `bash`, `sh`, `shell`, `zsh`.
  public static let bash = TreeSitterGrammar(
    languages: ["bash", "sh", "shell", "zsh"], language: tree_sitter_bash(),
    highlights: tree_sitter_bash_highlights())
}
