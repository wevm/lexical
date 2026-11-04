import Lexical

// Port of packages/lexical-markdown/src/index.ts.

/// `$convertFromMarkdownString(markdown, transformers, node,
/// shouldPreserveNewLines, shouldMergeAdjacentLines)`: replaces the
/// children of `node` (the root by default) with `markdown`'s.
public func convertFromMarkdownString(
  _ markdown: String, _ transformers: [any Transformer] = TRANSFORMERS,
  _ node: ElementNode? = nil, _ shouldPreserveNewLines: Bool = false,
  _ shouldMergeAdjacentLines: Bool = false
) throws {
  let sanitizedMarkdown =
    shouldPreserveNewLines ? markdown : normalizeMarkdown(markdown, shouldMergeAdjacentLines)
  let root = try node ?? getRoot()
  try root.clear()
  try importMarkdownNodes(sanitizedMarkdown, root, transformers, shouldPreserveNewLines)
  if try getSelection() != nil {
    try root.selectStart()
  }
}

/// `$generateNodesFromMarkdownString(markdown, transformers,
/// shouldPreserveNewLines, shouldMergeAdjacentLines)`: `markdown`'s nodes,
/// not attached to the document.
public func generateNodesFromMarkdownString(
  _ markdown: String, _ transformers: [any Transformer] = TRANSFORMERS,
  _ shouldPreserveNewLines: Bool = false, _ shouldMergeAdjacentLines: Bool = false
) throws -> [LexicalNode] {
  let sanitizedMarkdown =
    shouldPreserveNewLines ? markdown : normalizeMarkdown(markdown, shouldMergeAdjacentLines)
  let container = try ArtificialNode__DO_NOT_USE()
  try importMarkdownNodes(sanitizedMarkdown, container, transformers, shouldPreserveNewLines)
  return try container.getChildren()
}

/// `$convertToMarkdownString(transformers, node, shouldPreserveNewLines)`:
/// `node` (the root by default) as markdown.
public func convertToMarkdownString(
  _ transformers: [any Transformer] = TRANSFORMERS, _ node: ElementNode? = nil,
  _ shouldPreserveNewLines: Bool = false
) throws -> String {
  try createMarkdownExport(transformers, shouldPreserveNewLines)(node)
}

/// `$convertSelectionToMarkdownString(transformers, selection,
/// shouldPreserveNewLines)`: the selected content as markdown, or `""` for
/// no selection or a collapsed range.
public func convertSelectionToMarkdownString(
  _ transformers: [any Transformer] = TRANSFORMERS, _ selection: (any BaseSelection)?,
  _ shouldPreserveNewLines: Bool = false
) throws -> String {
  guard let selection else { return "" }
  if let range = selection as? RangeSelection, range.isCollapsed() { return "" }
  return try createSelectionMarkdownExport(transformers, shouldPreserveNewLines)(selection)
}
