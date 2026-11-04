import Lexical
import LexicalHTML

// Port of packages/lexical-code-core/src/CodeImportExtension.ts: the
// `DOMImportExtension` rules for code blocks (`@experimental` upstream),
// which `CodeExtension` contributes.

private let LANGUAGE_DATA_ATTRIBUTE = "data-language"
private let THEME_DATA_ATTRIBUTE = "data-theme"

/// Joins the children's text, with a line break between a block and its
/// neighbors, and splits it into code nodes.
private func normalizeCodeChildren(_ children: [LexicalNode]) throws -> [LexicalNode] {
  var text = ""
  var previousWasBlock = false
  for (i, child) in children.enumerated() {
    let isBlock = try isBlockLevel(child)
    if i > 0 && (isBlock || previousWasBlock) { text += "\n" }
    text += try child.getTextContent()
    previousWasBlock = isBlock
  }
  return try plainifyCodeContent(text)
}

private func isMonospaceElement(_ el: DOMElement) -> Bool {
  el.style["font-family"].contains("monospace")
}

private func isMonospaceDescendant(_ node: DOMElement) -> Bool {
  var parent = node.parentElement
  while let current = parent {
    if isMonospaceElement(current) { return true }
    parent = current.parentElement
  }
  return false
}

/// `el.querySelector(tag)` for a lower-case tag name.
private func hasDescendant(_ node: DOMNode, _ localName: String) -> Bool {
  node.childNodes.contains {
    ($0 as? DOMElement)?.localName == localName || hasDescendant($0, localName)
  }
}

private func codeBlock(_ el: DOMElement?, _ children: [LexicalNode]) throws -> [LexicalNode] {
  [
    try createCodeNode(
      el?.getAttribute(LANGUAGE_DATA_ATTRIBUTE), el?.getAttribute(THEME_DATA_ATTRIBUTE)
    ).splice(0, 0, children)
  ]
}

nonisolated(unsafe) private let GitHubCodeTableOverlayRules = defineOverlayRules([
  defineImportRule(name: "@lexical/code/github-code-table/unwrap", match: sel.tag("tr", "td")) {
    ctx, el, _ in try ctx.importChildren(el)
  }
])

nonisolated(unsafe) private let PreRule = defineImportRule(
  name: "@lexical/code/pre", match: sel.tag("pre")
) { ctx, el, _ in
  try codeBlock(el, normalizeCodeChildren(ctx.importChildren(el)))
}

nonisolated(unsafe) private let MultilineCodeRule = defineImportRule(
  name: "@lexical/code/code-multiline", match: sel.tag("code")
) { ctx, el, next in
  // `/\r?\n/` matches any "\n"; over UTF-16, because Swift reads "\r\n" as
  // one Character.
  let isMultiLine = el.textContent.utf16.contains(0x0A) || hasDescendant(el, "br")
  if !isMultiLine { return try next() }
  return try codeBlock(el, normalizeCodeChildren(ctx.importChildren(el)))
}

private func isMonospacePreElement(_ el: DOMElement) -> Bool {
  el.style["font-family"].lowercased().contains("monospace")
    && el.style["white-space"].hasPrefix("pre")
}

/// The lines of a VS Code wrapper: one per `<div>` or `<br>` child, with
/// the inline content between them as a line of its own. `nil` when it has
/// no block child.
private func splitMonospaceWrapperLines(_ el: DOMElement) -> [String]? {
  var hasBlockChild = false
  var lines: [String] = []
  var acc = ""
  var hasAcc = false
  func flush() {
    if hasAcc {
      lines.append(acc)
      acc = ""
      hasAcc = false
    }
  }
  for child in el.childNodes {
    if let child = child as? DOMElement {
      if child.tagName == "DIV" {
        flush()
        lines.append(child.textContent)
        hasBlockChild = true
      } else if child.tagName == "BR" {
        flush()
        lines.append("")
        hasBlockChild = true
      } else {
        acc += child.textContent
        hasAcc = true
      }
    } else if let child = child as? DOMText {
      let t = child.textContent
      if !t.isEmpty {
        acc += t
        hasAcc = true
      }
    }
  }
  flush()
  return hasBlockChild ? lines : nil
}

