import Lexical
import LexicalExtension

// Port of packages/lexical-html/src/import/coreImportRules.ts: the
// `DOMImportExtension` rules for paragraphs, text, line breaks, the inline
// format tags, `<hr>` and transparent blocks (`@experimental` upstream).

/// A text format's bits to set and to clear when descending into an
/// element.
private struct FormatOverride {
  var set = 0
  var clear = 0
}

/// The inline style properties that affect text format.
private struct FormatStyle {
  var fontWeight = ""
  var fontStyle = ""
  var textDecoration = ""
  var verticalAlign = ""
  var textTransform = ""
}

/// The style each inline format tag implies.
private let TAG_DEFAULT_STYLE: [String: FormatStyle] = [
  "B": FormatStyle(fontWeight: "bold"),
  "EM": FormatStyle(fontStyle: "italic"),
  "I": FormatStyle(fontStyle: "italic"),
  "S": FormatStyle(textDecoration: "line-through"),
  "STRONG": FormatStyle(fontWeight: "bold"),
  "SUB": FormatStyle(verticalAlign: "sub"),
  "SUP": FormatStyle(verticalAlign: "super"),
  "U": FormatStyle(textDecoration: "underline"),
]

/// Tags whose format has no CSS analog.
private let TAG_ONLY_SET: [String: Int] = ["CODE": IS_CODE, "MARK": IS_HIGHLIGHT]

private func readElementFormatStyle(_ el: DOMElement) -> FormatStyle {
  let style = el.style
  return FormatStyle(
    fontWeight: style.getPropertyValue("font-weight"),
    fontStyle: style.getPropertyValue("font-style"),
    textDecoration: style.getPropertyValue("text-decoration"),
    verticalAlign: style.getPropertyValue("vertical-align"),
    textTransform: style.getPropertyValue("text-transform"))
}

private func mergeStyles(_ defaults: FormatStyle, _ override: FormatStyle) -> FormatStyle {
  func pick(_ a: String, _ b: String) -> String { a.isEmpty ? b : a }
  return FormatStyle(
    fontWeight: pick(override.fontWeight, defaults.fontWeight),
    fontStyle: pick(override.fontStyle, defaults.fontStyle),
    textDecoration: pick(override.textDecoration, defaults.textDecoration),
    verticalAlign: pick(override.verticalAlign, defaults.verticalAlign),
    textTransform: pick(override.textTransform, defaults.textTransform))
}

/// The properties `ImportTextFormat` owns, which `ImportTextStyle` does not
/// write onto text nodes.
private let FORMAT_BIT_STYLE_PROPS: Set<String> = [
  "font-weight", "font-style", "text-decoration", "text-transform", "vertical-align",
]

private func styleFormatOverride(_ style: FormatStyle) -> FormatOverride {
  var o = FormatOverride()
  switch style.fontWeight {
  case "700", "bold": o.set |= IS_BOLD
  case "normal", "400": o.clear |= IS_BOLD
  default: break
  }
  if style.fontStyle == "italic" {
    o.set |= IS_ITALIC
  } else if style.fontStyle == "normal" {
    o.clear |= IS_ITALIC
  }
  if !style.textDecoration.isEmpty {
    let parts = style.textDecoration.split(separator: " ", omittingEmptySubsequences: false)
    if parts.contains("underline") { o.set |= IS_UNDERLINE }
    if parts.contains("line-through") { o.set |= IS_STRIKETHROUGH }
    if parts.contains("none") { o.clear |= IS_UNDERLINE | IS_STRIKETHROUGH }
  }
  // TextNode.exportDOM writes one of these for the capitalization formats,
  // which are mutually exclusive (#8915).
  switch style.textTransform {
  case "lowercase":
    o.set |= IS_LOWERCASE
    o.clear |= IS_UPPERCASE | IS_CAPITALIZE
  case "uppercase":
    o.set |= IS_UPPERCASE
    o.clear |= IS_LOWERCASE | IS_CAPITALIZE
  case "capitalize":
    o.set |= IS_CAPITALIZE
    o.clear |= IS_LOWERCASE | IS_UPPERCASE
  case "none":
    o.clear |= IS_LOWERCASE | IS_UPPERCASE | IS_CAPITALIZE
  default: break
  }
  switch style.verticalAlign {
  case "sub":
    o.set |= IS_SUBSCRIPT
    o.clear |= IS_SUPERSCRIPT
  case "super":
    o.set |= IS_SUPERSCRIPT
    o.clear |= IS_SUBSCRIPT
  case "baseline":
    o.clear |= IS_SUBSCRIPT | IS_SUPERSCRIPT
  default: break
  }
  return o
}

