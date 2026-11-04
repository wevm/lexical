import Lexical
import LexicalHTML

// Port of packages/lexical-link/src/LinkImportExtension.ts: the
// `DOMImportExtension` rules for links (`@experimental` upstream), which
// `LinkExtension` contributes.

nonisolated(unsafe) private let AnchorRule = defineImportRule(
  name: "@lexical/link/a", match: sel.tag("a")
) { ctx, el, _ in
  if el.textContent.isEmpty && el.children.isEmpty {
    return []
  }
  // No schema: block descendants of the `<a>` are lifted so each block
  // becomes a sibling at the link's level, with the link wrapping its
  // inline content.
  let href = el.getAttribute("href") ?? ""
  let attrs = LinkAttributes(
    rel: .some(el.getAttribute("rel")), target: .some(el.getAttribute("target")),
    title: .some(el.getAttribute("title")))
  return try distributeInlineWrapper(ctx.importChildren(el)) { try createLinkNode(href, attrs) }
}

/// `LinkImportRules` (`@experimental`): the import rule for `LinkNode`.
nonisolated(unsafe) public let LinkImportRules = [AnchorRule]