private func looksLikeVscodePaste(_ root: DOMNode) -> Bool {
  for child in root.children {
    if isMonospacePreElement(child) {
      if splitMonospaceWrapperLines(child) != nil { return true }
      if let next = child.nextElementSibling, isMonospacePreElement(next) { return true }
      continue
    }
    if looksLikeVscodePaste(child) { return true }
  }
  return false
}

nonisolated(unsafe) private let VscodeWrapperRule = defineImportRule(
  name: "@lexical/code/vscode-wrapper", match: sel.tag("div")
) { _, el, next in
  if !isMonospacePreElement(el) || isMonospaceDescendant(el) { return try next() }
  guard let lines = splitMonospaceWrapperLines(el), !lines.isEmpty else { return try next() }
  return [try createCodeNode().splice(0, 0, plainifyCodeContent(lines.joined(separator: "\n")))]
}

nonisolated(unsafe) private let VscodeLineRunRule = defineImportRule(
  name: "@lexical/code/vscode-line-run", match: sel.tag("div", "br")
) { _, el, next in
  if !isMonospacePreElement(el) || isMonospaceDescendant(el) { return try next() }
  if let prev = el.previousElementSibling, isMonospacePreElement(prev) {
    // An earlier sibling's walk already absorbed `el` into its run.
    return []
  }
  var lines: [String] = []
  var cur: DOMElement? = el
  while let current = cur, isMonospacePreElement(current) {
    lines.append(current.tagName == "BR" ? "" : current.textContent)
    cur = current.nextElementSibling
  }
  if lines.count < 2 { return try next() }
  return [try createCodeNode().splice(0, 0, plainifyCodeContent(lines.joined(separator: "\n")))]
}

nonisolated(unsafe) private let VscodeCodePasteOverlay = defineOverlayRules([
  VscodeWrapperRule, VscodeLineRunRule,
])

/// `$installVscodeCodePasteOverlay`: when the HTML looks like a VS Code
/// paste (monospace `white-space: pre` wrappers), adds the rules that turn
/// its lines into a code block.
public func installVscodeCodePasteOverlay(
  _ dom: DOMNode, _ ctx: DOMPreprocessContext, _ next: () throws -> Void
) throws {
  let root: DOMNode = (dom as? DOMDocument)?.body ?? dom
  if looksLikeVscodePaste(root) {
    ctx.session.update(ImportOverlays) { $0 + [VscodeCodePasteOverlay] }
  }
  try next()
}

nonisolated(unsafe) private let DivRule = defineImportRule(
  name: "@lexical/code/div", match: sel.tag("div")
) { ctx, el, next in
  if isMonospaceElement(el) {
    return [try createCodeNode().splice(0, 0, normalizeCodeChildren(ctx.importChildren(el)))]
  }
  return try next()
}

nonisolated(unsafe) private let GitHubCodeTableRule = defineImportRule(
  name: "@lexical/code/github-code-table",
  match: sel.tag("table").classAll("js-file-line-container")
) { ctx, el, _ in
  [
    try createCodeNode().splice(
      0, 0,
      normalizeCodeChildren(
        ctx.importChildren(el, ImportChildrenOpts(rules: GitHubCodeTableOverlayRules))))
  ]
}

nonisolated(unsafe) private let GitHubCodeCellByClassRule = defineImportRule(
  name: "@lexical/code/github-code-cell-by-class", match: sel.tag("td").classAll("js-file-line")
) { ctx, el, _ in try ctx.importChildren(el) }

/// `CodeImportRules`, the more specific rules first.
nonisolated(unsafe) public let CodeImportRules = [
  GitHubCodeTableRule,
  GitHubCodeCellByClassRule,
  MultilineCodeRule,
  PreRule,
  DivRule,
]