private func applyFormatOverride(_ format: Int, _ ov: FormatOverride) -> Int {
  (format & ~ov.clear) | ov.set
}

/// The inline format tags and `<span>`: the tag's implied style merged
/// with the element's inline style, as bits to set and clear.
nonisolated(unsafe) private let InlineFormatRule = defineImportRule(
  name: "@lexical/html/inline-format",
  match: sel.tag("b", "strong", "em", "i", "code", "mark", "s", "sub", "sup", "u", "span")
) { ctx, el, _ in
  let inherited = try ctx.get(ImportTextFormat)
  let elStyle = readElementFormatStyle(el)
  let effective = TAG_DEFAULT_STYLE[el.nodeName].map { mergeStyles($0, elStyle) } ?? elStyle
  var merged = applyFormatOverride(inherited, styleFormatOverride(effective))
  if let tagOnly = TAG_ONLY_SET[el.nodeName] { merged |= tagOnly }
  if merged == inherited { return try ctx.importChildren(el) }
  return try ctx.importChildren(
    el, ImportChildrenOpts(context: [contextValue(ImportTextFormat, merged)]))
}

private func isInsidePreserveWhitespace(_ node: DOMNode, _ ws: WhitespaceImportConfig) -> Bool {
  var current = node.parentNode
  while let c = current {
    if ws.preservesWhitespace(c) { return true }
    current = c.parentNode
  }
  return false
}

private func findAdjacentTextOnLine(
  _ text: DOMText, _ forward: Bool, _ ws: WhitespaceImportConfig
) -> DOMText? {
  var node: DOMNode = text
  while true {
    var sibling = forward ? node.nextSibling : node.previousSibling
    while sibling == nil {
      guard let parent = node.parentNode else { return nil }
      node = parent
      sibling = forward ? node.nextSibling : node.previousSibling
    }
    node = sibling!
    if !ws.isInline(node) { return nil }
    while let descendant = forward ? node.firstChild : node.lastChild {
      node = descendant
    }
    if let text = node as? DOMText { return text }
    if node.nodeName == "BR" { return nil }
  }
}

private func isCollapsibleSpace(_ u: UInt16) -> Bool { u == 0x20 || u == 0x09 || u == 0x0A }

private func collapseWhitespace(_ textNode: DOMText, _ ws: WhitespaceImportConfig) -> String {
  var units: [UInt16] = []
  var inSpace = false
  for u in textNode.textContent.utf16 where u != 0x0D {
    if isCollapsibleSpace(u) {
      if !inSpace { units.append(0x20) }
      inSpace = true
    } else {
      units.append(u)
      inSpace = false
    }
  }
  if units.isEmpty { return "" }
  if units[0] == 0x20 {
    var neighbor: DOMText? = textNode
    var isStartOfLine = true
    while let n = neighbor, let found = findAdjacentTextOnLine(n, false, ws) {
      neighbor = found
      let content = Array(found.textContent.utf16)
      if !content.isEmpty {
        if isCollapsibleSpace(content.last!) { units.removeFirst() }
        isStartOfLine = false
        break
      }
    }
    if isStartOfLine { units.removeFirst() }
  }
  if let last = units.last, last == 0x20 {
    var neighbor: DOMText? = textNode
    var isEndOfLine = true
    while let n = neighbor, let found = findAdjacentTextOnLine(n, true, ws) {
      neighbor = found
      // `replace(/^( |\t|\r?\n)+/, '')`.
      let content = found.textContent.replacing(/^( |\t|\r?\n)+/, with: "")
      if !content.isEmpty {
        isEndOfLine = false
        break
      }
    }
    if isEndOfLine { units.removeLast() }
  }
  return String(decoding: units, as: UTF16.self)
}

