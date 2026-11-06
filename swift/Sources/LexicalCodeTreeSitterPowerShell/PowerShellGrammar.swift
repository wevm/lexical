import LexicalCodeTreeSitter
import TreeSitterPowerShell

extension TreeSitterGrammar {
  /// PowerShell, from the vendored grammar in Sources/TreeSitterPowerShell, for the
  /// code languages `powershell`, `ps1`.
  public static let powerShell = TreeSitterGrammar(
    languages: ["powershell", "ps1"], language: tree_sitter_powershell(),
    highlights: tree_sitter_powershell_highlights())
}
