import Lexical
import LexicalHTML

// Port of packages/lexical-rich-text/src/RichTextImportExtension.ts: the
// `DOMImportExtension` rules for headings and quotes (`@experimental`
// upstream).

/// Google Docs writes the document title as `<span style="font-size:
/// 26pt">`.
private func isGoogleDocsTitleSpan(_ node: DOMNode) -> Bool {
  guard let element = node as? DOMElement, element.nodeName == "SPAN" else { return false }
  return element.style.getPropertyValue("font-size") == "26pt"
}

nonisolated(unsafe) private let HeadingRule = defineImportRule(
  name: "@lexical/rich-text/heading", match: sel.tag("h1", "h2", "h3", "h4", "h5", "h6")
) { ctx, el, _ in
  let node = try createHeadingNode(el.nodeName.lowercased())
  try setNodeIndentFromDOM(el, node)
  try setFormatFromDOM(node, el)
  try setDirectionFromDOM(node, el)
  return [try node.splice(0, 0, ctx.importChildren(el))]
}

nonisolated(unsafe) private let QuoteRule = defineImportRule(
  name: "@lexical/rich-text/blockquote", match: sel.tag("blockquote")
) { ctx, el, _ in
  let node = try createQuoteNode()
  try setFormatFromDOM(node, el)
  try setNodeIndentFromDOM(el, node)
  try setDirectionFromDOM(node, el)
  return [try node.splice(0, 0, ctx.importChildren(el))]
}

/// `ShadowRootQuoteRule`: an opt-in `<blockquote>` rule that imports a
/// shadow root `QuoteNode`, keeping block children as blocks
/// (`BlockSchema`). Contribute it from somewhere that outranks
/// `RichTextImportRules`, such as the editor builder or an extension that
/// depends on `RichTextExtension`.
nonisolated(unsafe) public let ShadowRootQuoteRule = defineImportRule(
  name: "@lexical/rich-text/blockquote-shadow-root", match: sel.tag("blockquote")
) { ctx, el, _ in
  let node = try createQuoteNode(shadowRoot: true)
  try setFormatFromDOM(node, el)
  try setNodeIndentFromDOM(el, node)
  try setDirectionFromDOM(node, el)
  return [try node.splice(0, 0, ctx.importChildren(el, ImportChildrenOpts(schema: BlockSchema)))]
}

/// A Google Docs `<p>` around a title span: its children unwrapped, so the
/// span rule makes the heading at this level.
nonisolated(unsafe) private let GoogleDocsTitleParagraphRule = defineImportRule(
  name: "@lexical/rich-text/google-docs-title-p", match: sel.tag("p")
) { ctx, el, next in
  if let first = el.firstChild, isGoogleDocsTitleSpan(first) {
    return try ctx.importChildren(el)
  }
  return try next()
}

nonisolated(unsafe) private let GoogleDocsTitleSpanRule = defineImportRule(
  name: "@lexical/rich-text/google-docs-title-span", match: sel.tag("span")
) { ctx, el, next in
  if el.style.getPropertyValue("font-size") != "26pt" { return try next() }
  return [try createHeadingNode("h1").splice(0, 0, ctx.importChildren(el))]
}

/// `RichTextImportRules`: headings, quotes and the Google Docs title, which
/// `RichTextExtension` contributes in front of `CoreImportRules`.
nonisolated(unsafe) public let RichTextImportRules = [
  HeadingRule, QuoteRule, GoogleDocsTitleParagraphRule, GoogleDocsTitleSpanRule,
]