private func applyFormat(_ node: LexicalNode, _ format: Int) throws {
  if format != 0, let text = node as? TextNode { try text.setFormat(format) }
}

/// The inverse of `getStyleObjectFromCSS`, without the properties the
/// format bits own.
private func styleObjectToCSS(_ style: StyleObject) -> String {
  var css = ""
  for (prop, value) in style where !FORMAT_BIT_STYLE_PROPS.contains(prop) {
    css += "\(prop): \(value); "
  }
  while css.last == " " { css.removeLast() }
  return css
}

private func applyTextStyle(_ node: LexicalNode, _ style: StyleObject) throws {
  if let text = node as? TextNode {
    let css = styleObjectToCSS(style)
    if !css.isEmpty { try text.setStyle(css) }
  }
}

/// `#text`: whitespace preserved (split into line breaks and tabs) inside
/// a preserving ancestor, otherwise collapsed against its neighbors.
nonisolated(unsafe) private let TextRule = defineImportRule(
  name: "@lexical/html/#text", match: sel.text()
) { ctx, el, _ in
  let format = try ctx.get(ImportTextFormat)
  let style = try ctx.get(ImportTextStyle)
  let ws = try ctx.get(ImportWhitespaceConfig)
  if isInsidePreserveWhitespace(el, ws) {
    let out = try generateNodesFromRawText(el.textContent)
    for node in out {
      try applyFormat(node, format)
      try applyTextStyle(node, style)
    }
    return out
  }
  let collapsed = collapseWhitespace(el, ws)
  if collapsed.isEmpty { return [] }
  let text = try createTextNode(collapsed)
  try applyFormat(text, format)
  try applyTextStyle(text, style)
  return [text]
}

/// Drops `<script>` and `<style>` and their contents.
nonisolated(unsafe) private let IgnoreScriptStyleRule = defineImportRule(
  name: "@lexical/html/script-style-ignore", match: sel.tag("script", "style")
) { _, _, _ in [] }

/// `<br>`, except one that is the only or last child of a block.
nonisolated(unsafe) private let LineBreakRule = defineImportRule(
  name: "@lexical/html/br", match: sel.tag("br")
) { _, el, _ in
  isOnlyChildInBlockNode(el) || isLastChildInBlockNode(el) ? [] : [try createLineBreakNode()]
}

/// `<p>`, with its format, indent, direction and `align` attribute.
nonisolated(unsafe) private let ParagraphRule = defineImportRule(
  name: "@lexical/html/p", match: sel.tag("p")
) { ctx, el, _ in
  let p = try createParagraphNode()
  try setFormatFromDOM(p, el)
  try setNodeIndentFromDOM(el, p)
  if try p.getFormatType() == "", let align = el.getAttribute("align"), isAlignmentValue(align) {
    try p.setFormat(align)
  }
  try setDirectionFromDOM(p, el)
  return [try p.splice(0, 0, ctx.importChildren(el))]
}

/// `<hr>`, when `HorizontalRuleNode` is registered; otherwise the next
/// rule.
nonisolated(unsafe) let HorizontalRuleRule = defineImportRule(
  name: "@lexical/html/hr", match: sel.tag("hr")
) { _, _, next in
  try getEditor().hasNode(HorizontalRuleNode.self) ? [try createHorizontalRuleNode()] : next()
}

/// Any other block element: its children through `BlockSchema`, so each
/// inline run becomes a paragraph with the element's `text-align`.
nonisolated(unsafe) private let TransparentBlockRule = defineImportRule(
  name: "@lexical/html/transparent-block", match: sel.any()
) { ctx, el, next in
  if !isBlockDomNode(el) { return try next() }
  return try propagateTextAlignToBlockChildren(
    ctx.importChildren(el, ImportChildrenOpts(schema: BlockSchema)), el)
}

/// `CoreImportRules`: the rules for the core nodes and `<hr>`.
nonisolated(unsafe) public let CoreImportRules = [
  IgnoreScriptStyleRule, ParagraphRule, HorizontalRuleRule, TransparentBlockRule, TextRule,
  LineBreakRule, InlineFormatRule,
]
