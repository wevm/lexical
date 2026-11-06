import LexicalCodeTreeSitter
import TreeSitterMarkdown

extension TreeSitterGrammar {
  /// Markdown, from the vendored grammar in Sources/TreeSitterMarkdown, for the
  /// code languages `markdown`, `md`.
  /// Its block grammar only: headings, fences, lists and quotes, not the
  /// inline grammar's emphasis, links and code spans.
  public static let markdown = TreeSitterGrammar(
    languages: ["markdown", "md"], language: tree_sitter_markdown(),
    highlights: tree_sitter_markdown_highlights())
}
