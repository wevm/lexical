import LexicalCodeTreeSitter
import TreeSitterXML

extension TreeSitterGrammar {
  /// XML, from the vendored grammar in Sources/TreeSitterXML, for the
  /// code languages `xml`.
  public static let xml = TreeSitterGrammar(
    languages: ["xml"], language: tree_sitter_xml(),
    highlights: tree_sitter_xml_highlights())
}
